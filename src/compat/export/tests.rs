use super::*;
use std::io::Read;
fn fixture() -> Document {
    let mut doc = Document::new("Formats & café");
    let mut heading = Paragraph::from_text("First chapter 😀");
    heading.set_heading(2);
    let mut p = Paragraph::from_text("Bold & <tag> {braces} \\ 茶 e\u{301}\nnext");
    p.chars[0].style.bold = true;
    p.chars[0].style.link = Some("https://example.com/?a=1&b=2".into());
    doc.blocks = vec![
        Block::Paragraph(heading),
        Block::Paragraph(p),
        Block::Image(EmbeddedImage {
            name: "Book".into(),
            alt_text: "Purple & white".into(),
            data: include_bytes!("../../../assets/alphadocs-icon.png")
                .to_vec()
                .into(),
            width: 120.0,
            height: 90.0,
            ..Default::default()
        }),
        Block::Table(vec![vec![
            Paragraph::from_text("Cell | one"),
            Paragraph::from_text("Cell two"),
        ]]),
    ];
    let mut editor = crate::engine::Editor::new(doc);
    editor.add_tab();
    editor.insert("Last tab survives");
    editor.doc
}
#[test]
fn native_download_round_trips_all_tabs_styles_original_media_without_mutation() {
    let mut doc = fixture();
    doc.citations.push(crate::writing::Citation {
        author: "Ada Lovelace".into(),
        title: "Notes".into(),
        year: "1843".into(),
        publisher: "Analytical Engine".into(),
        url: String::new(),
    });
    let original = doc.clone();
    let path = std::env::temp_dir().join(format!("alphadocs-native-export-{}.adocs", doc.id));
    let report = write(&doc, Format::Native, &path).unwrap();
    assert!(report.notices.is_empty());
    assert_eq!(persistence::load_document(&path).unwrap(), original);
    assert_eq!(doc, original);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn odt_and_rtf_reports_explain_paged_line_number_and_citation_limits() {
    let mut doc = Document::new("Export notices");
    doc.page.pageless = true;
    doc.page.line_numbers = LineNumbering::Continuous;
    doc.citations.push(crate::writing::Citation {
        author: "Ada Lovelace".into(),
        title: "Notes".into(),
        year: "1843".into(),
        publisher: "Analytical Engine".into(),
        url: String::new(),
    });
    doc.blocks = vec![Block::Paragraph(Paragraph::from_text("Lovelace (1843)"))];

    for format in [Format::Odt, Format::Rtf] {
        let notices = report(&doc, format).notices.join(" ");
        assert!(notices.contains("pageless canvas setting"), "{notices}");
        assert!(notices.contains("document line numbering"), "{notices}");
        assert!(notices.contains("citation source library"), "{notices}");
        assert!(
            notices.contains("citation text already inserted"),
            "{notices}"
        );
    }
    assert!(text::plain(&doc).contains("Lovelace (1843)"));
}

fn package(bytes: Vec<u8>) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut entries = std::collections::BTreeMap::new();
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).unwrap();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes).unwrap();
        entries.insert(file.name().to_owned(), bytes);
    }
    entries
}
fn valid_xml(bytes: &[u8]) {
    let mut reader = quick_xml::Reader::from_reader(bytes);
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(_) => {}
            Err(e) => panic!("Invalid XML: {e}"),
        }
    }
}
#[test]
fn packaged_formats_have_real_standard_parts_unicode_all_tabs_and_images() {
    let doc = fixture();
    let original = doc.clone();
    for format in [Format::Odt, Format::Epub, Format::Html] {
        let bytes = match format {
            Format::Odt => odt::encode(&doc).unwrap(),
            Format::Epub => web::epub(&doc).unwrap(),
            _ => web::archive(&doc).unwrap(),
        };
        if format != Format::Html {
            let mut zip = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
            let mime = zip.by_index(0).unwrap();
            assert_eq!(mime.name(), "mimetype");
            assert_eq!(mime.compression(), zip::CompressionMethod::Stored);
        }
        let parts = package(bytes);
        let joined = parts
            .values()
            .filter_map(|b| std::str::from_utf8(b).ok())
            .collect::<String>();
        assert!(
            joined.contains("Last tab survives")
                || joined.contains("Last<text:s text:c=\"1\"/>tab<text:s text:c=\"1\"/>survives"),
            "{format:?} lost inactive-tab text"
        );
        assert!(joined.contains("First"));
        assert!(joined.contains("😀"));
        assert!(joined.contains("&amp;"));
        assert!(!joined.contains("<tag>"));
        let images: Vec<_> = parts
            .iter()
            .filter(|(name, _)| name.ends_with(".png"))
            .collect();
        assert_eq!(images.len(), 1);
        assert!(image::load_from_memory(images[0].1).is_ok());
        for (name, bytes) in &parts {
            if name.ends_with(".xml")
                || name.ends_with(".xhtml")
                || name.ends_with(".opf")
                || name.ends_with(".html")
            {
                valid_xml(bytes);
            }
        }
    }
    assert_eq!(doc, original);
}
#[test]
fn rtf_uses_native_unicode_tables_links_and_picture_controls() {
    let text = String::from_utf8(rtf::encode(&fixture()).unwrap()).unwrap();
    assert!(text.starts_with("{\\rtf1"));
    assert!(text.contains("\\u-10179?\\u-8704?"));
    assert!(text.contains("\\trowd"));
    assert!(text.contains("\\cell"));
    assert!(text.contains("\\pngblip"));
    assert!(text.contains("HYPERLINK"));
    assert!(text.contains("\\{braces\\}"));
    assert!(text.contains("Last tab survives"));
}
#[test]
fn markdown_images_are_portable_files_and_failures_keep_existing_document() {
    let doc = fixture();
    let root = std::env::temp_dir().join(format!("alphadocs-export-{}", doc.id));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("Book.md");
    write(&doc, Format::Markdown, &path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("## "));
    assert!(text.contains("First chapter 😀"));
    assert!(text.contains("Last tab survives"));
    assert!(text.contains("Book_files/image-"));
    assert!(text.contains("Cell \\| one"));
    let assets = root.join("Book_files");
    let image = std::fs::read_dir(&assets)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(image::load_from_memory(&std::fs::read(&image).unwrap()).is_ok());
    write(&doc, Format::Markdown, &path).unwrap();
    assert_eq!(std::fs::read_dir(&assets).unwrap().count(), 1);
    std::fs::write(&image, b"conflicting existing data").unwrap();
    assert!(write(&doc, Format::Markdown, &path).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    assert_eq!(std::fs::read(&image).unwrap(), b"conflicting existing data");
    std::fs::remove_file(image).unwrap();
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(assets).unwrap();
    std::fs::remove_dir(root).unwrap();
}
#[test]
fn text_exports_are_utf8_and_invalid_media_cannot_truncate_existing_files() {
    let mut doc = fixture();
    let text = text::plain(&doc);
    assert!(text.contains("😀"));
    assert!(text.contains("Cell | one\tCell two"));
    assert!(text.contains("\x0c\r\nLast tab survives"));
    let first = &mut doc.tabs[0].blocks;
    let Block::Image(image) = &mut first[2] else {
        panic!()
    };
    image.data = b"invalid image".to_vec().into();
    let path = std::env::temp_dir().join(format!("alphadocs-export-existing-{}.odt", doc.id));
    std::fs::write(&path, b"keep this file").unwrap();
    assert!(write(&doc, Format::Odt, &path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"keep this file");
    std::fs::remove_file(path).unwrap();
}
