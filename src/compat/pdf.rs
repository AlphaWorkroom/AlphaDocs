use crate::{
    fonts,
    layout::{Layout, Object},
    model::*,
    persistence,
};
use printpdf::*;
use std::{collections::HashMap, path::Path};

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
pub fn export(doc: &Document, layout: &Layout, path: &Path) -> Result<(), String> {
    persistence::atomic_write(path, &encode(doc, layout)?)
}

/// Encode the same measured layout used by the canvas, without doing file I/O.
pub fn encode(doc: &Document, layout: &Layout) -> Result<Vec<u8>, String> {
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
        for obj in &layout.objects {
            match obj {
                Object::Image {
                    block,
                    page: p,
                    rect,
                    ..
                } if *p == page => {
                    if let Block::Image(i) = &doc.blocks[*block] {
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
                Object::Border { page: p, rect, .. } if *p == page => {
                    stroke(&mut ops, *rect, h, [128; 3], true)
                }
                Object::Rule { page: p, rect } if *p == page => {
                    stroke(&mut ops, *rect, h, [100; 3], false)
                }
                _ => {}
            }
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
            let mut start = line.start;
            while start < line.end {
                let s = &p.chars[start].style;
                let key = fonts::symbol_key(s, p.chars[start].ch);
                let mut end = start + 1;
                while end < line.end
                    && p.chars[end].style == *s
                    && p.chars[end].ch != '\t'
                    && fonts::symbol_key(s, p.chars[end].ch) == key
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
                let face = &faces[&key];
                let font = harfrust::FontRef::from_index(face.font.as_ref(), face.index)
                    .map_err(|e| e.to_string())?;
                let data = harfrust::ShaperData::new(&font);
                let shaper = data.shaper(&font).build();
                let mut buffer = harfrust::UnicodeBuffer::new();
                buffer.push_str(&text);
                buffer.guess_segment_properties();
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
                                font: PdfFontHandle::External(ids[&key].clone()),
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
    fn pdf_symbol_fallback_preserves_unicode() {
        let mut doc = Document::new("PDF symbols");
        doc.blocks = vec![Block::Paragraph(Paragraph::from_text("┌─┐│├┤└┘→←↑↓↔"))];
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
        assert!(extracted.contains("┌─┐│├┤└┘→←↑↓↔"), "{extracted}");
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
}
