use crate::model::{Document, TextStyle};
use eframe::egui::{Context, FontData, FontDefinitions, FontFamily};
use fontdb::{Database, Family, Query, Style, Weight};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
};

pub const FONTS: &[(&str, &[u8])] = &[
    ("Emoji", include_bytes!("../assets/fonts/NotoEmoji.ttf")),
    ("Math", include_bytes!("../assets/fonts/NotoSansMath.ttf")),
    (
        "Mono",
        include_bytes!("../assets/fonts/NotoSansMono-Regular.ttf"),
    ),
    (
        "Mono-Bold",
        include_bytes!("../assets/fonts/NotoSansMono-Bold.ttf"),
    ),
    (
        "Sans",
        include_bytes!("../assets/fonts/NotoSans-Regular.ttf"),
    ),
    (
        "Sans-Bold",
        include_bytes!("../assets/fonts/NotoSans-Bold.ttf"),
    ),
    (
        "Sans-Italic",
        include_bytes!("../assets/fonts/NotoSans-Italic.ttf"),
    ),
    (
        "Sans-BoldItalic",
        include_bytes!("../assets/fonts/NotoSans-BoldItalic.ttf"),
    ),
    (
        "Serif",
        include_bytes!("../assets/fonts/NotoSerif-Regular.ttf"),
    ),
    (
        "Serif-Bold",
        include_bytes!("../assets/fonts/NotoSerif-Bold.ttf"),
    ),
    (
        "Serif-Italic",
        include_bytes!("../assets/fonts/NotoSerif-Italic.ttf"),
    ),
    (
        "Serif-BoldItalic",
        include_bytes!("../assets/fonts/NotoSerif-BoldItalic.ttf"),
    ),
];
fn bundled_key(s: &TextStyle) -> String {
    if s.font == "Noto Sans Mono" {
        return if s.bold { "Mono-Bold" } else { "Mono" }.into();
    }
    format!(
        "{}{}",
        if s.font == "Noto Serif" {
            "Serif"
        } else {
            "Sans"
        },
        match (s.bold, s.italic) {
            (true, true) => "-BoldItalic",
            (true, false) => "-Bold",
            (false, true) => "-Italic",
            _ => "",
        }
    )
}
struct Catalog {
    db: Database,
    families: Vec<String>,
    loaded: Mutex<HashMap<fontdb::ID, Arc<FontData>>>,
    resolved: Mutex<HashMap<(String, bool, bool), Option<fontdb::ID>>>,
}
fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut db = Database::new();
        for (_, bytes) in FONTS {
            db.load_font_data(bytes.to_vec());
        }
        db.load_system_fonts();
        let families: BTreeSet<_> = db
            .faces()
            .filter_map(|face| {
                face.families
                    .first()
                    .filter(|f| !f.0.trim().is_empty())
                    .map(|f| f.0.clone())
            })
            .collect();
        let mut families: Vec<_> = families.into_iter().collect();
        families.sort_by_key(|name| name.to_lowercase());
        Catalog {
            db,
            families,
            loaded: Mutex::new(HashMap::new()),
            resolved: Mutex::new(HashMap::new()),
        }
    })
}
pub fn families() -> &'static [String] {
    &catalog().families
}
pub fn available(name: &str) -> bool {
    catalog()
        .families
        .iter()
        .any(|n| n.eq_ignore_ascii_case(name))
}
pub fn preferred_body_family() -> &'static str {
    ["Arial", "Arimo", "Liberation Sans", "Noto Sans"]
        .into_iter()
        .find(|name| available(name))
        .unwrap_or("Noto Sans")
}
fn query(s: &TextStyle) -> Option<fontdb::ID> {
    let mut resolved = catalog().resolved.lock().unwrap_or_else(|e| e.into_inner());
    *resolved
        .entry((s.font.clone(), s.bold, s.italic))
        .or_insert_with(|| {
            catalog().db.query(&Query {
                families: &[Family::Name(&s.font)],
                weight: if s.bold { Weight::BOLD } else { Weight::NORMAL },
                style: if s.italic {
                    Style::Italic
                } else {
                    Style::Normal
                },
                ..Default::default()
            })
        })
}
pub fn key(s: &TextStyle) -> String {
    if s.font == "Noto Sans Math" {
        return "Math".into();
    }
    if s.font == "Noto Emoji" {
        return "Emoji".into();
    }
    if matches!(
        s.font.as_str(),
        "Noto Sans" | "Noto Serif" | "Noto Sans Mono"
    ) {
        return bundled_key(s);
    }
    query(s)
        .map(|id| format!("System-{id:?}"))
        .unwrap_or_else(|| bundled_key(s))
}
/// Match the renderer's bundled symbol fallback when embedding PDF glyphs.
/// Cache coverage probes, rather than shaping every symbol on every repaint.
pub fn symbol_key(s: &TextStyle, ch: char) -> String {
    let primary = key(s);
    if !matches!(ch as u32, 0x2190..=0x21ff | 0x2500..=0x259f | 0x27f0..=0x27ff | 0x2900..=0x297f) {
        return primary;
    }
    static COVERAGE: OnceLock<Mutex<HashMap<(String, char), String>>> = OnceLock::new();
    let mut coverage = COVERAGE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    coverage
        .entry((primary.clone(), ch))
        .or_insert_with(|| {
            if has_glyph(&face(s), ch) {
                return primary;
            }
            let mono = if s.bold { "Mono-Bold" } else { "Mono" };
            if FONTS
                .iter()
                .find(|(name, _)| *name == mono)
                .is_some_and(|(_, bytes)| has_glyph(&FontData::from_static(bytes), ch))
            {
                return mono.into();
            }
            if FONTS
                .iter()
                .find(|(name, _)| *name == "Math")
                .is_some_and(|(_, bytes)| has_glyph(&FontData::from_static(bytes), ch))
            {
                return "Math".into();
            }
            primary
        })
        .clone()
}
/// Resolve a complete emoji grapheme so ZWJ and modifier sequences stay in one
/// shaping run. The ordinary body face often has no glyph for these sequences.
pub fn emoji_fallback_key(s: &TextStyle, text: &str) -> Option<&'static str> {
    if emojis::get(text).is_some() && (text.chars().nth(1).is_some() || !has_glyphs(&face(s), text))
    {
        Some("Emoji")
    } else {
        None
    }
}
fn has_glyph(data: &FontData, ch: char) -> bool {
    has_glyphs(data, ch.encode_utf8(&mut [0; 4]))
}
fn has_glyphs(data: &FontData, text: &str) -> bool {
    let Ok(font) = harfrust::FontRef::from_index(data.font.as_ref(), data.index) else {
        return false;
    };
    let data = harfrust::ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let mut buffer = harfrust::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    let glyphs = shaper
        .shape(buffer, harfrust::ShapeOptions::new())
        .glyph_infos()
        .to_vec();
    !glyphs.is_empty() && glyphs.iter().all(|g| g.glyph_id != 0)
}

/// Actual bundled-face advance, in points, for fitting pasted text diagrams.
pub fn mono_width(text: &str, size: f32, bold: bool) -> f32 {
    let style = TextStyle {
        font: "Noto Sans Mono".into(),
        size,
        bold,
        ..Default::default()
    };
    let data = face(&style);
    let font =
        harfrust::FontRef::from_index(data.font.as_ref(), data.index).expect("Bundled mono font");
    let data = harfrust::ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let mut buffer = harfrust::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    shaper
        .shape(buffer, harfrust::ShapeOptions::new())
        .glyph_positions()
        .iter()
        .map(|p| p.x_advance as f32)
        .sum::<f32>()
        * size
        / shaper.units_per_em() as f32
}
/// The exact face used by both the native renderer and PDF shaper. System faces
/// are loaded on demand; merely opening the picker does not load every font.
pub fn face(s: &TextStyle) -> Arc<FontData> {
    let name = key(s);
    if name == "Emoji" {
        return Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/NotoEmoji.ttf"
        )));
    }
    if let Some((_, bytes)) = FONTS.iter().find(|(n, _)| *n == name) {
        return Arc::new(FontData::from_static(bytes));
    }
    let id = query(s).expect("Resolved system face");
    let mut loaded = catalog().loaded.lock().unwrap_or_else(|e| e.into_inner());
    loaded
        .entry(id)
        .or_insert_with(|| {
            catalog()
                .db
                .with_face_data(id, |bytes, index| {
                    let mut data = FontData::from_owned(bytes.to_vec());
                    data.index = index;
                    Arc::new(data)
                })
                .unwrap_or_else(|| {
                    Arc::new(FontData::from_static(include_bytes!(
                        "../assets/fonts/NotoSans-Regular.ttf"
                    )))
                })
        })
        .clone()
}
fn definitions() -> FontDefinitions {
    let mut definitions = FontDefinitions::default();
    definitions.font_data.insert(
        "NotoEmoji-Regular".into(),
        FontData::from_static(include_bytes!("../assets/fonts/NotoEmoji.ttf")).into(),
    );
    definitions
        .families
        .insert(FontFamily::Name("Emoji".into()), vec!["Emoji".into()]);
    for (name, bytes) in FONTS {
        definitions
            .font_data
            .insert((*name).into(), FontData::from_static(bytes).into());
        let mut family = vec![(*name).into()];
        if *name != "Math" {
            family.push(
                if name.ends_with("Bold") || name.ends_with("BoldItalic") {
                    "Mono-Bold"
                } else {
                    "Mono"
                }
                .into(),
            );
            family.push("Math".into());
        }
        family.push("NotoEmoji-Regular".into());
        definitions
            .families
            .insert(FontFamily::Name((*name).into()), family);
    }
    let ui_style = TextStyle {
        font: preferred_body_family().into(),
        ..Default::default()
    };
    let ui_key = key(&ui_style);
    definitions
        .font_data
        .entry(ui_key.clone())
        .or_insert_with(|| face(&ui_style));
    definitions.families.insert(
        FontFamily::Name(ui_key.clone().into()),
        vec![
            ui_key.clone(),
            "Sans".into(),
            "Mono".into(),
            "Math".into(),
            "NotoEmoji-Regular".into(),
        ],
    );
    definitions
        .families
        .get_mut(&FontFamily::Proportional)
        .expect("Default proportional font")
        .insert(0, ui_key);
    definitions
}
pub fn install(ctx: &Context) {
    ctx.set_fonts(definitions());
}

/// Returns true when egui needs another pass to activate newly requested fonts.
/// Definitions are scoped to the context so independent windows/tests cannot
/// accidentally believe another context already installed a face.
pub fn prepare(ctx: &Context, doc: &Document, typing: &TextStyle) -> bool {
    let id = eframe::egui::Id::new("alphadocs-font-definitions");
    let mut definitions = ctx
        .data_mut(|d| d.get_temp::<FontDefinitions>(id))
        .unwrap_or_else(definitions);
    let mut styles = vec![typing, &doc.page.header.style, &doc.page.footer.style];
    styles.extend(
        doc.page
            .header
            .chars
            .iter()
            .chain(&doc.page.footer.chars)
            .map(|c| &c.style),
    );
    for block in (0..doc.tabs.len()).flat_map(|i| doc.tab_blocks(i)) {
        let paragraphs: Vec<_> = match block {
            crate::model::Block::Paragraph(p) => vec![p],
            crate::model::Block::Table(rows) => rows.iter().flatten().collect(),
            _ => vec![],
        };
        for p in paragraphs {
            styles.push(&p.style);
            let mut previous = &p.style;
            for c in &p.chars {
                let style = &c.style;
                if style.font != previous.font
                    || style.bold != previous.bold
                    || style.italic != previous.italic
                {
                    styles.push(style);
                    previous = style;
                }
            }
        }
    }
    let mut requested = HashSet::new();
    let mut changed = false;
    for style in styles {
        let name = key(style);
        if requested.insert(name.clone()) && !definitions.font_data.contains_key(&name) {
            definitions.font_data.insert(name.clone(), face(style));
            definitions.families.insert(
                FontFamily::Name(name.clone().into()),
                vec![
                    name,
                    "Sans".into(),
                    if style.bold { "Mono-Bold" } else { "Mono" }.into(),
                    "Math".into(),
                    "NotoEmoji-Regular".into(),
                ],
            );
            changed = true;
        }
    }
    if changed {
        ctx.set_fonts(definitions.clone());
    }
    ctx.data_mut(|d| d.insert_temp(id, definitions));
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_symbol_fallback_covers_boxes_and_arrows_for_screen_and_pdf() {
        for bold in [false, true] {
            let style = TextStyle {
                font: "Noto Sans".into(),
                bold,
                ..Default::default()
            };
            for ch in (0x2500..=0x257f)
                .filter_map(char::from_u32)
                .chain("→←↑↓↔⇒⇐⇔⟵⟶⟷⟸⟹⟺".chars())
            {
                let resolved = symbol_key(&style, ch);
                assert!(
                    resolved.starts_with("Mono") || resolved == "Math",
                    "{ch} {resolved}"
                );
                let (_, bytes) = FONTS.iter().find(|(name, _)| *name == resolved).unwrap();
                assert!(has_glyph(&FontData::from_static(bytes), ch), "{ch}");
            }
        }
        let ctx = Context::default();
        install(&ctx);
        let mut out = ctx.run_ui(Default::default(), |_| {
            ctx.fonts_mut(|fonts| {
                let family = eframe::egui::FontId::new(11.0, FontFamily::Name("Sans".into()));
                // epaint's has_glyph reports false when a real fallback glyph shares
                // the replacement glyph's face. Compare actual raster atlas regions.
                let g = fonts.layout_no_wrap(
                    "┌─┐│├┤└┘→←↑↓↔⟵⟶⟷⟸⟹⟺\u{10ffff}".into(),
                    family,
                    eframe::egui::Color32::BLACK,
                );
                let glyphs = &g.rows[0].glyphs;
                let tofu = glyphs.last().unwrap().uv_rect;
                for glyph in &glyphs[..glyphs.len() - 1] {
                    assert_ne!(glyph.uv_rect, tofu, "{}", glyph.chr);
                }
            });
        });
        out.textures_delta.clear();
    }
    #[test]
    fn current_emoji_font_contains_newer_faces_without_missing_glyphs() {
        let face = harfrust::FontRef::new(include_bytes!("../assets/fonts/NotoEmoji.ttf")).unwrap();
        let data = harfrust::ShaperData::new(&face);
        let shaper = data.shaper(&face).build();
        for text in ["😀", "🫠", "🫨", "🤥", "🫪", "👩‍💻", "👋🏽"] {
            let mut buffer = harfrust::UnicodeBuffer::new();
            buffer.push_str(text);
            buffer.guess_segment_properties();
            let result = shaper.shape(buffer, Default::default());
            assert!(
                result.glyph_infos().iter().all(|g| g.glyph_id != 0),
                "Missing emoji glyph: {text}"
            );
            assert_eq!(
                result.glyph_infos().len(),
                1,
                "Emoji sequence was split: {text}"
            );
        }
        for emoji in emojis::iter() {
            let mut buffer = harfrust::UnicodeBuffer::new();
            buffer.push_str(emoji.as_str());
            buffer.guess_segment_properties();
            let result = shaper.shape(buffer, Default::default());
            assert!(
                result.glyph_infos().iter().all(|g| g.glyph_id != 0),
                "Missing picker emoji: {} ({})",
                emoji.as_str(),
                emoji.name()
            );
        }
    }
    #[test]
    fn new_font_is_activated_before_measuring_a_document() {
        let ctx = Context::default();
        install(&ctx);
        let mut doc = Document::new("Font activation");
        if let crate::model::Block::Paragraph(p) = &mut doc.blocks[0] {
            p.style.font = "Noto Sans Mono".into();
        }
        let mut measured = false;
        let mut output = ctx.run_ui(Default::default(), |_| {
            if prepare(&ctx, &doc, &TextStyle::default()) {
                ctx.request_discard("Test newly selected face");
                return;
            }
            crate::layout::Layout::measure(&ctx, &doc);
            measured = true;
        });
        output.textures_delta.clear();
        assert!(measured);
    }
    #[test]
    fn discovered_faces_are_real_and_bundled_fonts_remain_available() {
        assert!(available("Noto Sans") && available("Noto Serif"));
        for family in families().iter().take(16) {
            let style = TextStyle {
                font: family.clone(),
                ..Default::default()
            };
            let data = face(&style);
            assert!(
                harfrust::FontRef::from_index(data.font.as_ref(), data.index).is_ok(),
                "{family}"
            );
        }
        let unknown = TextStyle {
            font: "A missing family".into(),
            ..Default::default()
        };
        assert_eq!(key(&unknown), "Sans");
    }
}
