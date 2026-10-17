use crate::{
    fonts,
    layout::{Layout, Object},
    model::*,
    persistence,
};
use printpdf::*;
use std::{collections::HashMap, path::Path};
use unicode_segmentation::UnicodeSegmentation;
#[path = "pdf_vectors.rs"]
mod vectors;

fn rgb(c: [u8; 3]) -> Color {
    Color::Rgb(Rgb::new(
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        None,
    ))
}
fn point(x: f32, y: f32) -> Point {
    Point { x: Pt(x), y: Pt(y) }
}
fn stroke(ops: &mut Vec<Op>, rect: eframe::egui::Rect, h: f32, c: [u8; 3], closed: bool) {
    let pts = if closed {
        vec![
            point(rect.left(), h - rect.top()),
            point(rect.right(), h - rect.top()),
            point(rect.right(), h - rect.bottom()),
            point(rect.left(), h - rect.bottom()),
        ]
    } else {
        vec![
            point(rect.left(), h - rect.top()),
            point(rect.right(), h - rect.top()),
        ]
    };
    ops.push(Op::SetOutlineColor { col: rgb(c) });
    ops.push(Op::SetOutlineThickness { pt: Pt(0.5) });
    ops.push(Op::DrawLine {
        line: Line {
            points: pts
                .into_iter()
                .map(|p| LinePoint { p, bezier: false })
                .collect(),
            is_closed: closed,
        },
    });
}
fn fill(ops: &mut Vec<Op>, rect: eframe::egui::Rect, h: f32, c: [u8; 3]) {
    ops.push(Op::SetFillColor { col: rgb(c) });
    ops.push(Op::DrawPolygon {
        polygon: Polygon {
            rings: vec![PolygonRing {
                points: vec![
                    point(rect.left(), h - rect.top()),
                    point(rect.right(), h - rect.top()),
                    point(rect.right(), h - rect.bottom()),
                    point(rect.left(), h - rect.bottom()),
                ]
                .into_iter()
                .map(|p| LinePoint { p, bezier: false })
                .collect(),
            }],
            mode: PaintMode::Fill,
            winding_order: WindingOrder::NonZero,
        },
    });
}
fn styled_box(ops: &mut Vec<Op>, rect: eframe::egui::Rect, h: f32, style: BoxStyle) {
    if let Some(colour) = style.fill {
        fill(ops, rect, h, colour);
    }
    for (a, b, border) in crate::layout::box_edges(rect, style) {
        if border.width == 0.0 {
            continue;
        }
        ops.push(Op::SetOutlineColor {
            col: rgb(border.colour),
        });
        ops.push(Op::SetOutlineThickness {
            pt: Pt(border.width),
        });
        ops.push(Op::DrawLine {
            line: Line {
                points: vec![a, b]
                    .into_iter()
                    .map(|p| LinePoint {
                        p: point(p.x, h - p.y),
                        bezier: false,
                    })
                    .collect(),
                is_closed: false,
            },
        });
    }
}

/// Checklists use document-coloured vectors, independent of font glyph coverage
/// and the application's light/dark palette. Geometry matches the canvas marker.
fn checklist_marker(
    ops: &mut Vec<Op>,
    rect: eframe::egui::Rect,
    h: f32,
    checked: bool,
    color: [u8; 3],
) {
    use eframe::egui::{Rect, pos2, vec2};
    let center = pos2(rect.left() - 11.0, rect.center().y);
    let size = crate::layout::checklist_mark_size(rect.height());
    let ratio = size / 15.0;
    let square = Rect::from_center_size(center, vec2(size, size));
    ops.push(Op::SaveGraphicsState);
    stroke(ops, square, h, color, true);
    if checked {
        ops.push(Op::SetOutlineThickness {
            pt: Pt(1.7 * ratio),
        });
        ops.push(Op::DrawLine {
            line: Line {
                points: [vec2(-4.0, 0.0), vec2(-1.0, 3.0), vec2(4.5, -3.5)]
                    .into_iter()
                    .map(|offset| LinePoint {
                        p: point(center.x + offset.x * ratio, h - center.y - offset.y * ratio),
                        bezier: false,
                    })
                    .collect(),
                is_closed: false,
            },
        });
    }
    ops.push(Op::RestoreGraphicsState);
}
pub fn export(doc: &Document, layout: &Layout, path: &Path) -> Result<(), String> {
    persistence::atomic_write(path, &encode(doc, layout)?)
}

/// Encode the same measured layout used by the canvas, without doing file I/O.
pub fn encode(doc: &Document, layout: &Layout) -> Result<Vec<u8>, String> {
    if doc.page.pageless {
        return Err("Measure a paged document copy before PDF export or printing".into());
    }
    let mut pdf = PdfDocument::new(&doc.title);
    let mut ids = HashMap::new();
    let mut warnings = vec![];
    let default_key = fonts::key(&TextStyle::default());
    let mut styles = vec![
        TextStyle::default(),
        TextStyle {
            font: "Noto Sans Mono".into(),
            ..Default::default()
        },
        TextStyle {
            font: "Noto Sans Mono".into(),
            bold: true,
            ..Default::default()
        },
        // Embed the bundled symbol face so characters routed through
        // `symbol_key` stay the same Unicode glyphs in searchable PDF text.
        TextStyle {
            font: "Noto Sans Math".into(),
            ..Default::default()
        },
        TextStyle {
            font: "Noto Emoji".into(),
            ..Default::default()
        },
        doc.page.header.style.clone(),
        doc.page.footer.style.clone(),
    ];
    styles.extend(
        doc.page
            .header
            .chars
            .iter()
            .chain(&doc.page.footer.chars)
            .map(|c| c.style.clone()),
    );
    for address in doc.addresses() {
        if let Some(p) = doc.paragraph(address) {
            styles.push(p.style.clone());
            styles.extend(p.chars.iter().map(|c| c.style.clone()));
        }
    }
    let mut faces = HashMap::new();
    for style in &styles {
        let name = fonts::key(style);
        if !ids.contains_key(&name) {
            let face = fonts::face(style);
            let parsed =
                ParsedFont::from_bytes(face.font.as_ref(), face.index as usize, &mut Vec::new())
                    .ok_or_else(|| format!("Cannot embed font {}", style.font))?;
            ids.insert(name.clone(), pdf.add_font(&parsed));
            faces.insert(name, face);
        }
    }
    let (w, h) = doc.page.size();
    let mut pages = vec![];
    for page in 0..layout.pages {
        let mut ops = vec![];
        let mut front_images = vec![];
        if doc.page.color != [255; 3] {
            fill(
                &mut ops,
                eframe::egui::Rect::from_min_size(
                    eframe::egui::Pos2::ZERO,
                    eframe::egui::vec2(w, h),
                ),
                h,
                doc.page.color,
            );
        }
        for (pn, rect, style) in crate::layout::paragraph_boxes(layout, doc) {
            if pn == page {
                styled_box(&mut ops, rect, h, style);
            }
        }
        for obj in &layout.objects {
            match obj {
                Object::Image {
                    block,
                    page: p,
                    rect,
                    ..
                } if *p == page => {
                    if let Block::Image(i) = &doc.blocks[*block] {
                        if let Some(object) = &i.editable
                            && i.visible_crop() == [0.0, 0.0, 1.0, 1.0]
                            && !i.padding.enabled
                            && i.effects == ImageEffects::default()
                        {
                            let vector_ops = vectors::render(object, *rect, h)?;
                            if i.placement.flow == ImageFlow::Front {
                                front_images.extend(vector_ops);
                            } else {
                                ops.extend(vector_ops);
                            }
                            continue;
                        }
                        let bytes = if i.visible_crop() != [0.0, 0.0, 1.0, 1.0]
                            || i.padding.enabled
                            || i.effects != ImageEffects::default()
                        {
                            crate::image_edit::flattened_png(i)?
                        } else {
                            i.data.to_vec()
                        };
                        let image = RawImage::decode_from_bytes(&bytes, &mut warnings)
                            .map_err(|e| e.to_string())?;
                        let iw = image.width as f32;
                        let ih = image.height as f32;
                        let id = pdf.add_image(&image);
                        let image_op = Op::UseXobject {
                            id,
                            transform: XObjectTransform {
                                translate_x: Some(Pt(rect.left())),
                                translate_y: Some(Pt(h - rect.bottom())),
                                scale_x: Some(rect.width() / iw),
                                scale_y: Some(rect.height() / ih),
                                dpi: Some(72.0),
                                ..Default::default()
                            },
                        };
                        if i.placement.flow == ImageFlow::Front {
                            front_images.push(image_op);
                        } else {
                            ops.push(image_op);
                        }
                    }
                }
                Object::Border {
                    page: p,
                    rect,
                    address,
                } if *p == page => styled_box(
                    &mut ops,
                    *rect,
                    h,
                    doc.paragraph(*address)
                        .and_then(|p| p.box_style)
                        .unwrap_or_default(),
                ),
                Object::Rule { page: p, rect } if *p == page => {
                    stroke(&mut ops, *rect, h, [100; 3], false)
                }
                _ => {}
            }
        }
        for (line, number) in layout
            .numbered_lines(doc)
            .into_iter()
            .filter(|(l, _)| l.page == page)
        {
            ops.extend([
                Op::StartTextSection,
                Op::SetFont {
                    font: PdfFontHandle::External(ids[&default_key].clone()),
                    size: Pt(8.0),
                },
                Op::SetTextCursor {
                    pos: point(
                        (line.rect.left() - 28.0).max(2.0),
                        h - line.rect.top() - 9.0,
                    ),
                },
                Op::SetFillColor {
                    col: rgb([100, 100, 100]),
                },
                Op::ShowText {
                    items: vec![TextItem::Text(number.to_string())],
                },
                Op::EndTextSection,
            ]);
        }
        for line in layout
            .lines
            .iter()
            .chain(&layout.running)
            .filter(|l| l.page == page)
        {
            let p = doc
                .paragraph(line.address)
                .ok_or("Invalid paragraph in layout")?;
            let glyphs = &line.row.glyphs;
            if let Some(marker) = &line.marker {
                if marker == "☐" || marker == "☑" {
                    checklist_marker(&mut ops, line.rect, h, p.checked, p.style.color);
                } else {
                    ops.extend([
                        Op::StartTextSection,
                        Op::SetFont {
                            font: PdfFontHandle::External(ids[&default_key].clone()),
                            size: Pt(p.style.size),
                        },
                        Op::SetTextCursor {
                            pos: point(line.rect.left() - 18.0, h - line.rect.top() - p.style.size),
                        },
                        Op::SetFillColor {
                            col: rgb(p.style.color),
                        },
                        Op::ShowText {
                            items: vec![TextItem::Text(marker.clone())],
                        },
                        Op::EndTextSection,
                    ]);
                }
            }
            let line_text: String = p.chars[line.start..line.end].iter().map(|c| c.ch).collect();
            let mut font_keys = Vec::with_capacity(line.end - line.start);
            let mut offset = line.start;
            for grapheme in line_text.graphemes(true) {
                let count = grapheme.chars().count();
                let emoji = fonts::emoji_fallback_key(&p.chars[offset].style, grapheme);
                for ch in &p.chars[offset..offset + count] {
                    font_keys.push(
                        emoji.map_or_else(|| fonts::symbol_key(&ch.style, ch.ch), str::to_owned),
                    );
                }
                offset += count;
            }
            let mut start = line.start;
            while start < line.end {
                let s = &p.chars[start].style;
                let key = &font_keys[start - line.start];
                let mut end = start + 1;
                while end < line.end
                    && p.chars[end].style == *s
                    && p.chars[end].ch != '\t'
                    && &font_keys[end - line.start] == key
                {
                    end += 1;
                }
                let x = line.rect.left() + line.x(start);
                let right = line.rect.left() + line.x(end);
                let y = h
                    - line.rect.top()
                    - glyphs.get(start - line.start).map_or(s.size, |g| g.pos.y);
                if let Some(c) = s.highlight {
                    fill(
                        &mut ops,
                        eframe::egui::Rect::from_min_max(
                            eframe::egui::pos2(x, line.rect.top()),
                            eframe::egui::pos2(right, line.rect.bottom()),
                        ),
                        h,
                        c,
                    );
                }
                let text: String = p.chars[start..end].iter().map(|c| c.ch).collect();
                let face = &faces[key];
                let font = harfrust::FontRef::from_index(face.font.as_ref(), face.index)
                    .map_err(|e| e.to_string())?;
                let data = harfrust::ShaperData::new(&font);
                let shaper = data.shaper(&font).build();
                let mut buffer = harfrust::UnicodeBuffer::new();
                buffer.push_str(&text);
                buffer.guess_segment_properties();
                if key == "Emoji" {
                    // Variation selectors are consumed by shaping. Do not emit
                    // their hidden space glyph with a second ToUnicode entry.
                    buffer.set_flags(harfrust::BufferFlags::REMOVE_DEFAULT_IGNORABLES);
                }
                let shaped = shaper.shape(buffer, harfrust::ShapeOptions::new());
                let factor = s.rendered_size() / shaper.units_per_em() as f32;
                let mut pen = 0.0;
                let mut cluster_base: HashMap<u32, (f32, f32)> = HashMap::new();
                for (info, position) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
                    let cluster = info.cluster as usize;
                    let offset = text.get(..cluster).unwrap_or("").chars().count();
                    let ch = text
                        .get(cluster..)
                        .and_then(|t| t.chars().next())
                        .unwrap_or(' ');
                    let cluster_end = shaped
                        .glyph_infos()
                        .iter()
                        .map(|g| g.cluster as usize)
                        .filter(|end| *end > cluster)
                        .min()
                        .unwrap_or(text.len());
                    let extraction = text.get(cluster..cluster_end).unwrap_or(" ");
                    let anchor = line.rect.left() + line.x(start + offset);
                    let (base, first_offset) = *cluster_base
                        .entry(info.cluster)
                        .or_insert((pen, position.x_offset as f32));
                    let gx =
                        anchor + (pen - base + position.x_offset as f32 - first_offset) * factor;
                    let gy = y + position.y_offset as f32 * factor;
                    if ch != '\t' {
                        ops.extend([
                            Op::StartTextSection,
                            Op::SetFont {
                                font: PdfFontHandle::External(ids[key].clone()),
                                size: Pt(s.rendered_size()),
                            },
                            Op::SetTextCursor { pos: point(gx, gy) },
                            Op::SetFillColor { col: rgb(s.color) },
                            Op::ShowText {
                                items: vec![TextItem::GlyphIds(vec![Codepoint::with_cid(
                                    info.glyph_id as u16,
                                    0.0,
                                    extraction.to_string(),
                                )])],
                            },
                            Op::EndTextSection,
                        ]);
                    }
                    pen += position.x_advance as f32;
                }
                if s.underline || s.link.is_some() {
                    let r = eframe::egui::Rect::from_min_max(
                        eframe::egui::pos2(x, h - y + 1.3),
                        eframe::egui::pos2(right, h - y + 1.3),
                    );
                    stroke(&mut ops, r, h, s.color, false);
                }
                if let Some(url) = &s.link {
                    ops.push(Op::LinkAnnotation {
                        link: LinkAnnotation::new(
                            Rect::from_xywh(
                                Pt(x),
                                Pt(h - line.rect.bottom()),
                                Pt(right - x),
                                Pt(line.rect.height()),
                            ),
                            Actions::Uri(url.clone()),
                            Some(BorderArray::Solid([0.0; 3])),
                            None,
                            None,
                        ),
                    });
                }
                if s.strike {
                    let r = eframe::egui::Rect::from_min_max(
                        eframe::egui::pos2(x, h - y - s.size * 0.3),
                        eframe::egui::pos2(right, h - y - s.size * 0.3),
                    );
                    stroke(&mut ops, r, h, s.color, false);
                }
                start = end;
            }
        }
        if doc.page.page_numbers {
            ops.extend([
                Op::StartTextSection,
                Op::SetFont {
                    font: PdfFontHandle::External(ids[&default_key].clone()),
                    size: Pt(9.0),
                },
                Op::SetTextCursor {
                    pos: point(w * 0.5, h - (h - doc.page.margins[1] * 0.5)),
                },
                Op::SetFillColor { col: rgb([80; 3]) },
                Op::ShowText {
                    items: vec![TextItem::Text((page + 1).to_string())],
                },
                Op::EndTextSection,
            ]);
        }
        ops.extend(front_images);
        pages.push(PdfPage::new(Mm(w * 25.4 / 72.0), Mm(h * 25.4 / 72.0), ops));
    }
    pdf.with_pages(pages);
    let bytes = pdf.save(&PdfSaveOptions::default(), &mut warnings);
    if bytes.is_empty() {
        return Err("PDF encoder produced no data".into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checklist_pdf_uses_vectors_for_both_states_without_missing_font_glyphs() {
        let mut doc = Document::new("Checklist PDF");
        doc.blocks = [false, true]
            .into_iter()
            .map(|checked| {
                let mut paragraph = Paragraph::from_text(if checked { "Done" } else { "Todo" });
                paragraph.list = ListKind::Check;
                paragraph.checked = checked;
                Block::Paragraph(paragraph)
            })
            .collect();
        let before = doc.clone();
        let ctx = eframe::egui::Context::default();
        fonts::install(&ctx);
        let mut bytes = Vec::new();
        let mut output = ctx.run_ui(Default::default(), |_| {
            bytes = encode(&doc, &Layout::measure(&ctx, &doc)).unwrap();
        });
        output.textures_delta.clear();
        let parsed =
            PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut Vec::new()).unwrap();
        let lines: Vec<_> = parsed.pages[0]
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::DrawLine { line } => Some(line),
                _ => None,
            })
            .collect();
        // The PDF parser represents closed stroked paths as polygons.
        assert_eq!(
            parsed.pages[0]
                .ops
                .iter()
                .filter(|op| matches!(op,
                    Op::DrawPolygon { polygon } if polygon.mode == PaintMode::Stroke
                        && polygon.rings.len() == 1 && polygon.rings[0].points.len() >= 4
                ))
                .count(),
            2
        );
        assert_eq!(
            lines
                .iter()
                .filter(|line| !line.is_closed && line.points.len() == 3)
                .count(),
            1
        );
        let text = parsed.pages[0].extract_text(&parsed.resources).join("");
        assert!(text.contains("Todo") && text.contains("Done"), "{text:?}");
        assert!(!text.contains(['☐', '☑', '\u{fffd}']));
        assert_eq!(doc, before);
    }
    #[test]
    fn pdf_symbol_fallback_preserves_unicode() {
        let mut doc = Document::new("PDF symbols");
        let arrows = "⟵⟶⟷⟸⟹⟺";
        doc.blocks = vec![Block::Paragraph(Paragraph::from_text(&format!(
            "┌─┐│├┤└┘→←↑↓↔{arrows}"
        )))];
        let ctx = eframe::egui::Context::default();
        fonts::install(&ctx);
        let mut bytes = Vec::new();
        let mut out = ctx.run_ui(Default::default(), |_| {
            bytes = encode(&doc, &Layout::measure(&ctx, &doc)).unwrap();
        });
        out.textures_delta.clear();
        let parsed =
            PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut Vec::new()).unwrap();
        let extracted = parsed
            .pages
            .iter()
            .flat_map(|p| p.extract_text(&parsed.resources))
            .collect::<Vec<_>>()
            .join("");
        assert!(
            extracted.contains(&format!("┌─┐│├┤└┘→←↑↓↔{arrows}")),
            "{extracted}"
        );
    }
    #[test]
    fn pdf_body_emoji_fallback_preserves_joined_sequences_without_tofu() {
        let text = "Emoji: 😀 🫠 👩‍💻 👋🏽 👨‍👩‍👧‍👦 ❤️ 🇦🇺 and arrows → ⟹.";
        let mut doc = Document::new("Body emoji PDF");
        doc.blocks = vec![Block::Paragraph(Paragraph::from_text(text))];
        let before = doc.clone();
        let ctx = eframe::egui::Context::default();
        fonts::install(&ctx);
        let mut bytes = Vec::new();
        let mut output = ctx.run_ui(Default::default(), |_| {
            bytes = encode(&doc, &Layout::measure(&ctx, &doc)).unwrap();
        });
        output.textures_delta.clear();
        let parsed =
            PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut Vec::new()).unwrap();
        let extracted = parsed.pages[0].extract_text(&parsed.resources).join("");
        let compact = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        assert!(
            compact(&extracted).contains(&compact(text)),
            "{extracted:?}"
        );
        assert_eq!(doc, before);
    }
    #[test]
    fn pdf_preserves_measured_pages_fonts_and_links() {
        let mut d = Document::new("PDF test");
        let mut p = Paragraph::from_text("Café office affinity e\u{301}");
        for c in &mut p.chars {
            c.style.link = Some("https://example.com".into());
        }
        d.blocks = vec![
            Block::Paragraph(p),
            Block::PageBreak,
            Block::Paragraph(Paragraph::from_text("Second page")),
        ];
        let ctx = eframe::egui::Context::default();
        fonts::install(&ctx);
        let mut layout = None;
        let mut output = ctx.run_ui(eframe::egui::RawInput::default(), |_| {
            layout = Some(Layout::measure(&ctx, &d));
        });
        output.textures_delta.clear();
        let path = std::env::temp_dir().join(format!("alphadocs-{}.pdf", d.id));
        export(&d, &layout.unwrap(), &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        let parsed =
            PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut Vec::new()).unwrap();
        assert_eq!(parsed.pages.len(), 2);
        assert!(!parsed.resources.fonts.map.is_empty());
        assert!(parsed.pages[0].ops.iter().any(|op|matches!(op,Op::LinkAnnotation{link} if matches!(&link.actions,Actions::Uri(uri) if uri=="https://example.com"))));
        assert!((parsed.pages[0].media_box.width.0 - d.page.size().0).abs() < 0.1);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn every_paper_preset_exports_at_its_physical_size_in_both_orientations() {
        for paper in Paper::STANDARD.into_iter().chain([Paper::Custom {
            width: 703.25,
            height: 911.75,
        }]) {
            for landscape in [false, true] {
                let mut doc = Document::new("PDF page size");
                doc.page.paper = paper;
                doc.page.landscape = landscape;
                doc.blocks = vec![Block::Paragraph(Paragraph::from_text("Physical size"))];
                let ctx = eframe::egui::Context::default();
                fonts::install(&ctx);
                let mut pdf_bytes = None;
                let mut output = ctx.run_ui(Default::default(), |_| {
                    let layout = Layout::measure(&ctx, &doc);
                    pdf_bytes = Some(encode(&doc, &layout).unwrap());
                });
                output.textures_delta.clear();
                let parsed = PdfDocument::parse(
                    &pdf_bytes.unwrap(),
                    &PdfParseOptions::default(),
                    &mut Vec::new(),
                )
                .unwrap();
                let (width, height) = doc.page.size();
                let box_ = &parsed.pages[0].media_box;
                assert!((box_.width.0 - width).abs() < 0.1, "{paper:?}");
                assert!((box_.height.0 - height).abs() < 0.1, "{paper:?}");
                let text = parsed.pages[0].extract_text(&parsed.resources).join("");
                assert!(
                    text.contains("Physical") && text.contains("size"),
                    "{paper:?}: {text:?}"
                );
            }
        }
    }

    #[test]
    fn pdf_line_numbers_are_printed_and_pageless_documents_require_paged_geometry() {
        let mut doc = Document::new("PDF line numbers");
        doc.page.line_numbers = LineNumbering::Continuous;
        doc.blocks = vec![Block::Paragraph(Paragraph::from_text("First\nSecond"))];
        let ctx = eframe::egui::Context::default();
        fonts::install(&ctx);
        let mut result = None;
        let mut output = ctx.run_ui(Default::default(), |_| {
            let layout = Layout::measure(&ctx, &doc);
            result = Some(encode(&doc, &layout).unwrap());
        });
        output.textures_delta.clear();
        let parsed = PdfDocument::parse(
            &result.unwrap(),
            &PdfParseOptions::default(),
            &mut Vec::new(),
        )
        .unwrap();
        let text = parsed.pages[0].extract_text(&parsed.resources).join("");
        assert!(text.contains("1") && text.contains("2"), "{text:?}");

        doc.page.pageless = true;
        let layout = Layout::measure(&ctx, &doc);
        assert!(encode(&doc, &layout).is_err());
    }
}
