use super::*;
use std::fmt::Write as _;
fn escaped(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '\\' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            '\n' => out.push_str("\\line "),
            '\t' => out.push_str("\\tab "),
            c if c.is_ascii() && !c.is_control() => out.push(c),
            c => {
                let mut units = [0; 2];
                for unit in c.encode_utf16(&mut units) {
                    let _ = write!(out, "\\u{}?", *unit as i16);
                }
            }
        }
    }
    out
}
fn twips(n: f32) -> i32 {
    (n * 20.0).round() as i32
}
struct Rtf {
    fonts: Vec<String>,
    colours: Vec<[u8; 3]>,
}
impl Rtf {
    fn style(&self, s: &TextStyle) -> String {
        let font = self.fonts.iter().position(|f| f == &s.font).unwrap_or(0);
        let colour = self
            .colours
            .iter()
            .position(|c| c == &s.color)
            .map_or(0, |i| i + 1);
        let highlight = s
            .highlight
            .and_then(|c| self.colours.iter().position(|v| *v == c))
            .map_or(0, |i| i + 1);
        format!(
            "\\plain\\f{font}\\fs{}\\b{}\\i{}{}\\strike{}\\cf{colour}\\highlight{highlight}{} ",
            (s.size * 2.0).round() as i32,
            i32::from(s.bold),
            i32::from(s.italic),
            if s.underline { "\\ul" } else { "\\ulnone" },
            i32::from(s.strike),
            match s.baseline {
                Baseline::Normal => "\\nosupersub",
                Baseline::Superscript => "\\super",
                Baseline::Subscript => "\\sub",
            }
        )
    }
    fn paragraph(&self, p: &Paragraph, cell: bool, number: usize) -> String {
        let style = if p.heading <= 8 { p.heading } else { 0 };
        let mut out = format!(
            "{{\\pard\\s{style}{}{}\\li{}\\fi{}\\sb{}\\sa{}\\sl{}\\slmult1{}{}{}{} ",
            if cell { "\\intbl" } else { "" },
            match p.alignment {
                Alignment::Left => "\\ql",
                Alignment::Center => "\\qc",
                Alignment::Right => "\\qr",
                Alignment::Justified => "\\qj",
            },
            twips(p.indent),
            twips(p.first_indent),
            twips(p.before),
            twips(p.after),
            (p.line_spacing * 240.0).round() as i32,
            if p.keep_next { "\\keepn" } else { "" },
            if p.keep_together { "\\keep" } else { "" },
            if p.widow_control {
                "\\widctlpar"
            } else {
                "\\nowidctlpar"
            },
            if p.page_break_before { "\\pagebb" } else { "" }
        );
        match p.list {
            ListKind::Bullet => out.push_str("\\ls1\\ilvl0 {\\listtext \\u8226?\\tab }"),
            ListKind::Number => {
                out.push_str(&format!("\\ls2\\ilvl0 {{\\listtext {number}.\\tab }}"))
            }
            ListKind::Check => out.push_str(if p.checked { "\\u9745? " } else { "\\u9744? " }),
            ListKind::None => {}
        }
        if p.chars.is_empty() {
            out.push_str(&self.style(&p.style));
        }
        for (style, text) in runs(p) {
            let content = format!("{{{}{}}}", self.style(style), escaped(&text));
            if let Some(link) = style.link.as_deref().and_then(safe_link) {
                out.push_str(&format!(
                    "{{\\field{{\\*\\fldinst HYPERLINK \"{}\"}}{{\\fldrslt {content}}}}}",
                    escaped(&link.replace('"', "%22"))
                ));
            } else {
                out.push_str(&content);
            }
        }
        out.push_str(if cell { "\\cell}" } else { "\\par}\n" });
        out
    }
    fn blocks(&self, blocks: &[Block], width: f32) -> Result<String, String> {
        let mut out = String::new();
        let mut number = 0;
        for b in blocks {
            match b {
                Block::Paragraph(p) => {
                    if p.list == ListKind::Number {
                        number += 1;
                    } else {
                        number = 0;
                    }
                    out.push_str(&self.paragraph(p, false, number));
                }
                Block::Table(rows) => {
                    number = 0;
                    for row in rows {
                        out.push_str("\\trowd\\trgaph108\\trleft0");
                        for (i, cell) in row.iter().enumerate() {
                            let style = cell.box_style.unwrap_or_default();
                            if let Some(fill) = style.fill {
                                let index = self
                                    .colours
                                    .iter()
                                    .position(|c| *c == fill)
                                    .map_or(0, |i| i + 1);
                                let _ = write!(out, "\\clcbpat{index}");
                            }
                            for (edge, side) in style.borders.iter().zip(["t", "b", "l", "r"]) {
                                let index = self
                                    .colours
                                    .iter()
                                    .position(|c| *c == edge.colour)
                                    .map_or(0, |i| i + 1);
                                let _ = write!(
                                    out,
                                    "\\clbrdr{side}{}\\brdrw{}\\brdrcf{index}",
                                    if edge.width == 0.0 {
                                        "\\brdrnil"
                                    } else {
                                        "\\brdrs"
                                    },
                                    twips(edge.width)
                                );
                            }
                            let _ = write!(
                                out,
                                "\\cellx{}",
                                twips(width * (i + 1) as f32 / row.len().max(1) as f32)
                            );
                        }
                        out.push('\n');
                        for p in row {
                            out.push_str(&self.paragraph(p, true, 1));
                        }
                        out.push_str("\\row\n");
                    }
                    out.push_str("\\pard\n");
                }
                Block::Image(i) => {
                    number = 0;
                    let bytes = picture(i)?;
                    let (pw, ph) = image::ImageReader::new(Cursor::new(&bytes))
                        .with_guessed_format()
                        .map_err(|e| e.to_string())?
                        .into_dimensions()
                        .map_err(|e| e.to_string())?;
                    let (w, h) = i.visible_size();
                    let _ = writeln!(
                        out,
                        "{{\\pard{{\\pict\\pngblip\\picw{pw}\\pich{ph}\\picwgoal{}\\pichgoal{}",
                        twips(w),
                        twips(h)
                    );
                    for byte in bytes {
                        let _ = write!(out, "{byte:02x}");
                    }
                    out.push_str("}\\par}\n");
                }
                Block::PageBreak => {
                    number = 0;
                    out.push_str("\\page\n");
                }
                Block::Rule => {
                    number = 0;
                    out.push_str("{\\pard\\brdrb\\brdrs\\brdrw10\\par}\n");
                }
            }
        }
        Ok(out)
    }
}
pub(super) fn encode(doc: &Document) -> Result<Vec<u8>, String> {
    let mut rtf = Rtf {
        fonts: vec!["Noto Sans".into()],
        colours: vec![],
    };
    for p in doc
        .tabs
        .iter()
        .enumerate()
        .flat_map(|(i, _)| doc.tab_blocks(i))
        .flat_map(|b| match b {
            Block::Paragraph(p) => vec![p],
            Block::Table(rows) => rows.iter().flatten().collect(),
            _ => vec![],
        })
        .chain([&doc.page.header, &doc.page.footer])
    {
        for colour in p
            .box_style
            .unwrap_or_default()
            .borders
            .iter()
            .map(|b| b.colour)
            .chain(p.box_style.and_then(|s| s.fill))
        {
            if !rtf.colours.contains(&colour) {
                rtf.colours.push(colour);
            }
        }
        for s in std::iter::once(&p.style).chain(p.chars.iter().map(|c| &c.style)) {
            if !rtf.fonts.contains(&s.font) {
                rtf.fonts.push(s.font.clone());
            }
            for c in std::iter::once(s.color).chain(s.highlight) {
                if !rtf.colours.contains(&c) {
                    rtf.colours.push(c);
                }
            }
        }
    }
    let mut out = String::from("{\\rtf1\\ansi\\ansicpg1252\\deff0\\uc1\n{\\fonttbl");
    for (i, font) in rtf.fonts.iter().enumerate() {
        let _ = write!(
            out,
            "{{\\f{i}\\fnil {};}}",
            escaped(&font.replace(';', " "))
        );
    }
    out.push_str("}\n{\\colortbl;");
    for [r, g, b] in &rtf.colours {
        let _ = write!(out, "\\red{r}\\green{g}\\blue{b};");
    }
    out.push_str("}\n{\\stylesheet{\\s0 Normal;}");
    for h in 1..=6 {
        let _ = write!(out, "{{\\s{h}\\outlinelevel{} Heading {h};}}", h - 1);
    }
    out.push_str("{\\s7 Title;}{\\s8 Subtitle;}}\n");
    out.push_str(r#"{\*\listtable{\list\listtemplateid1{\listlevel\levelnfc23\leveljc0\levelfollow0\levelstartat1{\leveltext\'01\u8226?;}{\levelnumbers;}\fi-240\li360}\listid1}{\list\listtemplateid2{\listlevel\levelnfc0\leveljc0\levelfollow0\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}\fi-240\li360}\listid2}}{\*\listoverridetable{\listoverride\listid1\listoverridecount0\ls1}{\listoverride\listid2\listoverridecount0\ls2}}"#);
    let (w, h) = doc.page.size();
    let [top, bottom, left, right] = doc.page.margins;
    let _ = write!(
        out,
        "\\paperw{}\\paperh{}\\margt{}\\margb{}\\margl{}\\margr{}\\cols{}\\colsx{}{}\n{{\\info{{\\title {}}}}}\n",
        twips(w),
        twips(h),
        twips(top),
        twips(bottom),
        twips(left),
        twips(right),
        doc.page.columns,
        twips(doc.page.column_gap),
        if doc.page.landscape {
            "\\landscape"
        } else {
            ""
        },
        escaped(&doc.title)
    );
    if !doc.page.header.chars.is_empty() {
        let _ = write!(
            out,
            "{{\\header {}}}",
            rtf.paragraph(&doc.page.header, false, 1)
        );
    }
    if !doc.page.footer.chars.is_empty() || doc.page.page_numbers {
        let _ = write!(
            out,
            "{{\\footer {}{}}}",
            rtf.paragraph(&doc.page.footer, false, 1),
            if doc.page.page_numbers {
                "{\\pard\\qc{\\field{\\*\\fldinst PAGE}{\\fldrslt 1}}\\par}"
            } else {
                ""
            }
        );
    }
    for (i, _) in doc.tabs.iter().enumerate() {
        if i > 0 {
            out.push_str("\\page\n");
        }
        out.push_str(&rtf.blocks(doc.tab_blocks(i), w - left - right)?);
    }
    out.push('}');
    Ok(out.into_bytes())
}
