use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use unicode_segmentation::UnicodeSegmentation;
mod annotations;
mod shared;
mod text_storage;
pub use shared::SharedVec;
mod paper;
pub use paper::{LineNumbering, Paper};
pub const DOCUMENT_VERSION: u32 = 10;

#[cfg(test)]
mod storage_tests {
    use super::*;
    #[test]
    fn legacy_arrays_and_new_field_defaults_load_without_migration() {
        let json = r#"{"version":1,"id":"legacy-test","title":"Old document","starred":false,"blocks":[{"Paragraph":{"chars":[{"ch":"é","style":{}}]}}],"page":{"paper":"A4","landscape":false,"margins":[72,72,72,72],"color":[255,255,255],"page_numbers":false}}"#;
        let doc: Document = serde_json::from_str(json).unwrap();
        doc.validate().unwrap();
        assert_eq!(doc.text(), "é");
        assert_eq!(doc.page.columns, 1);
        assert_eq!(
            doc.paragraph(Address::default()).unwrap().chars[0]
                .style
                .baseline,
            Baseline::Normal
        );
    }
    #[test]
    fn legacy_image_placement_keeps_freeform_movement_with_visible_guides() {
        let placement: ImagePlacement = serde_json::from_str(
            r#"{"flow":"Wrap","offset":[24,12],"margins":[2,2,2,2],"wrap_side":"Both"}"#,
        )
        .unwrap();
        assert_eq!(placement.offset, [24.0, 12.0]);
        assert_eq!(placement.snap, ImageSnap::Freeform);
        assert!(placement.guides);
        assert!(!placement.snap_guides);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TextStyle {
    pub font: String,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub color: [u8; 3],
    pub highlight: Option<[u8; 3]>,
    pub link: Option<String>,
    pub baseline: Baseline,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub enum Baseline {
    #[default]
    Normal,
    Superscript,
    Subscript,
}
impl TextStyle {
    pub fn rendered_size(&self) -> f32 {
        self.size
            * if self.baseline == Baseline::Normal {
                1.0
            } else {
                0.7
            }
    }
}
impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font: "Noto Sans".into(),
            size: 11.0,
            bold: false,
            italic: false,
            underline: false,
            strike: false,
            color: [31, 31, 31],
            highlight: None,
            link: None,
            baseline: Baseline::Normal,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StyledChar {
    pub ch: char,
    pub style: TextStyle,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
    Justified,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub enum ListKind {
    #[default]
    None,
    Bullet,
    Number,
    Check,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Paragraph {
    #[serde(with = "text_storage")]
    pub chars: SharedVec<StyledChar>,
    pub style: TextStyle,
    pub heading: u8,
    pub alignment: Alignment,
    pub indent: f32,
    pub first_indent: f32,
    pub line_spacing: f32,
    pub before: f32,
    pub after: f32,
    pub list: ListKind,
    pub checked: bool,
    pub keep_next: bool,
    pub page_break_before: bool,
    pub keep_together: bool,
    pub widow_control: bool,
    pub annotations: SharedVec<TextAnnotation>,
}
impl Default for Paragraph {
    fn default() -> Self {
        Self {
            chars: SharedVec::default(),
            style: TextStyle::default(),
            heading: 0,
            alignment: Alignment::Left,
            indent: 0.0,
            first_indent: 0.0,
            line_spacing: 1.15,
            before: 0.0,
            after: 0.0,
            list: ListKind::None,
            checked: false,
            keep_next: false,
            page_break_before: false,
            keep_together: false,
            widow_control: true,
            annotations: Default::default(),
        }
    }
}
impl Paragraph {
    pub fn snap(&self, offset: usize) -> usize {
        let offset = offset.min(self.chars.len());
        let mut previous = 0;
        for g in self.text().graphemes(true) {
            let next = previous + g.chars().count();
            if offset == previous || offset == next {
                return offset;
            }
            if offset < next {
                return if offset - previous <= next - offset {
                    previous
                } else {
                    next
                };
            }
            previous = next;
        }
        previous
    }
    pub fn text(&self) -> String {
        self.chars.iter().map(|c| c.ch).collect()
    }
    pub fn from_text(text: &str) -> Self {
        let mut p = Self::default();
        p.chars = text
            .chars()
            .map(|ch| StyledChar {
                ch,
                style: p.style.clone(),
            })
            .collect();
        p
    }
    pub fn set_heading(&mut self, heading: u8) {
        self.heading = heading;
        self.keep_next = heading > 0;
        let size = match heading {
            1 => 24.0,
            2 => 18.0,
            3 => 14.0,
            4..=6 => 12.0,
            7 => 30.0,
            8 => 16.0,
            _ => 11.0,
        };
        self.style.size = size;
        self.style.bold = (1..=6).contains(&heading);
        self.before = if heading > 0 { 12.0 } else { 0.0 };
        for c in &mut self.chars {
            c.style.size = size;
            c.style.bold = self.style.bold;
        }
    }
    pub fn boundary(&self, index: usize, forward: bool) -> usize {
        let text = self.text();
        let mut count = 0;
        for g in text.graphemes(true) {
            let next = count + g.chars().count();
            if forward && next > index {
                return next;
            }
            if !forward && next >= index {
                return count;
            }
            count = next;
        }
        self.chars.len()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EmbeddedImage {
    #[serde(default = "new_object_id")]
    pub id: String,
    pub name: String,
    pub data: SharedVec<u8>,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub crop: Option<[f32; 4]>,
    #[serde(default)]
    pub trim_transparency: bool,
    #[serde(default)]
    pub trim_bounds: Option<[f32; 4]>,
    #[serde(default)]
    pub padding: ImagePadding,
    #[serde(default)]
    pub alt_text: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub effects: ImageEffects,
    #[serde(default)]
    pub placement: ImagePlacement,
    #[serde(default)]
    pub comments: SharedVec<ImageComment>,
    #[serde(default)]
    pub reactions: SharedVec<String>,
}
pub fn new_object_id() -> String {
    static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!(
        "image-{:x}-{:x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}
impl Default for EmbeddedImage {
    fn default() -> Self {
        Self {
            id: new_object_id(),
            name: String::new(),
            data: Default::default(),
            width: 0.0,
            height: 0.0,
            crop: None,
            trim_transparency: false,
            trim_bounds: None,
            padding: Default::default(),
            alt_text: String::new(),
            title: String::new(),
            effects: Default::default(),
            placement: Default::default(),
            comments: Default::default(),
            reactions: Default::default(),
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ImagePadding {
    pub enabled: bool,
    /// Each border is this fraction of the cropped image's shorter side.
    pub fraction: f32,
}
impl Default for ImagePadding {
    fn default() -> Self {
        Self {
            enabled: false,
            fraction: 0.02,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImageFlow {
    #[default]
    Break,
    Wrap,
    Behind,
    Front,
}
impl ImageFlow {
    pub fn label(self) -> &'static str {
        match self {
            Self::Break => "Break text",
            Self::Wrap => "Wrap text",
            Self::Behind => "Behind text",
            Self::Front => "In front of text",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum WrapSide {
    #[default]
    Both,
    Left,
    Right,
    Largest,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ImagePlacement {
    pub flow: ImageFlow,
    /// Document points, relative to the column's left edge and the block anchor.
    pub offset: [f32; 2],
    /// Top, bottom, left, right clearances in document points.
    pub margins: [f32; 4],
    pub wrap_side: WrapSide,
    pub snap: ImageSnap,
    pub guides: bool,
    pub snap_guides: bool,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImageSnap {
    #[default]
    Freeform,
    TextLines,
    NearTextLines,
}
impl Default for ImagePlacement {
    fn default() -> Self {
        Self {
            flow: ImageFlow::Break,
            offset: [0.0; 2],
            margins: [1.44; 4],
            wrap_side: WrapSide::Both,
            snap: ImageSnap::Freeform,
            guides: true,
            snap_guides: false,
        }
    }
}
impl ImagePlacement {
    pub fn valid(self) -> bool {
        self.offset
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 2400.0)
            && self
                .margins
                .iter()
                .all(|v| v.is_finite() && (0.0..=720.0).contains(v))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ImageComment {
    #[serde(default)]
    pub author: String,
    pub id: String,
    pub text: String,
    pub created: u64,
    pub resolved: bool,
    pub replies: SharedVec<String>,
    #[serde(default)]
    pub reactions: SharedVec<String>,
}
/// A range belongs to its paragraph, so block/table moves retain the anchor.
/// Segments spanning paragraphs share one thread ID and immutable thread data.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TextAnnotation {
    pub id: String,
    pub start: usize,
    pub end: usize,
    pub comments: SharedVec<ImageComment>,
    pub reactions: SharedVec<String>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ImageEffects {
    pub opacity: f32,
    pub brightness: f32,
    pub contrast: f32,
    pub grayscale: bool,
}
impl Default for ImageEffects {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            brightness: 0.0,
            contrast: 0.0,
            grayscale: false,
        }
    }
}
impl ImageEffects {
    pub fn valid(&self) -> bool {
        self.opacity.is_finite()
            && (0.0..=1.0).contains(&self.opacity)
            && self.brightness.is_finite()
            && (-1.0..=1.0).contains(&self.brightness)
            && self.contrast.is_finite()
            && (-1.0..=1.0).contains(&self.contrast)
    }
}
impl EmbeddedImage {
    pub fn visible_crop(&self) -> [f32; 4] {
        if self.trim_transparency {
            self.trim_bounds.or(self.crop)
        } else {
            self.crop
        }
        .unwrap_or([0.0, 0.0, 1.0, 1.0])
    }
    pub fn visible_size(&self) -> (f32, f32) {
        let [left, top, right, bottom] = self.visible_crop();
        let border = self.padding_points() * 2.0;
        (
            self.width * (right - left) + border,
            self.height * (bottom - top) + border,
        )
    }
    pub fn padding_points(&self) -> f32 {
        if !self.padding.enabled {
            return 0.0;
        }
        let [l, t, r, b] = self.visible_crop();
        (self.width * (r - l)).min(self.height * (b - t)) * self.padding.fraction
    }
    /// Cropped source coordinates including the transparent area outside it.
    pub fn drawing_crop(&self) -> [f32; 4] {
        let [l, t, r, b] = self.visible_crop();
        let p = self.padding_points();
        [
            l - p / self.width,
            t - p / self.height,
            r + p / self.width,
            b + p / self.height,
        ]
    }
    pub fn source_size_for_visible(&self, width: f32, height: f32) -> (f32, f32) {
        let f = if self.padding.enabled {
            self.padding.fraction
        } else {
            0.0
        };
        let p = width.min(height) * f / (1.0 + 2.0 * f);
        let [l, t, r, b] = self.visible_crop();
        ((width - 2.0 * p) / (r - l), (height - 2.0 * p) / (b - t))
    }
    /// Fraction of the padded frame occupied by the retained source pixels.
    pub fn content_inset_fraction(&self) -> [f32; 2] {
        let (w, h) = self.visible_size();
        let p = self.padding_points();
        [p / w, p / h]
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Block {
    Paragraph(Paragraph),
    Table(Vec<Vec<Paragraph>>),
    Image(EmbeddedImage),
    PageBreak,
    Rule,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PageSetup {
    pub paper: Paper,
    pub landscape: bool,
    pub margins: [f32; 4],
    pub color: [u8; 3],
    pub page_numbers: bool,
    pub columns: u8,
    pub column_gap: f32,
    pub header: Paragraph,
    pub footer: Paragraph,
    pub pageless: bool,
    pub line_numbers: LineNumbering,
}
impl Default for PageSetup {
    fn default() -> Self {
        Self {
            paper: Paper::A4,
            landscape: false,
            margins: [72.0; 4],
            color: [255; 3],
            page_numbers: false,
            columns: 1,
            column_gap: 18.0,
            header: running_paragraph(),
            footer: running_paragraph(),
            pageless: false,
            line_numbers: LineNumbering::Off,
        }
    }
}
fn running_paragraph() -> Paragraph {
    let mut p = Paragraph::default();
    p.style.size = 9.0;
    p.style.color = [95; 3];
    p.line_spacing = 1.0;
    p.after = 0.0;
    p
}
impl PageSetup {
    pub fn size(&self) -> (f32, f32) {
        let (w, h) = self.paper.size();
        if self.landscape { (h, w) } else { (w, h) }
    }
    pub fn valid(&self) -> bool {
        let (w, h) = self.size();
        w.is_finite()
            && h.is_finite()
            && (72.0..=14400.0).contains(&w)
            && (72.0..=14400.0).contains(&h)
            && self.margins.iter().all(|m| m.is_finite() && *m >= 0.0)
            && (1..=3).contains(&self.columns)
            && self.column_gap.is_finite()
            && (0.0..=72.0).contains(&self.column_gap)
            && (w
                - self.margins[2]
                - self.margins[3]
                - self.column_gap * (self.columns as f32 - 1.0))
                / self.columns as f32
                >= 36.0
            && self.margins[0] + self.margins[1] <= h - 36.0
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Document {
    pub version: u32,
    pub id: String,
    pub title: String,
    pub blocks: Vec<Block>,
    pub page: PageSetup,
    pub starred: bool,
    /// The active tab's blocks live in `blocks`; its entry has an empty payload.
    /// Inactive tab payloads share immutable storage with undo snapshots.
    #[serde(default = "default_tabs")]
    pub tabs: Vec<ContentTab>,
    #[serde(default)]
    pub active_tab: usize,
    #[serde(default)]
    pub citations: SharedVec<crate::writing::Citation>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ContentTab {
    pub id: String,
    pub title: String,
    pub blocks: SharedVec<Block>,
}
fn default_tabs() -> Vec<ContentTab> {
    vec![ContentTab {
        id: "tab-1".into(),
        title: "Tab 1".into(),
        blocks: SharedVec::default(),
    }]
}
#[derive(
    Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct Address {
    pub block: usize,
    pub row: usize,
    pub cell: usize,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub address: Address,
    pub offset: usize,
}
impl Document {
    pub fn valid_id(id: &str) -> bool {
        !id.is_empty()
            && id.len() <= 128
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    }
    pub fn new(title: &str) -> Self {
        Self {
            version: DOCUMENT_VERSION,
            id: format!(
                "{:x}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ),
            title: title.into(),
            blocks: vec![Block::Paragraph(Paragraph::default())],
            page: PageSetup::default(),
            starred: false,
            tabs: default_tabs(),
            active_tab: 0,
            citations: SharedVec::default(),
        }
    }
    pub fn tab_blocks(&self, index: usize) -> &[Block] {
        if index == self.active_tab {
            &self.blocks
        } else {
            &self.tabs[index].blocks
        }
    }
    pub(crate) fn select_tab(&mut self, index: usize) -> bool {
        if index >= self.tabs.len() || index == self.active_tab {
            return false;
        }
        self.tabs[self.active_tab].blocks = std::mem::take(&mut self.blocks).into();
        self.blocks = std::mem::take(&mut self.tabs[index].blocks).to_vec();
        self.active_tab = index;
        true
    }
    /// Interchange formats without document tabs receive every tab in order.
    pub fn flattened(&self) -> Self {
        let mut result = self.clone();
        result.page.pageless = false;
        result.blocks.clear();
        for index in 0..self.tabs.len() {
            if index > 0 {
                result.blocks.push(Block::PageBreak);
            }
            result.blocks.extend_from_slice(self.tab_blocks(index));
        }
        result.tabs = default_tabs();
        result.active_tab = 0;
        result
    }
    pub fn addresses(&self) -> Vec<Address> {
        let mut a = vec![];
        for (block, b) in self.blocks.iter().enumerate() {
            match b {
                Block::Paragraph(_) => a.push(Address {
                    block,
                    row: 0,
                    cell: 0,
                }),
                Block::Table(rows) => {
                    for (row, cells) in rows.iter().enumerate() {
                        for cell in 0..cells.len() {
                            a.push(Address { block, row, cell });
                        }
                    }
                }
                _ => {}
            }
        }
        a
    }
    pub fn paragraph(&self, a: Address) -> Option<&Paragraph> {
        if a.block == usize::MAX {
            return match a.row {
                0 => Some(&self.page.header),
                1 => Some(&self.page.footer),
                _ => None,
            };
        }
        match self.blocks.get(a.block)? {
            Block::Paragraph(p) => Some(p),
            Block::Table(rows) => rows.get(a.row)?.get(a.cell),
            _ => None,
        }
    }
    pub fn paragraph_mut(&mut self, a: Address) -> Option<&mut Paragraph> {
        if a.block == usize::MAX {
            return match a.row {
                0 => Some(&mut self.page.header),
                1 => Some(&mut self.page.footer),
                _ => None,
            };
        }
        match self.blocks.get_mut(a.block)? {
            Block::Paragraph(p) => Some(p),
            Block::Table(rows) => rows.get_mut(a.row)?.get_mut(a.cell),
            _ => None,
        }
    }
    pub fn text(&self) -> String {
        self.addresses()
            .iter()
            .filter_map(|a| self.paragraph(*a))
            .map(Paragraph::text)
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn counts(&self) -> (usize, usize, usize) {
        let t = self.text();
        (
            t.unicode_words().count(),
            t.chars().count(),
            t.chars().filter(|c| !c.is_whitespace()).count(),
        )
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.citations.len() > 1000 || self.citations.iter().any(|c| !c.valid()) {
            return Err("Invalid citation library".into());
        }
        if self.tabs.is_empty()
            || self.active_tab >= self.tabs.len()
            || !self.tabs[self.active_tab].blocks.is_empty()
        {
            return Err("Invalid document tabs".into());
        }
        let mut ids = std::collections::HashSet::new();
        for (index, tab) in self.tabs.iter().enumerate() {
            if !Self::valid_id(&tab.id) || !ids.insert(&tab.id) || tab.title.trim().is_empty() {
                return Err("Invalid document tab identifier or title".into());
            }
            if index != self.active_tab {
                let mut content = self.clone();
                content.blocks = tab.blocks.to_vec();
                content.tabs = default_tabs();
                content.active_tab = 0;
                content.validate()?;
            }
        }
        if !Self::valid_id(&self.id) {
            return Err("Document identifier is invalid".into());
        }
        if !(1..=DOCUMENT_VERSION).contains(&self.version) {
            return Err(format!(
                "Unsupported AlphaDocs document version {}",
                self.version
            ));
        }
        if !self.page.valid() {
            return Err("Invalid page dimensions or margins".into());
        }
        if self.addresses().is_empty() {
            return Err("Document has no editable paragraph".into());
        }
        let addresses = self.addresses();
        for p in addresses
            .into_iter()
            .filter_map(|a| self.paragraph(a))
            .chain([&self.page.header, &self.page.footer])
        {
            for n in &p.annotations {
                if n.id.is_empty()
                    || n.start > n.end
                    || n.end > p.chars.len()
                    || p.snap(n.start) != n.start
                    || p.snap(n.end) != n.end
                    || n.comments.iter().any(|c| {
                        c.id.is_empty()
                            || c.text.trim().is_empty()
                            || c.reactions.iter().any(|r| emojis::get(r).is_none())
                    })
                    || n.reactions.iter().any(|r| emojis::get(r).is_none())
                {
                    return Err("Invalid text annotation anchor or thread".into());
                }
            }
            if !p.line_spacing.is_finite()
                || !(0.5..=5.0).contains(&p.line_spacing)
                || !p.indent.is_finite()
                || !p.before.is_finite()
                || !p.after.is_finite()
                || !p.first_indent.is_finite()
                || p.indent < 0.0
                || p.before < 0.0
                || p.after < 0.0
            {
                return Err("Invalid paragraph measurements".into());
            }
            for s in std::iter::once(&p.style).chain(p.chars.iter().map(|c| &c.style)) {
                if !s.size.is_finite() || !(1.0..=400.0).contains(&s.size) {
                    return Err("Invalid font size".into());
                }
            }
        }
        let mut image_ids = std::collections::HashSet::new();
        for b in &self.blocks {
            if let Block::Image(i) = b {
                if i.id.is_empty() || !image_ids.insert(&i.id) {
                    return Err("Invalid or duplicate image identifier".into());
                }
                let mut comment_ids = std::collections::HashSet::new();
                if i.comments.iter().any(|c| {
                    c.id.is_empty()
                        || c.text.trim().is_empty()
                        || !comment_ids.insert(&c.id)
                        || c.reactions.iter().any(|r| emojis::get(r).is_none())
                }) {
                    return Err("Invalid image comments".into());
                }
                if i.reactions.iter().any(|r| emojis::get(r).is_none()) {
                    return Err("Invalid image reaction".into());
                }
                if !i.placement.valid() {
                    return Err("Invalid image placement".into());
                }
                if !i.effects.valid() {
                    return Err("Invalid image adjustments".into());
                }
                for bounds in [i.crop, i.trim_bounds].into_iter().flatten() {
                    if bounds
                        .iter()
                        .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
                        || bounds[0] >= bounds[2]
                        || bounds[1] >= bounds[3]
                    {
                        return Err("Invalid image crop".into());
                    }
                }
            }
            if let Block::Image(i) = b
                && (!i.width.is_finite()
                    || !i.padding.fraction.is_finite()
                    || !(0.0..=0.25).contains(&i.padding.fraction)
                    || !i.height.is_finite()
                    || i.width <= 0.0
                    || i.height <= 0.0)
            {
                return Err("Invalid image size".into());
            }
        }
        Ok(())
    }
}
