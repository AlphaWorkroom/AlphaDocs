//! A deliberate OOXML subset. Unsupported content produces import warnings.
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
                    ..Default::default()
                };
                for a in e.attributes().flatten() {
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
            Event::Eof => break,
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
    let mut x = format!(
        "<w:rPr><w:rFonts w:ascii=\"{}\" w:hAnsi=\"{}\"/><w:sz w:val=\"{}\"/><w:color w:val=\"{}\"/>",
        esc(&s.font),
        esc(&s.font),
        (s.size * 2.0).round() as i32,
        rgb(s.color)
    );
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
    if s.bold {
        x.push_str("<w:b/>");
    }
    if s.italic {
        x.push_str("<w:i/>");
    }
    if s.underline {
        x.push_str("<w:u w:val=\"single\"/>");
    }
    if s.strike {
        x.push_str("<w:strike/>");
    }
    if let Some(c) = s.highlight {
        x.push_str(&format!("<w:shd w:fill=\"{}\"/>", rgb(c)));
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
        x.push_str(&format!("<w:pStyle w:val=\"{sty}\"/><w:jc w:val=\"{}\"/><w:ind w:left=\"{}\" w:firstLine=\"{}\"/><w:spacing w:before=\"{}\" w:after=\"{}\" w:line=\"{}\" w:lineRule=\"auto\"/>",match p.alignment{Alignment::Left=>"left",Alignment::Center=>"center",Alignment::Right=>"right",Alignment::Justified=>"both"},twips(p.indent),twips(p.first_indent),twips(p.before),twips(p.after),(p.line_spacing*240.0).round()as i32));
        if p.page_break_before {
            x.push_str("<w:pageBreakBefore/>");
        }
        if p.keep_together {
            x.push_str("<w:keepLines/>");
        }
        x.push_str(if p.widow_control {
            "<w:widowControl/>"
        } else {
            "<w:widowControl w:val=\"0\"/>"
        });
        if p.keep_next {
            x.push_str("<w:keepNext/>");
        }
        if matches!(p.list, ListKind::Bullet | ListKind::Number) {
            x.push_str(&format!(
                "<w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"{}\"/></w:numPr>",
                if p.list == ListKind::Number { 2 } else { 1 }
            ));
        }
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
        x.push_str("</w:p>");
        x
    }
    fn image(&mut self, i: &EmbeddedImage) -> Result<String, String> {
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
            match b{Block::Paragraph(p)=>body.push_str(&writer.paragraph(p)),Block::Table(rows)=>{body.push_str("<w:tbl><w:tblPr><w:tblW w:w=\"0\" w:type=\"auto\"/><w:tblBorders>");for side in ["top","left","bottom","right","insideH","insideV"]{body.push_str(&format!("<w:{side} w:val=\"single\" w:sz=\"4\" w:color=\"808080\"/>"));}body.push_str("</w:tblBorders></w:tblPr>");for row in rows{body.push_str("<w:tr>");for cell in row{body.push_str("<w:tc>");body.push_str(&writer.paragraph(cell));body.push_str("</w:tc>");}body.push_str("</w:tr>");}body.push_str("</w:tbl>");},Block::Image(i)=>body.push_str(&writer.image(i)?),Block::PageBreak=>body.push_str("<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>"),Block::Rule=>body.push_str("<w:p><w:pPr><w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\"/></w:pBdr></w:pPr></w:p>")}
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
            "<w:sectPr>",
            &format!("<w:sectPr><w:footerReference w:type=\"default\" r:id=\"{id}\"/>"),
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
            "</w:sectPr>",
            &format!(
                "<w:lnNumType w:countBy=\"1\" w:distance=\"120\" w:restart=\"{}\"/></w:sectPr>",
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
    let rels=format!("<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{}</Relationships>",writer.rels.iter().map(|(id,typ,target)|format!("<Relationship Id=\"{id}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/{typ}\" Target=\"{}\"{}/>",esc(target),if typ=="hyperlink"{" TargetMode=\"External\""}else{""})).collect::<String>());
    let mut zip = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let styles=format!("<w:styles xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:style w:type=\"paragraph\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/></w:style>{}</w:styles>",(1..=6).map(|h|format!("<w:style w:type=\"paragraph\" w:styleId=\"Heading{h}\"><w:name w:val=\"heading {h}\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:outlineLvl w:val=\"{}\"/></w:pPr></w:style>",h-1)).collect::<String>());
    let numbering = "<w:numbering xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:abstractNum w:abstractNumId=\"1\"><w:lvl w:ilvl=\"0\"><w:start w:val=\"1\"/><w:numFmt w:val=\"bullet\"/><w:lvlText w:val=\"•\"/></w:lvl></w:abstractNum><w:abstractNum w:abstractNumId=\"2\"><w:lvl w:ilvl=\"0\"><w:start w:val=\"1\"/><w:numFmt w:val=\"decimal\"/><w:lvlText w:val=\"%1.\"/></w:lvl></w:abstractNum><w:num w:numId=\"1\"><w:abstractNumId w:val=\"1\"/></w:num><w:num w:numId=\"2\"><w:abstractNumId w:val=\"2\"/></w:num></w:numbering>";
    let types = "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Default Extension=\"png\" ContentType=\"image/png\"/><Default Extension=\"jpeg\" ContentType=\"image/jpeg\"/><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/><Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/><Override PartName=\"/word/numbering.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml\"/><Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/></Types>";
    let root_rels = "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/></Relationships>";
    let types = types.replace("</Types>", "<Override PartName=\"/word/footer1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml\"/></Types>");
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
    doc.blocks.clear();
    let mut tab_started = false;
    let mut nodes = Vec::new();
    for child in &body.children {
        if child.name == "sdt" && child.child("sdtContent").is_some() {
            if child.attr("tabId").is_some()
                || child
                    .descendant("tag")
                    .and_then(Node::val)
                    .is_some_and(|v| {
                        v.starts_with("alphadocs-tab:") || v.starts_with(crate::legacy::TAB_TAG)
                    })
            {
                nodes.push(child);
            }
            nodes.extend(&child.child("sdtContent").unwrap().children);
        } else {
            nodes.push(child);
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
                if has_text || n.descendant("drawing").is_none() && n.descendant("br").is_none() {
                    if n.descendant("pBdr").is_some() {
                        doc.blocks.push(Block::Rule);
                    } else {
                        doc.blocks.push(Block::Paragraph(p));
                    }
                }
                if let Some(br) = n.descendant("br")
                    && br.attr("type") == Some("page")
                {
                    doc.blocks.push(Block::PageBreak);
                }
                if let Some(drawing) = n.descendant("drawing")
                    && let Some(id) = drawing.descendant("blip").and_then(|n| n.attr("embed"))
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
                                            && (e.brightness - standard.brightness).abs() <= 0.00001
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
                }
            }
            "tbl" => {
                let rows = n
                    .children
                    .iter()
                    .filter(|n| n.name == "tr")
                    .map(|r| {
                        r.children
                            .iter()
                            .filter(|n| n.name == "tc")
                            .map(|c| {
                                let mut paragraphs = c
                                    .children
                                    .iter()
                                    .filter(|n| n.name == "p")
                                    .map(|p| paragraph(p, &rels, &styles, &numbering));
                                let mut p = paragraphs.next().unwrap_or_default();
                                for more in paragraphs {
                                    p.chars.push(StyledChar {
                                        ch: '\n',
                                        style: p.style.clone(),
                                    });
                                    p.chars.extend(more.chars);
                                }
                                p
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
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
