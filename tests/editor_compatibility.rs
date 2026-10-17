use alphadocs::{compat::docx, editable_objects, engine, model::*};
use std::{
    fs::File,
    io::{Read, Write},
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

#[test]
fn horizontal_word_lines_use_zero_vertical_geometry_and_keep_native_object_boxes() {
    let ctx = eframe::egui::Context::default();
    let mut doc = Document::new("Horizontal Word geometry");
    for kind in [ShapeKind::Line, ShapeKind::Arrow] {
        let mut image = editable_objects::prepare(
            &ctx,
            ObjectSource::Drawing {
                kind,
                text: Box::default(),
                fill: None,
                border: CellBorder::default(),
            },
            240.0,
            80.0,
        )
        .unwrap();
        image.placement.offset = [12.0, 24.0];
        doc.blocks.push(Block::Image(image));
    }
    let path = std::env::temp_dir().join(format!("alphadocs-line-{}.docx", new_object_id()));
    docx::export(&doc, &path).unwrap();
    let mut archive = ZipArchive::new(File::open(&path).unwrap()).unwrap();
    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    assert_eq!(
        xml.matches("<wp:extent cx=\"3048000\" cy=\"0\"/>").count(),
        2
    );
    assert_eq!(xml.matches("<a:ext cx=\"3048000\" cy=\"0\"/>").count(), 2);
    let mut settings = String::new();
    archive
        .by_name("word/settings.xml")
        .unwrap()
        .read_to_string(&mut settings)
        .unwrap();
    assert!(settings.contains("compatibilityMode") && settings.contains("w:val=\"15\""));
    drop(archive);
    let (loaded, warnings) = docx::import(&path).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    let images = |doc: &Document| {
        doc.blocks
            .iter()
            .filter_map(|b| {
                if let Block::Image(i) = b {
                    Some(i.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    for (before, after) in images(&doc).iter().zip(images(&loaded)) {
        assert_eq!((before.width, before.height), (after.width, after.height));
        assert_eq!(before.placement.offset, after.placement.offset);
        assert_eq!(
            before.editable.as_ref().unwrap().source,
            after.editable.as_ref().unwrap().source
        );
    }
    // Word removes private attributes. Standard paragraph spacing must retain
    // the native anchor box and offset after a real external save.
    let standard_path = path.with_extension("standard.docx");
    let mut archive = ZipArchive::new(File::open(&path).unwrap()).unwrap();
    let mut output = ZipWriter::new(File::create(&standard_path).unwrap());
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let name = entry.name().to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if name == "word/document.xml" {
            bytes = String::from_utf8(bytes)
                .unwrap()
                .replace(" alphadocs:lineBoxHeight=\"80\"", "")
                .into_bytes();
        }
        output
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    output.finish().unwrap();
    let (standard, warnings) = docx::import(&standard_path).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    for (before, after) in images(&doc).iter().zip(images(&standard)) {
        assert_eq!((before.width, before.height), (after.width, after.height));
        assert_eq!(before.placement.offset, after.placement.offset);
    }
    drop(archive);
    std::fs::remove_file(standard_path).unwrap();
    std::fs::remove_file(path).unwrap();
}

#[test]
fn drawing_symbols_embed_real_fallback_fonts_and_preserve_emoji_sequences() {
    let image = editable_objects::prepare(
        &eframe::egui::Context::default(),
        ObjectSource::Drawing {
            kind: ShapeKind::TextBox,
            text: Box::new(Paragraph::from_text("café 👩🏽‍💻 ⟶ ┌──┐")),
            fill: None,
            border: CellBorder::default(),
        },
        400.0,
        80.0,
    )
    .unwrap();
    let object = image.editable.as_ref().unwrap();
    assert!(object.svg.contains(">👩🏽‍💻</text>"));
    assert!(object.svg.contains(">⟶</text>"));
    let fonts = editable_objects::source_fonts(&object.source);
    assert!(fonts.contains_key("Emoji"));
    assert!(fonts.contains_key("Math") || fonts.contains_key("Mono"));
    assert!(image::load_from_memory(&image.data).is_ok());
}

#[test]
fn two_equations_in_one_word_paragraph_are_both_retained() {
    let path = std::env::temp_dir().join(format!("alphadocs-two-math-{}.docx", new_object_id()));
    let mut archive = ZipWriter::new(File::create(&path).unwrap());
    archive
        .start_file("word/document.xml", SimpleFileOptions::default())
        .unwrap();
    write!(archive,r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"><w:body><w:p><w:r><w:t>Before and after</w:t></w:r><m:oMath><m:r><m:t>x+1</m:t></m:r></m:oMath><m:oMath><m:r><m:t>y+2</m:t></m:r></m:oMath></w:p></w:body></w:document>"#).unwrap();
    archive.finish().unwrap();
    let (doc, warnings) = docx::import(&path).unwrap();
    assert!(doc.text().contains("Before and after"));
    assert_eq!(
        doc.blocks
            .iter()
            .filter(|b| matches!(b,Block::Image(i) if i.editable.is_some()))
            .count(),
        2
    );
    assert!(warnings.iter().any(|w| w.contains("Inline equations")));
    doc.validate().unwrap();
    std::fs::remove_file(path).unwrap();
}

#[test]
fn drawing_text_edits_preserve_unchanged_rich_runs_and_soft_newlines() {
    let mut paragraph = Paragraph::from_text("Bold café 👩🏽‍💻 ending");
    for ch in &mut paragraph.chars[..4] {
        ch.style.bold = true;
    }
    for ch in paragraph.chars.iter_mut().rev().take(6) {
        ch.style.italic = true;
    }
    let edited = engine::edit_drawing_text(&paragraph, "Bold café 👩🏽‍💻\nnew ending");
    assert_eq!(edited.text(), "Bold café 👩🏽‍💻\nnew ending");
    assert!(edited.chars[..4].iter().all(|c| c.style.bold));
    assert!(edited.chars.iter().rev().take(6).all(|c| c.style.italic));
    assert_eq!(paragraph.text(), "Bold café 👩🏽‍💻 ending");
}

#[test]
fn shorthand_table_input_is_stored_as_a_standard_word_formula_instruction() {
    let mut editor = engine::Editor::new(Document::new("Formula input"));
    editor.insert_table(1, 1);
    editor
        .set_table_formula(TableFormula {
            expression: " 2+3 ".into(),
            decimals: None,
            word_number_format: None,
        })
        .unwrap();
    let paragraph = editor.doc.paragraph(editor.caret.address).unwrap();
    assert_eq!(paragraph.text(), "5");
    assert_eq!(paragraph.formula.as_ref().unwrap().expression, "=2+3");
    let original = editor.doc.clone();
    assert!(
        editor
            .set_table_formula(TableFormula {
                expression: "=2+3".into(),
                decimals: None,
                word_number_format: Some("$0".into()),
            })
            .is_err()
    );
    assert_eq!(editor.doc, original);
}
