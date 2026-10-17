use super::*;
struct Web {
    assets: Vec<Entry>,
    image: usize,
}
fn box_css(style: BoxStyle) -> String {
    let mut out = style
        .fill
        .map(|c| format!("background-color:{};", colour(c)))
        .unwrap_or_default();
    for (edge, side) in style.borders.iter().zip(["top", "bottom", "left", "right"]) {
        out.push_str(&format!(
            "border-{side}:{}pt solid {};",
            edge.width,
            colour(edge.colour)
        ));
    }
    out
}
fn css_string(s: &str) -> String {
    s.chars()
        .map(|c| {
            if matches!(c, '\'' | '"' | '\\' | '<' | '>') || c.is_control() {
                format!("\\{:x} ", c as u32)
            } else {
                c.to_string()
            }
        })
        .collect()
}
fn text_style(s: &TextStyle) -> String {
    let mut css = format!(
        "font-family:'{}';font-size:{}pt;color:{};font-weight:{};font-style:{};",
        css_string(&s.font),
        s.size,
        colour(s.color),
        if s.bold { "bold" } else { "normal" },
        if s.italic { "italic" } else { "normal" }
    );
    if s.underline || s.strike {
        css.push_str(&format!(
            "text-decoration:{} {};",
            if s.underline { "underline" } else { "" },
            if s.strike { "line-through" } else { "" }
        ));
    }
    if let Some(c) = s.highlight {
        css.push_str(&format!("background-color:{};", colour(c)));
    }
    if s.baseline != Baseline::Normal {
        css.push_str(&format!(
            "vertical-align:{};font-size:70%;",
            if s.baseline == Baseline::Superscript {
                "super"
            } else {
                "sub"
            }
        ));
    }
    css
}
fn inline(p: &Paragraph) -> String {
    let mut out = String::new();
    for (s, text) in runs(p) {
        let text = xml(&text).replace('\n', "<br />").replace('\t', "&#9;");
        let span = format!("<span style=\"{}\">{text}</span>", xml(&text_style(s)));
        if let Some(link) = s.link.as_deref().and_then(safe_link) {
            out.push_str(&format!("<a href=\"{}\">{span}</a>", xml(link)));
        } else {
            out.push_str(&span);
        }
    }
    if out.is_empty() {
        out.push_str("<br />");
    }
    out
}
fn paragraph(p: &Paragraph, id: &str, in_cell: bool) -> String {
    let tag = match p.heading {
        1..=6 => format!("h{}", p.heading),
        7 => "h1".into(),
        _ => "p".into(),
    };
    let mut style = format!(
        "{}text-align:{};margin-top:{}pt;margin-bottom:{}pt;margin-left:{}pt;text-indent:{}pt;line-height:{};{}{}{}",
        text_style(&p.style),
        match p.alignment {
            Alignment::Left => "left",
            Alignment::Center => "center",
            Alignment::Right => "right",
            Alignment::Justified => "justify",
        },
        p.before,
        p.after,
        p.indent,
        p.first_indent,
        p.line_spacing,
        if p.page_break_before {
            "break-before:page;"
        } else {
            ""
        },
        if p.keep_together {
            "break-inside:avoid;"
        } else {
            ""
        },
        if p.keep_next {
            "break-after:avoid;"
        } else {
            ""
        }
    );
    if !in_cell && let Some(decoration) = p.box_style {
        style.push_str(&box_css(decoration));
    }
    format!(
        "<{tag} id=\"{}\" style=\"{}\">{}{}</{tag}>\n",
        xml(id),
        xml(&style),
        if p.list == ListKind::Check {
            if p.checked { "☑ " } else { "☐ " }
        } else {
            ""
        },
        inline(p)
    )
}
impl Web {
    fn blocks(&mut self, blocks: &[Block], tab: usize) -> Result<String, String> {
        let mut out = String::new();
        let mut list = None;
        for (index, block) in blocks.iter().enumerate() {
            let kind = match block {
                Block::Paragraph(p) if p.list != ListKind::None => {
                    Some(if p.list == ListKind::Number {
                        "ol"
                    } else {
                        "ul"
                    })
                }
                _ => None,
            };
            if kind != list {
                if let Some(tag) = list {
                    out.push_str(&format!("</{tag}>\n"));
                }
                if let Some(tag) = kind {
                    out.push_str(&format!("<{tag}>\n"));
                }
                list = kind;
            }
            match block {
                Block::Paragraph(p) => {
                    if kind.is_some() {
                        out.push_str("<li>");
                    }
                    out.push_str(&paragraph(p, &format!("p{tab}-{index}"), false));
                    if kind.is_some() {
                        out.push_str("</li>\n");
                    }
                }
                Block::Table(rows) => {
                    out.push_str("<table><tbody>\n");
                    for (r, row) in rows.iter().enumerate() {
                        out.push_str("<tr>");
                        for (c, p) in row.iter().enumerate() {
                            out.push_str(&format!(
                                "<td style=\"{}\">",
                                box_css(p.box_style.unwrap_or_default())
                            ));
                            out.push_str(&paragraph(p, &format!("p{tab}-{index}-{r}-{c}"), true));
                            out.push_str("</td>");
                        }
                        out.push_str("</tr>\n");
                    }
                    out.push_str("</tbody></table>\n");
                }
                Block::Image(i) => {
                    let name = format!("images/image{}.png", self.image);
                    self.image += 1;
                    self.assets.push((name.clone(), picture(i)?));
                    let (w, h) = i.visible_size();
                    out.push_str(&format!("<p class=\"picture\"><img src=\"{name}\" alt=\"{}\" title=\"{}\" style=\"width:{w}pt;height:{h}pt;max-width:100%;object-fit:contain;\" /></p>\n",xml(if i.alt_text.is_empty(){&i.name}else{&i.alt_text}),xml(&i.title)));
                }
                Block::PageBreak => {
                    out.push_str("<div class=\"page-break\" aria-label=\"Page break\"></div>\n")
                }
                Block::Rule => out.push_str("<hr />\n"),
            }
        }
        if let Some(tag) = list {
            out.push_str(&format!("</{tag}>\n"));
        }
        Ok(out)
    }
}
fn css(doc: &Document) -> String {
    format!(
        "html{{color:#1f1f1f;background:{}}}body{{font-family:sans-serif;max-width:60rem;margin:2rem auto;padding:0 1rem}}p,h1,h2,h3,h4,h5,h6{{white-space:pre-wrap;overflow-wrap:break-word}}table{{border-collapse:collapse;width:100%}}td{{border:1px solid #808080;padding:6pt;vertical-align:top}}img{{max-width:100%}}.page-break{{break-before:page}}section+section{{break-before:page}}header,footer{{white-space:pre-wrap}}@page{{size:{}pt {}pt;margin:{}pt {}pt {}pt {}pt}}",
        colour(doc.page.color),
        doc.page.size().0,
        doc.page.size().1,
        doc.page.margins[0],
        doc.page.margins[3],
        doc.page.margins[1],
        doc.page.margins[2]
    )
}
fn xhtml(title: &str, body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE html><html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"en-AU\" lang=\"en-AU\"><head><meta charset=\"utf-8\" /><title>{}</title><link rel=\"stylesheet\" type=\"text/css\" href=\"styles.css\" /></head><body>{body}</body></html>",
        xml(title)
    )
}
pub(super) fn archive(doc: &Document) -> Result<Vec<u8>, String> {
    let mut web = Web {
        assets: vec![],
        image: 0,
    };
    let mut body = String::new();
    if !doc.page.header.chars.is_empty() {
        body.push_str(&format!("<header>{}</header>", inline(&doc.page.header)));
    }
    for (index, tab) in doc.tabs.iter().enumerate() {
        body.push_str(&format!(
            "<section id=\"tab{index}\" aria-label=\"{}\">{}</section>\n",
            xml(&tab.title),
            web.blocks(doc.tab_blocks(index), index)?
        ));
    }
    if !doc.page.footer.chars.is_empty() {
        body.push_str(&format!("<footer>{}</footer>", inline(&doc.page.footer)));
    }
    let mut entries = vec![
        ("index.html".into(), xhtml(&doc.title, &body).into_bytes()),
        ("styles.css".into(), css(doc).into_bytes()),
    ];
    entries.extend(web.assets);
    zip(entries, None)
}
pub(super) fn epub(doc: &Document) -> Result<Vec<u8>, String> {
    let mut web = Web {
        assets: vec![],
        image: 0,
    };
    let mut entries = vec![];
    let mut manifest = String::new();
    let mut spine = String::new();
    let mut nav = String::new();
    for (index, tab) in doc.tabs.iter().enumerate() {
        let mut body = web.blocks(doc.tab_blocks(index), index)?;
        if index == 0 && !doc.page.header.chars.is_empty() {
            body = format!("<header>{}</header>{body}", inline(&doc.page.header));
        }
        if index + 1 == doc.tabs.len() && !doc.page.footer.chars.is_empty() {
            body.push_str(&format!("<footer>{}</footer>", inline(&doc.page.footer)));
        }
        let name = format!("chapter{index}.xhtml");
        entries.push((
            format!("EPUB/{name}"),
            xhtml(&tab.title, &body).into_bytes(),
        ));
        manifest.push_str(&format!(
            "<item id=\"chapter{index}\" href=\"{name}\" media-type=\"application/xhtml+xml\"/>"
        ));
        spine.push_str(&format!("<itemref idref=\"chapter{index}\"/>"));
        nav.push_str(&format!("<li><a href=\"{name}\">{}</a>", xml(&tab.title)));
        let headings: Vec<_> = doc
            .tab_blocks(index)
            .iter()
            .enumerate()
            .filter_map(|(i, b)| match b {
                Block::Paragraph(p) if p.heading > 0 && !p.chars.is_empty() => Some((i, p)),
                _ => None,
            })
            .collect();
        if !headings.is_empty() {
            nav.push_str("<ol>");
            for (i, p) in headings {
                nav.push_str(&format!(
                    "<li><a href=\"{name}#p{index}-{i}\">{}</a></li>",
                    xml(&p.text())
                ));
            }
            nav.push_str("</ol>");
        }
        nav.push_str("</li>");
    }
    for (i, (name, bytes)) in web.assets.into_iter().enumerate() {
        manifest.push_str(&format!(
            "<item id=\"image{i}\" href=\"{name}\" media-type=\"image/png\"/>"
        ));
        entries.push((format!("EPUB/{name}"), bytes));
    }
    let modified = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| e.to_string())?;
    // EPUB requires whole-second UTC timestamps.
    let modified = format!(
        "{}Z",
        modified
            .split('.')
            .next()
            .unwrap_or(&modified)
            .trim_end_matches('Z')
    );
    let opf = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><package xmlns=\"http://www.idpf.org/2007/opf\" unique-identifier=\"book-id\" version=\"3.0\" xml:lang=\"en-AU\"><metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:identifier id=\"book-id\">urn:alphadocs:{}</dc:identifier><dc:title>{}</dc:title><dc:language>en-AU</dc:language><meta property=\"dcterms:modified\">{modified}</meta></metadata><manifest><item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/><item id=\"css\" href=\"styles.css\" media-type=\"text/css\"/>{manifest}</manifest><spine>{spine}</spine></package>",
        xml(&doc.id),
        xml(&doc.title)
    );
    entries.extend([("EPUB/package.opf".into(),opf.into_bytes()),("EPUB/nav.xhtml".into(),xhtml("Contents",&format!("<nav epub:type=\"toc\" id=\"toc\"><h1>Contents</h1><ol>{nav}</ol></nav>")).into_bytes()),("EPUB/styles.css".into(),css(doc).into_bytes()),("META-INF/container.xml".into(),br#"<?xml version="1.0" encoding="UTF-8"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="EPUB/package.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#.to_vec())]);
    zip(entries, Some("application/epub+zip"))
}
