use crate::model::Document;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
mod migration;
pub use migration::migrate_previous_installation;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub theme: Theme,
    pub zoom: f32,
    pub outline: bool,
    pub ruler: bool,
    pub counts: bool,
    pub autosave: bool,
    pub auto_recalculate_tables: bool,
    pub compact_menus: bool,
    pub pinned_emojis: Vec<String>,
    pub pinned_colours: Vec<[u8; 3]>,
    pub recent_emojis: Vec<String>,
    pub personal_dictionary: Vec<String>,
    pub check_updates: bool,
    pub default_page: Option<crate::model::PageSetup>,
    pub spelling_suggestions: bool,
    pub grammar_suggestions: bool,
    pub online_translation: bool,
    pub nearby_invites: bool,
    pub accessible_text: bool,
    pub interface_scale: f32,
    pub typing: crate::writing::TypingPreferences,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            zoom: 1.0,
            outline: true,
            ruler: true,
            counts: true,
            autosave: true,
            auto_recalculate_tables: false,
            compact_menus: false,
            pinned_emojis: Vec::new(),
            pinned_colours: Vec::new(),
            recent_emojis: Vec::new(),
            personal_dictionary: Vec::new(),
            check_updates: true,
            default_page: None,
            spelling_suggestions: true,
            grammar_suggestions: true,
            online_translation: false,
            nearby_invites: true,
            accessible_text: true,
            interface_scale: 1.0,
            typing: Default::default(),
        }
    }
}
pub fn data_dir() -> PathBuf {
    std::env::var_os("ALPHADOCS_DATA_DIR")
        .or_else(|| std::env::var_os(crate::legacy::DATA_ENV))
        .map(PathBuf::from)
        .or_else(|| {
            ProjectDirs::from("studio", "Alpha Workroom", "AlphaDocs")
                .map(|p| p.data_local_dir().to_path_buf())
        })
        .unwrap_or_else(|| std::env::temp_dir().join("AlphaDocs"))
}
pub fn is_native_path(path: &Path) -> bool {
    path.extension().is_some_and(|e| {
        e.eq_ignore_ascii_case("adocs") || e.eq_ignore_ascii_case(crate::legacy::EXTENSION)
    })
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = path.with_file_name(format!(
        ".{}.{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id(),
        sequence
    ));
    let result = (|| {
        let mut f = File::options()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| e.to_string())?;
        f.write_all(bytes)
            .and_then(|_| f.sync_all())
            .map_err(|e| e.to_string())?;
        drop(f);
        fs::rename(&tmp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
pub fn readable_time(seconds: u64) -> String {
    use time::{OffsetDateTime, UtcOffset, macros::format_description};
    OffsetDateTime::from_unix_timestamp(seconds as i64)
        .ok()
        .map(|t| t.to_offset(UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC)))
        .and_then(|t| {
            t.format(format_description!(
                "[day] [month repr:short] [year] at [hour]:[minute]:[second]"
            ))
            .ok()
        })
        .unwrap_or_else(|| "Unknown date".into())
}
pub fn save_document(doc: &Document, path: &Path) -> Result<(), String> {
    doc.validate()?;
    // Earlier builds reject version 2, preventing them from silently discarding
    // columns, running text or baseline styles. Version-1 files still load.
    let mut saved = doc.clone();
    saved.version = crate::model::DOCUMENT_VERSION;
    let bytes = serde_json::to_vec(&saved).map_err(|e| e.to_string())?;
    if bytes.len() > 128 * 1024 * 1024 {
        return Err(
            "Document exceeds the 128 MB native file limit; existing file was preserved".into(),
        );
    }
    atomic_write(path, &bytes)
}
pub fn load_document(path: &Path) -> Result<Document, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > 128 * 1024 * 1024 {
        return Err("Document exceeds the 128 MB limit".into());
    }
    let mut bytes = vec![];
    file.take(128 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 128 * 1024 * 1024 {
        return Err("Document exceeds the 128 MB limit".into());
    }
    let doc: Document = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Cannot read AlphaDocs document: {e}"))?;
    doc.validate()?;
    Ok(doc)
}
pub fn library_path(doc: &Document) -> PathBuf {
    data_dir()
        .join("documents")
        .join(format!("{}.adocs", doc.id))
}
pub fn existing_library_path(id: &str) -> Option<PathBuf> {
    if !Document::valid_id(id) {
        return None;
    }
    let dir = data_dir().join("documents");
    ["adocs", crate::legacy::EXTENSION]
        .into_iter()
        .map(|ext| dir.join(format!("{id}.{ext}")))
        .find(|path| path.is_file())
}
pub fn autosave(doc: &Document) -> Result<(), String> {
    save_document(doc, &library_path(doc))
}
pub fn preferences() -> Preferences {
    let mut prefs: Preferences = fs::read(data_dir().join("preferences.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    prefs.zoom = if prefs.zoom.is_finite() {
        prefs.zoom.clamp(0.25, 3.0)
    } else {
        1.0
    };
    prefs.interface_scale = if prefs.interface_scale.is_finite() {
        prefs.interface_scale.clamp(1.0, 1.5)
    } else {
        1.0
    };
    prefs.pinned_colours = normalize_pinned_colours(prefs.pinned_colours);
    prefs
}

fn normalize_pinned_colours(colours: Vec<[u8; 3]>) -> Vec<[u8; 3]> {
    let mut unique = Vec::with_capacity(colours.len().min(24));
    for colour in colours {
        if unique.len() == 24 {
            break;
        }
        if !unique.contains(&colour) {
            unique.push(colour);
        }
    }
    unique
}
pub fn save_preferences(prefs: &Preferences) -> Result<(), String> {
    atomic_write(
        &data_dir().join("preferences.json"),
        &serde_json::to_vec(prefs).map_err(|e| e.to_string())?,
    )
}
pub struct Recent {
    pub path: PathBuf,
    pub doc: Document,
    pub modified: u64,
}
pub fn recent() -> Vec<Recent> {
    let mut result = vec![];
    if let Ok(entries) = fs::read_dir(data_dir().join("documents")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_native_path(&path)
                && let Ok(doc) = load_document(&path)
            {
                let modified = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs());
                result.push(Recent {
                    path,
                    doc,
                    modified,
                });
            }
        }
    }
    result.sort_by_key(|a| std::cmp::Reverse(a.modified));
    let current: std::collections::HashSet<_> = result
        .iter()
        .filter(|r| {
            r.path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("adocs"))
        })
        .map(|r| r.doc.id.clone())
        .collect();
    let mut seen = std::collections::HashSet::new();
    result.retain(|r| {
        (r.path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("adocs"))
            || !current.contains(&r.doc.id))
            && seen.insert(r.doc.id.clone())
    });
    result
}
pub fn snapshot(doc: &Document) -> Result<(), String> {
    let dir = data_dir().join("history").join(&doc.id);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    save_document(doc, &dir.join(format!("{now}.adocs")))?;
    let mut files: Vec<_> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_native_path(p))
        .collect();
    files.sort();
    let remove = files.len().saturating_sub(30);
    for p in files.into_iter().take(remove) {
        fs::remove_file(p).map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn history(doc: &Document) -> Vec<PathBuf> {
    let mut files: Vec<_> = fs::read_dir(data_dir().join("history").join(&doc.id))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_native_path(p))
        .collect();
    files.sort_by(|a, b| b.cmp(a));
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    #[test]
    fn saved_pinned_colours_are_deduplicated_and_limited() {
        let mut colours: Vec<_> = (0..30).map(|i| [i, 1, 2]).collect();
        colours.insert(2, [0, 1, 2]);
        let normalized = normalize_pinned_colours(colours);
        assert_eq!(normalized.len(), 24);
        assert_eq!(normalized[0], [0, 1, 2]);
        assert_eq!(
            normalized
                .iter()
                .filter(|colour| **colour == [0, 1, 2])
                .count(),
            1
        );
    }
    #[test]
    fn document_ids_cannot_escape_library_directory() {
        let mut d = Document::new("Invalid");
        for id in ["../outside", "C:\\outside", "", "..", "a/b"] {
            d.id = id.into();
            assert!(d.validate().is_err());
        }
    }
    #[test]
    fn concurrent_atomic_writes_leave_complete_content() {
        let root = std::env::temp_dir().join(format!("alphadocs-atomic-{}", Document::new("").id));
        fs::create_dir_all(&root).unwrap();
        let file = root.join("atomic.json");
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let file = file.clone();
                std::thread::spawn(move || {
                    let bytes = serde_json::to_vec(&vec![i; 10000]).unwrap();
                    atomic_write(&file, &bytes)
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap().unwrap();
        }
        let content: Vec<u8> = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        assert_eq!(content.len(), 10000);
        assert!(content.iter().all(|v| *v == content[0]));
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_file(file).unwrap();
        fs::remove_dir(root).unwrap();
    }
    #[test]
    fn structured_native_roundtrip() {
        let mut d = Document::new("格式 café");
        let mut p = Paragraph::from_text("Unicode 茶");
        p.set_heading(2);
        p.chars[0].style.highlight = Some([255, 255, 0]);
        d.blocks = vec![
            Block::Paragraph(p),
            Block::Table(vec![vec![Paragraph::from_text("cell")]]),
            Block::Image(EmbeddedImage {
                name: "test.png".into(),
                data: vec![1, 2, 3].into(),
                width: 50.0,
                height: 30.0,
                ..Default::default()
            }),
            Block::PageBreak,
        ];
        let path = std::env::temp_dir().join(format!("alphadocs-test-{}.adocs", d.id));
        save_document(&d, &path).unwrap();
        save_document(&d, &path).unwrap();
        assert_eq!(load_document(&path).unwrap(), d);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn bad_version_is_rejected() {
        let mut d = Document::new("bad");
        d.version = 12;
        assert!(d.validate().is_err());
        d.version = 1;
        d.page.margins = [600.0; 4];
        assert!(d.validate().is_err());
    }
}
