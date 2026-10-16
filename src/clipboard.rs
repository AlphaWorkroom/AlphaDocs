//! Native clipboard interchange. HTML is parsed into document objects, never rendered.
use crate::{engine::ClipboardContent, model::*};
use base64::{Engine, engine::general_purpose::STANDARD};
use clipboard_rs::{Clipboard, ClipboardContext, ContentFormat, common::RustImage};
use scraper::{ElementRef, Html, Node, Selector};

pub struct Import {
    pub content: ClipboardContent,
    pub warnings: Vec<String>,
}
const MAX_BYTES: usize = 32 * 1024 * 1024;
const NATIVE_FORMAT: &str = "AlphaDocs structured fragment v1";

pub fn box_diagram(text: &str) -> bool {
    text.chars()
        .filter(|c| matches!(*c as u32, 0x2500..=0x257f))
        .take(2)
        .count()
        == 2
}
/// Repair known arrow spellings from text producers, never unknown entities.
/// HTML itself is decoded once by the HTML parser; code elements bypass this repair.
pub fn arrow_text(text: &str) -> String {
    arrow_entities(text, &[';', '&'])
}
fn arrow_entities(text: &str, endings: &[char]) -> String {
    let mut result = text.to_owned();
    if !text.contains('&') {
        return result;
    }
    for (name, arrow) in [
        ("rightarrow", "→"),
        ("rarr", "→"),
        ("leftarrow", "←"),
        ("larr", "←"),
        ("uparrow", "↑"),
        ("uarr", "↑"),
        ("downarrow", "↓"),
        ("darr", "↓"),
        ("leftrightarrow", "↔"),
        ("harr", "↔"),
        ("Rightarrow", "⇒"),
        ("Leftarrow", "⇐"),
    ] {
        for ending in endings {
            let entity = format!("&{name}{ending}");
            if result.contains(&entity) {
                result = result.replace(&entity, arrow);
            }
        }
    }
    result
}
fn format_diagrams(blocks: &mut [Block], width: f32) {
    // Fit using actual font advances. A common point size keeps adjacent rows aligned.
    let mut factor = 1.0_f32;
    for block in blocks.iter() {
        if let Block::Paragraph(p) = block
            && box_diagram(&p.text())
        {
            for row in p.text().split('\n') {
                let measured = crate::fonts::mono_width(row, p.style.size, p.style.bold);
                factor = factor.min((width.max(1.0) - 1.0).max(1.0) / measured.max(1.0));
            }
        }
    }
    for block in blocks {
        if let Block::Paragraph(p) = block
            && box_diagram(&p.text())
        {
            p.style.font = "Noto Sans Mono".into();
            p.style.size = (p.style.size * factor).max(1.0);
            p.alignment = Alignment::Left;
            p.before = 0.0;
            p.after = 0.0;
            // Bundled Mono regular/bold: U+2502 spans 1,225 font units;
            // ascent - descent + leading is 1,362. Match adjoining box strokes.
            p.line_spacing = 1225.0 / 1362.0;
            p.first_indent = 0.0;
            for c in &mut p.chars {
                c.style.font = p.style.font.clone();
                c.style.size = p.style.size;
            }
        }
    }
}
pub fn plain_content(text: &str, style: &TextStyle, width: f32) -> ClipboardContent {
    let text = arrow_text(&text.replace("\r\n", "\n").replace('\r', "\n"));
    let mut blocks: Vec<_> = text
        .split('\n')
        .map(|line| {
            let mut p = Paragraph::from_text(line);
            p.style = style.clone();
            for c in &mut p.chars {
                c.style = style.clone();
            }
            Block::Paragraph(p)
        })
        .collect();
    format_diagrams(&mut blocks, width);
    ClipboardContent { text, blocks }
}

#[cfg(not(test))]
pub fn write(content: &ClipboardContent) -> Result<(), String> {
    let bytes = serde_json::to_vec(content).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("Clipboard fragment exceeds 32 MiB".into());
    }
    ClipboardContext::new()
        .map_err(|e| e.to_string())?
        .set(vec![
            clipboard_rs::ClipboardContent::Text(content.text.clone()),
            clipboard_rs::ClipboardContent::Other(NATIVE_FORMAT.into(), bytes),
        ])
        .map_err(|e| e.to_string())
}
#[cfg(test)]
pub fn write(_: &ClipboardContent) -> Result<(), String> {
    Err("Unit tests use the frame clipboard rather than the system clipboard".into())
}

pub enum Capture {
    Native(Vec<u8>),
    Html(String, Option<Vec<u8>>),
    Image(Vec<u8>),
    RtfFallback(String),
}
pub fn capture() -> Result<Option<Capture>, String> {
    let clipboard = ClipboardContext::new().map_err(|e| e.to_string())?;
    for format in [NATIVE_FORMAT, crate::legacy::CLIPBOARD] {
        if clipboard.has(ContentFormat::Other(format.into())) {
            let bytes = clipboard.get_buffer(format).map_err(|e| e.to_string())?;
            if bytes.len() > MAX_BYTES {
                return Err("Clipboard fragment exceeds 32 MiB".into());
            }
            return Ok(Some(Capture::Native(bytes)));
        }
    }
    let bitmap = || {
        clipboard
            .get_image()
            .ok()?
            .to_png()
            .ok()
            .map(|p| p.get_bytes().to_vec())
    };
    if clipboard.has(ContentFormat::Html) {
        return clipboard
            .get_html()
            .map(|html| Some(Capture::Html(html, bitmap())))
            .map_err(|e| e.to_string());
    }
    if clipboard.has(ContentFormat::Image) {
        return Ok(bitmap().map(Capture::Image));
    }
    if clipboard.has(ContentFormat::Rtf) {
        return clipboard
            .get_text()
            .map(|text| Some(Capture::RtfFallback(text)))
            .map_err(|e| e.to_string());
    }
    Ok(None)
}
impl Capture {
    pub fn import(self, style: &TextStyle, width: f32) -> Result<Import, String> {
        match self {
            Self::RtfFallback(text) => {
                if text.is_empty() {
                    return Err(
                        "This RTF clipboard has no supported text or image representation".into(),
                    );
                }
                Ok(Import{content:plain_content(&text, style, width),warnings:vec!["RTF-only content pasted as plain text; copy HTML to retain tables and media".into()]})
            }
            Self::Native(bytes) => {
                let content: ClipboardContent =
                    serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                let mut doc = Document::new("Clipboard validation");
                doc.blocks = content.blocks.clone();
                if !doc.blocks.iter().any(|b| matches!(b, Block::Paragraph(_))) {
                    doc.blocks.push(Block::Paragraph(Paragraph::default()));
                }
                doc.validate().map_err(|e| e.to_string())?;
                Ok(Import {
                    content,
                    warnings: vec![],
                })
            }
            Self::Html(html, bitmap) => parse_html(&html, style, width, bitmap),
            Self::Image(bytes) => Ok(Import {
                content: ClipboardContent {
                    text: String::new(),
                    blocks: vec![image_block(bytes, "Pasted image", width)?],
                },
                warnings: vec![],
            }),
        }
    }
}
fn image_block(data: Vec<u8>, name: &str, width: f32) -> Result<Block, String> {
    if data.len() > MAX_BYTES {
        return Err("Clipboard image exceeds 32 MiB".into());
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(&data))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(64 * 1024 * 1024);
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    reader.limits(limits);
    let im = reader.decode().map_err(|e| e.to_string())?;
    let w = (im.width() as f32 * 0.75).min(width.max(1.0));
    Ok(Block::Image(EmbeddedImage {
        name: name.into(),
        width: w,
        height: w * im.height() as f32 / im.width().max(1) as f32,
        data: data.into(),
        ..Default::default()
    }))
}
struct Parser {
    blocks: Vec<Block>,
    paragraph: Paragraph,
    warnings: Vec<String>,
    bitmap: Option<Vec<u8>>,
    width: f32,
    preserve: bool,
    literal: bool,
    rules: std::sync::Arc<Vec<(Selector, String)>>,
    deadline: std::time::Instant,
    requests: usize,
    bytes: usize,
}
impl Parser {
    fn flush(&mut self, empty: bool) {
        if empty || !self.paragraph.chars.is_empty() {
            self.blocks
                .push(Block::Paragraph(std::mem::take(&mut self.paragraph)));
        }
    }
    fn walk(&mut self, node: ego_tree::NodeRef<'_, Node>, style: &TextStyle, depth: usize) {
        if depth > 128 {
            self.warnings
                .push("Deeply nested content was truncated".into());
            return;
        }
        if let Node::Text(text) = node.value() {
            let text = if self.literal {
                text.to_string()
            } else {
                arrow_entities(text, &['&'])
            };
            let preserve = self.preserve || box_diagram(&text);
            for ch in text.chars() {
                let ch = if !preserve && ch.is_whitespace() && ch != '\u{a0}' {
                    ' '
                } else {
                    ch
                };
                if !preserve
                    && ch == ' '
                    && self
                        .paragraph
                        .chars
                        .last()
                        .is_none_or(|c| c.ch.is_whitespace())
                {
                    continue;
                }
                self.paragraph.chars.push(StyledChar {
                    ch,
                    style: style.clone(),
                });
            }
            return;
        }
        let Some(el) = ElementRef::wrap(node) else {
            return;
        };
        let name = el.value().name();
        if matches!(name, "script" | "style" | "head" | "meta" | "link") {
            return;
        }
        let mut style = style.clone();
        if matches!(name, "pre" | "code") {
            style.font = "Noto Sans Mono".into();
        }
        if let Some(h) = name
            .strip_prefix('h')
            .and_then(|v| v.parse::<u8>().ok())
            .filter(|h| (1..=6).contains(h))
        {
            style.size = [24.0, 18.0, 14.0, 12.0, 11.0, 11.0][usize::from(h - 1)];
            style.bold = true;
        }
        style.bold |= matches!(name, "b" | "strong" | "th");
        style.italic |= matches!(name, "i" | "em");
        style.underline |= name == "u";
        style.strike |= matches!(name, "s" | "del" | "strike");
        if name == "a" {
            style.link = el
                .attr("href")
                .filter(|s| {
                    s.starts_with("https://")
                        || s.starts_with("http://")
                        || s.starts_with("mailto:")
                })
                .map(str::to_owned);
        }
        let declarations = self
            .rules
            .iter()
            .filter(|(selector, _)| selector.matches(&el))
            .map(|(_, css)| css.as_str())
            .chain(el.attr("style"))
            .collect::<Vec<_>>()
            .join(";");
        for entry in declarations.split(';') {
            if let Some((key, value)) = entry.split_once(':') {
                let value = value.trim();
                match key.trim() {
                    "font-weight" => {
                        style.bold = value == "bold"
                            || value == "bolder"
                            || value.parse::<u16>().is_ok_and(|n| n >= 600)
                    }
                    "font-style" => style.italic = value == "italic" || value == "oblique",
                    "text-decoration" | "text-decoration-line" => {
                        style.underline = value.contains("underline");
                        style.strike = value.contains("line-through");
                    }
                    "font-family" => {
                        style.font = value
                            .split(',')
                            .next()
                            .unwrap_or(value)
                            .trim_matches(['\'', '"', ' '])
                            .into()
                    }
                    "font-size" => {
                        if let Some(size) = points(value, style.size) {
                            style.size = size.clamp(1.0, 400.0);
                        }
                    }
                    "color" => {
                        if let Some(c) = colour(value) {
                            style.color = c
                        }
                    }
                    "background-color" => style.highlight = colour(value),
                    _ => {}
                }
            }
        }
        if matches!(name, "img" | "v:imagedata") {
            self.flush(false);
            let source = el.attr("src").unwrap_or("");
            let data = if source.starts_with("data:image/") {
                source.split_once(',').and_then(|(head, body)| {
                    head.ends_with(";base64")
                        .then(|| STANDARD.decode(body).ok())
                        .flatten()
                })
            } else if source.starts_with("file://") {
                // Explicit local image references supplied by the clipboard owner.
                let path = source
                    .trim_start_matches("file://")
                    .trim_start_matches('/')
                    .replace("%20", " ");
                std::fs::metadata(&path)
                    .ok()
                    .filter(|m| m.len() <= MAX_BYTES as u64)
                    .and_then(|_| std::fs::read(path).ok())
            } else if source.starts_with("https://")
                && self.requests < 16
                && std::time::Instant::now() < self.deadline
            {
                self.requests += 1;
                fetch_image(source, self.deadline).ok()
            } else {
                None
            }
            .or_else(|| self.bitmap.take());
            let data = data.filter(|d| {
                self.bytes = self.bytes.saturating_add(d.len());
                self.bytes <= MAX_BYTES
            });
            match data.and_then(|d| {
                image_block(d, el.attr("alt").unwrap_or("Pasted image"), self.width).ok()
            }) {
                Some(mut block) => {
                    if let Block::Image(im) = &mut block {
                        let w = el
                            .attr("width")
                            .and_then(|s| s.parse::<f32>().ok())
                            .map(|w| w * 0.75);
                        if let Some(w) = w.filter(|w| w.is_finite() && *w > 0.0) {
                            let w = w.min(self.width);
                            im.height *= w / im.width;
                            im.width = w;
                        }
                    }
                    self.blocks.push(block);
                }
                None => {
                    self.warnings.push("An image could not be embedded; copy it as an image or use a local/embedded image source".into());
                    self.blocks.push(Block::Paragraph(Paragraph::from_text(
                        el.attr("alt").unwrap_or("[Image unavailable]"),
                    )));
                }
            }
            return;
        }
        if name == "table" {
            self.flush(false);
            let row_selector = Selector::parse("tr").expect("Static selector");
            let mut rows = vec![];
            for row in el.select(&row_selector).take(100) {
                let mut cells = vec![];
                for cell in row
                    .children()
                    .filter_map(ElementRef::wrap)
                    .filter(|c| matches!(c.value().name(), "td" | "th"))
                    .take(32)
                {
                    let mut parser = Parser {
                        blocks: vec![],
                        paragraph: Paragraph::default(),
                        warnings: vec![],
                        bitmap: None,
                        width: self.width,
                        preserve: self.preserve,
                        literal: self.literal,
                        rules: self.rules.clone(),
                        deadline: self.deadline,
                        requests: self.requests,
                        bytes: self.bytes,
                    };
                    parser.walk(*cell, &style, depth + 1);
                    parser.flush(false);
                    let mut paragraph = Paragraph::default();
                    for block in parser.blocks {
                        if let Block::Paragraph(p) = block {
                            if !paragraph.chars.is_empty() {
                                paragraph.chars.push(StyledChar {
                                    ch: '\n',
                                    style: style.clone(),
                                });
                            }
                            paragraph.append_text(p);
                        } else {
                            self.blocks.push(block);
                            self.warnings.push(
                                "An image inside a table was retained before the table".into(),
                            );
                        }
                    }
                    self.requests = parser.requests;
                    self.bytes = parser.bytes;
                    self.warnings.extend(parser.warnings);
                    cells.push(paragraph);
                }
                if !cells.is_empty() {
                    rows.push(cells);
                }
            }
            let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
            for row in &mut rows {
                row.resize(columns, Paragraph::default());
            }
            if !rows.is_empty() {
                self.blocks.push(Block::Table(rows));
            }
            if el
                .select(&Selector::parse("[rowspan], [colspan]").expect("Static selector"))
                .next()
                .is_some()
            {
                self.warnings
                    .push("Merged table cells were flattened".into());
            }
            return;
        }
        if matches!(name, "svg" | "canvas" | "v:shape" | "object") {
            self.flush(false);
            let drawing = if name == "svg" {
                raster_svg(&el.html()).ok()
            } else {
                self.bitmap.take()
            };
            if let Some(bytes) =
                drawing.and_then(|d| image_block(d, "Pasted drawing", self.width).ok())
            {
                self.blocks.push(bytes);
                self.warnings.push("Drawing pasted as an image".into());
            } else {
                self.warnings
                    .push("Editable drawings are not supported by this clipboard format".into());
            }
            return;
        }
        if name == "br" {
            self.paragraph.chars.push(StyledChar { ch: '\n', style });
            return;
        }
        if name == "hr" {
            self.flush(false);
            self.blocks.push(Block::Rule);
            return;
        }
        let block = matches!(
            name,
            "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" | "pre" | "blockquote"
        );
        if block {
            self.flush(false);
            self.paragraph.style = style.clone();
        }
        if name == "li" {
            self.paragraph.list = if el
                .parent()
                .and_then(ElementRef::wrap)
                .is_some_and(|p| p.value().name() == "ol")
            {
                ListKind::Number
            } else {
                ListKind::Bullet
            };
        }
        if name.starts_with('h')
            && name.len() == 2
            && let Ok(h) = name[1..].parse::<u8>()
        {
            self.paragraph.heading = h;
        }
        let preserve = self.preserve;
        let literal = self.literal;
        self.literal |= name == "code";
        self.preserve |= name == "pre";
        for entry in declarations.split(';') {
            if let Some((key, value)) = entry.split_once(':') {
                let value = value.trim();
                match key.trim() {
                    "white-space" => {
                        self.preserve = matches!(value, "pre" | "pre-wrap" | "break-spaces")
                    }
                    "text-align" if block => {
                        self.paragraph.alignment = match value {
                            "center" => Alignment::Center,
                            "right" | "end" => Alignment::Right,
                            "justify" => Alignment::Justified,
                            _ => Alignment::Left,
                        }
                    }
                    "margin-top" if block => {
                        if let Some(v) = points(value, style.size) {
                            self.paragraph.before = v.max(0.0);
                        }
                    }
                    "margin-bottom" if block => {
                        if let Some(v) = points(value, style.size) {
                            self.paragraph.after = v.max(0.0);
                        }
                    }
                    "text-indent" if block => {
                        if let Some(v) = points(value, style.size) {
                            self.paragraph.first_indent = v;
                        }
                    }
                    "line-height" if block => {
                        if let Some(v) = value
                            .parse::<f32>()
                            .ok()
                            .or_else(|| {
                                value
                                    .strip_suffix('%')
                                    .and_then(|v| v.parse::<f32>().ok())
                                    .map(|n| n / 100.0)
                            })
                            .or_else(|| points(value, style.size).map(|v| v / style.size))
                        {
                            self.paragraph.line_spacing = v.clamp(0.5, 5.0);
                        }
                    }
                    _ => {}
                }
            }
        }
        for child in node.children() {
            self.walk(child, &style, depth + 1);
        }
        self.preserve = preserve;
        self.literal = literal;
        if block {
            self.flush(name == "p");
        }
    }
}
fn points(value: &str, inherited: f32) -> Option<f32> {
    let (number, factor) = if let Some(v) = value.strip_suffix("pt") {
        (v, 1.0)
    } else if let Some(v) = value.strip_suffix("px") {
        (v, 0.75)
    } else if let Some(v) = value.strip_suffix("em") {
        (v, inherited)
    } else if let Some(v) = value.strip_suffix('%') {
        (v, inherited / 100.0)
    } else {
        (value, 0.75)
    };
    number
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|n| n.is_finite())
        .map(|n| n * factor)
}
fn colour(value: &str) -> Option<[u8; 3]> {
    if let Some(v) = value.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let components = v
            .split(',')
            .map(|s| s.trim().parse::<u8>())
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        return (components.len() == 3).then(|| [components[0], components[1], components[2]]);
    }
    let v = value.strip_prefix('#')?;
    if v.len() != 6 || !v.is_ascii() {
        return None;
    }
    Some([
        u8::from_str_radix(&v[0..2], 16).ok()?,
        u8::from_str_radix(&v[2..4], 16).ok()?,
        u8::from_str_radix(&v[4..6], 16).ok()?,
    ])
}
pub fn parse_html(
    html: &str,
    style: &TextStyle,
    width: f32,
    bitmap: Option<Vec<u8>>,
) -> Result<Import, String> {
    if html.len() > MAX_BYTES {
        return Err("Clipboard exceeds 32 MiB".into());
    }
    let source = Html::parse_fragment(html);
    let mut rules = vec![];
    for tag in source.select(&Selector::parse("style").expect("Static selector")) {
        let css = tag.text().collect::<String>();
        for rule in css.split('}').take(2048) {
            if let Some((selector, body)) = rule.split_once('{') {
                for selector in selector.split(',') {
                    if let Ok(parsed) = Selector::parse(selector.trim()) {
                        rules.push((parsed, body.to_owned()));
                    }
                }
            }
        }
    }
    let rules = std::sync::Arc::new(rules);
    let html = if let Some((_, fragment)) = html.split_once("<!--StartFragment-->") {
        fragment
            .split("<!--EndFragment-->")
            .next()
            .unwrap_or(fragment)
    } else {
        html
    };
    let tree = Html::parse_fragment(html);
    let mut parser = Parser {
        blocks: vec![],
        paragraph: Paragraph::default(),
        warnings: vec![],
        bitmap,
        width,
        preserve: false,
        literal: false,
        rules,
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(8),
        requests: 0,
        bytes: 0,
    };
    // Rich representations carry their own formatting. The destination's bold/heading
    // typing state is only a fallback for plain text, never a cascade root.
    let _ = style;
    parser.walk(*tree.root_element(), &TextStyle::default(), 0);
    parser.flush(false);
    format_diagrams(&mut parser.blocks, width);
    let text = parser
        .blocks
        .iter()
        .filter_map(|b| {
            if let Block::Paragraph(p) = b {
                Some(p.text())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    parser.warnings.sort();
    parser.warnings.dedup();
    Ok(Import {
        content: ClipboardContent {
            text,
            blocks: parser.blocks,
        },
        warnings: parser.warnings,
    })
}

fn fetch_image(source: &str, deadline: std::time::Instant) -> Result<Vec<u8>, String> {
    let config = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(3)
        .timeout_global(Some(
            deadline
                .saturating_duration_since(std::time::Instant::now())
                .min(std::time::Duration::from_secs(3)),
        ))
        .build();
    let agent: ureq::Agent = config.into();
    agent
        .get(source)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .with_config()
        .limit(8 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| e.to_string())
}
fn raster_svg(source: &str) -> Result<Vec<u8>, String> {
    let mut options = resvg::usvg::Options::default();
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    options.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_str(source, &options).map_err(|e| e.to_string())?;
    let size = tree.size().to_int_size();
    if size.width() > 8192
        || size.height() > 8192
        || u64::from(size.width()) * u64::from(size.height()) > 16_000_000
    {
        return Err("Drawing is too large".into());
    }
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("Invalid drawing size")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Editor;
    #[test]
    fn diagrams_keep_spaces_use_measured_monospace_and_fit_the_page() {
        let diagram = format!(
            "┌{}┐\n│{:^72}│\n└{}┘",
            "─".repeat(72),
            "Pre-Launch → Post-Launch",
            "─".repeat(72)
        );
        let content = plain_content(&diagram, &TextStyle::default(), 451.0);
        assert_eq!(content.text, diagram);
        let mut doc = Document::new("Diagram");
        doc.blocks = content.blocks;
        let ctx = eframe::egui::Context::default();
        crate::fonts::install(&ctx);
        let mut out = ctx.run_ui(Default::default(), |_| {
            let mut widths = vec![];
            for block in &doc.blocks {
                let Block::Paragraph(p) = block else { panic!() };
                assert_eq!(p.style.font, "Noto Sans Mono");
                assert_eq!(p.after, 0.0);
                let galley = crate::layout::shape(&ctx, p, 451.0);
                assert_eq!(galley.rows.len(), 1);
                assert!(galley.size().x <= 451.0);
                widths.push(galley.size().x);
            }
            assert!(widths.iter().all(|w| (w - widths[0]).abs() < 0.1));
            let layout = crate::layout::Layout::measure(&ctx, &doc);
            let bytes = crate::compat::pdf::encode(&doc, &layout).unwrap();
            assert!(bytes.starts_with(b"%PDF"));
        });
        out.textures_delta.clear();
        let html = format!(
            "<pre>{diagram}</pre><p>&rightarrow; &rightarrow&amp; &amp;rightarrow;</p><code>&amp;rightarrow&amp;</code>"
        );
        let imported = parse_html(&html, &TextStyle::default(), 451.0, None).unwrap();
        assert!(imported.content.text.starts_with(&diagram));
        assert!(
            imported.content.text.contains("→ → &rightarrow;"),
            "{}",
            imported.content.text
        );
        assert!(imported.content.text.ends_with("&rightarrow&"));
        assert_eq!(
            arrow_text("&rightarrow& &leftarrow; &unknown;"),
            "→ ← &unknown;"
        );
    }
    fn fixture() -> Import {
        let png = include_bytes!("../assets/alphadocs-icon.png");
        let html = format!(
            r#"<p style="font-family:Arial;font-size:11pt">Hello <b>bold 😀</b></p><img alt="Book" src="data:image/png;base64,{}" width="80"><table><tr><td>One</td><td><i>Two</i></td></tr></table><svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="blue"/></svg><p>End</p>"#,
            STANDARD.encode(png)
        );
        parse_html(&html, &TextStyle::default(), 450.0, None).unwrap()
    }
    #[test]
    fn google_docs_overrides_destination_and_wrapper_styles_preserving_structure() {
        let html = r#"<style>.body {font-family: Arial; font-size:11pt; font-weight:400; color:rgb(20,30,40)}</style><!--StartFragment--><b id="docs-internal-guid" style="font-weight:normal"><h2 style="font-size:16pt;margin-top:12pt;margin-bottom:6pt">Chapter</h2><p class="body" style="white-space: pre-wrap;line-height:1.15">One  <span style="font-weight:700">bold <span style="font-weight:400;font-style:normal">normal</span></span><br>next <a href="https://example.com">link</a> end</p><p class="body">Second</p><p></p></b><!--EndFragment-->"#;
        let destination = TextStyle {
            bold: true,
            italic: true,
            size: 30.0,
            ..Default::default()
        };
        let import = parse_html(html, &destination, 450.0, None).unwrap();
        assert_eq!(import.content.blocks.len(), 4);
        let p = |i| {
            let Block::Paragraph(p) = &import.content.blocks[i] else {
                panic!()
            };
            p
        };
        assert_eq!(
            (p(0).heading, p(0).style.size, p(0).before, p(0).after),
            (2, 16.0, 12.0, 6.0)
        );
        assert_eq!(p(1).text(), "One  bold normal\nnext link end");
        assert_eq!(p(1).chars[0].style.font, "Arial");
        assert_eq!(p(1).chars[0].style.size, 11.0);
        assert_eq!(p(1).chars[0].style.color, [20, 30, 40]);
        assert!(!p(1).chars[0].style.bold);
        assert!(!p(1).chars[0].style.italic);
        assert!(p(1).chars[5].style.bold);
        assert!(!p(1).chars[10].style.bold);
        assert!(p(1).chars[22].style.link.is_some());
        assert!(p(1).chars.last().unwrap().style.link.is_none());
        assert_eq!(p(1).line_spacing, 1.15);
        assert!(!p(2).chars[0].style.bold);
        assert!(p(3).chars.is_empty());
    }
    #[test]
    fn mixed_html_preserves_order_styles_table_image_drawing_and_atomic_select_all_undo() {
        let import = fixture();
        assert!(matches!(
            import.content.blocks.as_slice(),
            [
                Block::Paragraph(_),
                Block::Image(_),
                Block::Table(_),
                Block::Image(_),
                Block::Paragraph(_)
            ]
        ));
        let Block::Paragraph(p) = &import.content.blocks[0] else {
            panic!()
        };
        assert_eq!(p.text(), "Hello bold 😀");
        assert!(p.chars[6].style.bold);
        assert_eq!(p.chars[0].style.font, "Arial");
        assert_eq!(import.warnings, ["Drawing pasted as an image"]);
        for ends_in_table in [false, true] {
            let mut doc = Document::new("Before");
            doc.blocks = vec![
                Block::Paragraph(Paragraph::from_text("Old text")),
                Block::Image(EmbeddedImage::default()),
            ];
            if ends_in_table {
                doc.blocks
                    .push(Block::Table(vec![vec![Paragraph::from_text("Old cell")]]));
            } else {
                doc.blocks
                    .push(Block::Paragraph(Paragraph::from_text("Old end")));
            }
            let original = doc.clone();
            let mut editor = Editor::new(doc);
            editor.select_all();
            editor.paste_fragment(&import.content);
            assert_eq!(editor.doc.blocks.len(), 5);
            assert!(!editor.doc.text().contains("Old"));
            assert_eq!(
                editor.doc.paragraph(editor.caret.address).unwrap().text(),
                "End"
            );
            editor.undo();
            assert_eq!(editor.doc, original);
            assert!(!editor.can_undo());
            editor.redo();
            assert_eq!(editor.doc.blocks.len(), 5);
            let roundtrip: Document =
                serde_json::from_slice(&serde_json::to_vec(&editor.doc).unwrap()).unwrap();
            assert_eq!(roundtrip, editor.doc);
        }
    }
    #[test]
    fn malformed_or_unavailable_images_report_and_keep_text_no_execution() {
        let result=parse_html(r#"<script>bad()</script><p>Before</p><img src="javascript:bad()" alt="Missing"><p style="color:#ééé">After</p>"#,&TextStyle::default(),450.0,None).unwrap();
        assert!(!result.content.text.contains("bad()"));
        assert!(result.content.text.contains("Before"));
        assert!(result.content.text.contains("After"));
        assert_eq!(result.warnings.len(), 1);
        let result =
            parse_html("<p>😀 &amp; café</p>", &TextStyle::default(), 450.0, None).unwrap();
        assert_eq!(result.content.text, "😀 & café");
    }
    #[test]
    fn replacing_selection_keeps_tab_content_and_embedded_source_bytes() {
        let mut editor = Editor::new(Document::new("Tabs"));
        editor.insert("First tab");
        editor.add_tab();
        editor.insert("Replace me");
        editor.select_all();
        editor.paste_fragment(&fixture().content);
        let Block::Image(image) = &editor.doc.blocks[1] else {
            panic!()
        };
        assert_eq!(
            image.data.as_slice(),
            include_bytes!("../assets/alphadocs-icon.png")
        );
        assert!(editor.switch_tab(0));
        assert_eq!(editor.doc.text(), "First tab");
    }
}
