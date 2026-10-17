//! Native editable drawing/equation sources with derived vector/raster previews.
use crate::{fonts, layout, model::*};
use eframe::egui::Context;
use std::collections::BTreeMap;
use unicode_segmentation::UnicodeSegmentation;

fn esc(s: &str) -> String {
    quick_xml::escape::escape(s).into_owned()
}
fn colour(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}
pub fn equation_omml(latex: &str) -> Result<String, String> {
    let parsed = mathema::from_latex_with(
        latex,
        &mathema::Config::default().with_max_input_bytes(Some(4096)),
    )
    .map_err(|e| e.to_string())?;
    if let Some(warning) = parsed.diagnostics().first() {
        return Err(warning.message.clone());
    }
    let (xml, warnings) = parsed
        .to_omml_with_diagnostics()
        .map_err(|e| e.to_string())?;
    if let Some(warning) = warnings.first() {
        return Err(warning.message.clone());
    }
    Ok(xml)
}
pub fn latex_from_omml(xml: &str) -> Result<String, String> {
    let parsed = mathema::from_omml_with(
        xml,
        &mathema::Config::default().with_max_input_bytes(Some(64 * 1024)),
    )
    .map_err(|e| e.to_string())?;
    if let Some(warning) = parsed.diagnostics().first() {
        return Err(warning.message.clone());
    }
    parsed.to_latex().map_err(|e| e.to_string())
}

pub fn source_fonts(source: &ObjectSource) -> BTreeMap<String, Vec<u8>> {
    if let ObjectSource::Drawing { text, .. } = source {
        drawing_fonts(text)
            .into_iter()
            .map(|(key, face)| (key, face.font.to_vec()))
            .collect()
    } else {
        BTreeMap::new()
    }
}
fn drawing_key(style: &TextStyle, grapheme: &str) -> String {
    fonts::emoji_fallback_key(style, grapheme)
        .map(str::to_owned)
        .unwrap_or_else(|| fonts::symbol_key(style, grapheme.chars().next().unwrap_or(' ')))
}
fn drawing_fonts(text: &Paragraph) -> BTreeMap<String, std::sync::Arc<eframe::egui::FontData>> {
    let mut result = BTreeMap::new();
    result.insert(fonts::key(&text.style), fonts::face(&text.style));
    let mut offset = 0;
    for grapheme in text.text().graphemes(true) {
        let style = text
            .chars
            .get(offset)
            .map(|c| &c.style)
            .unwrap_or(&text.style);
        let key = drawing_key(style, grapheme);
        result.entry(key.clone()).or_insert_with(|| {
            fonts::FONTS
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, bytes)| std::sync::Arc::new(eframe::egui::FontData::from_static(bytes)))
                .unwrap_or_else(|| fonts::face(style))
        });
        offset += grapheme.chars().count();
    }
    result
}

pub fn validate_svg(svg: &[u8]) -> Result<(), String> {
    if svg.len() > 4_000_000 {
        return Err("Drawing vector preview is too large".into());
    }
    let mut reader = quick_xml::Reader::from_reader(svg);
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(e) | quick_xml::events::Event::Empty(e)) => {
                let local = e.local_name();
                if !matches!(
                    local.as_ref(),
                    b"svg"
                        | b"g"
                        | b"defs"
                        | b"path"
                        | b"use"
                        | b"rect"
                        | b"ellipse"
                        | b"line"
                        | b"polyline"
                        | b"polygon"
                        | b"text"
                        | b"tspan"
                        | b"clipPath"
                        | b"title"
                        | b"desc"
                ) {
                    return Err("Unsupported drawing vector element".into());
                }
                for attr in e.attributes() {
                    let attr = attr.map_err(|e| e.to_string())?;
                    let name = attr.key.local_name();
                    let value = attr
                        .decode_and_unescape_value(reader.decoder())
                        .map_err(|e| e.to_string())?;
                    if name.as_ref() == b"href" && !value.starts_with('#')
                        || value.to_ascii_lowercase().contains("url(")
                        || name.as_ref().starts_with(b"on")
                    {
                        return Err("External or active vector content is not allowed".into());
                    }
                }
            }
            Ok(quick_xml::events::Event::DocType(_)) => {
                return Err("Vector document types are not allowed".into());
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(e) => return Err(e.to_string()),
            _ => {}
        }
    }
    Ok(())
}

pub fn validate(object: &EditableObject) -> Result<(), String> {
    match &object.source {
        ObjectSource::Equation { latex, size, .. } => {
            if latex.is_empty()
                || latex.len() > 4096
                || !size.is_finite()
                || !(6.0..=100.0).contains(size)
            {
                return Err("Invalid equation source or size".into());
            }
        }
        ObjectSource::Drawing { text, border, .. } => {
            if text.chars.len() > 10_000
                || !border.width.is_finite()
                || !(0.0..=12.0).contains(&border.width)
            {
                return Err("Invalid drawing source or border".into());
            }
            let mut doc = Document::new("Drawing validation");
            doc.blocks = vec![Block::Paragraph((**text).clone())];
            doc.validate()?;
        }
    }
    validate_svg(object.svg.as_bytes())
}

pub fn prepare(
    ctx: &Context,
    source: ObjectSource,
    width: f32,
    height: f32,
) -> Result<EmbeddedImage, String> {
    let svg = match &source {
        ObjectSource::Equation {
            latex,
            size,
            colour: c,
        } => {
            if !size.is_finite() || !(6.0..=100.0).contains(size) {
                return Err("Equation size must be 6–100 pt".into());
            }
            // Validate both the displayed expression and its editable Word form.
            equation_omml(latex)?;
            mathema::from_latex_with(
                latex,
                &mathema::Config::default().with_max_input_bytes(Some(4096)),
            )
            .map_err(|e| e.to_string())?
            .to_svg_with(&mathema::RenderOptions {
                font_size_px: *size as f64,
                fg_color: colour(*c),
                display_mode: Some(true),
                ..Default::default()
            })
            .map_err(|e| e.to_string())?
        }
        ObjectSource::Drawing {
            kind,
            text,
            fill,
            border,
        } => drawing_svg(ctx, *kind, text, *fill, *border, width, height)?,
    };
    let object = EditableObject {
        source,
        svg: svg.into(),
    };
    let (data, width, height) = raster_preview(&object)?;
    let name = match &object.source {
        ObjectSource::Equation { .. } => "Equation",
        ObjectSource::Drawing { kind, .. } => kind.label(),
    };
    Ok(EmbeddedImage {
        name: name.into(),
        title: name.into(),
        width,
        height,
        data,
        editable: Some(object),
        ..Default::default()
    })
}

pub(crate) fn raster_preview(object: &EditableObject) -> Result<(SharedVec<u8>, f32, f32), String> {
    validate(object)?;
    let mut options = resvg::usvg::Options::default();
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    for bytes in source_fonts(&object.source).into_values() {
        options.fontdb_mut().load_font_data(bytes);
    }
    let tree = resvg::usvg::Tree::from_str(&object.svg, &options).map_err(|e| e.to_string())?;
    let width = tree.size().width();
    let height = tree.size().height();
    if width > 2000.0 || height > 2000.0 || width <= 0.0 || height <= 0.0 {
        return Err("Drawing or equation exceeds the supported size".into());
    }
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new((width * 3.0).ceil() as u32, (height * 3.0).ceil() as u32)
            .ok_or("Could not allocate drawing preview")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(3.0, 3.0),
        &mut pixmap.as_mut(),
    );
    Ok((
        pixmap.encode_png().map_err(|e| e.to_string())?.into(),
        width,
        height,
    ))
}

fn drawing_svg(
    _ctx: &Context,
    kind: ShapeKind,
    text: &Paragraph,
    fill: Option<[u8; 3]>,
    border: CellBorder,
    width: f32,
    height: f32,
) -> Result<String, String> {
    if !width.is_finite()
        || !height.is_finite()
        || !(24.0..=1500.0).contains(&width)
        || !(12.0..=1500.0).contains(&height)
        || text.chars.len() > 10_000
    {
        return Err("Drawing dimensions or text exceed the supported limits".into());
    }
    // Activate actual font faces before measuring this drawing's editable text.
    let mut doc = Document::new("Drawing");
    doc.blocks = vec![Block::Paragraph(text.clone())];
    let ctx = Context::default();
    fonts::install(&ctx);
    fonts::prepare(&ctx, &doc, &text.style);
    // An inscribed rectangle keeps text inside an ellipse at every height.
    let inset_x = if kind == ShapeKind::Ellipse {
        width * 0.15 + 8.0
    } else {
        8.0
    };
    let text_width = (width - 2.0 * inset_x).max(1.0);
    let mut measured = None;
    let mut frame = ctx.run_ui(Default::default(), |_| {
        measured = Some(layout::shape(&ctx, text, text_width))
    });
    frame.textures_delta.clear();
    let galley = measured.ok_or("Could not measure drawing text")?;
    let height = if kind == ShapeKind::Ellipse {
        height.max((galley.rect.height() + 16.0) / 0.7)
    } else {
        height.max(galley.rect.height() + 16.0)
    };
    let inset_y = if kind == ShapeKind::Ellipse {
        height * 0.15 + 8.0
    } else {
        8.0
    };
    let c = colour(border.colour);
    let fill = fill.map(colour).unwrap_or_else(|| "none".into());
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">"
    );
    match kind {
        ShapeKind::TextBox|ShapeKind::Rectangle=>svg.push_str(&format!("<rect x=\"1\" y=\"1\" width=\"{}\" height=\"{}\" fill=\"{fill}\" stroke=\"{c}\" stroke-width=\"{}\"/>",width-2.0,height-2.0,border.width)),
        ShapeKind::Ellipse=>svg.push_str(&format!("<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"{fill}\" stroke=\"{c}\" stroke-width=\"{}\"/>",width/2.0,height/2.0,width/2.0-1.0,height/2.0-1.0,border.width)),
        ShapeKind::Line|ShapeKind::Arrow=> {
            svg.push_str(&format!("<line x1=\"2\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{c}\" stroke-width=\"{}\"/>",height/2.0,width-3.0,height/2.0,border.width));
            if kind==ShapeKind::Arrow {svg.push_str(&format!("<polyline points=\"{},{} {},{} {},{}\" fill=\"none\" stroke=\"{c}\" stroke-width=\"{}\"/>",width-12.0,height/2.0-6.0,width-3.0,height/2.0,width-12.0,height/2.0+6.0,border.width));}
        }
    }
    let mut offset = 0;
    let mut families = BTreeMap::new();
    for (key, face) in drawing_fonts(text) {
        families.entry(key.clone()).or_insert_with(|| {
            let mut db = resvg::usvg::fontdb::Database::new();
            db.load_font_data(face.font.to_vec());
            db.faces()
                .find(|info| info.index == face.index)
                .and_then(|info| info.families.first())
                .map(|(name, _)| name.clone())
                .unwrap_or(key)
        });
    }
    for row in &galley.rows {
        let shift = match text.alignment {
            Alignment::Center => (text_width - row.size.x) * 0.5,
            Alignment::Right => text_width - row.size.x,
            _ => 0.0,
        };
        // Preserve measured positions without breaking combining/ZWJ sequences.
        let row_text: String = text
            .chars
            .iter()
            .skip(offset)
            .take(row.glyphs.len())
            .map(|c| c.ch)
            .collect();
        let mut local = 0;
        for grapheme in row_text.graphemes(true) {
            let glyph = &row.glyphs[local];
            let count = grapheme.chars().count();
            let advance: f32 = row.glyphs[local..local + count]
                .iter()
                .map(|g| g.advance_width)
                .sum();
            let style = text
                .chars
                .get(offset + local)
                .map(|c| &c.style)
                .unwrap_or(&text.style);
            let x = inset_x + shift + glyph.pos.x;
            let y = inset_y + row.pos.y + glyph.pos.y;
            if let Some(fill) = style.highlight {
                svg.push_str(&format!(
                    "<rect x=\"{x}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
                    y - glyph.font_ascent,
                    advance,
                    glyph.font_height,
                    colour(fill)
                ));
            }
            svg.push_str(&format!("<text xml:space=\"preserve\" x=\"{x}\" y=\"{y}\" font-family=\"{}\" font-size=\"{}\" font-weight=\"{}\" font-style=\"{}\" fill=\"{}\">{}</text>",esc(&families[&drawing_key(style, grapheme)]),style.rendered_size(),if style.bold {"bold"}else{"normal"},if style.italic {"italic"}else{"normal"},colour(style.color),esc(grapheme)));
            for dy in [
                style.underline.then_some(1.0),
                style.strike.then_some(-style.rendered_size() * 0.3),
            ]
            .into_iter()
            .flatten()
            {
                svg.push_str(&format!("<line x1=\"{x}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"0.7\"/>",y+dy,x+advance,y+dy,colour(style.color)));
            }
            local += count;
        }
        offset += row.glyphs.len() + usize::from(row.ends_with_newline);
    }
    svg.push_str("</svg>");
    Ok(svg)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equations_and_all_basic_shapes_keep_editable_sources_and_safe_vectors() {
        let ctx = Context::default();
        for latex in [
            r"x^2+y^2=z^2",
            r"\frac{-b\pm\sqrt{b^2-4ac}}{2a}",
            r"\sum_{i=1}^{n} i",
            r"\int_0^1 x^2 dx",
        ] {
            let image = prepare(
                &ctx,
                ObjectSource::Equation {
                    latex: latex.into(),
                    size: 18.0,
                    colour: [31; 3],
                },
                0.0,
                0.0,
            )
            .unwrap();
            assert!(image.width > 5.0 && image.height > 10.0);
            assert!(image::load_from_memory(&image.data).is_ok());
            validate(image.editable.as_ref().unwrap()).unwrap();
            let standard = equation_omml(latex).unwrap();
            assert!(!latex_from_omml(&standard).unwrap().is_empty());
        }
        for kind in ShapeKind::ALL {
            let image = prepare(
                &ctx,
                ObjectSource::Drawing {
                    kind,
                    text: Box::new(Paragraph::from_text("Editable café text")),
                    fill: Some([211, 227, 253]),
                    border: CellBorder {
                        colour: [11, 87, 208],
                        width: 1.0,
                    },
                },
                240.0,
                80.0,
            )
            .unwrap();
            assert!(image.width >= 240.0);
            validate(image.editable.as_ref().unwrap()).unwrap();
        }
        for source in [
            r#"<svg><image href="file:///private.png"/></svg>"#,
            r#"<svg><use href="https://example.com/a"/></svg>"#,
            r#"<svg><path style="fill:url(file:///private.svg)"/></svg>"#,
            r#"<svg onload="execute()"/>"#,
        ] {
            assert!(validate_svg(source.as_bytes()).is_err());
        }
        assert!(equation_omml(&"x".repeat(4097)).is_err());
    }

    #[test]
    fn editable_objects_roundtrip_word_and_native_resize_and_undo_without_flattening_sources() {
        let ctx = Context::default();
        let mut doc = Document::new("Object interchange");
        let equation = prepare(
            &ctx,
            ObjectSource::Equation {
                latex: r"\frac{a}{b}+x^2".into(),
                size: 18.0,
                colour: [31; 3],
            },
            0.0,
            0.0,
        )
        .unwrap();
        let drawing = prepare(
            &ctx,
            ObjectSource::Drawing {
                kind: ShapeKind::TextBox,
                text: Box::new(Paragraph::from_text("Editable original café words")),
                fill: Some([211, 227, 253]),
                border: CellBorder {
                    colour: [11, 87, 208],
                    width: 1.5,
                },
            },
            240.0,
            70.0,
        )
        .unwrap();
        doc.blocks = vec![
            Block::Image(equation),
            Block::Image(drawing),
            Block::Paragraph(Paragraph::from_text("Body remains editable")),
        ];
        let original = doc.clone();
        let mut editor = crate::engine::Editor::new(doc);
        let Block::Image(equation) = &editor.doc.blocks[0] else {
            panic!()
        };
        let (w, h) = (equation.width, equation.height);
        editor.resize_image(0, w * 1.5, h * 1.5);
        let Block::Image(resized) = &editor.doc.blocks[0] else {
            panic!()
        };
        assert!(
            matches!(&resized.editable.as_ref().unwrap().source,ObjectSource::Equation{size,..} if (*size-27.0).abs()<0.01)
        );
        assert_eq!(
            resized.id,
            if let Block::Image(i) = &original.blocks[0] {
                &i.id
            } else {
                panic!()
            }
            .as_str()
        );
        editor.undo();
        assert_eq!(editor.doc, original);
        editor.resize_image(1, 180.0, 70.0);
        let Block::Image(resized) = &editor.doc.blocks[1] else {
            panic!()
        };
        assert!(
            matches!(&resized.editable.as_ref().unwrap().source,ObjectSource::Drawing{text,..} if text.text()=="Editable original café words" && text.style.size==11.0)
        );
        editor.undo();
        assert_eq!(editor.doc, original);
        let path = std::env::temp_dir().join(format!("alphadocs-objects-{}.docx", new_object_id()));
        crate::compat::docx::export(&editor.doc, &path).unwrap();
        let (loaded, warnings) = crate::compat::docx::import(&path).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(loaded.text(), editor.doc.text());
        assert_eq!(
            loaded
                .blocks
                .iter()
                .filter(|b| matches!(b,Block::Image(i) if i.editable.is_some()))
                .count(),
            2
        );
        let native: Document =
            serde_json::from_slice(&serde_json::to_vec(&editor.doc).unwrap()).unwrap();
        native.validate().unwrap();
        assert_eq!(native, editor.doc);
        std::fs::remove_file(path).unwrap();
    }
}
