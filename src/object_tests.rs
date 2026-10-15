use crate::{
    compat,
    engine::Editor,
    fonts,
    layout::{Layout, Object},
    model::*,
    persistence,
};
use eframe::egui::{self, Rect};

fn image() -> EmbeddedImage {
    let pixels = image::RgbaImage::from_pixel(16, 16, image::Rgba([40, 120, 210, 220]));
    let mut bytes = std::io::Cursor::new(vec![]);
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    EmbeddedImage {
        data: bytes.into_inner().into(),
        width: 120.0,
        height: 150.0,
        name: "QA image".into(),
        ..Default::default()
    }
}
fn measured(doc: &Document) -> Layout {
    let ctx = egui::Context::default();
    fonts::install(&ctx);
    let mut layout = None;
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {
        layout = Some(Layout::measure(&ctx, doc));
    });
    output.textures_delta.clear();
    layout.unwrap()
}
#[test]
fn measured_wrapping_uses_both_sides_then_full_width_without_losing_characters() {
    let mut doc = Document::new("Wrapping");
    let mut image = image();
    image.placement.flow = ImageFlow::Wrap;
    image.placement.offset = [145.0, 0.0];
    image.placement.margins = [10.0; 4];
    let paragraph = Paragraph::from_text(&"Unicode café words flow around an image. ".repeat(45));
    doc.blocks = vec![
        Block::Image(image),
        Block::Paragraph(paragraph.clone()),
        Block::Table(vec![vec![Paragraph::from_text("After the float")]]),
    ];
    let l = measured(&doc);
    let Object::Image { rect, .. } = l.objects[0] else {
        panic!()
    };
    let exclusion = rect.expand(10.0);
    let lines: Vec<_> = l
        .lines
        .iter()
        .filter(|line| line.address.block == 1)
        .collect();
    assert!(
        lines
            .iter()
            .any(|line| line.rect.right() <= exclusion.left() + 0.01
                && line.rect.top() < exclusion.bottom())
    );
    assert!(
        lines
            .iter()
            .any(|line| line.rect.left() >= exclusion.right() - 0.01
                && line.rect.top() < exclusion.bottom())
    );
    assert!(
        lines
            .iter()
            .any(|line| line.rect.width() > rect.width() && line.rect.top() >= exclusion.bottom())
    );
    assert!(
        lines
            .iter()
            .all(|line| line.rect.intersect(exclusion).area() <= 0.001),
        "Overlaps: {:?}, exclusion: {:?}",
        lines
            .iter()
            .filter(|l| l.rect.intersect(exclusion).area() > 0.001)
            .map(|l| l.rect)
            .collect::<Vec<_>>(),
        exclusion
    );
    let mut offset = 0;
    for line in lines {
        assert_eq!(line.start, offset);
        offset = line.end;
    }
    assert_eq!(offset, paragraph.chars.len());
    assert!(
        l.lines
            .iter()
            .filter(|line| line.address.block == 2)
            .all(|line| line.page > 0 || line.rect.top() >= exclusion.bottom())
    );
}
#[test]
fn wrap_soft_break_has_a_trailing_editable_line_and_side_restrictions() {
    for side in [WrapSide::Left, WrapSide::Right, WrapSide::Largest] {
        let mut doc = Document::new("Soft break");
        let mut image = image();
        image.placement.flow = ImageFlow::Wrap;
        image.placement.offset = [145.0, 0.0];
        image.placement.wrap_side = side;
        doc.blocks = vec![
            Block::Image(image),
            Block::Paragraph(Paragraph::from_text("Café\n")),
        ];
        let l = measured(&doc);
        let lines: Vec<_> = l.lines.iter().filter(|l| l.address.block == 1).collect();
        assert_eq!(lines.len(), 2);
        assert_eq!((lines[1].start, lines[1].end), (5, 5));
        assert!(lines[1].rect.top() > lines[0].rect.top());
        let Object::Image { rect, .. } = l.objects[0] else {
            panic!()
        };
        if side == WrapSide::Left {
            assert!(lines[0].rect.right() < rect.left());
        }
        if side == WrapSide::Right {
            assert!(lines[0].rect.left() > rect.right());
        }
    }
}
#[test]
fn image_threads_and_reactions_keep_their_anchor_across_move_tabs_undo_and_roundtrips() {
    let mut doc = Document::new("Object threads");
    doc.blocks = vec![
        Block::Image(image()),
        Block::Paragraph(Paragraph::from_text("Following text")),
    ];
    let mut e = Editor::new(doc);
    assert!(e.add_image_comment(0, "A useful comment"));
    let Block::Image(i) = &e.doc.blocks[0] else {
        panic!()
    };
    let comment = i.comments[0].id.clone();
    let object_id = i.id.clone();
    let bytes = i.data.clone();
    assert!(e.reply_image_comment(0, &comment, "Reply retained"));
    assert!(e.edit_image_comment(0, &comment, "Edited café comment & \"quotes\""));
    e.delete_image_comment(0, &comment);
    e.undo();
    let Block::Image(i) = &e.doc.blocks[0] else {
        panic!()
    };
    assert_eq!(i.comments[0].text, "Edited café comment & \"quotes\"");
    assert!(e.toggle_image_reaction(0, "😀"));
    e.resolve_image_comment(0, &comment, true);
    e.undo();
    let mut placement = ImagePlacement {
        flow: ImageFlow::Wrap,
        offset: [100.0, 3.125],
        margins: [2.3, 4.25, 6.0, 8.75],
        wrap_side: WrapSide::Right,
        snap: ImageSnap::NearTextLines,
        guides: false,
        snap_guides: true,
    };
    assert!(e.set_image_placement(0, placement));
    let before_move = e.doc.clone();
    assert_eq!(e.move_image(0, 2), Some(1));
    let Block::Image(i) = &e.doc.blocks[1] else {
        panic!()
    };
    assert_eq!(i.id, object_id);
    assert_eq!(i.data, bytes);
    assert_eq!(i.comments[0].replies[0], "Reply retained");
    assert_eq!(i.reactions[0], "😀");
    e.undo();
    assert_eq!(e.doc, before_move);
    e.redo();
    let saved = e.doc.clone();
    e.add_tab();
    e.switch_tab(0);
    assert_eq!(e.doc.blocks, saved.blocks);
    for flow in [
        ImageFlow::Break,
        ImageFlow::Wrap,
        ImageFlow::Behind,
        ImageFlow::Front,
    ] {
        placement.flow = flow;
        e.set_image_placement(1, placement);
        let native = std::env::temp_dir().join(format!("{}.adocs", e.doc.id));
        let word = native.with_extension("docx");
        persistence::save_document(&e.doc, &native).unwrap();
        let reopened = persistence::load_document(&native).unwrap();
        assert_eq!(reopened, e.doc);
        compat::docx::export(&reopened, &word).unwrap();
        let (imported, warnings) = compat::docx::import(&word).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(imported.blocks, e.doc.blocks);
        std::fs::remove_file(native).unwrap();
        std::fs::remove_file(word).unwrap();
    }
    assert!(!e.toggle_image_reaction(1, "invalid"));
    assert!(!e.add_image_comment(1, "   "));
    let mut invalid = placement;
    invalid.offset[0] = f32::NAN;
    assert!(!e.set_image_placement(1, invalid));
}
#[test]
fn previous_native_image_schema_gets_distinct_stable_object_ids_on_save() {
    let mut doc = Document::new("Legacy image IDs");
    let a = image();
    let mut b = a.clone();
    b.id = new_object_id();
    doc.blocks = vec![
        Block::Image(a),
        Block::Image(b),
        Block::Paragraph(Paragraph::default()),
    ];
    let mut json = serde_json::to_value(&doc).unwrap();
    json["version"] = serde_json::json!(4);
    for block in json["blocks"].as_array_mut().unwrap() {
        if let Some(image) = block.get_mut("Image").and_then(|i| i.as_object_mut()) {
            for field in ["id", "placement", "comments", "reactions"] {
                image.remove(field);
            }
        }
    }
    let path = std::env::temp_dir().join(format!("{}.adocs", doc.id));
    std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    let legacy = persistence::load_document(&path).unwrap();
    let Block::Image(a) = &legacy.blocks[0] else {
        panic!()
    };
    let Block::Image(b) = &legacy.blocks[1] else {
        panic!()
    };
    assert_ne!(a.id, b.id);
    let ids = (a.id.clone(), b.id.clone());
    persistence::save_document(&legacy, &path).unwrap();
    let reopened = persistence::load_document(&path).unwrap();
    assert_eq!(reopened.version, DOCUMENT_VERSION);
    let Block::Image(a) = &reopened.blocks[0] else {
        panic!()
    };
    let Block::Image(b) = &reopened.blocks[1] else {
        panic!()
    };
    assert_eq!((a.id.clone(), b.id.clone()), ids);
    std::fs::remove_file(path).unwrap();
}
#[test]
fn external_docx_position_and_wrap_edits_override_stale_extensions() {
    use std::io::{Read, Write};
    let mut doc = Document::new("External positions");
    let mut image = image();
    image.placement = ImagePlacement {
        flow: ImageFlow::Wrap,
        offset: [100.0, 20.0],
        margins: [7.0; 4],
        wrap_side: WrapSide::Right,
        snap: ImageSnap::TextLines,
        guides: false,
        snap_guides: true,
    };
    doc.blocks = vec![
        Block::Image(image),
        Block::Paragraph(Paragraph::from_text("Text")),
    ];
    let path = std::env::temp_dir().join(format!("{}.docx", doc.id));
    compat::docx::export(&doc, &path).unwrap();
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(std::fs::read(&path).unwrap())).unwrap();
    let mut parts = vec![];
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let mut data = vec![];
        entry.read_to_end(&mut data).unwrap();
        let name = entry.name().to_owned();
        if name == "word/document.xml" {
            let xml = String::from_utf8(data)
                .unwrap()
                .replace(
                    "<wp:posOffset>1270000</wp:posOffset>",
                    "<wp:posOffset>2540000</wp:posOffset>",
                )
                .replace("wrapText=\"right\"", "wrapText=\"left\"")
                .replace("distB=\"88900\"", "distB=\"127000\"");
            data = xml.into_bytes();
        }
        parts.push((name, data));
    }
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(vec![]));
    for (name, data) in parts {
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&data).unwrap();
    }
    std::fs::write(&path, writer.finish().unwrap().into_inner()).unwrap();
    let (imported, warnings) = compat::docx::import(&path).unwrap();
    assert!(warnings.is_empty());
    let Block::Image(image) = &imported.blocks[0] else {
        panic!()
    };
    assert_eq!(image.placement.offset, [200.0, 20.0]);
    assert_eq!(image.placement.wrap_side, WrapSide::Left);
    assert_eq!(image.placement.margins[1], 10.0);
    assert_eq!(image.placement.snap, ImageSnap::TextLines);
    assert!(!image.placement.guides);
    assert!(image.placement.snap_guides);
    std::fs::remove_file(path).unwrap();
}
#[test]
fn whole_page_wrap_does_not_overlap_text_or_create_character_based_page_breaks() {
    let mut doc = Document::new("Full width float");
    let mut image = image();
    image.width = 450.0;
    image.placement.flow = ImageFlow::Wrap;
    doc.blocks = vec![
        Block::Image(image),
        Block::Paragraph(Paragraph::from_text(&"Measured page flow. ".repeat(3000))),
    ];
    let l = measured(&doc);
    let Object::Image { rect, .. } = l.objects[0] else {
        panic!()
    };
    assert!(l.pages > 1);
    assert!(
        l.lines
            .iter()
            .all(|line| line.page != 0 || !line.rect.intersects(rect))
    );
    assert!(l.lines.iter().all(|line| {
        Rect::from_min_max(
            egui::pos2(72.0, 72.0),
            egui::pos2(l.size.x - 72.0, l.size.y - 72.0),
        )
        .contains_rect(line.rect)
    }));
}
