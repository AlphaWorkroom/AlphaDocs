//! Standard native export writers; no browser, service or external converter.
mod odt;
mod rtf;
#[cfg(test)]
mod tests;
mod text;
mod web;
use crate::{model::*, persistence};
use std::{
    io::{Cursor, Write},
    path::Path,
};
use zip::{ZipWriter, write::SimpleFileOptions};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Native,
    Odt,
    Txt,
    Rtf,
    Html,
    Epub,
    Markdown,
}
impl Format {
    pub const ALL: [Self; 7] = [
        Self::Native,
        Self::Odt,
        Self::Txt,
        Self::Rtf,
        Self::Html,
        Self::Epub,
        Self::Markdown,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Native => "AlphaDocs document (.adocs)",
            Self::Odt => "OpenDocument Format (.odt)",
            Self::Txt => "Plain Text (.txt)",
            Self::Rtf => "Rich Text Format (.rtf)",
            Self::Html => "Web Page (.html, zipped)",
            Self::Epub => "EPUB Publication (.epub)",
            Self::Markdown => "Markdown (.md)",
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            Self::Native => "adocs",
            Self::Odt => "odt",
            Self::Txt => "txt",
            Self::Rtf => "rtf",
            Self::Html => "zip",
            Self::Epub => "epub",
            Self::Markdown => "md",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Native => {
                "Complete editable document, all tabs, original pictures and annotations"
            }
            Self::Odt => "Editable OpenDocument text, styles, tables and embedded pictures",
            Self::Txt => "UTF-8 text; table cells separated by tabs. No text styles or pictures",
            Self::Rtf => "Editable rich text, Unicode, links, tables and embedded pictures",
            Self::Html => "Offline HTML document and pictures in a ZIP archive",
            Self::Epub => "Reflowable EPUB 3 book with navigation, chapters and embedded pictures",
            Self::Markdown => {
                "Headings, emphasis, links, lists, tables and a companion picture folder"
            }
        }
    }
}
pub struct Report {
    pub notices: Vec<String>,
}
pub fn write(doc: &Document, format: Format, path: &Path) -> Result<Report, String> {
    doc.validate()?;
    let bytes = match format {
        Format::Native => {
            persistence::save_document(doc, path)?;
            return Ok(Report { notices: vec![] });
        }
        Format::Odt => odt::encode(doc)?,
        Format::Txt => text::plain(doc).into_bytes(),
        Format::Rtf => rtf::encode(doc)?,
        Format::Html => web::archive(doc)?,
        Format::Epub => web::epub(doc)?,
        Format::Markdown => return text::write_markdown(doc, path),
    };
    persistence::atomic_write(path, &bytes)?;
    Ok(report(doc, format))
}
fn report(doc: &Document, format: Format) -> Report {
    let mut notices = vec![];
    match format {
        Format::Txt=>notices.push("Plain text contains text and tab-separated table cells; text formatting and images are not included.".into()),
        Format::Markdown=>notices.push("Markdown preserves supported text structure and pictures in its companion folder; font sizes, colours and physical page layout are not represented.".into()),
        Format::Html|Format::Epub=>notices.push("Web and EPUB content reflows to the reader's viewport. Floating objects are placed in reading order; running headers/footers are included once.".into()),
        Format::Rtf=>notices.push("Pictures are placed in reading order; floating-image wrapping and offsets are not retained in RTF.".into()),
        Format::Odt|Format::Native=>{}
    }
    if format==Format::Odt && doc.tabs.iter().enumerate().flat_map(|(i,_)|doc.tab_blocks(i)).any(|b|matches!(b,Block::Image(i) if i.placement.flow!=ImageFlow::Break || i.placement.offset!=[0.0;2])) {
        notices.push("Floating pictures are placed in reading order in OpenDocument; their page offsets and wrapping are not retained.".into());
    }
    if format == Format::Rtf && doc.page.color != [255; 3] {
        notices.push(
            "RTF uses the reader's page background; the document page colour is not retained."
                .into(),
        );
    }
    if matches!(format, Format::Odt | Format::Rtf) {
        if doc.page.line_numbers != LineNumbering::Off {
            notices.push(format!(
                "{} does not retain AlphaDocs document line numbering.",
                if format == Format::Odt {
                    "OpenDocument"
                } else {
                    "RTF"
                }
            ));
        }
        if !doc.citations.is_empty() {
            notices.push(format!(
                "The AlphaDocs citation source library is not included in {}; citation text already inserted into the document is retained.",
                if format == Format::Odt {
                    "OpenDocument"
                } else {
                    "RTF"
                }
            ));
        }
        if doc.page.pageless {
            notices.push(format!(
                "{} uses paged layout; the document's pageless canvas setting is not retained.",
                if format == Format::Odt {
                    "OpenDocument"
                } else {
                    "RTF"
                }
            ));
        }
    }
    let paragraphs = doc
        .tabs
        .iter()
        .enumerate()
        .flat_map(|(i, _)| doc.tab_blocks(i))
        .flat_map(|b| match b {
            Block::Paragraph(p) => vec![p],
            Block::Table(rows) => rows.iter().flatten().collect(),
            _ => vec![],
        });
    if paragraphs.into_iter().any(|p| !p.annotations.is_empty())
        || doc
            .tabs
            .iter()
            .enumerate()
            .flat_map(|(i, _)| doc.tab_blocks(i))
            .any(|b| matches!(b,Block::Image(i) if !i.comments.is_empty()||!i.reactions.is_empty()))
    {
        notices.push("Local comments, reactions and collaboration history are not included in this export. Keep the AlphaDocs file to retain them.".into());
    }
    Report { notices }
}
fn xml(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c == '\t' || c == '\n' || c == '\r' || c >= ' ' => out.push(c),
            _ => {}
        }
    }
    out
}
fn colour(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}
fn runs(p: &Paragraph) -> Vec<(&TextStyle, String)> {
    let mut out: Vec<(&TextStyle, String)> = vec![];
    for ch in &p.chars {
        if let Some((style, text)) = out.last_mut()
            && **style == ch.style
        {
            text.push(ch.ch);
        } else {
            out.push((&ch.style, ch.ch.to_string()));
        }
    }
    out
}
fn safe_link(s: &str) -> Option<&str> {
    let s = s.trim();
    (!s.chars().any(char::is_control)
        && (s.starts_with("https://")
            || s.starts_with("http://")
            || s.starts_with("mailto:")
            || s.starts_with('#')))
    .then_some(s)
}
fn picture(i: &EmbeddedImage) -> Result<Vec<u8>, String> {
    crate::image_edit::flattened_png(i)
        .map_err(|e| format!("Could not export image '{}': {e}", i.name))
}
type Entry = (String, Vec<u8>);
fn zip(entries: Vec<Entry>, mimetype: Option<&str>) -> Result<Vec<u8>, String> {
    let mut zip = ZipWriter::new(Cursor::new(vec![]));
    if let Some(mime) = mimetype {
        zip.start_file(
            "mimetype",
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
        )
        .map_err(|e| e.to_string())?;
        zip.write_all(mime.as_bytes()).map_err(|e| e.to_string())?;
    }
    for (name, bytes) in entries {
        zip.start_file(
            name,
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )
        .map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    zip.finish()
        .map(|f| f.into_inner())
        .map_err(|e| e.to_string())
}
