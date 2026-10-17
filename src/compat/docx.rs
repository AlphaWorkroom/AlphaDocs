//! A deliberate OOXML subset. Unsupported content produces import warnings.
#[path = "docx_objects.rs"]
mod objects;
use crate::model::*;
use quick_xml::{Reader, events::Event};
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    path::Path,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

#[derive(Clone, Default, Debug)]
struct Node {
    qualified_name: String,
    qualified_attrs: Vec<(String, String)>,
    name: String,
    attrs: HashMap<String, String>,
    text: String,
    children: Vec<Node>,
}
impl Node {
    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(String::as_str)
    }
    fn val(&self) -> Option<&str> {
        self.attr("val")
    }
    fn descendant(&self, name: &str) -> Option<&Node> {
        if self.name == name {
            Some(self)
        } else {
            self.children.iter().find_map(|c| c.descendant(name))
        }
    }
    fn descendants<'a>(&'a self, name: &str, out: &mut Vec<&'a Node>) {
        if self.name == name {
            out.push(self);
            return;
        }
        if self.name == "AlternateContent" {
            if let Some(branch) = self.child("Choice").or_else(|| self.child("Fallback")) {
                branch.descendants(name, out);
            }
        } else {
            for child in &self.children {
                child.descendants(name, out);
            }
        }
    }
}
fn local(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .rsplit(':')
        .next()
        .unwrap_or("")
        .into()
}
fn import_placement(drawing: &Node, properties: Option<&Node>) -> ImagePlacement {
    let stored: Option<ImagePlacement> = properties
        .and_then(|n| n.attr("placement"))
        .and_then(|s| serde_json::from_str(s).ok());
    let Some(anchor) = drawing.descendant("anchor") else {
        return ImagePlacement {
            flow: ImageFlow::Break,
            offset: [0.0; 2],
            ..stored.unwrap_or_default()
        };
    };
    let number = |node: Option<&Node>, name: &str, default: f32| {
        node.and_then(|n| n.attr(name))
            .and_then(|s| s.parse::<f32>().ok())
            .map(|v| v / 12700.0)
            .unwrap_or(default)
    };
    let offset = |name| {
        anchor
            .child(name)
            .and_then(|n| n.child("posOffset"))
            .and_then(|n| n.text.parse::<f32>().ok())
            .unwrap_or(0.0)
            / 12700.0
    };
    let square = anchor.child("wrapSquare");
    let standard = ImagePlacement {
        flow: if square.is_some() {
            ImageFlow::Wrap
        } else if anchor.child("wrapTopAndBottom").is_some() {
            ImageFlow::Break
        } else if anchor
            .attr("behindDoc")
            .is_some_and(|v| v == "1" || v == "true")
        {
            ImageFlow::Behind
        } else {
            ImageFlow::Front
        },
        offset: [offset("positionH"), offset("positionV")],
        margins: [
            number(Some(anchor), "distT", 1.44),
            number(Some(anchor), "distB", 1.44),
            number(Some(anchor), "distL", 1.44),
            number(Some(anchor), "distR", 1.44),
        ],
        wrap_side: match square.and_then(|n| n.attr("wrapText")) {
            Some("left") => WrapSide::Left,
            Some("right") => WrapSide::Right,
            Some("largest") => WrapSide::Largest,
            _ => WrapSide::Both,
        },
        ..stored.unwrap_or_default()
    };
    stored
        .filter(|s| {
            s.flow == standard.flow
                && (s.flow != ImageFlow::Wrap || s.wrap_side == standard.wrap_side)
                && s.offset
                    .iter()
                    .zip(standard.offset)
                    .chain(s.margins.iter().zip(standard.margins))
                    .all(|(a, b)| (*a - b).abs() < 0.0001)
        })
        .unwrap_or(standard)
}
fn parse(xml: &str) -> Result<Node, String> {
    let mut reader = Reader::from_str(xml);
    let mut stack = vec![Node::default()];
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) | Event::Empty(e) => {
                let empty = xml
                    .as_bytes()
                    .get(reader.buffer_position().saturating_sub(2) as usize)
                    == Some(&b'/');
                let mut n = Node {
                    name: local(e.name().as_ref()),
                    qualified_name: String::from_utf8_lossy(e.name().as_ref()).into_owned(),
                    ..Default::default()
                };
                for a in e.attributes() {
                    let a = a.map_err(|e| e.to_string())?;
                    n.qualified_attrs.push((
                        String::from_utf8_lossy(a.key.as_ref()).into_owned(),
                        a.decode_and_unescape_value(reader.decoder())
                            .map_err(|e| e.to_string())?
                            .into_owned(),
                    ));
                    n.attrs.insert(
                        local(a.key.as_ref()),
                        a.decode_and_unescape_value(reader.decoder())
                            .map_err(|e| e.to_string())?
                            .into_owned(),
                    );
                }
                if empty {
                    stack.last_mut().ok_or("Malformed XML")?.children.push(n);
                } else {
                    if stack.len() >= 256 {
                        return Err("DOCX XML nesting is too deep".into());
                    }
                    stack.push(n);
                }
            }
            Event::End(_) => {
                if stack.len() > 1 {
                    let n = stack.pop().ok_or("Malformed XML")?;
                    stack.last_mut().ok_or("Malformed XML")?.children.push(n);
                }
            }
            Event::Text(e) => {
                let t = e.decode().map_err(|e| e.to_string())?;
                stack
                    .last_mut()
                    .ok_or("Malformed XML")?
                    .text
                    .push_str(&quick_xml::escape::unescape(&t).map_err(|e| e.to_string())?);
            }
            Event::GeneralRef(e) => {
                let name = e.decode().map_err(|e| e.to_string())?;
                let t = format!("&{name};");
                stack
                    .last_mut()
                    .ok_or("Malformed XML")?
                    .text
                    .push_str(&quick_xml::escape::unescape(&t).map_err(|e| e.to_string())?);
            }
            Event::CData(e) => {
                stack
                    .last_mut()
                    .ok_or("Malformed XML")?
                    .text
                    .push_str(&e.decode().map_err(|e| e.to_string())?);
            }
            Event::Eof => {
                if stack.len() != 1 {
                    return Err("Incomplete DOCX XML".into());
                }
                break;
            }
            _ => {}
        }
    }
    Ok(stack.remove(0))
}
fn esc(t: &str) -> String {
    quick_xml::escape::escape(t).into_owned()
}
fn rgb(c: [u8; 3]) -> String {
    format!("{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}
fn unrgb(s: &str) -> Option<[u8; 3]> {
    if s.len() != 6 {
        return None;
    }
    Some([
        u8::from_str_radix(&s[0..2], 16).ok()?,
        u8::from_str_radix(&s[2..4], 16).ok()?,
        u8::from_str_radix(&s[4..6], 16).ok()?,
    ])
}
fn twips(v: f32) -> i32 {
    (v * 20.0).round() as i32
}
fn rpr(s: &TextStyle) -> String {
    // CT_RPr is an ordered sequence, not an unordered property bag.
    let mut x = format!(
        "<w:rPr><w:rFonts w:ascii=\"{}\" w:hAnsi=\"{}\"/>",
        esc(&s.font),
        esc(&s.font)
    );
    if s.bold {
        x.push_str("<w:b/>");
    }
    if s.italic {
        x.push_str("<w:i/>");
    }
    if s.strike {
        x.push_str("<w:strike/>");
    }
    x.push_str(&format!(
        "<w:color w:val=\"{}\"/><w:sz w:val=\"{}\"/>",
        rgb(s.color),
        (s.size * 2.0).round() as i32
    ));
    if s.underline {
        x.push_str("<w:u w:val=\"single\"/>");
    }
    if let Some(c) = s.highlight {
        x.push_str(&format!("<w:shd w:fill=\"{}\"/>", rgb(c)));
    }
    if s.baseline != Baseline::Normal {
        x.push_str(&format!(
            "<w:vertAlign w:val=\"{}\"/>",
            if s.baseline == Baseline::Superscript {
                "superscript"
            } else {
                "subscript"
            }
        ));
    }
    x.push_str("</w:rPr>");
    x
}

struct Writer {
    rels: Vec<(String, String, String)>,
    images: Vec<(String, SharedVec<u8>)>,
    counter: usize,
}
impl Writer {
    fn relation(&mut self, typ: &str, target: &str) -> String {
        self.counter += 1;
        let id = format!("rId{}", self.counter);
        self.rels.push((id.clone(), typ.into(), target.into()));
        id
    }
    fn paragraph(&mut self, p: &Paragraph) -> String {
        self.paragraph_inner(p, false)
    }
    fn table(&mut self, rows: &[Vec<Paragraph>], width: f32) -> String {
        let mut out = String::from("<w:tbl><w:tblPr><w:tblW w:w=\"0\" w:type=\"auto\"/></w:tblPr>");
        let columns = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
        let cell_width = twips(width / columns as f32);
        out.push_str("<w:tblGrid>");
        for _ in 0..columns {
            out.push_str(&format!("<w:gridCol w:w=\"{cell_width}\"/>"));
        }
        out.push_str("</w:tblGrid>");
        for row in rows {
            out.push_str("<w:tr>");
            for cell in row {
                out.push_str(&format!(
                    "<w:tc><w:tcPr><w:tcW w:w=\"{cell_width}\" w:type=\"dxa\"/>"
                ));
                out.push_str(&write_box_style(
                    cell.box_style.unwrap_or_default(),
                    "tcBorders",
                ));
                out.push_str("</w:tcPr>");
                out.push_str(&self.paragraph_inner(cell, true));
                out.push_str("</w:tc>");
            }
            out.push_str("</w:tr>");
        }
        out.push_str("</w:tbl>");
        out
    }
    fn paragraph_inner(&mut self, p: &Paragraph, in_cell: bool) -> String {
        let mut x = format!(
            "<w:p alphadocs:annotations=\"{}\" alphadocs:annotationText=\"{}\"><w:pPr>",
            esc(&serde_json::to_string(&p.annotations).expect("Annotations serialize")),
            if p.annotations.is_empty() {
                String::new()
            } else {
                esc(&p.text())
            }
        );
        let sty = match p.heading {
            1..=6 => format!("Heading{}", p.heading),
            7 => "Title".into(),
            8 => "Subtitle".into(),
            _ => "Normal".into(),
        };
        x.push_str(&format!("<w:pStyle w:val=\"{sty}\"/>"));
        if p.keep_next {
            x.push_str("<w:keepNext/>");
        }
        if p.keep_together {
            x.push_str("<w:keepLines/>");
        }
        if p.page_break_before {
            x.push_str("<w:pageBreakBefore/>");
        }
        x.push_str(if p.widow_control {
            "<w:widowControl/>"
        } else {
            "<w:widowControl w:val=\"0\"/>"
        });
        if matches!(p.list, ListKind::Bullet | ListKind::Number) {
            x.push_str(&format!(
                "<w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"{}\"/></w:numPr>",
                if p.list == ListKind::Number { 2 } else { 1 }
            ));
        }
        if !in_cell && let Some(style) = p.box_style {
            x.push_str(&write_box_style(style, "pBdr"));
        }
        x.push_str(&format!("<w:spacing w:before=\"{}\" w:after=\"{}\" w:line=\"{}\" w:lineRule=\"auto\"/><w:ind w:left=\"{}\" w:firstLine=\"{}\"/><w:jc w:val=\"{}\"/>", twips(p.before), twips(p.after), (p.line_spacing*240.0).round() as i32, twips(p.indent), twips(p.first_indent), match p.alignment { Alignment::Left=>"left", Alignment::Center=>"center", Alignment::Right=>"right", Alignment::Justified=>"both" }));
        x.push_str(&rpr(&p.style));
        x.push_str("</w:pPr>");
        if p.list == ListKind::Check {
            // A standard Word 2010 checkbox content control, not private state
            // or a bullet alias. Its visible content must be exactly one glyph.
            self.counter += 1;
            x.push_str(&format!(
                "<w:sdt xmlns:w14=\"http://schemas.microsoft.com/office/word/2010/wordml\" xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" mc:Ignorable=\"w14\"><w:sdtPr><w:id w:val=\"{}\"/><w14:checkbox><w14:checked w14:val=\"{}\"/><w14:checkedState w14:val=\"2611\" w14:font=\"Segoe UI Symbol\"/><w14:uncheckedState w14:val=\"2610\" w14:font=\"Segoe UI Symbol\"/></w14:checkbox></w:sdtPr><w:sdtContent><w:r><w:rPr><w:rFonts w:ascii=\"Segoe UI Symbol\" w:hAnsi=\"Segoe UI Symbol\"/></w:rPr><w:t>{}</w:t></w:r></w:sdtContent></w:sdt><w:r><w:t xml:space=\"preserve\"> </w:t></w:r>",
                self.counter, u8::from(p.checked), if p.checked { '☑' } else { '☐' }
            ));
        }
        if in_cell && let Some(formula) = &p.formula {
            let number = formula.word_number_format.clone().or_else(|| {
                formula.decimals.map(|d| {
                    if d == 0 {
                        "0".into()
                    } else {
                        format!("0.{}", "0".repeat(d as usize))
                    }
                })
            });
            let instruction = format!(
                "{}{}",
                formula.expression,
                number.map(|s| format!(" \\# \"{s}\" ")).unwrap_or_default()
            );
            x.push_str(&format!("<w:fldSimple w:instr=\"{}\">", esc(&instruction)));
        }
        let mut start = 0;
        while start < p.chars.len() {
            let s = &p.chars[start].style;
            let mut end = start + 1;
            while end < p.chars.len() && p.chars[end].style == *s {
                end += 1;
            }
            let mut text = String::new();
            for c in &p.chars[start..end] {
                match c.ch {
                    '\n' => text.push_str("</w:t><w:br/><w:t xml:space=\"preserve\">"),
                    '\t' => text.push_str("</w:t><w:tab/><w:t xml:space=\"preserve\">"),
                    ch => text.push_str(&esc(&ch.to_string())),
                }
            }
            let run = format!(
                "<w:r>{}<w:t xml:space=\"preserve\">{text}</w:t></w:r>",
                rpr(s)
            );
            if let Some(link) = &s.link {
                let id = self.relation("hyperlink", link);
                x.push_str(&format!("<w:hyperlink r:id=\"{id}\">{run}</w:hyperlink>"));
            } else {
                x.push_str(&run);
            }
            start = end;
        }
        if in_cell && p.formula.is_some() {
            x.push_str("</w:fldSimple>");
        }
        x.push_str("</w:p>");
        x
    }
    fn image(&mut self, i: &EmbeddedImage) -> Result<String, String> {
        if i.editable.is_some() {
            return objects::write(self, i);
        }
        let ext = if i.data.starts_with(&[0x89, b'P', b'N', b'G']) {
            "png"
        } else {
            "jpeg"
        };
        let mut original_media = String::new();
        let (payload, ext) = if i.padding.enabled && i.padding.fraction > 0.0 {
            let name = format!("image{}-original.{ext}", self.images.len() + 1);
            original_media = self.relation("image", &format!("media/{name}"));
            self.images.push((name, i.data.clone()));
            let mut view = i.clone();
            view.effects = ImageEffects::default();
            (crate::image_edit::flattened_png(&view)?.into(), "png")
        } else {
            (i.data.clone(), ext)
        };
        let name = format!("image{}.{}", self.images.len() + 1, ext);
        let id = self.relation("image", &format!("media/{name}"));
        self.images.push((name, payload));
        let (width, height) = i.visible_size();
        let cx = (width * 12700.0) as i64;
        let cy = (height * 12700.0) as i64;
        let [left, top, right, bottom] = if original_media.is_empty() {
            i.visible_crop()
        } else {
            [0.0, 0.0, 1.0, 1.0]
        };
        let crop = format!(
            "<a:srcRect l=\"{}\" t=\"{}\" r=\"{}\" b=\"{}\"/>",
            (left * 100000.0).round() as i32,
            (top * 100000.0).round() as i32,
            ((1.0 - right) * 100000.0).round() as i32,
            ((1.0 - bottom) * 100000.0).round() as i32
        );
        let mut properties = format!(
            "descr=\"{}\" title=\"{}\" alphadocs:sourceWidth=\"{}\" alphadocs:sourceHeight=\"{}\" alphadocs:trim=\"{}\" alphadocs:crop=\"{}\" alphadocs:trimBounds=\"{}\" alphadocs:effects=\"{}\" alphadocs:objectId=\"{}\" alphadocs:placement=\"{}\" alphadocs:comments=\"{}\" alphadocs:reactions=\"{}\" alphadocs:padding=\"{}\"",
            esc(&i.alt_text),
            esc(&i.title),
            i.width,
            i.height,
            i.trim_transparency,
            esc(&serde_json::to_string(&i.crop).expect("Finite crop")),
            esc(&serde_json::to_string(&i.trim_bounds).expect("Finite crop")),
            esc(&serde_json::to_string(&i.effects).expect("Finite effects")),
            esc(&i.id),
            esc(&serde_json::to_string(&i.placement).expect("Finite placement")),
            esc(&serde_json::to_string(&i.comments).expect("Comments serialize")),
            esc(&serde_json::to_string(&i.reactions).expect("Reactions serialize")),
            esc(&serde_json::to_string(&i.padding).expect("Finite padding"))
        );
        if !original_media.is_empty() {
            properties.push_str(&format!(" alphadocs:originalMedia=\"{original_media}\""));
        }
        let effects = format!(
            "{}<a:lum bright=\"{}\" contrast=\"{}\"/><a:alphaModFix amt=\"{}\"/>",
            if i.effects.grayscale {
                "<a:grayscl/>"
            } else {
                ""
            },
            (i.effects.brightness * 100000.0).round() as i32,
            (i.effects.contrast * 100000.0).round() as i32,
            (i.effects.opacity * 100000.0).round() as i32
        );
        let xml = format!(
            "<w:p><w:r><w:drawing><wp:inline><wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:docPr id=\"{}\" name=\"{}\" {properties}/><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic><pic:nvPicPr><pic:cNvPr id=\"0\" name=\"{}\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed=\"{id}\"/>{crop}<a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>",
            self.images.len(),
            esc(&i.name),
            esc(&i.name)
        ).replace(&format!("<a:blip r:embed=\"{id}\"/>"), &format!("<a:blip r:embed=\"{id}\">{effects}</a:blip>"));
        let placement = i.placement;
        let [top, bottom, left, right] = placement.margins.map(|v| (v * 12700.0).round() as i64);
        let wrap = match placement.flow {
            ImageFlow::Break => "<wp:wrapTopAndBottom/>".into(),
            ImageFlow::Wrap => format!(
                "<wp:wrapSquare wrapText=\"{}\"/>",
                match placement.wrap_side {
                    WrapSide::Both => "bothSides",
                    WrapSide::Left => "left",
                    WrapSide::Right => "right",
                    WrapSide::Largest => "largest",
                }
            ),
            _ => "<wp:wrapNone/>".into(),
        };
        let open = format!(
            "<wp:anchor distT=\"{top}\" distB=\"{bottom}\" distL=\"{left}\" distR=\"{right}\" simplePos=\"0\" relativeHeight=\"251658240\" behindDoc=\"{}\" locked=\"0\" layoutInCell=\"1\" allowOverlap=\"1\"><wp:simplePos x=\"0\" y=\"0\"/><wp:positionH relativeFrom=\"column\"><wp:posOffset>{}</wp:posOffset></wp:positionH><wp:positionV relativeFrom=\"paragraph\"><wp:posOffset>{}</wp:posOffset></wp:positionV>",
            u8::from(placement.flow == ImageFlow::Behind),
            (placement.offset[0] * 12700.0).round() as i64,
            (placement.offset[1] * 12700.0).round() as i64
        );
        Ok(xml
            .replace("<wp:inline>", &open)
            .replace("<wp:docPr", &format!("{wrap}<wp:docPr"))
            .replace("</wp:inline>", "</wp:anchor>"))
    }
}
pub fn export_notices(doc: &Document) -> Vec<String> {
    let mut notices = Vec::new();
    for block in doc
        .tabs
        .iter()
        .enumerate()
        .flat_map(|(index, _)| doc.tab_blocks(index))
    {
        if let Block::Image(image) = block
            && image
                .editable
                .as_ref()
                .is_some_and(|o| matches!(o.source, ObjectSource::Equation { .. }))
            && (image.placement.flow != ImageFlow::Break
                || image.placement.offset.iter().any(|v| *v < 0.0)
                || image.effects != ImageEffects::default()
                || image.crop.is_some())
        {
            notices.push("Equations are exported as editable left-aligned Office Math blocks. Advanced wrapping, negative offsets and picture effects on equations are not represented in DOCX.".into());
            break;
        }
    }
    notices
}
pub fn export(doc: &Document, path: &Path) -> Result<(), String> {
    doc.validate()?;
    let mut writer = Writer {
        rels: vec![],
        images: vec![],
        counter: 0,
    };
    let mut body = String::new();
    for index in 0..doc.tabs.len() {
        let tab = &doc.tabs[index];
        body.push_str(&format!("<w:sdt alphadocs:tabId=\"{}\" alphadocs:tabTitle=\"{}\"><w:sdtPr><w:alias w:val=\"{}\"/><w:tag w:val=\"alphadocs-tab:{}\"/></w:sdtPr><w:sdtContent>", esc(&tab.id), esc(&tab.title), esc(&tab.title), esc(&tab.id)));
        for b in doc.tab_blocks(index) {
            match b{Block::Paragraph(p)=>body.push_str(&writer.paragraph(p)),Block::Table(rows)=>body.push_str(&writer.table(rows, (doc.page.size().0 - doc.page.margins[2] - doc.page.margins[3] - doc.page.column_gap * (doc.page.columns - 1) as f32) / doc.page.columns as f32)),Block::Image(i)=>body.push_str(&writer.image(i)?),Block::PageBreak=>body.push_str("<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>"),Block::Rule=>body.push_str("<w:p><w:pPr><w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\"/></w:pBdr></w:pPr></w:p>")}
        }
        body.push_str("</w:sdtContent></w:sdt>");
    }
    let (w, h) = doc.page.size();
    let m = doc.page.margins;
    body.push_str(&format!("<w:sectPr><w:pgSz w:w=\"{}\" w:h=\"{}\" w:orient=\"{}\"/><w:pgMar w:top=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:right=\"{}\"/></w:sectPr>",twips(w),twips(h),if doc.page.landscape{"landscape"}else{"portrait"},twips(m[0]),twips(m[1]),twips(m[2]),twips(m[3])));
    body = body.replace(
        "</w:sectPr>",
        &format!(
            "<w:cols w:num=\"{}\" w:space=\"{}\"/></w:sectPr>",
            doc.page.columns,
            twips(doc.page.column_gap)
        ),
    );
    if !doc.page.header.chars.is_empty() {
        let id = writer.relation("header", "header1.xml");
        body = body.replace(
            "<w:sectPr>",
            &format!("<w:sectPr><w:headerReference w:type=\"default\" r:id=\"{id}\"/>"),
        );
    }
    if doc.page.page_numbers || !doc.page.footer.chars.is_empty() {
        let id = writer.relation("footer", "footer1.xml");
        body = body.replace(
            "<w:pgSz",
            &format!("<w:footerReference w:type=\"default\" r:id=\"{id}\"/><w:pgSz"),
        );
    }
    let header = format!(
        "<w:hdr xmlns:alphadocs=\"urn:alphadocs:image:v1\" xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" mc:Ignorable=\"alphadocs\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">{}</w:hdr>",
        writer.paragraph(&doc.page.header)
    );
    let mut footer_text = writer.paragraph(&doc.page.footer);
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><w:background w:color=\"{}\"/><w:body>{body}</w:body></w:document>",
        rgb(doc.page.color)
    );
    let xml = xml.replace("<w:document ", &format!("<w:document alphadocs:activeTab=\"{}\" xmlns:alphadocs=\"urn:alphadocs:image:v1\" xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" mc:Ignorable=\"alphadocs\" ", esc(&doc.tabs[doc.active_tab].id)));
    let xml = xml.replace(
        "<w:document ",
        &format!(
            "<w:document alphadocs:citations=\"{}\" ",
            esc(&serde_json::to_string(&doc.citations).map_err(|e| e.to_string())?)
        ),
    );
    let xml = if doc.page.line_numbers != LineNumbering::Off {
        xml.replace(
            "<w:cols",
            &format!(
                "<w:lnNumType w:countBy=\"1\" w:distance=\"120\" w:restart=\"{}\"/><w:cols",
                if doc.page.line_numbers == LineNumbering::EachPage {
                    "newPage"
                } else {
                    "continuous"
                }
            ),
        )
    } else {
        xml
    };
    writer.relation("styles", "styles.xml");
    writer.relation("numbering", "numbering.xml");
    // Without a compatibility setting Word treats a newly generated package
    // as an old-format document and offers to downgrade modern editable shapes.
    writer.relation("settings", "settings.xml");
    let settings = "<w:settings xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:compat><w:compatSetting w:name=\"compatibilityMode\" w:uri=\"http://schemas.microsoft.com/office/word\" w:val=\"15\"/></w:compat></w:settings>";
    let rels=format!("<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{}</Relationships>",writer.rels.iter().map(|(id,typ,target)|format!("<Relationship Id=\"{id}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/{typ}\" Target=\"{}\"{}/>",esc(target),if typ=="hyperlink"{" TargetMode=\"External\""}else{""})).collect::<String>());
    let mut zip = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let styles=format!("<w:styles xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:style w:type=\"paragraph\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/></w:style>{}</w:styles>",(1..=6).map(|h|format!("<w:style w:type=\"paragraph\" w:styleId=\"Heading{h}\"><w:name w:val=\"heading {h}\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:outlineLvl w:val=\"{}\"/></w:pPr></w:style>",h-1)).collect::<String>());
    let numbering = "<w:numbering xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:abstractNum w:abstractNumId=\"1\"><w:lvl w:ilvl=\"0\"><w:start w:val=\"1\"/><w:numFmt w:val=\"bullet\"/><w:lvlText w:val=\"•\"/></w:lvl></w:abstractNum><w:abstractNum w:abstractNumId=\"2\"><w:lvl w:ilvl=\"0\"><w:start w:val=\"1\"/><w:numFmt w:val=\"decimal\"/><w:lvlText w:val=\"%1.\"/></w:lvl></w:abstractNum><w:num w:numId=\"1\"><w:abstractNumId w:val=\"1\"/></w:num><w:num w:numId=\"2\"><w:abstractNumId w:val=\"2\"/></w:num></w:numbering>";
    let types = "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Default Extension=\"png\" ContentType=\"image/png\"/><Default Extension=\"jpeg\" ContentType=\"image/jpeg\"/><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/><Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/><Override PartName=\"/word/numbering.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml\"/><Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/></Types>";
    let root_rels = "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/></Relationships>";
    let types = types.replace("</Types>", "<Override PartName=\"/word/footer1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml\"/></Types>");
    let types = types.replace("</Types>", "<Override PartName=\"/word/settings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml\"/></Types>");
    let page_field = "<w:ftr xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr><w:r><w:rPr><w:sz w:val=\"18\"/></w:rPr><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:instrText xml:space=\"preserve\"> PAGE </w:instrText></w:r><w:r><w:fldChar w:fldCharType=\"separate\"/></w:r><w:r><w:t>1</w:t></w:r><w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p></w:ftr>";
    if doc.page.page_numbers
        && let Some(field) = page_field
            .strip_prefix(
                "<w:ftr xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">",
            )
            .and_then(|s| s.strip_suffix("</w:ftr>"))
    {
        footer_text.push_str(field);
    }
    let footer = format!(
        "<w:ftr xmlns:alphadocs=\"urn:alphadocs:image:v1\" xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" mc:Ignorable=\"alphadocs\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">{footer_text}</w:ftr>"
    );
    let types=types.replace("</Types>","<Override PartName=\"/word/header1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml\"/></Types>");
    let core = format!(
        "<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>{}</dc:title><dc:creator>AlphaDocs</dc:creator></cp:coreProperties>",
        esc(&doc.title)
    );
    for (name, data) in [
        ("[Content_Types].xml", types.as_str()),
        ("_rels/.rels", root_rels),
        ("word/document.xml", &xml),
        ("word/_rels/document.xml.rels", &rels),
        ("word/styles.xml", &styles),
        ("word/numbering.xml", numbering),
        ("word/settings.xml", settings),
        ("docProps/core.xml", &core),
        ("word/footer1.xml", footer.as_str()),
        ("word/header1.xml", header.as_str()),
        ("word/_rels/header1.xml.rels", rels.as_str()),
        ("word/_rels/footer1.xml.rels", rels.as_str()),
    ] {
        zip.start_file(name, opts).map_err(|e| e.to_string())?;
        zip.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
    }
    for (name, data) in writer.images {
        zip.start_file(format!("word/media/{name}"), opts)
            .map_err(|e| e.to_string())?;
        zip.write_all(&data).map_err(|e| e.to_string())?;
    }
    let bytes = zip.finish().map_err(|e| e.to_string())?.into_inner();
    crate::persistence::atomic_write(path, &bytes)
}
fn read_part(zip: &mut ZipArchive<File>, name: &str) -> Result<Vec<u8>, String> {
    let mut f = zip.by_name(name).map_err(|e| e.to_string())?;
    if f.size() > 64 * 1024 * 1024 {
        return Err("DOCX part exceeds 64 MB".into());
    }
    let mut bytes = vec![];
    f.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}
fn read_xml(zip: &mut ZipArchive<File>, name: &str) -> Result<Node, String> {
    parse(&String::from_utf8(read_part(zip, name)?).map_err(|e| e.to_string())?)
}
fn on(n: &Node, key: &str) -> bool {
    n.child(key)
        .is_some_and(|c| !matches!(c.val(), Some("0" | "false" | "off" | "none")))
}
fn read_box_style(pr: &Node, borders: &str, mut style: BoxStyle) -> BoxStyle {
    if let Some(shading) = pr.child("shd") {
        style.fill = shading.attr("fill").and_then(unrgb);
    }
    if let Some(edges) = pr.child(borders) {
        for (index, side) in ["top", "bottom", "left", "right"].iter().enumerate() {
            if let Some(edge) = edges.child(side) {
                style.borders[index] = CellBorder {
                    colour: edge.attr("color").and_then(unrgb).unwrap_or([128; 3]),
                    width: if matches!(edge.val(), None | Some("nil" | "none")) {
                        0.0
                    } else {
                        edge.attr("sz")
                            .and_then(|s| s.parse::<f32>().ok())
                            .filter(|v| v.is_finite())
                            .unwrap_or(4.0)
                            .clamp(0.0, 96.0)
                            / 8.0
                    },
                };
            }
        }
    }
    style
}
fn table_formula(cell: &Node) -> Option<TableFormula> {
    fn instructions(node: &Node, out: &mut String) {
        if node.name == "instrText" {
            out.push_str(&node.text);
        }
        if node.name == "fldSimple"
            && let Some(value) = node.attr("instr")
        {
            out.push_str(value);
        }
        for child in &node.children {
            instructions(child, out);
        }
    }
    let mut code = String::new();
    instructions(cell, &mut code);
    let code = code.trim();
    if !code.starts_with('=') {
        return None;
    }
    let expression = code.split('\\').next()?.trim().to_owned();
    let mask = code
        .split("\\#")
        .nth(1)
        .and_then(|mask| mask.trim().strip_prefix('"'))
        .and_then(|mask| mask.split('"').next());
    let decimals = mask.and_then(|mask| {
        if mask == "0" {
            Some(0)
        } else {
            mask.strip_prefix("0.")
                .filter(|s| s.bytes().all(|c| c == b'0'))
                .map(|s| s.len() as u8)
        }
    });
    let formula = TableFormula {
        expression,
        decimals,
        word_number_format: mask.filter(|_| decimals.is_none()).map(str::to_owned),
    };
    formula.valid().then_some(formula)
}
fn write_box_style(style: BoxStyle, tag: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("<w:{tag}>"));
    for (index, side) in [(0, "top"), (2, "left"), (1, "bottom"), (3, "right")] {
        let border = style.borders[index];
        out.push_str(&format!(
            "<w:{side} w:val=\"{}\" w:sz=\"{}\" w:color=\"{}\"{}/>",
            if border.width == 0.0 { "nil" } else { "single" },
            (border.width * 8.0).round() as u32,
            rgb(border.colour),
            if tag == "pBdr" { " w:space=\"2\"" } else { "" }
        ));
    }
    out.push_str(&format!("</w:{tag}>"));
    if let Some(fill) = style.fill {
        out.push_str(&format!(
            "<w:shd w:val=\"clear\" w:fill=\"{}\"/>",
            rgb(fill)
        ));
    }
    out
}
fn table_cell_style(
    pr: &Node,
    mut style: BoxStyle,
    row: usize,
    column: usize,
    rows: usize,
    columns: usize,
) -> BoxStyle {
    if let Some(shd) = pr.child("shd") {
        style.fill = shd.attr("fill").and_then(unrgb);
    }
    if let Some(borders) = pr.child("tblBorders") {
        for (index, name) in [
            (0, if row > 0 { "insideH" } else { "top" }),
            (1, if row + 1 < rows { "insideH" } else { "bottom" }),
            (2, if column > 0 { "insideV" } else { "left" }),
            (
                3,
                if column + 1 < columns {
                    "insideV"
                } else {
                    "right"
                },
            ),
        ] {
            if let Some(edge) = borders.child(name) {
                let mut border = edge.clone();
                border.name = ["top", "bottom", "left", "right"][index].into();
                let wrapper = Node {
                    children: vec![Node {
                        name: "tcBorders".into(),
                        children: vec![border],
                        ..Default::default()
                    }],
                    ..Default::default()
                };
                style = read_box_style(&wrapper, "tcBorders", style);
            }
        }
    }
    style
}
fn apply_style(s: &mut TextStyle, n: &Node) {
    if n.child("b").is_some() {
        s.bold = on(n, "b");
    }
    if n.child("i").is_some() {
        s.italic = on(n, "i");
    }
    if n.child("u").is_some() {
        s.underline = on(n, "u");
    }
    if n.child("strike").is_some() {
        s.strike = on(n, "strike");
    }
    if let Some(v) = n
        .child("sz")
        .and_then(Node::val)
        .and_then(|v| v.parse::<f32>().ok())
    {
        s.size = v * 0.5;
    }
    if let Some(v) = n.child("color").and_then(Node::val).and_then(unrgb) {
        s.color = v;
    }
    if let Some(v) = n.child("vertAlign").and_then(Node::val) {
        s.baseline = match v {
            "superscript" => Baseline::Superscript,
            "subscript" => Baseline::Subscript,
            _ => Baseline::Normal,
        };
    }
    if let Some(v) = n.child("rFonts").and_then(|n| n.attr("ascii")) {
        s.font = v.into();
    }
    if let Some(v) = n.child("shd").and_then(|n| n.attr("fill")).and_then(unrgb) {
        s.highlight = Some(v);
    }
    if let Some(v) = n.child("highlight").and_then(Node::val) {
        s.highlight = match v {
            "yellow" => Some([255, 255, 0]),
            "green" => Some([0, 255, 0]),
            "cyan" => Some([0, 255, 255]),
            "magenta" => Some([255, 0, 255]),
            _ => s.highlight,
        };
    }
}
#[derive(Default)]
struct Styles {
    defaults: Paragraph,
    nodes: HashMap<String, Node>,
    default_id: Option<String>,
    theme: HashMap<String, String>,
}
impl Styles {
    fn chain<'a>(&'a self, id: &str) -> Vec<&'a Node> {
        let mut chain: Vec<&Node> = vec![];
        let mut current = self.nodes.get(id);
        while let Some(n) = current {
            if chain.len() == 64 || chain.iter().any(|old| std::ptr::eq(*old, n)) {
                break;
            }
            chain.push(n);
            current = n
                .child("basedOn")
                .and_then(Node::val)
                .and_then(|id| self.nodes.get(id));
        }
        chain.reverse();
        chain
    }
    fn run(&self, style: &mut TextStyle, n: &Node) {
        apply_style(style, n);
        if let Some(fonts) = n.child("rFonts") {
            if fonts
                .attr("ascii")
                .or_else(|| fonts.attr("hAnsi"))
                .is_none()
                && let Some(font) = fonts
                    .attr("asciiTheme")
                    .or_else(|| fonts.attr("hAnsiTheme"))
                    .and_then(|key| self.theme.get(key))
            {
                style.font = font.clone();
            } else if let Some(font) = fonts.attr("ascii").or_else(|| fonts.attr("hAnsi")) {
                style.font = font.into();
            }
        }
    }
    fn properties(&self, p: &mut Paragraph, pr: &Node, numbering: &HashMap<String, ListKind>) {
        if pr.child("pBdr").is_some() || pr.child("shd").is_some() {
            p.box_style = Some(read_box_style(
                pr,
                "pBdr",
                p.box_style.unwrap_or_else(BoxStyle::unbordered),
            ));
        }
        if let Some(r) = pr.child("rPr") {
            self.run(&mut p.style, r);
        }
        if let Some(v) = pr
            .child("outlineLvl")
            .and_then(Node::val)
            .and_then(|s| s.parse::<u8>().ok())
        {
            p.heading = if v < 6 { v + 1 } else { 0 };
        }
        if let Some(v) = pr.child("jc").and_then(Node::val) {
            p.alignment = match v {
                "center" => Alignment::Center,
                "right" | "end" => Alignment::Right,
                "both" | "distribute" => Alignment::Justified,
                _ => Alignment::Left,
            };
        }
        let number = |n: &Node, key: &str| {
            n.attr(key)
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|v| v.is_finite())
        };
        if let Some(ind) = pr.child("ind") {
            if let Some(v) = number(ind, "left").or_else(|| number(ind, "start")) {
                p.indent = (v / 20.0).max(0.0);
            }
            if let Some(v) = number(ind, "firstLine") {
                p.first_indent = v / 20.0;
            }
            if let Some(v) = number(ind, "hanging") {
                p.first_indent = -v / 20.0;
            }
        }
        if let Some(sp) = pr.child("spacing") {
            if let Some(v) = number(sp, "before") {
                p.before = (v / 20.0).max(0.0);
            }
            if let Some(v) = number(sp, "after") {
                p.after = (v / 20.0).max(0.0);
            }
            if let Some(v) = number(sp, "line") {
                // Exact/minimum point spacing has no native representation yet.
                if sp.attr("lineRule").is_none_or(|v| v == "auto") {
                    p.line_spacing = (v / 240.0).clamp(0.5, 5.0);
                }
            }
        }
        for (key, destination) in [
            ("keepNext", &mut p.keep_next),
            ("keepLines", &mut p.keep_together),
            ("widowControl", &mut p.widow_control),
            ("pageBreakBefore", &mut p.page_break_before),
        ] {
            if pr.child(key).is_some() {
                *destination = on(pr, key);
            }
        }
        if let Some(num) = pr
            .child("numPr")
            .and_then(|n| n.child("numId"))
            .and_then(Node::val)
        {
            p.list = if num == "0" {
                ListKind::None
            } else {
                numbering.get(num).copied().unwrap_or(ListKind::Number)
            };
        }
    }
}
fn collect_runs(
    n: &Node,
    p: &mut Paragraph,
    rels: &HashMap<String, String>,
    styles: &Styles,
    link: Option<String>,
    check_separator: &mut bool,
) {
    for child in &n.children {
        match child.name.as_str() {
            "hyperlink" => collect_runs(
                child,
                p,
                rels,
                styles,
                child.attr("id").and_then(|id| rels.get(id)).cloned(),
                check_separator,
            ),
            "sdt" => {
                if p.chars.is_empty()
                    && p.list != ListKind::Check
                    && child.child("sdtContent").is_some_and(|content| {
                        content.children.iter().all(|run| {
                            run.name == "r"
                                && run
                                    .children
                                    .iter()
                                    .all(|node| matches!(node.name.as_str(), "rPr" | "t"))
                        }) && content
                            .children
                            .iter()
                            .filter(|run| run.name == "r")
                            .flat_map(|run| &run.children)
                            .filter(|node| node.name == "t")
                            .flat_map(|node| node.text.chars())
                            .take(2)
                            .count()
                            == 1
                    })
                    && let Some(checkbox) = child.child("sdtPr").and_then(|pr| pr.child("checkbox"))
                {
                    p.list = ListKind::Check;
                    p.checked = checkbox.child("checked").is_some() && on(checkbox, "checked");
                    *check_separator = true;
                    continue;
                }
                if let Some(content) = child.child("sdtContent") {
                    collect_runs(content, p, rels, styles, link.clone(), check_separator);
                }
            }
            "ins" | "moveTo" | "smartTag" | "customXml" | "fldSimple" => {
                collect_runs(child, p, rels, styles, link.clone(), check_separator)
            }
            "AlternateContent" => {
                if let Some(branch) = child.child("Choice").or_else(|| child.child("Fallback")) {
                    collect_runs(branch, p, rels, styles, link.clone(), check_separator);
                }
            }
            "r" => {
                let mut s = p.style.clone();
                if let Some(r) = child.child("rPr") {
                    if let Some(id) = r.child("rStyle").and_then(Node::val) {
                        for n in styles.chain(id) {
                            if let Some(pr) = n.child("rPr") {
                                styles.run(&mut s, pr);
                            }
                        }
                    }
                    styles.run(&mut s, r);
                }
                if link.is_some() {
                    s.link = link.clone();
                }
                for t in &child.children {
                    let mut text = match t.name.as_str() {
                        "t" => t.text.clone(),
                        "footnoteReference" => {
                            format!("[Footnote {}]", t.attr("id").unwrap_or("?"))
                        }
                        "endnoteReference" => format!("[Endnote {}]", t.attr("id").unwrap_or("?")),
                        "tab" => "\t".into(),
                        "br" | "cr" if t.attr("type") != Some("page") => "\n".into(),
                        _ => String::new(),
                    };
                    if *check_separator && !text.is_empty() {
                        // Remove only the single layout separator after the
                        // control, preserving the task's own leading spaces.
                        if text.starts_with([' ', '\t']) {
                            text.remove(0);
                        }
                        *check_separator = false;
                    }
                    p.chars.extend(text.chars().map(|ch| StyledChar {
                        ch,
                        style: s.clone(),
                    }));
                }
            }
            _ => {}
        }
    }
}
fn paragraph(
    n: &Node,
    rels: &HashMap<String, String>,
    styles: &Styles,
    numbering: &HashMap<String, ListKind>,
) -> Paragraph {
    let mut p = styles.defaults.clone();
    p.annotations = n
        .attr("annotations")
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    let id = n
        .child("pPr")
        .and_then(|n| n.child("pStyle"))
        .and_then(Node::val)
        .or(styles.default_id.as_deref());
    if let Some(id) = id {
        for n in styles.chain(id) {
            let name = n
                .child("name")
                .and_then(Node::val)
                .unwrap_or(id)
                .to_lowercase()
                .replace(' ', "");
            p.heading = if let Some(h) = name
                .strip_prefix("heading")
                .and_then(|s| s.parse::<u8>().ok())
                .filter(|h| (1..=6).contains(h))
            {
                h
            } else if name == "title" {
                7
            } else if name == "subtitle" {
                8
            } else {
                p.heading
            };
            if let Some(pr) = n.child("pPr") {
                styles.properties(&mut p, pr, numbering);
            }
            if let Some(pr) = n.child("rPr") {
                styles.run(&mut p.style, pr);
            }
        }
    }
    if let Some(pr) = n.child("pPr") {
        styles.properties(&mut p, pr, numbering);
    }
    collect_runs(n, &mut p, rels, styles, None, &mut false);
    if !p.annotations.is_empty() && n.attr("annotationText") != Some(p.text().as_str()) {
        for note in &mut p.annotations {
            note.start = 0;
            note.end = 0;
        }
    }
    p
}
fn tab_marker(node: &Node) -> bool {
    node.attr("tabId").is_some()
        || node
            .child("sdtPr")
            .and_then(|n| n.child("tag"))
            .and_then(Node::val)
            .is_some_and(|v| {
                v.starts_with("alphadocs-tab:") || v.starts_with(crate::legacy::TAB_TAG)
            })
}
fn body_nodes<'a>(node: &'a Node, out: &mut Vec<&'a Node>) {
    match node.name.as_str() {
        "body" | "customXml" | "ins" | "moveTo" => {
            for child in &node.children {
                body_nodes(child, out);
            }
        }
        "sdt" => {
            if tab_marker(node) {
                out.push(node);
            }
            if let Some(content) = node.child("sdtContent") {
                for child in &content.children {
                    body_nodes(child, out);
                }
            }
        }
        "AlternateContent" => {
            if let Some(choice) = node.child("Choice").or_else(|| node.child("Fallback")) {
                for child in &choice.children {
                    body_nodes(child, out);
                }
            }
        }
        "del" | "moveFrom" => {}
        _ => out.push(node),
    }
}
pub fn import(path: &Path) -> Result<(Document, Vec<String>), String> {
    let mut zip =
        ZipArchive::new(File::open(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let root = read_xml(&mut zip, "word/document.xml")?;
    let body = root.descendant("body").ok_or("DOCX has no document body")?;
    let mut warnings = vec![];
    let mut rels = HashMap::new();
    if let Ok(r) = read_xml(&mut zip, "word/_rels/document.xml.rels")
        && let Some(r) = r.child("Relationships")
    {
        for n in &r.children {
            if let (Some(id), Some(t)) = (n.attr("Id"), n.attr("Target")) {
                rels.insert(id.to_string(), t.to_string());
            }
        }
    }
    let mut styles = Styles::default();
    // Word's absent line-spacing default is single, not the editor's new-document preset.
    styles.defaults.line_spacing = 1.0;
    if let Ok(theme) = read_xml(&mut zip, "word/theme/theme1.xml") {
        for (kind, prefix) in [("majorFont", "major"), ("minorFont", "minor")] {
            if let Some(face) = theme
                .descendant(kind)
                .and_then(|n| n.child("latin"))
                .and_then(|n| n.attr("typeface"))
            {
                for suffix in ["Ascii", "HAnsi", "EastAsia", "Bidi"] {
                    styles
                        .theme
                        .insert(format!("{prefix}{suffix}"), face.into());
                }
            }
        }
    }
    if let Ok(r) = read_xml(&mut zip, "word/styles.xml")
        && let Some(r) = r.child("styles")
    {
        if let Some(defaults) = r.child("docDefaults") {
            let mut p = styles.defaults.clone();
            if let Some(pr) = defaults.child("pPrDefault").and_then(|n| n.child("pPr")) {
                styles.properties(&mut p, pr, &HashMap::new());
            }
            if let Some(pr) = defaults.child("rPrDefault").and_then(|n| n.child("rPr")) {
                styles.run(&mut p.style, pr);
            }
            styles.defaults = p;
        }
        for n in &r.children {
            if let Some(id) = n.attr("styleId") {
                if n.attr("type") == Some("paragraph")
                    && n.attr("default").is_some_and(|v| v == "1" || v == "true")
                {
                    styles.default_id = Some(id.into());
                }
                styles.nodes.insert(id.into(), n.clone());
            }
        }
    }
    fn fixed_spacing(n: &Node) -> bool {
        // A generated horizontal line uses an empty anchor paragraph solely
        // to reserve its object box. Its native image height retains that space.
        if n.name == "p"
            && n.descendant("t").is_none()
            && n.descendant("docPr")
                .is_some_and(|p| p.attr("lineBoxHeight").is_some())
            && n.descendant("prstGeom").and_then(|p| p.attr("prst")) == Some("line")
        {
            return false;
        }
        (n.name == "spacing"
            && n.attr("lineRule")
                .is_some_and(|v| v == "exact" || v == "atLeast"))
            || n.children.iter().any(fixed_spacing)
    }
    if fixed_spacing(&root) || styles.nodes.values().any(fixed_spacing) {
        warnings.push(
            "Exact/minimum line heights are approximated by measured native line spacing".into(),
        );
    }
    let mut numbering = HashMap::new();
    if let Ok(r) = read_xml(&mut zip, "word/numbering.xml")
        && let Some(r) = r.child("numbering")
    {
        let mut abstracts = HashMap::new();
        for n in &r.children {
            if n.name == "abstractNum"
                && let Some(id) = n.attr("abstractNumId")
            {
                abstracts.insert(
                    id.to_string(),
                    if n.descendant("numFmt").and_then(Node::val) == Some("bullet") {
                        ListKind::Bullet
                    } else {
                        ListKind::Number
                    },
                );
            }
        }
        for n in &r.children {
            if n.name == "num"
                && let (Some(id), Some(a)) = (
                    n.attr("numId"),
                    n.child("abstractNumId").and_then(Node::val),
                )
            {
                numbering.insert(
                    id.into(),
                    abstracts.get(a).copied().unwrap_or(ListKind::Number),
                );
            }
        }
    }
    let title = read_xml(&mut zip, "docProps/core.xml")
        .ok()
        .and_then(|r| r.descendant("title").map(|n| n.text.clone()))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| {
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });
    let mut doc = Document::new(&title);
    let object_context = eframe::egui::Context::default();
    doc.blocks.clear();
    let mut tab_started = false;
    let mut nodes = Vec::new();
    body_nodes(body, &mut nodes);
    for (name, message) in [
        (
            "ins",
            "Tracked insertions are shown as current text; revision history is not imported.",
        ),
        (
            "del",
            "Tracked deletions are omitted from current text; revision history is not imported.",
        ),
        (
            "gridSpan",
            "Merged table cells are imported as separate cells; merged geometry is not retained.",
        ),
        ("vMerge", "Vertical table merges are not retained."),
        (
            "footnoteReference",
            "Footnote references are retained as markers; footnote layout is not supported.",
        ),
        (
            "endnoteReference",
            "Endnote references are retained as markers; endnote layout is not supported.",
        ),
        (
            "commentRangeStart",
            "Standard Word comment threads are not imported; document text is retained.",
        ),
    ] {
        if root.descendant(name).is_some() {
            warnings.push(message.into());
        }
    }
    for n in nodes {
        match n.name.as_str() {
            "sdt" => {
                if tab_started {
                    if doc.blocks.is_empty() {
                        doc.blocks.push(Block::Paragraph(Paragraph::default()));
                    }
                    let last = doc.tabs.len() - 1;
                    doc.tabs[last].blocks = std::mem::take(&mut doc.blocks).into();
                } else {
                    doc.tabs.clear();
                    tab_started = true;
                }
                let id = n
                    .descendant("tag")
                    .and_then(Node::val)
                    .and_then(|v| {
                        v.strip_prefix("alphadocs-tab:")
                            .or_else(|| v.strip_prefix(crate::legacy::TAB_TAG))
                    })
                    .or_else(|| n.attr("tabId"))
                    .unwrap_or("tab-1");
                let title = n
                    .descendant("alias")
                    .and_then(Node::val)
                    .or_else(|| n.attr("tabTitle"))
                    .unwrap_or("Tab");
                doc.tabs.push(ContentTab {
                    id: id.into(),
                    title: title.into(),
                    blocks: SharedVec::default(),
                });
                doc.active_tab = doc.tabs.len() - 1;
            }
            "p" => {
                let p = paragraph(n, &rels, &styles, &numbering);
                let has_text = !p.chars.is_empty();
                if has_text
                    || n.descendant("drawing").is_none()
                        && n.descendant("br").is_none()
                        && n.descendant("oMath").is_none()
                {
                    // A paragraph border is formatting, never a replacement for
                    // its content. Word also writes empty/nil border overrides.
                    if !has_text
                        && n.child("pPr")
                            .and_then(|pr| pr.child("pBdr"))
                            .and_then(|borders| borders.child("bottom"))
                            .is_some_and(|border| {
                                border.val() == Some("single")
                                    && border.attr("sz") == Some("6")
                                    && border.attr("color").is_none_or(|c| c == "808080")
                                    && n.child("pPr")
                                        .and_then(|p| p.child("pBdr"))
                                        .is_some_and(|b| b.children.len() == 1)
                                    && p.box_style.is_none_or(|style| style.fill.is_none())
                            })
                    {
                        doc.blocks.push(Block::Rule);
                    } else {
                        doc.blocks.push(Block::Paragraph(p.clone()));
                    }
                }
                if let Some(br) = n.descendant("br")
                    && br.attr("type") == Some("page")
                {
                    doc.blocks.push(Block::PageBreak);
                }
                let mut equations = Vec::new();
                n.descendants("oMath", &mut equations);
                for equation in equations {
                    match objects::read_math(equation, &p, &object_context) {
                        Ok(mut image) => {
                            objects::restore_metadata(&mut image, Some(n));
                            if doc
                                .blocks
                                .iter()
                                .any(|b| matches!(b, Block::Image(i) if i.id == image.id))
                            {
                                image.id = new_object_id();
                            }
                            doc.blocks.push(Block::Image(image));
                        }
                        Err(error) => {
                            warnings.push(format!("Could not import equation: {error}; readable mathematical text was retained."));
                            let mut tokens = Vec::new();
                            equation.descendants("t", &mut tokens);
                            doc.blocks.push(Block::Paragraph(Paragraph::from_text(
                                &tokens.iter().map(|n| n.text.as_str()).collect::<String>(),
                            )));
                        }
                    }
                    if has_text {
                        warnings.push(
                            "Inline equations are imported as separate editable objects.".into(),
                        );
                    }
                }
                let mut drawings = Vec::new();
                n.descendants("drawing", &mut drawings);
                for drawing in drawings {
                    if drawing.descendant("wsp").is_some() {
                        match objects::read_drawing(
                            drawing,
                            &rels,
                            &styles,
                            &numbering,
                            &object_context,
                        ) {
                            Ok(image) => doc.blocks.push(Block::Image(image)),
                            Err(error) => {
                                warnings.push(format!("Could not import drawing: {error}"));
                                if let Some(content) = drawing.descendant("txbxContent") {
                                    for child in content.children.iter().filter(|n| n.name == "p") {
                                        doc.blocks.push(Block::Paragraph(paragraph(
                                            child, &rels, &styles, &numbering,
                                        )));
                                    }
                                }
                            }
                        }
                    } else if let Some(id) =
                        drawing.descendant("blip").and_then(|n| n.attr("embed"))
                        && let Some(target) = rels.get(id)
                    {
                        let target = if target.starts_with('/') {
                            target.trim_start_matches('/').to_string()
                        } else {
                            format!("word/{target}")
                        };
                        if let Ok(mut data) = read_part(&mut zip, &target) {
                            let ex = drawing.descendant("extent");
                            let width = ex
                                .and_then(|n| n.attr("cx"))
                                .and_then(|v| v.parse::<f32>().ok())
                                .unwrap_or(2540000.0)
                                / 12700.0;
                            let height = ex
                                .and_then(|n| n.attr("cy"))
                                .and_then(|v| v.parse::<f32>().ok())
                                .unwrap_or(2540000.0)
                                / 12700.0;
                            let properties = drawing.descendant("docPr");
                            if let Some(anchor) = drawing.descendant("anchor")
                                && (anchor.child("positionH").is_some_and(|n| {
                                    n.attr("relativeFrom") != Some("column")
                                        || n.child("align").is_some()
                                }) || anchor.child("positionV").is_some_and(|n| {
                                    n.attr("relativeFrom") != Some("paragraph")
                                        || n.child("align").is_some()
                                }))
                            {
                                warnings.push("Floating image positions relative to page/margins or using alignment are approximated as column/text-anchor offsets.".into());
                            }
                            let mut trim_transparency =
                                properties.and_then(|n| n.attr("trim")) == Some("true");
                            let trim_bounds = properties
                                .and_then(|n| n.attr("trimBounds"))
                                .and_then(|s| serde_json::from_str::<Option<[f32; 4]>>(s).ok())
                                .flatten();
                            let standard_crop = drawing.descendant("srcRect").map(|n| {
                                let offset = |name| {
                                    n.attr(name)
                                        .and_then(|s| s.parse::<f32>().ok())
                                        .unwrap_or(0.0)
                                        / 100000.0
                                };
                                [
                                    offset("l"),
                                    offset("t"),
                                    1.0 - offset("r"),
                                    1.0 - offset("b"),
                                ]
                            });
                            let mut crop = properties
                                .and_then(|n| n.attr("crop"))
                                .and_then(|s| serde_json::from_str::<Option<[f32; 4]>>(s).ok())
                                .unwrap_or(standard_crop);
                            let mut visible = if trim_transparency {
                                trim_bounds.or(crop)
                            } else {
                                crop
                            }
                            .unwrap_or([0.0, 0.0, 1.0, 1.0]);
                            let mut padding: ImagePadding = properties
                                .and_then(|n| n.attr("padding"))
                                .and_then(|s| serde_json::from_str(s).ok())
                                .unwrap_or_default();
                            if !padding.fraction.is_finite()
                                || !(0.0..=0.25).contains(&padding.fraction)
                            {
                                padding = ImagePadding::default();
                            }
                            let saved_w = properties
                                .and_then(|n| n.attr("sourceWidth"))
                                .and_then(|s| s.parse::<f32>().ok())
                                .unwrap_or(width);
                            let saved_h = properties
                                .and_then(|n| n.attr("sourceHeight"))
                                .and_then(|s| s.parse::<f32>().ok())
                                .unwrap_or(height);
                            let mut restored_source = false;
                            if let Some(original) = properties
                                .and_then(|n| n.attr("originalMedia"))
                                .and_then(|id| rels.get(id))
                            {
                                let original = if original.starts_with('/') {
                                    original.trim_start_matches('/').to_owned()
                                } else {
                                    format!("word/{original}")
                                };
                                if let Ok(source) = read_part(&mut zip, &original) {
                                    let view = EmbeddedImage {
                                        data: source.clone().into(),
                                        width: saved_w,
                                        height: saved_h,
                                        crop: Some(visible),
                                        padding,
                                        ..Default::default()
                                    };
                                    if crate::image_edit::flattened_png(&view)
                                        .is_ok_and(|bytes| bytes == data)
                                        && standard_crop.is_none_or(|v| v == [0.0, 0.0, 1.0, 1.0])
                                    {
                                        data = source;
                                        restored_source = true;
                                    }
                                }
                                if !restored_source {
                                    padding.enabled = false;
                                    trim_transparency = false;
                                    visible = standard_crop.unwrap_or([0.0, 0.0, 1.0, 1.0]);
                                    crop = Some(visible);
                                    warnings.push("An external editor changed padded image media/crop; its standard image takes precedence over AlphaDocs's reversible source.".into());
                                }
                            }
                            let expected = if restored_source {
                                [0.0, 0.0, 1.0, 1.0]
                            } else {
                                visible
                            };
                            if let Some(standard) = standard_crop
                                && standard
                                    .iter()
                                    .zip(expected)
                                    .any(|(a, b)| (a - b).abs() > 0.00001)
                            {
                                // A different editor changed the visible crop. Its standard
                                // DrawingML geometry takes precedence over stale extensions.
                                visible = [
                                    standard[0].clamp(0.0, 1.0),
                                    standard[1].clamp(0.0, 1.0),
                                    standard[2].clamp(0.0, 1.0),
                                    standard[3].clamp(0.0, 1.0),
                                ];
                                crop = Some(visible);
                                trim_transparency = false;
                                padding.enabled = false;
                                if standard.iter().any(|v| !(0.0..=1.0).contains(v)) {
                                    warnings.push("Externally edited negative image crop is approximated; AlphaDocs padding was cleared.".into());
                                }
                            }
                            let mut source_width = properties
                                .and_then(|n| n.attr("sourceWidth"))
                                .and_then(|s| s.parse::<f32>().ok())
                                .unwrap_or(width / (visible[2] - visible[0]));
                            let mut source_height = properties
                                .and_then(|n| n.attr("sourceHeight"))
                                .and_then(|s| s.parse::<f32>().ok())
                                .unwrap_or(height / (visible[3] - visible[1]));
                            let geometry = EmbeddedImage {
                                width: source_width,
                                height: source_height,
                                crop: Some(visible),
                                padding,
                                ..Default::default()
                            };
                            let (saved_visible_w, saved_visible_h) = geometry.visible_size();
                            if (saved_visible_w - width).abs() > 0.02
                                || (saved_visible_h - height).abs() > 0.02
                            {
                                (source_width, source_height) =
                                    geometry.source_size_for_visible(width, height);
                            }
                            doc.blocks.push(Block::Image(EmbeddedImage {
                                editable: None,
                                id: properties
                                    .and_then(|n| n.attr("objectId"))
                                    .filter(|id| {
                                        !doc.blocks
                                            .iter()
                                            .any(|b| matches!(b,Block::Image(i) if &i.id==id))
                                    })
                                    .map(str::to_owned)
                                    .unwrap_or_else(new_object_id),
                                placement: import_placement(drawing, properties),
                                comments: properties
                                    .and_then(|n| n.attr("comments"))
                                    .and_then(|s| serde_json::from_str(s).ok())
                                    .unwrap_or_default(),
                                reactions: properties
                                    .and_then(|n| n.attr("reactions"))
                                    .and_then(|s| serde_json::from_str(s).ok())
                                    .unwrap_or_default(),
                                name: properties
                                    .and_then(|n| n.attr("name"))
                                    .unwrap_or(&target)
                                    .into(),
                                data: data.into(),
                                width: source_width,
                                height: source_height,
                                crop,
                                trim_transparency,
                                trim_bounds,
                                padding,
                                effects: {
                                    let lum = drawing.descendant("lum");
                                    let fraction = |node: Option<&Node>, name: &str, default| {
                                        node.and_then(|n| n.attr(name))
                                            .and_then(|s| s.parse::<f32>().ok())
                                            .map(|v| v / 100000.0)
                                            .unwrap_or(default)
                                    };
                                    let standard = ImageEffects {
                                        opacity: fraction(
                                            drawing.descendant("alphaModFix"),
                                            "amt",
                                            1.0,
                                        ),
                                        brightness: fraction(lum, "bright", 0.0),
                                        contrast: fraction(lum, "contrast", 0.0),
                                        grayscale: drawing.descendant("grayscl").is_some(),
                                    };
                                    let stored: Option<ImageEffects> = properties
                                        .and_then(|n| n.attr("effects"))
                                        .and_then(|s| serde_json::from_str(s).ok());
                                    stored
                                        .filter(|e| {
                                            e.grayscale == standard.grayscale
                                                && (e.opacity - standard.opacity).abs() <= 0.00001
                                                && (e.brightness - standard.brightness).abs()
                                                    <= 0.00001
                                                && (e.contrast - standard.contrast).abs() <= 0.00001
                                        })
                                        .unwrap_or(standard)
                                },
                                alt_text: properties
                                    .and_then(|n| n.attr("descr"))
                                    .unwrap_or_default()
                                    .into(),
                                title: properties
                                    .and_then(|n| n.attr("title"))
                                    .unwrap_or_default()
                                    .into(),
                            }));
                            if has_text {
                                warnings.push(
                                    "Inline images are imported as separate image blocks.".into(),
                                );
                            }
                        } else {
                            warnings.push(format!("Could not load embedded image: {target}"));
                        }
                    } else {
                        warnings.push(
                            "An unsupported drawing was omitted; any contained text was retained."
                                .into(),
                        );
                        let mut content = Vec::new();
                        drawing.descendants("txbxContent", &mut content);
                        for body in content {
                            let mut paragraphs = Vec::new();
                            body.descendants("p", &mut paragraphs);
                            for child in paragraphs {
                                doc.blocks.push(Block::Paragraph(paragraph(
                                    child, &rels, &styles, &numbering,
                                )));
                            }
                        }
                    }
                }
                let mut legacy_pictures = Vec::new();
                n.descendants("pict", &mut legacy_pictures);
                for picture in legacy_pictures {
                    warnings.push("Legacy VML geometry is unsupported; text box contents were retained as editable paragraphs.".into());
                    let mut contents = Vec::new();
                    picture.descendants("txbxContent", &mut contents);
                    for body in contents {
                        let mut paragraphs = Vec::new();
                        body.descendants("p", &mut paragraphs);
                        for child in paragraphs {
                            doc.blocks.push(Block::Paragraph(paragraph(
                                child, &rels, &styles, &numbering,
                            )));
                        }
                    }
                }
            }
            "tbl" => {
                let row_nodes: Vec<_> = n.children.iter().filter(|n| n.name == "tr").collect();
                let row_count = row_nodes.len();
                let mut rows = Vec::new();
                for (ri, r) in row_nodes.into_iter().enumerate() {
                    let cells: Vec<_> = r.children.iter().filter(|n| n.name == "tc").collect();
                    let count = cells.len();
                    let mut row = Vec::new();
                    for (ci, c) in cells.into_iter().enumerate() {
                        let mut nodes = Vec::new();
                        for child in &c.children {
                            body_nodes(child, &mut nodes);
                        }
                        let mut paragraphs = nodes
                            .into_iter()
                            .flat_map(|node| {
                                let mut paragraphs = Vec::new();
                                node.descendants("p", &mut paragraphs);
                                paragraphs
                            })
                            .map(|n| paragraph(n, &rels, &styles, &numbering));
                        let mut p = paragraphs.next().unwrap_or_default();
                        for more in paragraphs {
                            p.chars.push(StyledChar {
                                ch: '\n',
                                style: p.style.clone(),
                            });
                            p.chars.extend(more.chars);
                        }
                        if c.descendant("tbl").is_some() {
                            warnings.push("Nested tables are flattened to editable text within the containing cell.".into());
                        }
                        if c.descendant("drawing").is_some() || c.descendant("oMath").is_some() {
                            warnings.push("Objects inside table cells are unsupported; cell text is retained.".into());
                        }
                        p.formula = table_formula(c);
                        let mut style = BoxStyle::unbordered();
                        if let Some(id) = n
                            .child("tblPr")
                            .and_then(|pr| pr.child("tblStyle"))
                            .and_then(Node::val)
                        {
                            for named in styles.chain(id) {
                                if let Some(pr) = named.child("tblPr") {
                                    style = table_cell_style(pr, style, ri, ci, row_count, count);
                                }
                            }
                        }
                        if let Some(pr) = n.child("tblPr") {
                            style = table_cell_style(pr, style, ri, ci, row_count, count);
                        }
                        if let Some(pr) = c.child("tcPr") {
                            style = read_box_style(pr, "tcBorders", style);
                        }
                        p.box_style = (style != BoxStyle::default()).then_some(style);
                        row.push(p);
                    }
                    rows.push(row);
                }
                doc.blocks.push(Block::Table(rows));
            }
            "sectPr" => {
                if let Some(sz) = n.child("pgSz") {
                    let w = sz
                        .attr("w")
                        .and_then(|v| v.parse::<f32>().ok())
                        .unwrap_or(11906.0)
                        / 20.0;
                    let h = sz
                        .attr("h")
                        .and_then(|v| v.parse::<f32>().ok())
                        .unwrap_or(16838.0)
                        / 20.0;
                    doc.page.landscape = sz.attr("orient").map_or(w > h, |s| s == "landscape");
                    doc.page.paper = if doc.page.landscape {
                        Paper::from_size(h, w)
                    } else {
                        Paper::from_size(w, h)
                    };
                }
                if let Some(line) = n.child("lnNumType") {
                    doc.page.line_numbers = if line.attr("restart") == Some("newPage") {
                        LineNumbering::EachPage
                    } else {
                        LineNumbering::Continuous
                    };
                }
                if let Some(m) = n.child("pgMar") {
                    for (i, key) in ["top", "bottom", "left", "right"].iter().enumerate() {
                        doc.page.margins[i] = m
                            .attr(key)
                            .and_then(|v| v.parse::<f32>().ok())
                            .unwrap_or(1440.0)
                            / 20.0;
                    }
                }
                if let Some(columns) = n.child("cols") {
                    doc.page.columns = columns
                        .attr("num")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(1);
                    doc.page.column_gap = columns
                        .attr("space")
                        .and_then(|v| v.parse::<f32>().ok())
                        .unwrap_or(360.0)
                        / 20.0;
                    if columns.attr("equalWidth") == Some("0")
                        || columns.children.iter().any(|n| n.name == "col")
                    {
                        warnings
                            .push("Unequal section columns are imported with equal widths.".into());
                    }
                }
                for (tag, is_header) in [("headerReference", true), ("footerReference", false)] {
                    if let Some(reference) = n.child(tag)
                        && let Some(target) = reference.attr("id").and_then(|id| rels.get(id))
                    {
                        let part = if target.starts_with('/') {
                            target.trim_start_matches('/').into()
                        } else {
                            format!("word/{target}")
                        };
                        match read_xml(&mut zip, &part) {
                            Ok(root) => {
                                let source = root.descendant(if is_header { "hdr" } else { "ftr" });
                                if let Some(source) = source {
                                    let mut paragraphs = source
                                        .children
                                        .iter()
                                        .filter(|n| n.name == "p")
                                        .filter(|n| {
                                            let page_field = n
                                                .descendant("instrText")
                                                .is_some_and(|t| t.text.trim() == "PAGE");
                                            if page_field {
                                                doc.page.page_numbers = true;
                                            }
                                            !page_field
                                        })
                                        .map(|n| paragraph(n, &rels, &styles, &numbering));
                                    if let Some(mut p) = paragraphs.next() {
                                        for more in paragraphs {
                                            if !p.chars.is_empty() {
                                                p.chars.push(StyledChar {
                                                    ch: '\n',
                                                    style: p.style.clone(),
                                                });
                                            }
                                            p.chars.extend(more.chars);
                                        }
                                        if is_header {
                                            doc.page.header = p;
                                        } else {
                                            doc.page.footer = p;
                                        }
                                    }
                                    if source.descendant("tbl").is_some()
                                        || source.descendant("drawing").is_some()
                                    {
                                        warnings.push("Header/footer objects are not imported; text is retained.".into());
                                    }
                                }
                            }
                            Err(e) => warnings.push(format!("Could not import {tag}: {e}")),
                        }
                    }
                }
            }
            _ => warnings.push(format!("Unsupported OOXML block: {}", n.name)),
        }
    }
    // Keep note contents readable/editable even though native dynamic footnote
    // placement is not part of this document model yet.
    for (part, tag, reference, label) in [
        (
            "word/footnotes.xml",
            "footnote",
            "footnoteReference",
            "Footnote",
        ),
        (
            "word/endnotes.xml",
            "endnote",
            "endnoteReference",
            "Endnote",
        ),
    ] {
        if root.descendant(reference).is_none() {
            continue;
        }
        match read_xml(&mut zip, part) {
            Ok(notes) => {
                fn collect<'a>(node: &'a Node, tag: &str, out: &mut Vec<&'a Node>) {
                    if node.name == tag {
                        out.push(node);
                    } else {
                        for child in &node.children {
                            collect(child, tag, out);
                        }
                    }
                }
                let mut entries = Vec::new();
                collect(&notes, tag, &mut entries);
                for note in entries {
                    if note.attr("type").is_some_and(|kind| kind != "normal") {
                        continue;
                    }
                    let Some(id) = note
                        .attr("id")
                        .and_then(|s| s.parse::<i32>().ok())
                        .filter(|id| *id >= 0)
                    else {
                        continue;
                    };
                    let mut paragraphs = Vec::new();
                    collect(note, "p", &mut paragraphs);
                    for (index, node) in paragraphs.into_iter().enumerate() {
                        let mut p = paragraph(node, &rels, &styles, &numbering);
                        if index == 0 {
                            let prefix = format!("[{label} {id}] ");
                            let style = p.style.clone();
                            p.replace_text(
                                0..0,
                                prefix
                                    .chars()
                                    .map(|ch| StyledChar {
                                        ch,
                                        style: style.clone(),
                                    })
                                    .collect(),
                            );
                        }
                        doc.blocks.push(Block::Paragraph(p));
                    }
                }
                warnings.push(format!("{label} contents are retained as editable paragraphs at the end; dynamic note placement/references are not supported."));
            }
            Err(error) => warnings.push(format!("Could not import {label} contents: {error}")),
        }
    }
    if let Some(c) = root
        .descendant("background")
        .and_then(|n| n.attr("color"))
        .and_then(unrgb)
    {
        doc.page.color = c;
    }
    if doc.addresses().is_empty() {
        doc.blocks.push(Block::Paragraph(Paragraph::default()));
    }
    if let Some(id) = root
        .descendant("document")
        .and_then(|n| n.attr("activeTab"))
        && let Some(index) = doc.tabs.iter().position(|t| t.id == id)
    {
        doc.select_tab(index);
    }
    if let Some(value) = root
        .descendant("document")
        .and_then(|n| n.attr("citations"))
    {
        doc.citations =
            serde_json::from_str(value).map_err(|_| "Invalid AlphaDocs citation metadata")?;
    }
    doc.validate()?;
    warnings.sort();
    warnings.dedup();
    Ok((doc, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn package_with_body(body: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("alphadocs-word-fixture-{}.docx", new_object_id()));
        let mut zip = ZipWriter::new(File::create(&path).unwrap());
        zip.start_file("word/document.xml", SimpleFileOptions::default())
            .unwrap();
        write!(zip, "<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}</w:body></w:document>").unwrap();
        zip.finish().unwrap();
        path
    }

    #[test]
    fn cell_shading_borders_and_real_word_formula_fields_roundtrip() {
        let mut editor = crate::engine::Editor::new(Document::new("Editable calculations"));
        editor.insert_table(3, 2);
        let block = editor.caret.address.block;
        let Block::Table(rows) = &mut editor.doc.blocks[block] else {
            panic!()
        };
        rows[0][0] = Paragraph::from_text("10.5");
        rows[1][0] = Paragraph::from_text("4");
        let style = BoxStyle {
            fill: Some([211, 227, 253]),
            borders: [CellBorder {
                colour: [11, 87, 208],
                width: 1.5,
            }; 4],
        };
        rows[1][0].box_style = Some(style);
        editor.place(
            Position {
                address: Address {
                    block,
                    row: 2,
                    cell: 0,
                },
                offset: 0,
            },
            false,
        );
        editor
            .set_table_formula(TableFormula {
                expression: "=SUM(ABOVE)".into(),
                decimals: Some(2),
                word_number_format: None,
            })
            .unwrap();
        let at = editor.caret.address;
        assert_eq!(editor.doc.paragraph(at).unwrap().text(), "14.50");
        let original = editor.doc.clone();
        assert!(
            editor
                .set_table_formula(TableFormula {
                    expression: "=A3".into(),
                    decimals: None,
                    word_number_format: None,
                })
                .is_err()
        );
        assert_eq!(editor.doc, original);
        editor.insert("x");
        assert!(editor.doc.paragraph(at).unwrap().formula.is_none());
        editor.undo();
        assert_eq!(editor.doc, original);
        let path =
            std::env::temp_dir().join(format!("alphadocs-table-format-{}.docx", new_object_id()));
        export(&editor.doc, &path).unwrap();
        let (imported, warnings) = import(&path).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(imported.blocks, editor.doc.blocks);
        let native: Document =
            serde_json::from_slice(&serde_json::to_vec(&editor.doc).unwrap()).unwrap();
        native.validate().unwrap();
        assert_eq!(native, editor.doc);
        let mut source = ZipArchive::new(File::open(&path).unwrap()).unwrap();
        let mut xml = String::new();
        source
            .by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        assert!(xml.contains("<w:fldSimple w:instr=\"=SUM(ABOVE)"));
        assert!(xml.contains("<w:tcBorders>"));
        assert!(xml.contains("w:fill=\"D3E3FD\""));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn word_paragraph_borders_never_replace_story_text_with_rules() {
        let source = package_with_body(
            r#"
            <w:p><w:pPr><w:pBdr><w:bottom w:val="single" w:sz="8"/></w:pBdr></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>Chapter one — café 😀</w:t></w:r></w:p>
            <w:p><w:pPr><w:pBdr><w:top w:val="nil"/><w:bottom w:val="none"/></w:pBdr></w:pPr><w:r><w:t>The story remains editable.</w:t></w:r></w:p>
            <w:p><w:pPr><w:pBdr/></w:pPr><w:r><w:t>Empty overrides are not rules.</w:t></w:r></w:p>
            <w:tbl><w:tr><w:tc><w:p><w:r><w:t>Table text survives too.</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
            <w:p><w:pPr><w:pBdr><w:bottom w:val="single" w:sz="6"/></w:pBdr></w:pPr></w:p>
            <w:p><w:pPr><w:pBdr><w:bottom w:val="nil"/></w:pBdr></w:pPr></w:p>
        "#,
        );
        let (doc, _) = import(&source).unwrap();
        assert_eq!(doc.blocks.len(), 6);
        assert!(matches!(doc.blocks[4], Block::Rule));
        assert!(matches!(doc.blocks[5], Block::Paragraph(_)));
        let first = doc.paragraph(Address::default()).unwrap();
        assert_eq!(first.text(), "Chapter one — café 😀");
        assert!(first.chars.iter().all(|c| c.style.bold));
        assert_eq!(
            doc.blocks
                .iter()
                .filter(|b| matches!(b, Block::Rule))
                .count(),
            1
        );
        let mut editor = crate::engine::Editor::new(doc);
        let original = editor.doc.text();
        editor.insert("Edited: ");
        editor.undo();
        assert_eq!(editor.doc.text(), original);
        let target = source.with_extension("roundtrip.docx");
        export(&editor.doc, &target).unwrap();
        let (roundtrip, _) = import(&target).unwrap();
        assert_eq!(roundtrip.text(), original);
        assert_eq!(
            roundtrip.paragraph(Address::default()).unwrap().chars,
            first_chars(&editor)
        );
        std::fs::remove_file(source).unwrap();
        std::fs::remove_file(target).unwrap();
    }

    fn first_chars(editor: &crate::engine::Editor) -> SharedVec<StyledChar> {
        editor
            .doc
            .paragraph(Address::default())
            .unwrap()
            .chars
            .clone()
    }

    #[test]
    fn word_wrappers_legacy_textboxes_nested_cell_text_and_custom_formula_masks_retain_content() {
        let source = package_with_body(
            r#"
            <w:sdt><w:sdtContent><w:p><w:r><w:t>Wrapped chapter</w:t></w:r></w:p></w:sdtContent></w:sdt>
            <w:p><mc:AlternateContent><mc:Choice><w:r><w:t>Selected branch</w:t></w:r></mc:Choice><mc:Fallback><w:r><w:t>Duplicate fallback</w:t></w:r></mc:Fallback></mc:AlternateContent></w:p>
            <w:p><w:r><w:pict><v:shape><v:textbox><w:txbxContent><w:p><w:r><w:t>Legacy editable text</w:t></w:r></w:p></w:txbxContent></v:textbox></v:shape></w:pict></w:r></w:p>
            <w:tbl><w:tr><w:tc><w:p><w:r><w:t>Outer cell</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Nested cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl>
            <w:tbl><w:tr><w:tc><w:p><w:fldSimple w:instr='=1+2 \# "$#,##0.00"'><w:r><w:t>$3.00</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr></w:tbl>
        "#,
        );
        let (doc, warnings) = import(&source).unwrap();
        let text = doc.text();
        for expected in [
            "Wrapped chapter",
            "Selected branch",
            "Legacy editable text",
            "Outer cell",
            "Nested cell",
            "$3.00",
        ] {
            assert!(text.contains(expected), "Missing {expected}: {text}");
        }
        assert!(!text.contains("Duplicate fallback"));
        assert!(warnings.iter().any(|w| w.contains("Legacy VML")));
        assert!(warnings.iter().any(|w| w.contains("Nested tables")));
        let block = doc
            .blocks
            .iter()
            .rposition(|b| matches!(b, Block::Table(_)))
            .unwrap();
        let at = Address {
            block,
            row: 0,
            cell: 0,
        };
        assert_eq!(
            doc.paragraph(at)
                .unwrap()
                .formula
                .as_ref()
                .unwrap()
                .word_number_format
                .as_deref(),
            Some("$#,##0.00")
        );
        let mut editor = crate::engine::Editor::new(doc);
        editor.place(
            Position {
                address: at,
                offset: 0,
            },
            false,
        );
        let before = editor.doc.clone();
        assert!(editor.recalculate_table().is_err());
        assert_eq!(before, editor.doc);
        let target = source.with_extension("roundtrip.docx");
        export(&editor.doc, &target).unwrap();
        let (loaded, _) = import(&target).unwrap();
        assert_eq!(loaded.text(), editor.doc.text());
        assert_eq!(
            loaded.paragraph(at).unwrap().formula,
            editor.doc.paragraph(at).unwrap().formula
        );
        std::fs::remove_file(source).unwrap();
        std::fs::remove_file(target).unwrap();
    }

    #[test]
    fn checklist_word_controls_preserve_states_text_styles_and_all_tabs() {
        let task = |checked| {
            let mut paragraph = Paragraph::from_text("  Keep the original spacing α");
            paragraph.list = ListKind::Check;
            paragraph.checked = checked;
            paragraph.chars[4].style.bold = true;
            paragraph
        };
        let mut editor = crate::engine::Editor::new(Document::new("Word checklist"));
        editor.doc.blocks = vec![
            Block::Paragraph(task(false)),
            Block::Table(vec![vec![task(true)]]),
        ];
        editor.add_tab();
        editor.doc.blocks = vec![Block::Paragraph(task(true))];
        editor.switch_tab(0);
        editor.doc.page.header = task(false);
        editor.doc.page.footer = task(true);
        let before = editor.doc.clone();
        let path = std::env::temp_dir().join(format!("alphadocs-checklist-{}.docx", before.id));
        export(&before, &path).unwrap();
        let (loaded, warnings) = import(&path).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        for index in 0..2 {
            assert_eq!(loaded.tab_blocks(index), before.tab_blocks(index));
        }
        assert_eq!(editor.doc, before);
        assert_eq!(loaded.page.header, before.page.header);
        assert_eq!(loaded.page.footer, before.page.footer);
        let mut zip = ZipArchive::new(File::open(&path).unwrap()).unwrap();
        let mut xml = String::new();
        zip.by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        assert_eq!(xml.matches("<w14:checkbox>").count(), 3);
        assert_eq!(xml.matches("<w14:checked w14:val=\"1\"").count(), 2);
        assert!(
            !xml.contains("<w:numPr>"),
            "Checkboxes must not degrade to bullets"
        );
        drop(zip);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn imported_checkbox_state_is_standard_word_state_not_private_metadata_or_display_glyph() {
        for checked in [false, true] {
            let xml = format!(
                "<p><sdt><sdtPr><checkbox><checked val=\"{}\"/></checkbox></sdtPr><sdtContent><r><t>☐</t></r></sdtContent></sdt><r><t> </t></r><r><t>  Task α</t></r></p>",
                u8::from(checked)
            );
            let root = parse(&xml).unwrap();
            let p = paragraph(
                root.child("p").unwrap(),
                &HashMap::new(),
                &Styles::default(),
                &HashMap::new(),
            );
            assert_eq!(p.list, ListKind::Check);
            assert_eq!(p.checked, checked);
            assert_eq!(p.text(), "  Task α");
        }
        // An inline or malformed multi-character control must retain its text.
        for (prefix, content, expected) in [
            ("Before ", "☑", "Before ☑ after"),
            ("", "Keep me", "Keep me after"),
        ] {
            let xml = format!(
                "<p><r><t>{prefix}</t></r><sdt><sdtPr><checkbox/></sdtPr><sdtContent><r><t>{content}</t></r></sdtContent></sdt><r><t> after</t></r></p>"
            );
            let root = parse(&xml).unwrap();
            let p = paragraph(
                root.child("p").unwrap(),
                &HashMap::new(),
                &Styles::default(),
                &HashMap::new(),
            );
            assert_eq!(p.list, ListKind::None);
            assert_eq!(p.text(), expected);
        }
    }
    #[test]
    fn defaults_style_ancestry_and_partial_overrides_preserve_word_metrics() {
        let xml=parse(r#"<styles><docDefaults><rPrDefault><rPr><rFonts ascii="Arial"/><sz val="22"/></rPr></rPrDefault><pPrDefault><pPr><spacing line="276" lineRule="auto"/></pPr></pPrDefault></docDefaults><style styleId="Normal"><pPr><spacing after="120"/></pPr></style><style styleId="Chapter"><basedOn val="Normal"/><name val="heading 2"/><pPr><keepNext/><spacing before="240"/></pPr><rPr><sz val="32"/><b/></rPr></style><style styleId="Emphasis"><rPr><i/></rPr></style></styles>"#).unwrap();
        let root = xml.child("styles").unwrap();
        let mut styles = Styles::default();
        let defaults = root.child("docDefaults").unwrap();
        let mut p = Paragraph::default();
        styles.properties(
            &mut p,
            defaults.child("pPrDefault").unwrap().child("pPr").unwrap(),
            &HashMap::new(),
        );
        styles.run(
            &mut p.style,
            defaults.child("rPrDefault").unwrap().child("rPr").unwrap(),
        );
        styles.defaults = p;
        styles.default_id = Some("Normal".into());
        for n in &root.children {
            if let Some(id) = n.attr("styleId") {
                styles.nodes.insert(id.into(), n.clone());
            }
        }
        let xml=parse(r#"<p><pPr><pStyle val="Chapter"/><keepNext val="0"/><spacing after="0"/></pPr><r><t>Heading </t></r><r><rPr><rStyle val="Emphasis"/><b val="0"/></rPr><t>plain italic</t></r></p>"#).unwrap();
        let p = paragraph(
            xml.child("p").unwrap(),
            &HashMap::new(),
            &styles,
            &HashMap::new(),
        );
        assert_eq!(p.heading, 2);
        assert_eq!(p.style.font, "Arial");
        assert_eq!(
            (p.style.size, p.line_spacing, p.before, p.after),
            (16.0, 1.15, 12.0, 0.0)
        );
        assert!(!p.keep_next);
        assert!(p.chars[0].style.bold);
        assert!(!p.chars[8].style.bold);
        assert!(p.chars[8].style.italic);
        assert_eq!(p.chars[8].style.size, 16.0);
        let cyclic = parse("<style styleId='Cycle'><basedOn val='Cycle'/></style>").unwrap();
        styles
            .nodes
            .insert("Cycle".into(), cyclic.child("style").unwrap().clone());
        assert_eq!(styles.chain("Cycle").len(), 1);
    }
    #[test]
    fn document_tabs_roundtrip_and_standard_word_content_remains_editable() {
        let mut editor = crate::engine::Editor::new(Document::new("Tabbed file"));
        editor.insert("First tab α");
        editor.add_tab();
        editor.rename_tab(1, "Research & notes");
        editor.insert_table(2, 2);
        editor.insert("Second tab cell");
        editor.switch_tab(0);
        let path = std::env::temp_dir().join(format!("alphadocs-tabs-{}.docx", editor.doc.id));
        export(&editor.doc, &path).unwrap();
        let (loaded, warnings) = import(&path).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(loaded.active_tab, 0);
        assert_eq!(loaded.tabs.len(), 2);
        for index in 0..2 {
            assert_eq!(loaded.tabs[index].id, editor.doc.tabs[index].id);
            assert_eq!(loaded.tabs[index].title, editor.doc.tabs[index].title);
            assert_eq!(loaded.tab_blocks(index), editor.doc.tab_blocks(index));
        }
        let mut zip = ZipArchive::new(File::open(&path).unwrap()).unwrap();
        let xml = read_xml(&mut zip, "word/document.xml").unwrap();
        let body = xml.descendant("body").unwrap();
        assert_eq!(body.children.iter().filter(|n| n.name == "sdt").count(), 2);
        assert!(body.descendant("tbl").is_some());
        std::fs::remove_file(&path).unwrap();
    }
    #[test]
    fn fonts_baselines_columns_and_running_text_roundtrip() {
        let mut doc = Document::new("Interchange");
        doc.page.columns = 2;
        doc.page.column_gap = 24.0;
        doc.page.header = Paragraph::from_text("Header café");
        doc.page.footer = Paragraph::from_text("Footer");
        doc.page.page_numbers = true;
        let mut paragraph = Paragraph::from_text("H2O");
        paragraph.keep_together = true;
        paragraph.widow_control = false;
        paragraph.chars[1].style.baseline = Baseline::Subscript;
        for ch in &mut paragraph.chars {
            ch.style.font = "Noto Sans Mono".into();
        }
        doc.blocks = vec![Block::Paragraph(paragraph)];
        let path = std::env::temp_dir().join(format!("alphadocs-format-{}.docx", doc.id));
        export(&doc, &path).unwrap();
        let (loaded, warnings) = import(&path).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(loaded.page.columns, 2);
        assert_eq!(loaded.page.column_gap, 24.0);
        assert_eq!(loaded.page.header.text(), doc.page.header.text());
        assert_eq!(loaded.page.footer.text(), "Footer");
        assert!(loaded.page.page_numbers);
        let p = loaded.paragraph(Address::default()).unwrap();
        assert_eq!(p.chars[1].style.baseline, Baseline::Subscript);
        assert_eq!(p.chars[0].style.font, "Noto Sans Mono");
        assert!(p.keep_together);
        assert!(!p.widow_control);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn all_paper_sizes_and_orientations_roundtrip_with_line_number_settings() {
        for paper in Paper::STANDARD.into_iter().chain([Paper::Custom {
            width: 703.25,
            height: 911.75,
        }]) {
            for landscape in [false, true] {
                let mut doc = Document::new("Page geometry");
                doc.page.paper = paper;
                doc.page.landscape = landscape;
                doc.page.margins = [36.0, 42.0, 48.0, 54.0];
                doc.page.line_numbers = if landscape {
                    LineNumbering::EachPage
                } else {
                    LineNumbering::Continuous
                };
                doc.blocks = vec![Block::Paragraph(Paragraph::from_text("Geometry"))];
                let path = std::env::temp_dir()
                    .join(format!("alphadocs-paper-{}-{}.docx", doc.id, landscape));
                export(&doc, &path).unwrap();
                let (loaded, warnings) = import(&path).unwrap();
                assert!(warnings.is_empty(), "{paper:?}: {warnings:?}");
                let (expected_width, expected_height) = doc.page.size();
                let (actual_width, actual_height) = loaded.page.size();
                assert!((actual_width - expected_width).abs() < 0.1, "{paper:?}");
                assert!((actual_height - expected_height).abs() < 0.1, "{paper:?}");
                assert_eq!(loaded.page.paper, paper, "{paper:?}");
                assert_eq!(loaded.page.landscape, landscape);
                assert_eq!(loaded.page.margins, doc.page.margins);
                assert_eq!(loaded.page.line_numbers, doc.page.line_numbers);
                std::fs::remove_file(path).unwrap();
            }
        }
    }

    #[test]
    fn citations_roundtrip_in_alpha_docs_metadata() {
        let mut doc = Document::new("Citation metadata");
        doc.citations.push(crate::writing::Citation {
            author: "Ada Lovelace".into(),
            title: "Notes".into(),
            year: "1843".into(),
            publisher: "Analytical Engine".into(),
            url: "https://example.com".into(),
        });
        let path = std::env::temp_dir().join(format!("alphadocs-citations-{}.docx", doc.id));
        export(&doc, &path).unwrap();
        let (loaded, warnings) = import(&path).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(loaded.citations, doc.citations);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn xml_entities_and_empty_nodes() {
        let n = parse("<x><a v=\"a&amp;b\"/><t>A &amp; B &#233;</t></x>").unwrap();
        assert_eq!(n.descendant("a").unwrap().attr("v"), Some("a&b"));
        assert_eq!(n.descendant("t").unwrap().text, "A & B é");
    }
    #[test]
    fn docx_structured_roundtrip() {
        let mut d = Document::new("Document & title");
        let mut p = Paragraph::from_text("Hello café\t茶\nnext");
        p.set_heading(2);
        p.alignment = Alignment::Center;
        p.chars[0].style.bold = true;
        p.chars[1].style.link = Some("https://example.com/?a=1&b=2".into());
        p.chars[2].style.highlight = Some([255, 220, 0]);
        d.blocks = vec![
            Block::Paragraph(p.clone()),
            Block::Table(vec![vec![
                Paragraph::from_text("cell 1"),
                Paragraph::from_text("cell 2"),
            ]]),
            Block::PageBreak,
            Block::Paragraph(Paragraph::from_text("last")),
        ];
        let path = std::env::temp_dir().join(format!("alphadocs-{}.docx", d.id));
        export(&d, &path).unwrap();
        let (imported, warnings) = import(&path).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(imported.title, d.title);
        assert_eq!(imported.text(), d.text());
        let Block::Paragraph(q) = &imported.blocks[0] else {
            panic!()
        };
        assert_eq!(q.heading, 2);
        assert_eq!(q.alignment, p.alignment);
        assert_eq!(q.chars, p.chars);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn previous_namespace_and_standard_tab_tags_remain_readable() {
        let mut editor = crate::engine::Editor::new(Document::new("Previous interchange"));
        editor.insert("First café");
        editor.doc.blocks.push(Block::Image(EmbeddedImage {
            name: "Book".into(),
            data: include_bytes!("../../assets/alphadocs-icon.png")
                .to_vec()
                .into(),
            width: 80.0,
            height: 70.0,
            ..Default::default()
        }));
        editor.add_tab();
        editor.insert("Second 😀");
        editor.switch_tab(0);
        let path =
            std::env::temp_dir().join(format!("alphadocs-old-metadata-{}.docx", editor.doc.id));
        export(&editor.doc, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let mut source = ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut target = ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for i in 0..source.len() {
            let mut entry = source.by_index(i).unwrap();
            let name = entry.name().to_owned();
            let mut bytes = vec![];
            entry.read_to_end(&mut bytes).unwrap();
            if name == "word/document.xml" {
                let mut text = String::from_utf8(bytes).unwrap();
                for tab in &editor.doc.tabs {
                    text = text
                        .replace(&format!(" alphadocs:tabId=\"{}\"", tab.id), "")
                        .replace(&format!(" alphadocs:tabTitle=\"{}\"", tab.title), "");
                }
                text = text
                    .replace(
                        "urn:alphadocs:image:v1",
                        &format!(
                            "urn:{}:image:v1",
                            crate::legacy::ORGANISATION.to_ascii_lowercase()
                        ),
                    )
                    .replace("alphadocs", crate::legacy::EXTENSION);
                bytes = text.into_bytes();
            }
            target
                .start_file(
                    name,
                    SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .unwrap();
            target.write_all(&bytes).unwrap();
        }
        crate::persistence::atomic_write(&path, &target.finish().unwrap().into_inner()).unwrap();
        let (loaded, warnings) = import(&path).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(loaded.tabs.len(), 2);
        for i in 0..2 {
            assert_eq!(loaded.tab_blocks(i), editor.doc.tab_blocks(i));
            assert_eq!(loaded.tabs[i].id, editor.doc.tabs[i].id);
        }
        std::fs::remove_file(path).unwrap();
    }
}
