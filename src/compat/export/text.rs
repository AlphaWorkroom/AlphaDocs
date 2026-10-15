use super::*;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::PathBuf,
};
fn table_text(text: &str) -> String {
    if text.contains(['\t', '\n', '\r', '"']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.into()
    }
}
pub(super) fn plain(doc: &Document) -> String {
    let mut out = String::new();
    if !doc.page.header.chars.is_empty() {
        out.push_str(&doc.page.header.text());
        out.push_str("\r\n\r\n");
    }
    for (tab, _) in doc.tabs.iter().enumerate() {
        if tab > 0 {
            out.push_str("\x0c\r\n");
        }
        let mut number = 0;
        for b in doc.tab_blocks(tab) {
            match b {
                Block::Paragraph(p) => {
                    match p.list {
                        ListKind::Number => {
                            number += 1;
                            out.push_str(&format!("{number}. "));
                        }
                        ListKind::Bullet => {
                            number = 0;
                            out.push_str("• ");
                        }
                        ListKind::Check => {
                            number = 0;
                            out.push_str(if p.checked { "☑ " } else { "☐ " });
                        }
                        ListKind::None => number = 0,
                    }
                    out.push_str(&p.text().replace('\n', "\r\n"));
                    out.push_str("\r\n");
                }
                Block::Table(rows) => {
                    number = 0;
                    for row in rows {
                        out.push_str(
                            &row.iter()
                                .map(|p| table_text(&p.text()))
                                .collect::<Vec<_>>()
                                .join("\t"),
                        );
                        out.push_str("\r\n");
                    }
                }
                Block::PageBreak => {
                    number = 0;
                    out.push_str("\x0c\r\n");
                }
                Block::Rule => {
                    number = 0;
                    out.push_str("────────\r\n");
                }
                Block::Image(_) => number = 0,
            }
        }
    }
    if !doc.page.footer.chars.is_empty() {
        out.push_str("\r\n");
        out.push_str(&doc.page.footer.text());
        out.push_str("\r\n");
    }
    out
}
fn escape(text: &str, cell: bool) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '\n' => out.push_str(if cell { "<br />" } else { "  \n" }),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            c if c.is_ascii_punctuation() => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}
fn uri(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/:@?#=&%+".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
fn inline(p: &Paragraph, cell: bool) -> String {
    let mut out = String::new();
    for (style, text) in runs(p) {
        let core = text.trim();
        let prefix = &text[..text.len() - text.trim_start().len()];
        let suffix = &text[text.trim_end().len()..];
        if core.is_empty() {
            out.push_str(&escape(&text, cell));
            continue;
        }
        out.push_str(&escape(prefix, cell));
        let mut text = escape(core, cell);
        if style.bold {
            text = format!("**{text}**");
        }
        if style.italic {
            text = format!("_{text}_");
        }
        if style.strike {
            text = format!("~~{text}~~");
        }
        if style.underline {
            text = format!("<u>{text}</u>");
        }
        match style.baseline {
            Baseline::Superscript => text = format!("<sup>{text}</sup>"),
            Baseline::Subscript => text = format!("<sub>{text}</sub>"),
            Baseline::Normal => {}
        }
        if let Some(link) = style.link.as_deref().and_then(safe_link) {
            text = format!("[{text}](<{}>)", uri(link));
        }
        out.push_str(&text);
        out.push_str(&escape(suffix, cell));
    }
    out
}
fn markdown(
    doc: &Document,
    images: &mut impl FnMut(&EmbeddedImage) -> Result<String, String>,
) -> Result<String, String> {
    let mut out = String::new();
    if !doc.page.header.chars.is_empty() {
        out.push_str(&inline(&doc.page.header, false));
        out.push_str("\n\n");
    }
    for (tab, _) in doc.tabs.iter().enumerate() {
        if tab > 0 {
            out.push_str("\n---\n\n");
        }
        let mut number = 0;
        for b in doc.tab_blocks(tab) {
            match b {
                Block::Paragraph(p) => {
                    if p.page_break_before {
                        out.push_str("<div style=\"break-before:page\"></div>\n\n");
                    }
                    if p.heading > 0 && p.heading != 8 {
                        out.push_str(&"#".repeat(if p.heading == 7 {
                            1
                        } else {
                            usize::from(p.heading.min(6))
                        }));
                        out.push(' ');
                    }
                    match p.list {
                        ListKind::Number => {
                            number += 1;
                            out.push_str(&format!("{number}. "));
                        }
                        ListKind::Bullet => {
                            number = 0;
                            out.push_str("- ");
                        }
                        ListKind::Check => {
                            number = 0;
                            out.push_str(if p.checked { "- [x] " } else { "- [ ] " });
                        }
                        ListKind::None => number = 0,
                    }
                    let text = inline(p, false);
                    out.push_str(&text);
                    out.push_str("\n\n");
                }
                Block::Table(rows) => {
                    number = 0;
                    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
                    // GFM requires a header row. Use an empty one so real cell content
                    // isn't falsely promoted to a heading or dropped.
                    out.push('|');
                    for _ in 0..columns {
                        out.push_str("   |");
                    }
                    out.push_str("\n|");
                    for _ in 0..columns {
                        out.push_str(" --- |");
                    }
                    out.push('\n');
                    for row in rows {
                        out.push('|');
                        for cell in row {
                            out.push(' ');
                            out.push_str(&inline(cell, true));
                            out.push_str(" |");
                        }
                        out.push('\n');
                    }
                    out.push('\n');
                }
                Block::Image(i) => {
                    number = 0;
                    out.push_str(&format!(
                        "![{}](<{}>)\n\n",
                        escape(
                            if i.alt_text.is_empty() {
                                &i.name
                            } else {
                                &i.alt_text
                            },
                            false
                        ),
                        images(i)?
                    ));
                }
                Block::PageBreak => {
                    number = 0;
                    out.push_str("<div style=\"break-before:page\"></div>\n\n");
                }
                Block::Rule => {
                    number = 0;
                    out.push_str("---\n\n");
                }
            }
        }
    }
    if !doc.page.footer.chars.is_empty() {
        out.push_str(&inline(&doc.page.footer, false));
        out.push_str("\n\n");
    }
    Ok(out)
}
pub(super) fn write_markdown(doc: &Document, path: &Path) -> Result<Report, String> {
    let stem = path
        .file_stem()
        .ok_or("Markdown export requires a file name")?
        .to_string_lossy();
    let directory = path.with_file_name(format!("{stem}_files"));
    let mut assets: Vec<Entry> = vec![];
    let text = markdown(doc, &mut |i| {
        let bytes = picture(i)?;
        let mut hash = DefaultHasher::new();
        bytes.hash(&mut hash);
        let name = format!("image-{:016x}.png", hash.finish());
        if let Some((_, old)) = assets.iter().find(|(n, _)| *n == name) {
            if *old != bytes {
                return Err("Image asset hash collision; export was not written".into());
            }
        } else {
            assets.push((name.clone(), bytes));
        }
        Ok(uri(&format!("{stem}_files/{name}")))
    })?;
    let mut created: Vec<PathBuf> = vec![];
    let result = (|| {
        if !assets.is_empty() {
            if let Ok(meta) = std::fs::symlink_metadata(&directory)
                && (meta.file_type().is_symlink() || !meta.is_dir())
            {
                return Err("Markdown picture folder must be a normal directory".into());
            }
            std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
            for (name, bytes) in assets {
                let target = directory.join(name);
                if let Ok(meta) = std::fs::symlink_metadata(&target) {
                    if meta.file_type().is_symlink()
                        || !meta.is_file()
                        || std::fs::read(&target).map_err(|e| e.to_string())? != bytes
                    {
                        return Err("An existing Markdown asset conflicts with this export; existing files were kept".into());
                    }
                } else {
                    persistence::atomic_write(&target, &bytes)?;
                    created.push(target);
                }
            }
        }
        persistence::atomic_write(path, text.as_bytes())
    })();
    if result.is_err() {
        for target in created {
            let _ = std::fs::remove_file(target);
        }
    }
    result?;
    Ok(report(doc, Format::Markdown))
}
