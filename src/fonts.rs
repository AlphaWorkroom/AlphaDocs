use crate::model::{Document, TextStyle};
use eframe::egui::{Context, FontData, FontDefinitions, FontFamily};
use fontdb::{Database, Family, Query, Style, Weight};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
};

pub const FONTS: &[(&str, &[u8])] = &[
    ("Emoji", include_bytes!("../assets/fonts/NotoEmoji.ttf")),
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
    if s.font == "Noto Emoji" {
        return "Emoji".into();
    }
    if s.font == "Noto Sans" || s.font == "Noto Serif" {
        return bundled_key(s);
    }
    query(s)
        .map(|id| format!("System-{id:?}"))
        .unwrap_or_else(|| bundled_key(s))
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
        definitions.families.insert(
            FontFamily::Name((*name).into()),
            vec![(*name).into(), "NotoEmoji-Regular".into()],
        );
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
        vec![ui_key.clone(), "Sans".into(), "NotoEmoji-Regular".into()],
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
                vec![name, "Sans".into(), "NotoEmoji-Regular".into()],
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
