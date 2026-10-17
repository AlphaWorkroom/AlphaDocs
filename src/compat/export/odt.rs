use super::*;
const NS: &str = r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" xmlns:dc="http://purl.org/dc/elements/1.1/""#;
struct Odt {
    styles: String,
    assets: Vec<Entry>,
    serial: usize,
    column_width: f32,
    needs_master: bool,
}
fn properties(s: &TextStyle) -> String {
    let mut out = format!(
        "fo:font-family=\"{}\" fo:font-size=\"{}pt\" fo:font-weight=\"{}\" fo:font-style=\"{}\" fo:color=\"{}\"",
        xml(&s.font),
        s.size,
        if s.bold { "bold" } else { "normal" },
        if s.italic { "italic" } else { "normal" },
        colour(s.color)
    );
    if s.underline {
        out.push_str(" style:text-underline-style=\"solid\" style:text-underline-width=\"auto\" style:text-underline-color=\"font-color\"");
    }
    if s.strike {
        out.push_str(" style:text-line-through-style=\"solid\"");
    }
    if let Some(c) = s.highlight {
        out.push_str(&format!(" fo:background-color=\"{}\"", colour(c)));
    }
    if s.baseline != Baseline::Normal {
        out.push_str(&format!(
            " style:text-position=\"{} 70%\"",
            if s.baseline == Baseline::Superscript {
                "super"
            } else {
                "sub"
            }
        ));
    }
    out
}
fn literal(text: &str) -> String {
    let mut out = String::new();
    let mut spaces = 0usize;
    let flush = |out: &mut String, n: &mut usize| {
        if *n > 0 {
            out.push_str(&format!("<text:s text:c=\"{n}\"/>"));
            *n = 0;
        }
    };
    for c in text.chars() {
        match c {
            ' ' => spaces += 1,
            '\t' => {
                flush(&mut out, &mut spaces);
                out.push_str("<text:tab/>");
            }
            '\n' => {
                flush(&mut out, &mut spaces);
                out.push_str("<text:line-break/>");
            }
            _ => {
                flush(&mut out, &mut spaces);
                out.push_str(&xml(&c.to_string()));
            }
        }
    }
    flush(&mut out, &mut spaces);
    out
}
impl Odt {
    fn master_reference(&mut self) -> &'static str {
        if std::mem::take(&mut self.needs_master) {
            " style:master-page-name=\"Standard\""
        } else {
            ""
        }
    }
    fn next(&mut self, prefix: &str) -> String {
        let id = format!("{prefix}{}", self.serial);
        self.serial += 1;
        id
    }
    fn paragraph(&mut self, p: &Paragraph) -> String {
        let name = self.next("P");
        let master = self.master_reference();
        self.styles.push_str(&format!("<style:style style:name=\"{name}\" style:family=\"paragraph\"{master}><style:paragraph-properties fo:text-align=\"{}\" fo:margin-left=\"{}pt\" fo:text-indent=\"{}pt\" fo:margin-top=\"{}pt\" fo:margin-bottom=\"{}pt\" fo:line-height=\"{}%\" fo:keep-with-next=\"{}\" fo:keep-together=\"{}\" fo:orphans=\"{}\" fo:widows=\"{}\" fo:break-before=\"{}\"/><style:text-properties {}/></style:style>",match p.alignment{Alignment::Left=>"start",Alignment::Center=>"center",Alignment::Right=>"end",Alignment::Justified=>"justify"},p.indent,p.first_indent,p.before,p.after,p.line_spacing*100.0,if p.keep_next{"always"}else{"auto"},if p.keep_together{"always"}else{"auto"},if p.widow_control{2}else{1},if p.widow_control{2}else{1},if p.page_break_before{"page"}else{"auto"},properties(&p.style)));
        let (tag, outline) = if (1..=7).contains(&p.heading) {
            (
                "text:h",
                format!(
                    " text:outline-level=\"{}\"",
                    if p.heading == 7 { 1 } else { p.heading }
                ),
            )
        } else {
            ("text:p", String::new())
        };
        let mut out = format!("<{tag} text:style-name=\"{name}\"{outline}>");
        if p.list == ListKind::Check {
            out.push_str(if p.checked {
                "☑<text:s/>"
            } else {
                "☐<text:s/>"
            });
        }
        for (s, text) in runs(p) {
            let style = self.next("T");
            self.styles.push_str(&format!("<style:style style:name=\"{style}\" style:family=\"text\"><style:text-properties {}/></style:style>",properties(s)));
            let span = format!(
                "<text:span text:style-name=\"{style}\">{}</text:span>",
                literal(&text)
            );
            if let Some(link) = s.link.as_deref().and_then(safe_link) {
                out.push_str(&format!(
                    "<text:a xlink:type=\"simple\" xlink:href=\"{}\">{span}</text:a>",
                    xml(link)
                ));
            } else {
                out.push_str(&span);
            }
        }
        out.push_str(&format!("</{tag}>"));
        out
    }
    fn blocks(&mut self, blocks: &[Block]) -> Result<String, String> {
        let mut out = String::new();
        let mut list = None;
        for block in blocks {
            let kind = match block {
                Block::Paragraph(p) if matches!(p.list, ListKind::Number | ListKind::Bullet) => {
                    Some(if p.list == ListKind::Number {
                        "Number"
                    } else {
                        "Bullet"
                    })
                }
                _ => None,
            };
            if list != kind {
                if list.is_some() {
                    out.push_str("</text:list>");
                }
                if let Some(kind) = kind {
                    out.push_str(&format!("<text:list text:style-name=\"{kind}\">"));
                }
                list = kind;
            }
            match block {
                Block::Paragraph(p) => {
                    if kind.is_some() {
                        out.push_str("<text:list-item>");
                    }
                    out.push_str(&self.paragraph(p));
                    if kind.is_some() {
                        out.push_str("</text:list-item>");
                    }
                }
                Block::Table(rows) => {
                    let name = self.next("Table");
                    let columns = rows.first().map_or(1, Vec::len).max(1);
                    let master = self.master_reference();
                    let width = self.column_width;
                    let cell_width = width / columns as f32;
                    self.styles.push_str(&format!("<style:style style:name=\"{name}\" style:family=\"table\"{master}><style:table-properties style:width=\"{width}pt\" table:align=\"margins\"/></style:style><style:style style:name=\"{name}Column\" style:family=\"table-column\"><style:table-column-properties style:column-width=\"{cell_width}pt\"/></style:style>"));
                    out.push_str(&format!("<table:table table:name=\"{name}\" table:style-name=\"{name}\"><table:table-column table:style-name=\"{name}Column\" table:number-columns-repeated=\"{columns}\"/>"));
                    for row in rows {
                        out.push_str("<table:table-row>");
                        for cell in row {
                            let cell_name = self.next("Cell");
                            let style = cell.box_style.unwrap_or_default();
                            let mut properties = String::from("fo:padding=\"6pt\"");
                            if let Some(fill) = style.fill {
                                properties.push_str(&format!(
                                    " fo:background-color=\"{}\"",
                                    colour(fill)
                                ));
                            }
                            for (edge, side) in
                                style.borders.iter().zip(["top", "bottom", "left", "right"])
                            {
                                properties.push_str(&format!(
                                    " fo:border-{side}=\"{}pt solid {}\"",
                                    edge.width,
                                    colour(edge.colour)
                                ));
                            }
                            self.styles.push_str(&format!("<style:style style:name=\"{cell_name}\" style:family=\"table-cell\"><style:table-cell-properties {properties}/></style:style>"));
                            out.push_str(&format!("<table:table-cell office:value-type=\"string\" table:style-name=\"{cell_name}\">"));
                            out.push_str(&self.paragraph(cell));
                            out.push_str("</table:table-cell>");
                        }
                        out.push_str("</table:table-row>");
                    }
                    out.push_str("</table:table>");
                }
                Block::Image(i) => {
                    let name = self.next("Image");
                    let path = format!("Pictures/{name}.png");
                    let master = self.master_reference();
                    self.styles.push_str(&format!("<style:style style:name=\"{name}Paragraph\" style:family=\"paragraph\"{master}/>"));
                    self.assets.push((path.clone(), picture(i)?));
                    let (w, h) = i.visible_size();
                    // Portable standard inline frames preserve the visible picture.
                    out.push_str(&format!("<text:p text:style-name=\"{name}Paragraph\"><draw:frame draw:name=\"{name}\" text:anchor-type=\"as-char\" svg:width=\"{w}pt\" svg:height=\"{h}pt\"><draw:image xlink:href=\"{path}\" xlink:type=\"simple\" xlink:show=\"embed\" xlink:actuate=\"onLoad\"/><svg:title>{}</svg:title><svg:desc>{}</svg:desc></draw:frame></text:p>",xml(&i.title),xml(if i.alt_text.is_empty(){&i.name}else{&i.alt_text})));
                }
                Block::PageBreak => out.push_str("<text:p text:style-name=\"PageBreak\"/>"),
                Block::Rule => out.push_str("<text:p text:style-name=\"Rule\"/>"),
            }
        }
        if list.is_some() {
            out.push_str("</text:list>");
        }
        Ok(out)
    }
}
pub(super) fn encode(doc: &Document) -> Result<Vec<u8>, String> {
    let mut odt = Odt {
        styles: String::new(),
        assets: vec![],
        serial: 0,
        needs_master: true,
        column_width: (doc.page.size().0
            - doc.page.margins[2]
            - doc.page.margins[3]
            - doc.page.column_gap * (doc.page.columns as f32 - 1.0))
            / doc.page.columns as f32,
    };
    let mut body = String::new();
    for (i, tab) in doc.tabs.iter().enumerate() {
        if i > 0 {
            body.push_str("<text:p text:style-name=\"PageBreak\"/>");
        }
        body.push_str(&format!(
            "<text:section text:name=\"Tab{}\">{}</text:section>",
            xml(&tab.id),
            odt.blocks(doc.tab_blocks(i))?
        ));
    }
    let header = if doc.page.header.chars.is_empty() {
        String::new()
    } else {
        format!(
            "<style:header>{}</style:header>",
            odt.paragraph(&doc.page.header)
        )
    };
    let footer = if doc.page.footer.chars.is_empty() && !doc.page.page_numbers {
        String::new()
    } else {
        format!(
            "<style:footer>{}{}</style:footer>",
            odt.paragraph(&doc.page.footer),
            if doc.page.page_numbers {
                "<text:p><text:page-number text:select-page=\"current\">1</text:page-number></text:p>"
            } else {
                ""
            }
        )
    };
    let content = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><office:document-content {NS} office:version=\"1.3\"><office:automatic-styles>{}</office:automatic-styles><office:body><office:text>{body}</office:text></office:body></office:document-content>",
        odt.styles
    );
    let (w, h) = doc.page.size();
    let [top, bottom, left, right] = doc.page.margins;
    let styles = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><office:document-styles {NS} office:version=\"1.3\"><office:styles><style:default-style style:family=\"paragraph\"><style:paragraph-properties fo:margin-top=\"0pt\" fo:margin-bottom=\"0pt\"/><style:text-properties fo:font-size=\"11pt\" fo:font-family=\"Noto Sans\"/></style:default-style><text:list-style style:name=\"Bullet\"><text:list-level-style-bullet text:level=\"1\" text:bullet-char=\"•\"/></text:list-style><text:list-style style:name=\"Number\"><text:list-level-style-number text:level=\"1\" style:num-format=\"1\" style:num-suffix=\".\"/></text:list-style><style:style style:name=\"Cell\" style:family=\"table-cell\"><style:table-cell-properties fo:border=\"0.5pt solid #808080\" fo:padding=\"6pt\"/></style:style><style:style style:name=\"PageBreak\" style:family=\"paragraph\"><style:paragraph-properties fo:break-before=\"page\"/></style:style><style:style style:name=\"Rule\" style:family=\"paragraph\"><style:paragraph-properties fo:border-bottom=\"0.5pt solid #808080\"/></style:style></office:styles><office:automatic-styles>{}<style:page-layout style:name=\"Layout\"><style:page-layout-properties fo:page-width=\"{w}pt\" fo:page-height=\"{h}pt\" fo:margin-top=\"{top}pt\" fo:margin-bottom=\"{bottom}pt\" fo:margin-left=\"{left}pt\" fo:margin-right=\"{right}pt\" fo:background-color=\"{}\" style:print-orientation=\"{}\"><style:columns fo:column-count=\"{}\" fo:column-gap=\"{}pt\"/></style:page-layout-properties></style:page-layout></office:automatic-styles><office:master-styles><style:master-page style:name=\"Standard\" style:page-layout-name=\"Layout\">{header}{footer}</style:master-page></office:master-styles></office:document-styles>",
        odt.styles,
        colour(doc.page.color),
        if doc.page.landscape {
            "landscape"
        } else {
            "portrait"
        },
        doc.page.columns,
        doc.page.column_gap
    );
    let meta = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><office:document-meta {NS} office:version=\"1.3\"><office:meta><dc:title>{}</dc:title><dc:language>en-AU</dc:language></office:meta></office:document-meta>",
        xml(&doc.title)
    );
    let mut entries = vec![
        ("content.xml".into(), content.into_bytes()),
        ("styles.xml".into(), styles.into_bytes()),
        ("meta.xml".into(), meta.into_bytes()),
    ];
    entries.extend(odt.assets);
    let mut manifest = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3"><manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.text" manifest:version="1.3"/>"#,
    );
    for (name, _) in &entries {
        manifest.push_str(&format!(
            "<manifest:file-entry manifest:full-path=\"{}\" manifest:media-type=\"{}\"/>",
            xml(name),
            if name.ends_with(".png") {
                "image/png"
            } else {
                "text/xml"
            }
        ));
    }
    manifest.push_str("</manifest:manifest>");
    entries.push(("META-INF/manifest.xml".into(), manifest.into_bytes()));
    zip(entries, Some("application/vnd.oasis.opendocument.text"))
}
