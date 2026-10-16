mod about;
mod chrome;
mod comments;
mod dialogs;
mod document_tabs;
mod emoji_picker;
mod exporting;
mod feature_search;
mod help;
mod homepage;
mod image_controls;
mod image_guides;
mod image_move;
mod image_sidebar;
mod input;
#[cfg(test)]
mod input_tests;
mod live_proofing;
mod pointer_gesture;
mod print_screen;
mod proofing;
mod sharing;
mod table_controls;
mod toolbar_chrome;
mod updating;
use crate::{
    compat,
    engine::ClipboardContent,
    fonts,
    layout::{self, Layout, Object},
    model::*,
    persistence::{self, Preferences, Recent, Theme},
    session::{DocumentSession, SessionManifest, TabStore},
    ui::{
        icons::{self, Icon},
        theme::{self, Palette},
    },
};
use eframe::egui::{
    self, Align, Align2, Color32, Context, Event, FontId, Frame, Id, Key, Layout as UiLayout,
    Margin, Pos2, Rect, RichText, Sense, Stroke, Ui, pos2, vec2,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy)]
enum Dialog {
    Rename,
    Page,
    Count,
    Link,
    Find,
    History,
    Preferences,
    Help,
    About,
    Symbols,
    Emojis,
    Print,
    Share,
    Delete,
    Message,
    ExportResult,
}
#[derive(Clone, Copy)]
enum Action {
    CheckUpdates,
    Rename,
    New,
    Open,
    Save,
    SaveAs,
    Home,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Find,
    Page,
    Count,
    History,
    Image,
    Link,
    Table(usize, usize),
    Break,
    Rule,
    ExportPdf,
    ExportDocx,
    Export(compat::export::Format),
    Print,
    Proofing,
    Share,
    Preferences,
    Help,
    About,
    Symbols,
    Emojis,
    Duplicate,
    Comment,
    Reaction,
    Delete,
    Clear,
    ToggleMenus,
}
pub struct AlphaDocs {
    session: DocumentSession,
    tabs: TabStore,
    prefs: Preferences,
    palette: Palette,
    home: bool,
    recents: Vec<Recent>,
    search: String,
    starred_only: bool,
    sort_title: bool,
    last_autosave: Instant,
    last_history: Instant,
    status: String,
    dialog: Option<Dialog>,
    page_draft: PageSetup,
    find_query: String,
    replace_text: String,
    case_sensitive: bool,
    find_index: usize,
    find_active: bool,
    link_url: String,
    link_text: String,
    image_lock_ratio: bool,
    image_options_open: bool,
    image_block: Option<usize>,
    image_selection_id: Option<String>,
    image_resize: Option<image_controls::ResizeDrag>,
    image_drag: Option<image_move::ImageDrag>,
    comments_open: bool,
    comment_draft: String,
    comment_focus: bool,
    comment_edit: Option<(String, String, String)>,
    comment_replies: HashMap<(String, String), String>,
    comment_draft_image: Option<String>,
    comment_drafts: HashMap<String, String>,
    comments_resolved: bool,
    emoji_reaction_target: Option<String>,
    emoji_reaction_comment: Option<String>,
    text_comment_range: Option<((Position, Position), u64)>,
    text_reaction_range: Option<((Position, Position), u64)>,
    active_comment: Option<(String, String)>,
    annotation_positions: HashMap<String, Rect>,
    annotation_rail_x: f32,
    selection_annotation_rect: Option<Rect>,
    image_effects_preview: Option<(usize, ImageEffects)>,
    layout: Option<Layout>,
    layout_revision: u64,
    layout_dpi: f32,
    measurement_cache: layout::MeasurementCache,
    textures: HashMap<usize, (SharedVec<u8>, ImageEffects, egui::TextureHandle)>,
    refocus: bool,
    scroll_caret: bool,
    nonprinting: bool,
    fullscreen: bool,
    ime_preedit: String,
    paste_plain: bool,
    history_files: Vec<PathBuf>,
    delete_id: Option<String>,
    action: Option<Action>,
    message: String,
    clipboard: Option<ClipboardContent>,
    font_search: String,
    find_focus: bool,
    fonts_checked: Option<(String, u64, String, bool, bool)>,
    content_cache: DocumentContentCache,
    emoji_picker: emoji_picker::EmojiPicker,
    table_drag: Option<table_controls::TableDrag>,
    context_pointer: super::platform::ContextPointer,
    tab_rename: Option<(String, String)>,
    view_tab_id: String,
    feature_search_open: bool,
    feature_query: String,
    feature_focus: bool,
    feature_index: usize,
    feature_pulse: Option<(Instant, feature_search::Target)>,
    feature_rect: Option<Rect>,
    title_draft: String,
    rename_focus: bool,
    tab_rename_focus: bool,
    print_state: Option<print_screen::PrintState>,
    proof_state: Option<proofing::ProofState>,
    live_proof: live_proofing::LiveProof,
    export_job: Option<exporting::ExportJob>,
    paste_state: Option<proofing::PasteState>,
    sharing: sharing::ShareState,
    update_state: updating::UpdateState,
}
#[derive(Default)]
struct DocumentContentCache {
    id: String,
    revision: u64,
    text: String,
    counts: (usize, usize, usize),
    headings: Vec<(Address, u8, String)>,
}
impl AlphaDocs {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let migration = persistence::migrate_previous_installation();
        fonts::install(&cc.egui_ctx);
        let mut app = Self::fresh(
            &cc.egui_ctx,
            persistence::preferences(),
            persistence::recent(),
        );
        app.restore_session();
        if let Some(path) = std::env::args_os().nth(1) {
            app.open_path(PathBuf::from(path));
        }
        if let Err(error) = migration {
            app.message = format!(
                "Previous library import is incomplete. Original files were retained.\n\n{error}"
            );
            app.status = "Previous library import needs attention".into();
            app.dialog = Some(Dialog::Message);
        }
        app.check_updates(&cc.egui_ctx, false);
        app
    }
    fn fresh(ctx: &Context, prefs: Preferences, recents: Vec<Recent>) -> Self {
        let palette = theme::apply(ctx, prefs.theme);
        let doc = blank_document("Untitled document");
        let page_draft = doc.page.clone();
        Self {
            session: DocumentSession::new(doc, None),
            tabs: TabStore::default(),
            prefs,
            palette,
            home: true,
            recents,
            search: String::new(),
            starred_only: false,
            sort_title: false,
            last_autosave: Instant::now(),
            last_history: Instant::now(),
            status: "All changes saved locally".into(),
            dialog: None,
            page_draft,
            find_query: String::new(),
            replace_text: String::new(),
            case_sensitive: false,
            find_index: 0,
            find_active: false,
            link_url: String::new(),
            link_text: String::new(),
            image_lock_ratio: true,
            image_options_open: false,
            image_block: None,
            image_selection_id: None,
            image_resize: None,
            image_drag: None,
            comments_open: false,
            comment_draft: String::new(),
            comment_focus: false,
            comment_edit: None,
            comment_replies: HashMap::new(),
            comment_draft_image: None,
            comment_drafts: HashMap::new(),
            comments_resolved: false,
            emoji_reaction_target: None,
            emoji_reaction_comment: None,
            text_comment_range: None,
            text_reaction_range: None,
            active_comment: None,
            annotation_positions: Default::default(),
            annotation_rail_x: 0.0,
            selection_annotation_rect: None,
            image_effects_preview: None,
            layout: None,
            layout_revision: u64::MAX,
            layout_dpi: 0.0,
            measurement_cache: layout::MeasurementCache::default(),
            textures: HashMap::new(),
            refocus: false,
            scroll_caret: false,
            nonprinting: false,
            fullscreen: false,
            ime_preedit: String::new(),
            paste_plain: false,
            history_files: vec![],
            delete_id: None,
            action: None,
            message: String::new(),
            clipboard: None,
            font_search: String::new(),
            find_focus: false,
            fonts_checked: None,
            content_cache: DocumentContentCache::default(),
            emoji_picker: emoji_picker::EmojiPicker::default(),
            table_drag: None,
            context_pointer: Default::default(),
            tab_rename: None,
            feature_search_open: false,
            feature_query: String::new(),
            feature_focus: false,
            feature_index: 0,
            feature_pulse: None,
            feature_rect: None,
            title_draft: String::new(),
            rename_focus: false,
            tab_rename_focus: false,
            print_state: None,
            proof_state: None,
            live_proof: Default::default(),
            export_job: None,
            paste_state: None,
            sharing: sharing::ShareState::default(),
            update_state: Default::default(),
            view_tab_id: "tab-1".into(),
        }
    }
    fn queue(&mut self, a: Action) {
        self.action = Some(a);
    }
    fn copy_selection(&mut self, ctx: &Context, cut: bool) {
        if !self.session.editor.selected() {
            return;
        }
        let fragment = self.session.editor.copy_fragment();
        if crate::clipboard::write(&fragment).is_err() {
            ctx.copy_text(fragment.text.clone());
        }
        self.clipboard = Some(fragment);
        if cut && !self.session.viewing {
            self.session.editor.delete_selection();
        }
    }
    fn paste_text(&mut self, text: &str) {
        if !self.paste_plain && self.paste_native() {
        } else if !self.paste_plain
            && let Some(fragment) = self.clipboard.as_ref().filter(|f| f.text == text)
        {
            self.session
                .editor
                .paste_fragment_replacing_image(fragment, self.image_block);
        } else if !text.is_empty() {
            if self.image_block.is_some() || crate::clipboard::box_diagram(text) {
                let page = &self.session.editor.doc.page;
                let columns = page.columns.max(1) as f32;
                let content = crate::clipboard::plain_content(
                    text,
                    &self.session.editor.typing,
                    (page.size().0
                        - page.margins[2]
                        - page.margins[3]
                        - page.column_gap * (columns - 1.0))
                        / columns,
                );
                self.session
                    .editor
                    .paste_fragment_replacing_image(&content, self.image_block);
            } else {
                self.session
                    .editor
                    .paste(&crate::clipboard::arrow_text(text), true);
            }
        }
        self.paste_plain = false;
    }
    fn load(&mut self, doc: Document, file: Option<PathBuf>) {
        self.proof_state = None;
        self.image_effects_preview = None;
        self.image_block = None;
        self.image_selection_id = None;
        self.image_resize = None;
        self.image_options_open = false;
        self.tabs
            .open(&mut self.session, DocumentSession::new(doc, file));
        self.layout_revision = u64::MAX;
        self.textures.clear();
        self.home = false;
        self.refocus = true;
        self.scroll_caret = true;
        self.session.viewing = false;
        self.status = "Opened locally".into();
        self.last_autosave = Instant::now();
        self.persist_session();
    }
    fn reset_document_view(&mut self) {
        self.proof_state = None;
        self.view_tab_id = self.session.editor.doc.tabs[self.session.editor.doc.active_tab]
            .id
            .clone();
        self.image_effects_preview = None;
        self.table_drag = None;
        self.image_drag = None;
        self.comments_open = false;
        self.comment_draft.clear();
        self.comment_focus = false;
        self.comment_edit = None;
        self.comment_replies.clear();
        self.comment_draft_image = None;
        self.comment_drafts.clear();
        self.emoji_reaction_target = None;
        self.emoji_reaction_comment = None;
        self.text_comment_range = None;
        self.text_reaction_range = None;
        self.active_comment = None;
        self.annotation_positions.clear();
        self.layout = None;
        self.layout_revision = u64::MAX;
        self.textures.clear();
        self.dialog = None;
        self.ime_preedit.clear();
        self.image_block = None;
        self.image_selection_id = None;
        self.image_resize = None;
        self.refocus = !self.home;
        self.scroll_caret = true;
        self.session.editor.clamp();
        self.session.editor.sync_style();
    }
    fn switch_tab(&mut self, index: usize) {
        self.flush();
        if self.tabs.switch(&mut self.session, index) {
            self.home = false;
            self.reset_document_view();
            self.status = if self.session.dirty() {
                "Changes waiting to save".into()
            } else {
                "All changes saved locally".into()
            };
            self.persist_session();
        }
    }
    fn persist_session(&mut self) {
        self.tabs.refresh(&self.session);
        #[cfg(not(test))]
        if let Ok(bytes) = serde_json::to_vec(&self.tabs.manifest(&self.session, self.home))
            && let Err(e) =
                persistence::atomic_write(&persistence::data_dir().join("session.json"), &bytes)
        {
            self.status = format!("Could not save open tabs: {e}");
        }
    }
    fn restore_session(&mut self) {
        let manifest: Option<SessionManifest> =
            std::fs::read(persistence::data_dir().join("session.json"))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok());
        let Some(manifest) = manifest else {
            return;
        };
        let active_id = manifest
            .active
            .and_then(|i| manifest.documents.get(i))
            .map(|d| d.id.clone());
        for entry in manifest.documents {
            if !Document::valid_id(&entry.id) {
                continue;
            }
            let Some(path) = persistence::existing_library_path(&entry.id) else {
                continue;
            };
            if let Ok(doc) = persistence::load_document(&path) {
                let mut session = DocumentSession::new(
                    doc,
                    entry.file.filter(|p| {
                        p.extension()
                            .is_some_and(|e| e.eq_ignore_ascii_case("adocs"))
                    }),
                );
                session.source = entry.source;
                session.editor.caret = entry.caret;
                session.editor.anchor = entry.anchor;
                session.editor.restore_tab_selections(entry.tab_selections);
                session.editor.clamp();
                session.editor.sync_style();
                session.viewing = entry.viewing;
                session.saved_library_revision = session.editor.revision;
                self.tabs.open(&mut self.session, session);
            }
        }
        if let Some(id) = active_id
            && let Some(i) = self.tabs.tabs.iter().position(|t| t.id == id)
        {
            self.tabs.switch(&mut self.session, i);
        }
        self.home = manifest.home || self.tabs.active.is_none();
        self.reset_document_view();
    }
    fn flush(&mut self) -> bool {
        if self.tabs.active.is_some()
            && self.session.saved_library_revision != self.session.editor.revision
        {
            match self.session.save_local() {
                Ok(()) => {
                    self.session.saved_library_revision = self.session.editor.revision;
                    self.status = "All changes saved locally".into();
                }
                Err(e) => {
                    self.status = format!("Autosave failed: {e}");
                    return false;
                }
            }
        }
        true
    }
    fn save_file(&mut self, as_new: bool) {
        self.flush();
        let path = if as_new || self.session.file.is_none() {
            rfd::FileDialog::new()
                .add_filter("AlphaDocs document", &["adocs"])
                .set_file_name(format!(
                    "{}.adocs",
                    safe_name(&self.session.editor.doc.title)
                ))
                .save_file()
        } else {
            self.session.file.clone()
        };
        if let Some(mut path) = path {
            if !path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("adocs"))
            {
                path.set_extension("adocs");
            }
            match persistence::save_document(&self.session.editor.doc, &path) {
                Ok(()) => {
                    self.session.file = Some(path);
                    self.status = "Document saved".into();
                    if let Err(e) = persistence::snapshot(&self.session.editor.doc) {
                        self.status = format!("Saved; history failed: {e}");
                    }
                }
                Err(e) => self.status = e,
            }
        }
    }
    fn open_path(&mut self, path: PathBuf) {
        self.flush();
        if let Some(index) = self.tabs.find_source(&self.session, &path) {
            self.switch_tab(index);
            return;
        }
        let ext = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let result = match ext.as_str() {
            "docx" => compat::docx::import(&path),
            "txt" => std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .map(|t| {
                    let mut d =
                        Document::new(&path.file_stem().unwrap_or_default().to_string_lossy());
                    d.blocks = t
                        .replace("\r\n", "\n")
                        .lines()
                        .map(|t| Block::Paragraph(Paragraph::from_text(t)))
                        .collect();
                    if d.blocks.is_empty() {
                        d.blocks.push(Block::Paragraph(Paragraph::default()));
                    }
                    (d, vec![])
                }),
            _ => persistence::load_document(&path).map(|d| (d, vec![])),
        };
        match result {
            Ok((d, warnings)) => {
                self.load(
                    d,
                    if ext == "adocs" {
                        Some(path.clone())
                    } else {
                        None
                    },
                );
                self.session.source = Some(path);
                if !warnings.is_empty() {
                    self.status = format!("Imported with limitations: {}", warnings.join(" "));
                }
            }
            Err(e) => self.status = e,
        }
    }
    fn ensure_layout(&mut self, ctx: &Context) {
        if self.layout.is_some()
            && self.layout_revision == self.session.editor.revision
            && self.layout_dpi == ctx.pixels_per_point()
        {
            return;
        }
        // These galleys provide geometry, not painted meshes. Visible lines are
        // rasterized through the current atlas, so atlas resets do not invalidate
        // unchanged physical page measurements.
        self.sync_image_selection();
        self.layout = Some(Layout::measure_cached(
            ctx,
            &self.session.editor.doc,
            &mut self.measurement_cache,
        ));
        self.layout_revision = self.session.editor.revision;
        self.layout_dpi = ctx.pixels_per_point();
    }
    fn sync_image_selection(&mut self) {
        if let Some(selected) = &self.image_selection_id {
            let previous = self.image_block;
            self.image_block = self
                .session
                .editor
                .doc
                .blocks
                .iter()
                .position(|b| matches!(b, Block::Image(i) if i.id == *selected));
            if self.image_block != previous {
                self.image_resize = None;
                self.image_drag = None;
                self.image_effects_preview = None;
            }
            if self.image_block.is_none() {
                self.image_selection_id = None;
                self.image_resize = None;
                self.image_drag = None;
                self.image_effects_preview = None;
                self.refocus = true;
            }
        }
    }
    fn prepare_fonts(&mut self, ctx: &Context) -> bool {
        let editor = &self.session.editor;
        let key = (
            editor.doc.id.clone(),
            editor.revision,
            editor.typing.font.clone(),
            editor.typing.bold,
            editor.typing.italic,
        );
        if self.fonts_checked.as_ref() == Some(&key) {
            return false;
        }
        let changed = fonts::prepare(ctx, &editor.doc, &editor.typing);
        if changed {
            self.measurement_cache.invalidate();
        }
        self.fonts_checked = Some(key);
        changed
    }
    fn refresh_content_cache(&mut self) {
        let editor = &self.session.editor;
        if self.content_cache.id == editor.doc.id && self.content_cache.revision == editor.revision
        {
            return;
        }
        let text = editor.doc.text();
        let counts = (
            text.unicode_words().count(),
            text.chars().count(),
            text.chars().filter(|c| !c.is_whitespace()).count(),
        );
        let headings = editor
            .doc
            .addresses()
            .into_iter()
            .filter_map(|a| {
                let p = editor.doc.paragraph(a)?;
                (p.heading > 0).then(|| (a, p.heading, p.text()))
            })
            .collect();
        self.content_cache = DocumentContentCache {
            id: editor.doc.id.clone(),
            revision: editor.revision,
            text,
            counts,
            headings,
        };
    }
    fn execute(&mut self, a: Action, ctx: &Context) {
        if self.session.viewing
            && matches!(
                a,
                Action::Undo
                    | Action::Redo
                    | Action::Cut
                    | Action::Paste
                    | Action::Table(..)
                    | Action::Break
                    | Action::Rule
                    | Action::Image
                    | Action::Symbols
                    | Action::Emojis
                    | Action::Comment
                    | Action::Reaction
                    | Action::Link
                    | Action::Clear
            )
        {
            self.status = "Switch to Editing to change this document".into();
            return;
        }
        match a {
            Action::Share => self.open_sharing(),
            Action::ToggleMenus => self.toggle_menus(ctx),
            Action::New => {
                self.flush();
                self.load(blank_document("Untitled document"), None);
            }
            Action::Open => {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter(
                        "Documents",
                        &["adocs", crate::legacy::EXTENSION, "docx", "txt"],
                    )
                    .pick_file()
                {
                    self.open_path(p);
                }
            }
            Action::Save => self.save_file(false),
            Action::SaveAs => self.save_file(true),
            Action::Home => {
                self.flush();
                self.home = true;
                self.recents = persistence::recent();
                self.persist_session();
            }
            Action::Undo => {
                self.image_effects_preview = None;
                self.session.editor.undo();
            }
            Action::Redo => {
                self.image_effects_preview = None;
                self.session.editor.redo();
            }
            Action::Copy => {
                self.copy_selection(ctx, false);
            }
            Action::Cut => {
                self.copy_selection(ctx, true);
            }
            Action::Paste => {
                if !self.paste_native() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                }
                self.refocus = true;
            }
            Action::SelectAll => self.session.editor.select_all(),
            Action::Find => {
                let selected = self.session.editor.selected_text();
                if !selected.is_empty() && !selected.contains('\n') {
                    self.find_query = selected;
                }
                self.find_focus = true;
                self.find_index = 0;
                self.find_active = false;
                self.dialog = Some(Dialog::Find);
            }
            Action::Rename => {
                self.title_draft = self.session.editor.doc.title.clone();
                self.rename_focus = true;
                self.dialog = Some(Dialog::Rename);
            }
            Action::Page => {
                self.page_draft = self.session.editor.doc.page.clone();
                self.dialog = Some(Dialog::Page);
            }
            Action::Count => self.dialog = Some(Dialog::Count),
            Action::History => {
                self.flush();
                self.history_files = persistence::history(&self.session.editor.doc);
                self.dialog = Some(Dialog::History);
            }
            Action::Image => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg"])
                    .pick_file()
                {
                    match std::fs::read(&path)
                        .map_err(|e| e.to_string())
                        .and_then(|data| {
                            image::load_from_memory(&data)
                                .map(|im| (data, im.width(), im.height()))
                                .map_err(|e| e.to_string())
                        }) {
                        Ok((data, w, h)) => {
                            let width = (w as f32 * 0.75)
                                .min(self.session.editor.doc.page.size().0 - 144.0);
                            self.session
                                .editor
                                .insert_block(Block::Image(EmbeddedImage {
                                    name: path
                                        .file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .into_owned(),
                                    data: data.into(),
                                    width,
                                    height: width * h as f32 / w as f32,
                                    ..Default::default()
                                }));
                            self.textures.clear();
                        }
                        Err(e) => self.status = e,
                    }
                }
            }
            Action::Link => {
                self.link_text = self.session.editor.selected_text();
                self.link_url = self.session.editor.typing.link.clone().unwrap_or_default();
                self.dialog = Some(Dialog::Link);
            }
            Action::Table(rows, cols) => self.session.editor.insert_table(rows, cols),
            Action::Break => self.session.editor.insert_block(Block::PageBreak),
            Action::Rule => self.session.editor.insert_block(Block::Rule),
            Action::ExportPdf => {
                self.ensure_layout(ctx);
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name(format!("{}.pdf", safe_name(&self.session.editor.doc.title)))
                    .add_filter("PDF", &["pdf"])
                    .save_file()
                {
                    let document = self.session.editor.doc.flattened();
                    let layout = Layout::measure(ctx, &document);
                    self.status = match compat::pdf::export(&document, &layout, &path) {
                        Ok(()) => "PDF exported".into(),
                        Err(e) => e,
                    };
                }
            }
            Action::ExportDocx => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name(format!(
                        "{}.docx",
                        safe_name(&self.session.editor.doc.title)
                    ))
                    .add_filter("Word document", &["docx"])
                    .save_file()
                {
                    self.status = match compat::docx::export(&self.session.editor.doc, &path) {
                        Ok(()) => "DOCX exported".into(),
                        Err(e) => e,
                    };
                }
            }
            Action::Export(format) => self.export_format(format, ctx),
            Action::Print => self.start_print(ctx),
            Action::Proofing => self.start_proofing(ctx),
            Action::Preferences => self.dialog = Some(Dialog::Preferences),
            Action::Help => self.dialog = Some(Dialog::Help),
            Action::About => self.dialog = Some(Dialog::About),
            Action::CheckUpdates => self.check_updates(ctx, true),
            Action::Symbols => self.dialog = Some(Dialog::Symbols),
            Action::Emojis => {
                self.emoji_reaction_target = None;
                self.emoji_reaction_comment = None;
                self.text_reaction_range = None;
                self.dialog = Some(Dialog::Emojis);
            }
            Action::Comment => {
                self.begin_comment();
            }
            Action::Reaction => {
                self.emoji_reaction_comment = None;
                self.text_reaction_range = None;
                if let Some(block) = self.image_block
                    && let Block::Image(i) = &self.session.editor.doc.blocks[block]
                {
                    self.emoji_reaction_target = Some(i.id.clone());
                    self.dialog = Some(Dialog::Emojis);
                } else if self.session.editor.selected() {
                    self.text_reaction_range =
                        Some((self.session.editor.range(), self.session.editor.revision));
                    self.emoji_reaction_target = None;
                    self.dialog = Some(Dialog::Emojis);
                }
            }
            Action::Duplicate => {
                self.flush();
                let mut doc = self.session.editor.doc.clone();
                doc.id = Document::new("").id;
                doc.title = format!("Copy of {}", doc.title);
                self.load(doc, None);
                self.flush();
            }
            Action::Delete => {
                self.delete_id = Some(self.session.editor.doc.id.clone());
                self.dialog = Some(Dialog::Delete);
            }
            Action::Clear => self.session.editor.format(|s| {
                *s = TextStyle {
                    font: fonts::preferred_body_family().into(),
                    ..Default::default()
                }
            }),
        }
        if self.dialog.is_none() && !self.home {
            self.refocus = true;
            self.scroll_caret = true;
        }
    }
    fn appearance(&mut self, ui: &mut Ui) {
        for (t, name) in [
            (Theme::System, "Follow system"),
            (Theme::Light, "Light"),
            (Theme::Dark, "Dark"),
        ] {
            if ui
                .selectable_value(&mut self.prefs.theme, t, name)
                .clicked()
            {
                let _ = persistence::save_preferences(&self.prefs);
                ui.close();
            }
        }
    }
    fn header(&mut self, ui: &mut Ui) {
        let p = self.palette;
        Frame::new()
            .fill(p.bg)
            .inner_margin(Margin {
                left: 16,
                right: 16,
                top: 8,
                bottom: 4,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, response) = ui.allocate_exact_size(vec2(32.0, 42.0), Sense::click());
                    icons::brand(
                        ui,
                        Rect::from_center_size(r.center(), vec2(28.0, 34.0)),
                        p.accent,
                    );
                    if response.on_hover_text("AlphaDocs home").clicked() {
                        self.queue(Action::Home);
                    }
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.horizontal(|ui| {
                            let mut document_title = self.session.editor.doc.title.clone();
                            let r = ui.add_enabled(
                                !self.session.viewing,
                                egui::TextEdit::singleline(&mut document_title)
                                    .font(FontId::proportional(18.0))
                                    .desired_width(290.0)
                                    .frame(Frame::NONE)
                                    .hint_text("Untitled document"),
                            );
                            if r.changed() {
                                self.session
                                    .editor
                                    .transaction(|e| e.doc.title = document_title);
                            }
                            if icons::button(
                                ui,
                                Icon::Star,
                                "Star document",
                                self.session.editor.doc.starred,
                                true,
                            )
                            .clicked()
                            {
                                self.session
                                    .editor
                                    .transaction(|e| e.doc.starred = !e.doc.starred);
                            }
                            let status = if self.session.dirty()
                                && matches!(
                                    self.status.as_str(),
                                    "All changes saved locally"
                                        | "Document saved"
                                        | "Opened locally"
                                ) {
                                if self.prefs.autosave {
                                    "Changes pending autosave"
                                } else {
                                    "Unsaved changes · Autosave off"
                                }
                            } else {
                                self.status.as_str()
                            };
                            ui.label(RichText::new(status).size(12.0).color(p.muted));
                        });
                        self.menus(ui);
                    });
                    ui.with_layout(UiLayout::right_to_left(Align::Center), |ui| {
                        if tonal(ui, "Share", p, true)
                            .on_hover_text("Share or join an encrypted LAN/VPN session")
                            .clicked()
                        {
                            self.queue(Action::Share);
                        }
                        if tonal(ui, "Save", p, true).clicked() {
                            self.queue(Action::Save);
                        }
                        let export = tonal(ui, "Export", p, true);
                        egui::Popup::menu(&export).show(|ui| self.export_choices(ui));
                        if icons::button(ui, Icon::History, "Version history", false, true)
                            .clicked()
                        {
                            self.queue(Action::History);
                        }
                        if icons::button(ui, Icon::Theme, "Appearance settings", false, true)
                            .clicked()
                        {
                            self.queue(Action::Preferences);
                        }
                    });
                });
            });
    }
    fn export_choices(&mut self, ui: &mut Ui) {
        let enabled = self.export_job.is_none();
        if menu(ui, "Microsoft Word (.docx)", "", enabled) {
            self.queue(Action::ExportDocx);
        }
        if menu(ui, "PDF Document (.pdf)", "", enabled) {
            self.queue(Action::ExportPdf);
        }
        for format in compat::export::Format::ALL {
            if menu(ui, format.label(), "", enabled) {
                self.queue(Action::Export(format));
            }
        }
        if !enabled {
            ui.label("Export in progress…");
        }
    }
    fn menus(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().interact_size.y = 24.0;
            ui.spacing_mut().item_spacing.x = 1.0;
            ui.spacing_mut().button_padding = vec2(7.0, 1.0);
            ui.menu_button("File", |ui| {
                for (label, key, a) in [
                    ("New document", "Ctrl+N", Action::New),
                    ("Open…", "Ctrl+O", Action::Open),
                    ("Make a copy", "", Action::Duplicate),
                ] {
                    if menu(ui, label, key, true) {
                        self.queue(a);
                    }
                }
                ui.separator();
                for (label, key, a) in [
                    ("Save", "Ctrl+S", Action::Save),
                    ("Save as…", "", Action::SaveAs),
                ] {
                    if menu(ui, label, key, true) {
                        self.queue(a);
                    }
                }
                submenu(ui, "Download", Icon::Download, |ui| self.export_choices(ui));
                if menu(ui, "Share or join…", "", true) {
                    self.queue(Action::Share);
                }
                ui.separator();
                for (label, key, a) in [
                    ("Version history", "", Action::History),
                    ("Move to trash…", "", Action::Delete),
                    ("Page setup", "", Action::Page),
                    ("Print…", "Ctrl+P", Action::Print),
                    ("Back to documents", "", Action::Home),
                ] {
                    if menu(ui, label, key, true) {
                        self.queue(a);
                    }
                }
            });
            ui.menu_button("Edit", |ui| {
                if menu(ui, "Undo", "Ctrl+Z", self.session.editor.can_undo()) {
                    self.queue(Action::Undo);
                }
                if menu(ui, "Redo", "Ctrl+Y", self.session.editor.can_redo()) {
                    self.queue(Action::Redo);
                }
                ui.separator();
                for (label, key, a, enabled) in [
                    (
                        "Cut",
                        "Ctrl+X",
                        Action::Cut,
                        self.session.editor.selected() && !self.session.viewing,
                    ),
                    (
                        "Copy",
                        "Ctrl+C",
                        Action::Copy,
                        self.session.editor.selected(),
                    ),
                    ("Paste", "Ctrl+V", Action::Paste, !self.session.viewing),
                    ("Select all", "Ctrl+A", Action::SelectAll, true),
                ] {
                    if menu(ui, label, key, enabled) {
                        self.queue(a);
                    }
                }
                ui.separator();
                if menu(ui, "Find and replace", "Ctrl+H", true) {
                    self.queue(Action::Find);
                }
            });
            ui.menu_button("View", |ui| {
                ui.menu_button("Mode", |ui| self.mode_choices(ui));
                if menu(
                    ui,
                    if self.prefs.compact_menus {
                        "Show the menus"
                    } else {
                        "Hide the menus"
                    },
                    "Ctrl+Shift+F",
                    true,
                ) {
                    self.queue(Action::ToggleMenus);
                }
                ui.separator();
                ui.checkbox(&mut self.prefs.outline, "Show document outline");
                ui.checkbox(&mut self.prefs.ruler, "Show ruler");
                ui.checkbox(&mut self.nonprinting, "Show non-printing characters");
                ui.checkbox(&mut self.prefs.counts, "Display word count while typing");
                ui.checkbox(&mut self.comments_resolved, "Show resolved comments");
                ui.menu_button("Appearance", |ui| self.appearance(ui));
                if menu(ui, "Full screen", "F11", true) {
                    self.fullscreen = !self.fullscreen;
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
                }
            });
            ui.menu_button("Insert", |ui| {
                ui.add_enabled_ui(!self.session.viewing, |ui| {
                    for (label, key, a) in [
                        ("Image from computer…", "", Action::Image),
                        ("Link…", "Ctrl+K", Action::Link),
                        ("Symbols", "", Action::Symbols),
                        ("Emojis", "", Action::Emojis),
                    ] {
                        if menu(ui, label, key, true) {
                            self.queue(a);
                        }
                    }
                    ui.menu_button("Table", |ui| self.table_picker(ui));
                    if menu(
                        ui,
                        "Comment",
                        "Ctrl+Alt+M",
                        self.image_block.is_some() || self.session.editor.selected(),
                    ) {
                        self.queue(Action::Comment);
                    }
                    if menu(
                        ui,
                        "Emoji reaction",
                        "",
                        self.image_block.is_some() || self.session.editor.selected(),
                    ) {
                        self.queue(Action::Reaction);
                    }
                    ui.separator();
                    if menu(ui, "Horizontal line", "", true) {
                        self.queue(Action::Rule);
                    }
                    if menu(ui, "Page break", "Ctrl+Enter", true) {
                        self.queue(Action::Break);
                    }
                    let mut nums = self.session.editor.doc.page.page_numbers;
                    if ui.checkbox(&mut nums, "Page numbers").changed() {
                        self.session
                            .editor
                            .transaction(|e| e.doc.page.page_numbers = nums);
                    }
                    let table = matches!(
                        self.session
                            .editor
                            .doc
                            .blocks
                            .get(self.session.editor.caret.address.block),
                        Some(Block::Table(_))
                    );
                    ui.add_enabled_ui(table, |ui| {
                        ui.separator();
                        self.table_commands(ui);
                    });
                });
            });
            ui.menu_button("Format", |ui| {
                ui.add_enabled_ui(!self.session.viewing, |ui| {
                    ui.menu_button("Text", |ui| {
                        for (label, key, field) in [
                            ("Bold", "Ctrl+B", 0),
                            ("Italic", "Ctrl+I", 1),
                            ("Underline", "Ctrl+U", 2),
                            ("Strikethrough", "Ctrl+Shift+X", 3),
                        ] {
                            if menu(ui, label, key, true) {
                                self.toggle_style(field);
                            }
                        }
                        ui.separator();
                        for (label, shortcut, value) in [
                            ("Superscript", "Ctrl+.", Baseline::Superscript),
                            ("Subscript", "Ctrl+,", Baseline::Subscript),
                        ] {
                            if menu(ui, label, shortcut, true) {
                                self.toggle_baseline(value);
                            }
                        }
                    });
                    ui.menu_button("Paragraph styles", |ui| self.styles(ui));
                    ui.menu_button("Align & indent", |ui| {
                        self.alignments(ui);
                        ui.separator();
                        if menu(ui, "Increase indent", "", true) {
                            self.indent(36.0);
                        }
                        if menu(ui, "Decrease indent", "", true) {
                            self.indent(-36.0);
                        }
                    });
                    ui.menu_button("Line & paragraph spacing", |ui| self.spacing(ui));
                    ui.menu_button("Columns", |ui| self.columns_picker(ui));
                    ui.menu_button("Bullets & numbering", |ui| self.lists(ui));
                    ui.separator();
                    if menu(ui, "Page orientation & margins", "", true) {
                        self.queue(Action::Page);
                    }
                    if menu(ui, "Clear formatting", "", true) {
                        self.queue(Action::Clear);
                    }
                });
            });
            ui.menu_button("Tools", |ui| {
                if menu(ui, "Spelling and grammar check", "Ctrl+Alt+X", true) {
                    self.queue(Action::Proofing);
                }
                ui.separator();
                for (label, a) in [
                    ("Word count", Action::Count),
                    ("Find and replace", Action::Find),
                    ("Preferences", Action::Preferences),
                ] {
                    if menu(ui, label, "", true) {
                        self.queue(a);
                    }
                }
            });
            ui.menu_button("Help", |ui| {
                if menu(ui, "Keyboard shortcuts & help", "", true) {
                    self.queue(Action::Help);
                }
                ui.separator();
                if menu(ui, "Send feedback or report a bug…", "", true) {
                    ui.ctx()
                        .open_url(egui::OpenUrl::new_tab(help::FEEDBACK_FORM));
                }
                if menu(ui, "About AlphaDocs", "", true) {
                    self.queue(Action::About);
                }
                if menu(ui, "Check for updates", "", true) {
                    self.queue(Action::CheckUpdates);
                }
            });
        });
    }
    fn toggle_style(&mut self, field: usize) {
        let s = &self.session.editor.typing;
        let value = !match field {
            0 => s.bold,
            1 => s.italic,
            2 => s.underline,
            _ => s.strike,
        };
        self.session.editor.format(|s| match field {
            0 => s.bold = value,
            1 => s.italic = value,
            2 => s.underline = value,
            _ => s.strike = value,
        });
        self.refocus = true;
    }
    fn toggle_baseline(&mut self, value: Baseline) {
        let value = if self.session.editor.typing.baseline == value {
            Baseline::Normal
        } else {
            value
        };
        self.session.editor.format(|s| s.baseline = value);
        self.refocus = true;
    }
    fn columns_picker(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for columns in 1..=3u8 {
                let (rect, response) = ui.allocate_exact_size(vec2(80.0, 70.0), Sense::click());
                let selected = self.session.editor.doc.page.columns == columns;
                ui.painter().rect_filled(
                    rect,
                    4.0,
                    if selected {
                        self.palette.selection
                    } else {
                        self.palette.toolbar
                    },
                );
                let width = 54.0 / columns as f32;
                for col in 0..columns {
                    for row in 0..6 {
                        let from =
                            rect.min + vec2(11.0 + col as f32 * width, 10.0 + row as f32 * 6.0);
                        ui.painter().line_segment(
                            [from, from + vec2(width - 4.0, 0.0)],
                            Stroke::new(1.0, self.palette.muted),
                        );
                    }
                }
                ui.painter().text(
                    rect.center_bottom() - vec2(0.0, 8.0),
                    Align2::CENTER_BOTTOM,
                    columns.to_string(),
                    FontId::proportional(12.0),
                    self.palette.text,
                );
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        true,
                        format!("{columns} columns"),
                    )
                });
                if response.clicked() {
                    self.session
                        .editor
                        .transaction(|e| e.doc.page.columns = columns);
                    self.refocus = true;
                    ui.close();
                }
            }
        });
        if menu(ui, "More options…", "", true) {
            self.queue(Action::Page);
        }
    }
    fn indent(&mut self, delta: f32) {
        self.session
            .editor
            .paragraph_format(|p| p.indent = (p.indent + delta).clamp(0.0, 288.0));
        self.refocus = true;
    }
    fn styles(&mut self, ui: &mut Ui) {
        for (h, label) in [
            (0, "Normal text"),
            (7, "Title"),
            (8, "Subtitle"),
            (1, "Heading 1"),
            (2, "Heading 2"),
            (3, "Heading 3"),
            (4, "Heading 4"),
            (5, "Heading 5"),
            (6, "Heading 6"),
        ] {
            if menu(ui, label, "", true) {
                self.session.editor.paragraph_format(|p| p.set_heading(h));
                self.refocus = true;
            }
        }
    }
    fn alignments(&mut self, ui: &mut Ui) {
        for (a, name, key) in [
            (Alignment::Left, "Left", "Ctrl+Shift+L"),
            (Alignment::Center, "Center", "Ctrl+Shift+E"),
            (Alignment::Right, "Right", "Ctrl+Shift+R"),
            (Alignment::Justified, "Justified", "Ctrl+Shift+J"),
        ] {
            if menu(ui, name, key, true) {
                self.session.editor.paragraph_format(|p| p.alignment = a);
                self.refocus = true;
            }
        }
    }
    fn lists(&mut self, ui: &mut Ui) {
        for (kind, name) in [
            (ListKind::None, "No list"),
            (ListKind::Bullet, "Bulleted list"),
            (ListKind::Number, "Numbered list"),
            (ListKind::Check, "Checklist"),
        ] {
            if menu(ui, name, "", true) {
                self.session.editor.paragraph_format(|p| p.list = kind);
                self.refocus = true;
            }
        }
    }
    fn spacing(&mut self, ui: &mut Ui) {
        let current = self
            .session
            .editor
            .doc
            .paragraph(self.session.editor.caret.address)
            .cloned()
            .unwrap_or_default();
        for (v, label) in [
            (1.0, "Single"),
            (1.15, "1.15"),
            (1.5, "1.5"),
            (2.0, "Double"),
        ] {
            if menu(ui, label, "", true) {
                self.session.editor.paragraph_format(|p| p.line_spacing = v);
                self.refocus = true;
            }
        }
        ui.separator();
        for (label, before, after) in [
            ("Add space before paragraph", 6.0, 0.0),
            ("Add space after paragraph", 0.0, 6.0),
            ("Remove paragraph spacing", -1.0, -1.0),
        ] {
            if menu(ui, label, "", true) {
                self.session.editor.paragraph_format(|p| {
                    if before < 0.0 {
                        p.before = 0.0;
                        p.after = 0.0;
                    } else {
                        p.before += before;
                        p.after += after;
                    }
                });
                self.refocus = true;
            }
        }
        if menu(ui, "Keep with next", "", true) {
            self.session
                .editor
                .paragraph_format(|p| p.keep_next = !p.keep_next);
            self.refocus = true;
        }
        if menu(
            ui,
            if current.keep_together {
                "✓ Keep lines together"
            } else {
                "Keep lines together"
            },
            "",
            true,
        ) {
            self.session
                .editor
                .paragraph_format(|p| p.keep_together = !current.keep_together);
            self.refocus = true;
        }
        if menu(
            ui,
            if current.widow_control {
                "✓ Prevent single lines"
            } else {
                "Prevent single lines"
            },
            "",
            true,
        ) {
            self.session
                .editor
                .paragraph_format(|p| p.widow_control = !current.widow_control);
            self.refocus = true;
        }
        if menu(ui, "Page break before", "", true) {
            self.session
                .editor
                .paragraph_format(|p| p.page_break_before = !p.page_break_before);
            self.refocus = true;
        }
    }
    fn table_commands(&mut self, ui: &mut Ui) {
        if table_commands(ui, &mut self.session.editor, self.session.viewing) {
            self.refocus = true;
            self.scroll_caret = true;
        }
    }
    fn table_picker(&mut self, ui: &mut Ui) {
        if matches!(
            self.session
                .editor
                .doc
                .blocks
                .get(self.session.editor.caret.address.block),
            Some(Block::Table(_))
        ) {
            ui.label("Select a paragraph outside the table to insert another table.");
            ui.separator();
            self.table_commands(ui);
            return;
        }
        ui.label("Insert table");
        if self.session.editor.caret.address.block == usize::MAX {
            ui.label("Tables can be inserted in the document body.");
            return;
        }
        let (rect, _) = ui.allocate_exact_size(vec2(220.0, 115.0), Sense::hover());
        let hovered = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| rect.contains(*p))
            .map(|p| {
                (
                    (((p.y - rect.top()) / 22.0) as usize + 1).min(5),
                    (((p.x - rect.left()) / 22.0) as usize + 1).min(10),
                )
            });
        let mut chosen = None;
        for row in 0usize..5 {
            for col in 0usize..10 {
                let hit = Rect::from_min_size(
                    rect.min + vec2(col as f32 * 22.0, row as f32 * 22.0),
                    vec2(22.0, 22.0),
                );
                let response = ui.interact(hit, Id::new(("table_size", row, col)), Sense::click());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        true,
                        format!("Insert table: {} columns, {} rows", col + 1, row + 1),
                    )
                });
                if response.clicked() {
                    chosen = Some((row + 1, col + 1));
                }
                let cell = Rect::from_min_size(
                    rect.min + vec2(col as f32 * 22.0, row as f32 * 22.0),
                    vec2(18.0, 18.0),
                );
                let selected =
                    hovered.is_some_and(|(r, c)| row < r && col < c) || response.has_focus();
                ui.painter().rect_filled(
                    cell,
                    0.0,
                    if selected {
                        self.palette.selection
                    } else {
                        self.palette.toolbar
                    },
                );
                ui.painter().rect_stroke(
                    cell,
                    0.0,
                    Stroke::new(
                        0.8,
                        if selected {
                            self.palette.accent
                        } else {
                            self.palette.border
                        },
                    ),
                    egui::StrokeKind::Inside,
                );
            }
        }
        ui.label(
            hovered
                .map(|(r, c)| format!("{c} × {r}"))
                .unwrap_or("Choose a size".into()),
        );
        if let Some((r, c)) = chosen {
            self.queue(Action::Table(r, c));
            ui.close();
        }
    }
    fn toolbar(&mut self, ui: &mut Ui) {
        let p = self.palette;
        self.toolbar_shell(ui, "toolbar_scroll", |app, ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (icon, label, a, enabled) in [
                (Icon::FindReplace, "Find and replace", Action::Find, true),
                (
                    Icon::Undo,
                    "Undo (Ctrl+Z)",
                    Action::Undo,
                    app.session.editor.can_undo() && !app.session.viewing,
                ),
                (
                    Icon::Redo,
                    "Redo (Ctrl+Y)",
                    Action::Redo,
                    app.session.editor.can_redo() && !app.session.viewing,
                ),
                (Icon::Print, "Print (Ctrl+P)", Action::Print, true),
                (
                    Icon::Proofing,
                    "Spelling and grammar check (Ctrl+Alt+X)",
                    Action::Proofing,
                    true,
                ),
            ] {
                if icons::button(ui, icon, label, false, enabled).clicked() && enabled {
                    app.queue(a);
                }
            }
            ui.separator();
            egui::ComboBox::from_id_salt("zoom")
                .selected_text(format!("{}%", (app.prefs.zoom * 100.0).round() as i32))
                .width(66.0)
                .show_ui(ui, |ui| {
                    for zoom in [0.5, 0.75, 0.9, 1.0, 1.25, 1.5, 2.0] {
                        ui.selectable_value(
                            &mut app.prefs.zoom,
                            zoom,
                            format!("{}%", (zoom * 100.0) as i32),
                        );
                    }
                });
            ui.separator();
            ui.add_enabled_ui(!app.session.viewing, |ui| {
                let h = app
                    .session
                    .editor
                    .doc
                    .paragraph(app.session.editor.caret.address)
                    .map_or(0, |p| p.heading);
                let style_label = match h {
                    0 => "Normal text".into(),
                    7 => "Title".into(),
                    8 => "Subtitle".into(),
                    n => format!("Heading {n}"),
                };
                dropdown(ui, &style_label, |ui| app.styles(ui));
                ui.separator();
                let mut font = app.session.editor.typing.font.clone();
                egui::ComboBox::from_id_salt("font")
                    .selected_text(&font)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .width(125.0)
                    .height(360.0)
                    .show_ui(ui, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut app.font_search)
                                .hint_text("Search installed fonts")
                                .desired_width(220.0),
                        );
                        ui.separator();
                        let filter = app.font_search.to_lowercase();
                        for name in fonts::families()
                            .iter()
                            .filter(|name| name.to_lowercase().contains(&filter))
                        {
                            if ui.selectable_value(&mut font, name.clone(), name).clicked() {
                                ui.close();
                            }
                        }
                    });
                if font != app.session.editor.typing.font {
                    app.session.editor.format(|s| s.font = font.clone());
                    app.refocus = true;
                }
                ui.separator();
                let mut size = app.session.editor.typing.size;
                if ui.small_button("−").clicked() {
                    size = (size - 1.0).max(1.0);
                }
                Frame::new()
                    .stroke(Stroke::new(1.0, p.muted))
                    .corner_radius(4)
                    .inner_margin(Margin::symmetric(2, 0))
                    .show(ui, |ui| {
                        ui.visuals_mut().button_frame = false;
                        ui.spacing_mut().interact_size = vec2(28.0, 24.0);
                        ui.spacing_mut().button_padding = vec2(3.0, 2.0);
                        ui.add(
                            egui::DragValue::new(&mut size)
                                .range(1.0..=400.0)
                                .custom_formatter(|n, _| {
                                    if n.fract().abs() < 0.001 {
                                        format!("{n:.0}")
                                    } else {
                                        format!("{n:.1}")
                                    }
                                })
                                .speed(0.5),
                        );
                    });
                if ui.small_button("+").clicked() {
                    size = (size + 1.0).min(400.0);
                }
                if size != app.session.editor.typing.size {
                    app.session.editor.format(|s| s.size = size);
                    app.refocus = true;
                }
                ui.separator();
                for (label, field) in [("B", 0), ("I", 1), ("U", 2)] {
                    let s = &app.session.editor.typing;
                    let value = match field {
                        0 => s.bold,
                        1 => s.italic,
                        _ => s.underline,
                    };
                    let text = match field {
                        0 => RichText::new(label).font(FontId::new(
                            14.0,
                            egui::FontFamily::Name("Sans-Bold".into()),
                        )),
                        1 => RichText::new(label).italics(),
                        _ => RichText::new(label).underline(),
                    }
                    .color(if value { p.accent } else { p.text });
                    if ui
                        .add(
                            egui::Button::new(text)
                                .selected(value)
                                .frame(false)
                                .min_size(vec2(30.0, 30.0)),
                        )
                        .on_hover_text(match field {
                            0 => "Bold (Ctrl+B)",
                            1 => "Italic (Ctrl+I)",
                            _ => "Underline (Ctrl+U)",
                        })
                        .clicked()
                    {
                        app.toggle_style(field);
                    }
                }
                let text_colour = icons::button(ui, Icon::TextColor, "Text colour", false, true);
                egui::Popup::menu(&text_colour).show(|ui| app.colours(ui, false));
                let highlight = icons::button(ui, Icon::Highlight, "Highlight colour", false, true);
                egui::Popup::menu(&highlight).show(|ui| app.colours(ui, true));
                ui.separator();
                if icons::button(ui, Icon::Link, "Insert link", false, true).clicked() {
                    app.queue(Action::Link);
                }
                if icons::button(ui, Icon::Image, "Insert image", false, true).clicked() {
                    app.queue(Action::Image);
                }
                if icons::button(
                    ui,
                    Icon::Comment,
                    "Add comment (Ctrl+Alt+M)",
                    false,
                    app.session.editor.selected(),
                )
                .clicked()
                {
                    app.queue(Action::Comment);
                }
                ui.separator();
                icon_menu(ui, Icon::AlignLeft, "Alignment", |ui| app.alignments(ui));
                icon_menu(ui, Icon::LineSpacing, "Line and paragraph spacing", |ui| {
                    app.spacing(ui)
                });
                for (icon, label, kind) in [
                    (Icon::Check, "Checklist", ListKind::Check),
                    (Icon::Bullet, "Bulleted list", ListKind::Bullet),
                    (Icon::Number, "Numbered list", ListKind::Number),
                ] {
                    icon_menu(ui, icon, label, |ui| {
                        if menu(ui, label, "", true) {
                            app.session.editor.paragraph_format(|p| p.list = kind);
                        }
                        if menu(ui, "Remove list", "", true) {
                            app.session
                                .editor
                                .paragraph_format(|p| p.list = ListKind::None);
                        }
                    });
                }
                if icons::button(ui, Icon::Outdent, "Decrease indent", false, true).clicked() {
                    app.indent(-36.0);
                }
                if icons::button(ui, Icon::Indent, "Increase indent", false, true).clicked() {
                    app.indent(36.0);
                }
                if icons::button(ui, Icon::Clear, "Clear formatting", false, true).clicked() {
                    app.queue(Action::Clear);
                }
            });
        });
    }
    fn colours(&mut self, ui: &mut Ui, highlight: bool) {
        ui.label(if highlight {
            "Highlight colour"
        } else {
            "Text colour"
        });
        let colours: [[u8; 3]; 24] = [
            [31, 31, 31],
            [95, 99, 104],
            [150, 150, 150],
            [210, 210, 210],
            [240, 240, 240],
            [255, 255, 255],
            [183, 28, 28],
            [230, 81, 0],
            [245, 180, 0],
            [56, 142, 60],
            [0, 137, 123],
            [11, 87, 208],
            [123, 31, 162],
            [194, 24, 91],
            [239, 154, 154],
            [255, 204, 128],
            [255, 245, 157],
            [165, 214, 167],
            [128, 203, 196],
            [144, 202, 249],
            [206, 147, 216],
            [244, 143, 177],
            [211, 227, 253],
            [232, 240, 254],
        ];
        egui::Grid::new(if highlight {
            "highlight_palette"
        } else {
            "text_palette"
        })
        .spacing(vec2(4.0, 4.0))
        .show(ui, |ui| {
            for (i, c) in colours.iter().enumerate() {
                if ui
                    .add(
                        egui::Button::new("")
                            .fill(layout::color(*c))
                            .min_size(vec2(26.0, 26.0)),
                    )
                    .clicked()
                {
                    self.session.editor.format(|s| {
                        if highlight {
                            s.highlight = Some(*c);
                        } else {
                            s.color = *c;
                        }
                    });
                    self.refocus = true;
                    ui.close();
                }
                if (i + 1) % 6 == 0 {
                    ui.end_row();
                }
            }
        });
        if highlight && ui.button("None").clicked() {
            self.session.editor.format(|s| s.highlight = None);
            self.refocus = true;
            ui.close();
        }
    }
}

impl AlphaDocs {
    fn editor_ui(&mut self, ui: &mut Ui) {
        self.refresh_content_cache();
        let p = self.palette;
        let ctx = ui.ctx().clone();
        egui::Panel::top("editor_chrome")
            .frame(Frame::new().fill(p.bg))
            .show(ui, |ui| {
                if !self.prefs.compact_menus {
                    self.header(ui);
                }
                self.collaboration_strip(ui);
                Frame::new()
                    .inner_margin(Margin {
                        left: 16,
                        right: 16,
                        top: 0,
                        bottom: 6,
                    })
                    .show(ui, |ui| {
                        if self.image_block.is_some() {
                            self.image_toolbar(ui);
                        } else {
                            self.toolbar(ui);
                        }
                    });
            });
        egui::Panel::bottom("status")
            .exact_size(28.0)
            .frame(
                Frame::new()
                    .fill(p.bg)
                    .inner_margin(Margin::symmetric(18, 4)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let pages = self.layout.as_ref().map_or(1, |l| l.pages);
                    let page = self
                        .layout
                        .as_ref()
                        .and_then(|l| l.caret_line(self.session.editor.caret))
                        .map_or(1, |l| l.page + 1);
                    ui.label(
                        RichText::new(format!("Page {page} of {pages}"))
                            .size(12.0)
                            .color(p.muted),
                    );
                    if self.prefs.counts {
                        ui.separator();
                        let words = self.content_cache.counts.0;
                        if ui
                            .link(RichText::new(format!("{words} words")).size(12.0))
                            .clicked()
                        {
                            self.queue(Action::Count);
                        }
                    }
                    ui.with_layout(UiLayout::right_to_left(Align::Center), |ui| {
                        if self.update_state.available.is_some()
                            && ui.link("Update available").clicked()
                        {
                            self.queue(Action::CheckUpdates);
                        }
                        ui.label(
                            RichText::new(if self.session.viewing {
                                "Viewing"
                            } else {
                                if self.session.dirty() {
                                    if self.prefs.autosave {
                                        "Changes pending autosave"
                                    } else {
                                        "Unsaved changes · Autosave off"
                                    }
                                } else {
                                    "Saved on this computer"
                                }
                            })
                            .size(12.0)
                            .color(p.muted),
                        );
                        let connection = self
                            .session
                            .collaboration
                            .as_ref()
                            .and_then(|s| s.room.lock().ok().map(|r| r.status.clone()));
                        if let Some(connection) = connection
                            && ui
                                .link(
                                    RichText::new(truncate(&connection, 70))
                                        .size(12.0)
                                        .color(p.accent),
                                )
                                .on_hover_text(&connection)
                                .clicked()
                        {
                            self.queue(Action::Share);
                        }
                    });
                });
            });
        if self.prepare_fonts(&ctx) {
            self.layout_revision = u64::MAX;
            ctx.request_discard("Activate selected font face");
            return;
        }
        self.document_tabs_panel(ui);
        egui::Panel::right("utility_rail")
            .exact_size(44.0)
            .resizable(false)
            .frame(
                Frame::new()
                    .fill(p.bg)
                    .inner_margin(Margin::symmetric(6, 12)),
            )
            .show(ui, |ui| {
                for (icon, label, action) in [
                    (Icon::FindReplace, "Find and replace", Action::Find),
                    (Icon::Document, "Word count", Action::Count),
                    (Icon::History, "Version history", Action::History),
                    (Icon::Theme, "Appearance", Action::Preferences),
                ] {
                    if icons::button(ui, icon, label, false, true).clicked() {
                        self.queue(action);
                    }
                    ui.add_space(10.0);
                }
                if icons::button(
                    ui,
                    Icon::Outline,
                    "Toggle document outline",
                    self.prefs.outline,
                    true,
                )
                .clicked()
                {
                    self.prefs.outline = !self.prefs.outline;
                }
            });
        self.image_sidebar(ui);
        self.comments_sidebar(ui);
        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(p.bg)
                    .inner_margin(Margin::symmetric(16, 0)),
            )
            .show(ui, |ui| {
                self.ensure_layout(&ctx);
                self.canvas(ui);
                self.comment_cards(ui);
            });
    }
    fn canvas(&mut self, ui: &mut Ui) {
        self.feature_destination(feature_search::Target::Canvas, ui.max_rect());
        let p = self.palette;
        let ctx = ui.ctx().clone();
        let scale = 4.0 / 3.0 * self.prefs.zoom;
        let proof_range = self.proofing_range();
        let live_issues = self.live_proof.current(&self.session.editor);
        let l = self.layout.as_ref().expect("Measured layout");
        let w = l.size.x * scale;
        let h = l.size.y * scale;
        let gap = 12.0;
        let ruler = if self.prefs.ruler { 24.0 } else { 0.0 };
        let page_setup = self.session.editor.doc.page.clone();
        let mut target = None;
        let mut image_clicked = None;
        let mut image_action = None;
        let mut image_move = None;
        let mut image_context = None;
        let mut margin_change = None;
        let mut check_change = None;
        let mut table_move = None;
        let mut annotation_clicked = None;
        self.annotation_positions.clear();
        self.selection_annotation_rect = None;
        let show_annotations = !self.image_options_open
            && (self.comments_open || comments::has_annotations(&self.session.editor.doc));
        let rail_width = if show_annotations { 440.0 } else { 0.0 };
        let salt = ("pages", &self.session.editor.doc.id);
        let mut scroll = egui::ScrollArea::both()
            .id_salt(salt)
            .auto_shrink([false, false]);
        let mut jumped = false;
        if self.scroll_caret
            && let Some(line) = l.caret_line(self.session.editor.caret)
        {
            let offset = egui::scroll_area::State::load(&ctx, ui.make_persistent_id(salt))
                .map_or(0.0, |s| s.offset.y);
            let top = ruler + 12.0 + line.page as f32 * (h + gap) + line.rect.top() * scale;
            let bottom = top + line.rect.height() * scale;
            let viewport = ui.available_height();
            if top < offset + 24.0 || bottom > offset + viewport - 24.0 {
                let destination = if top < offset + 24.0 {
                    top - 24.0
                } else {
                    bottom - viewport + 24.0
                };
                scroll = scroll.vertical_scroll_offset(destination.max(0.0));
                jumped = true;
            }
        }
        scroll.show(ui, |ui| {
            let size = vec2(
                (w + 48.0 + rail_width).max(ui.available_width()),
                (h + gap) * l.pages as f32 + 32.0 + ruler,
            );
            let (area, _) = ui.allocate_exact_size(size, Sense::hover());
            let response = ui.interact(area, Id::new("document_canvas"), Sense::click_and_drag());
            let left = (area.left() + (size.x - w - rail_width) * 0.5
                - if self.prefs.outline {
                    if show_annotations { 60.0 } else { 130.0 }
                } else {
                    0.0
                })
            .max(area.left() + 24.0);
            let start_y = area.top() + ruler + 12.0;
            self.annotation_rail_x = left + w + 24.0;
            let canvas_id = Id::new("document_canvas");
            ctx.memory_mut(|m| {
                m.set_focus_lock_filter(
                    canvas_id,
                    egui::EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        ..Default::default()
                    },
                )
            });
            response.widget_info(|| {
                let mut info = egui::WidgetInfo::text_edit(
                    !self.session.viewing,
                    "",
                    self.content_cache.text.clone(),
                    "Document editing surface",
                );
                info.label = Some("Document editing surface".into());
                info
            });
            if self.refocus && self.dialog.is_none() {
                // Closing a popup can restore its old focus at the end of
                // the pass. Keep requesting until a later pass confirms
                // that the document actually retained keyboard focus.
                if ctx.memory(|m| m.has_focus(canvas_id)) {
                    self.refocus = false;
                } else {
                    response.request_focus();
                }
            }
            let origin = |page: usize| pos2(left, start_y + page as f32 * (h + gap));
            if self.prefs.ruler {
                let rr = Rect::from_min_size(pos2(left, area.top()), vec2(w, 24.0));
                ui.painter().rect_filled(rr, 0.0, p.bg);
                let m = page_setup.margins;
                ui.painter().rect_filled(
                    Rect::from_min_size(rr.min, vec2(m[2] * scale, 24.0)),
                    0.0,
                    p.toolbar,
                );
                ui.painter().rect_filled(
                    Rect::from_min_size(
                        pos2(rr.right() - m[3] * scale, rr.top()),
                        vec2(m[3] * scale, 24.0),
                    ),
                    0.0,
                    p.toolbar,
                );
                for tick in 0..=((l.size.x / 9.0) as usize) {
                    let x = left + tick as f32 * 9.0 * scale;
                    let full = tick % 8 == 0;
                    ui.painter().line_segment(
                        [
                            pos2(x, rr.bottom() - if full { 10.0 } else { 5.0 }),
                            pos2(x, rr.bottom()),
                        ],
                        Stroke::new(0.6, p.muted),
                    );
                    if full {
                        ui.painter().text(
                            pos2(x + 2.0, rr.top() + 1.0),
                            Align2::LEFT_TOP,
                            (tick / 8).to_string(),
                            FontId::proportional(10.0),
                            p.muted,
                        );
                    }
                }
                for (i, x) in [left + m[2] * scale, left + w - m[3] * scale]
                    .into_iter()
                    .enumerate()
                {
                    ui.painter().add(egui::Shape::convex_polygon(
                        vec![
                            pos2(x - 5.0, rr.top() + 13.0),
                            pos2(x + 5.0, rr.top() + 13.0),
                            pos2(x, rr.top() + 19.0),
                        ],
                        p.accent,
                        Stroke::NONE,
                    ));
                    let drag = ui
                        .interact(
                            Rect::from_center_size(pos2(x, rr.top() + 15.0), vec2(14.0, 22.0)),
                            Id::new(("margin", i)),
                            Sense::drag(),
                        )
                        .on_hover_text("Drag to adjust margin");
                    if drag.dragged() && !self.session.viewing {
                        let idx = if i == 0 { 2 } else { 3 };
                        let delta = drag.drag_delta().x / scale;
                        let mut setup = page_setup.clone();
                        setup.margins[idx] =
                            (m[idx] + if i == 0 { delta } else { -delta }).max(0.0);
                        if setup.valid() {
                            margin_change = Some(setup);
                        }
                    }
                }
            }
            // Resolve navigation from measured geometry before visibility culling.
            // A heading on another page must be reachable without painting it first.
            let mut caret_rect = l.caret_line(self.session.editor.caret).map(|line| {
                let o = origin(line.page);
                Rect::from_min_size(
                    o + line.rect.min.to_vec2() * scale
                        + vec2(line.x(self.session.editor.caret.offset) * scale, 0.0),
                    vec2(1.2, line.rect.height() * scale),
                )
            });
            let (s, e) = self.session.editor.range();
            for page in 0..l.pages {
                let mut front_images = vec![];
                let o = origin(page);
                let paper = Rect::from_min_size(o, vec2(w, h));
                if !ui.clip_rect().intersects(paper) {
                    continue;
                }
                ui.painter().rect_filled(
                    paper.translate(vec2(1.0, 2.0)),
                    0.0,
                    Color32::from_black_alpha(if p.dark { 50 } else { 10 }),
                );
                ui.painter().rect_filled(
                    paper,
                    0.0,
                    if page_setup.color == [255; 3] {
                        p.paper
                    } else {
                        layout::color(page_setup.color)
                    },
                );
                ui.painter().rect_stroke(
                    paper,
                    0.0,
                    Stroke::new(0.7, p.border),
                    egui::StrokeKind::Inside,
                );
                if self.prefs.ruler {
                    for tick in 0..=(l.size.y / 18.0) as usize {
                        let y = o.y + tick as f32 * 18.0 * scale;
                        ui.painter().line_segment(
                            [
                                pos2(left - 12.0, y),
                                pos2(left - if tick % 4 == 0 { 22.0 } else { 17.0 }, y),
                            ],
                            Stroke::new(0.5, p.border),
                        );
                    }
                }
                for object in &l.objects {
                    match object {
                        Object::Image {
                            block,
                            page: pn,
                            rect,
                            ..
                        } if *pn == page => {
                            let Block::Image(image) = &self.session.editor.doc.blocks[*block]
                            else {
                                continue;
                            };
                            let r = Rect::from_min_size(
                                o + rect.min.to_vec2() * scale,
                                rect.size() * scale,
                            );
                            self.annotation_positions
                                .entry(image.id.clone())
                                .or_insert(r);
                            if !self.image_options_open
                                && (image.comments.iter().any(|c| !c.resolved)
                                    || !image.reactions.is_empty())
                            {
                                ui.painter().rect_stroke(
                                    r,
                                    0.0,
                                    Stroke::new(1.2, comments::highlight(p, true)),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            let r =
                                image_controls::preview_rect(r, *block, image, &self.image_resize);
                            let r = image_move::preview_rect(r, *block, &self.image_drag);
                            let effects = self
                                .image_effects_preview
                                .filter(|(b, _)| *b == *block)
                                .map(|(_, e)| e)
                                .unwrap_or(image.effects);
                            if self.textures.get(block).is_none_or(|(data, cached, _)| {
                                *data != image.data || *cached != effects
                            }) && let Ok(decoded) = image::load_from_memory(&image.data)
                            {
                                let mut im = decoded.to_rgba8();
                                crate::image_edit::apply_effects(&mut im, effects);
                                let color = egui::ColorImage::from_rgba_unmultiplied(
                                    [im.width() as usize, im.height() as usize],
                                    im.as_raw(),
                                );
                                self.textures.insert(
                                    *block,
                                    (
                                        image.data.clone(),
                                        effects,
                                        ctx.load_texture(
                                            format!("image_{block}"),
                                            color,
                                            egui::TextureOptions::LINEAR,
                                        ),
                                    ),
                                );
                            }
                            if let Some((_, _, t)) = self.textures.get(block) {
                                let [left, top, right, bottom] = image.visible_crop();
                                let [inset_x, inset_y] = image.content_inset_fraction();
                                let content_rect =
                                    r.shrink2(vec2(r.width() * inset_x, r.height() * inset_y));
                                if image.placement.flow == ImageFlow::Front {
                                    front_images.push((
                                        t.id(),
                                        content_rect,
                                        Rect::from_min_max(pos2(left, top), pos2(right, bottom)),
                                    ));
                                } else {
                                    ui.painter().image(
                                        t.id(),
                                        content_rect,
                                        Rect::from_min_max(pos2(left, top), pos2(right, bottom)),
                                        Color32::WHITE,
                                    );
                                }
                            }
                            let pointer_hit = ui.input(|i| {
                                i.events
                                    .iter()
                                    .find_map(|event| {
                                        if let Event::PointerButton {
                                            pos, pressed: true, ..
                                        } = event
                                        {
                                            Some(*pos)
                                        } else {
                                            None
                                        }
                                    })
                                    .or_else(|| i.pointer.interact_pos())
                            });
                            let text_hit = pointer_hit.is_some_and(|pointer| {
                                image_move::hit_image(
                                    l,
                                    &self.session.editor.doc,
                                    &origin,
                                    scale,
                                    pointer,
                                    ui.input(|i| i.modifiers.alt),
                                ) != Some(*block)
                            });
                            let image_response = ui.interact(
                                r,
                                Id::new(("image", block)),
                                if text_hit {
                                    Sense::hover()
                                } else {
                                    Sense::click()
                                },
                            );
                            // The drag widget overlays the image response. Use the
                            // shared pointer click so it cannot swallow context clicks.
                            let secondary =
                                !text_hit && pointer_gesture::secondary_clicked_in(ui, r);
                            let mut popup = egui::Popup::context_menu(&image_response);
                            if secondary {
                                popup = popup.open_memory(Some(egui::SetOpenCommand::Bool(true)));
                            }
                            popup.show(|ui| {
                                let mut action = image_controls::ImageAction::default();
                                image_controls::flow_menu(
                                    ui,
                                    image.placement.flow,
                                    &mut action,
                                    !self.session.viewing,
                                );
                                ui.separator();
                                if menu(ui, "Image options", "", true) {
                                    action.options = true;
                                }
                                if menu(ui, "Add comment", "Ctrl+Alt+M", !self.session.viewing) {
                                    action.comment = true;
                                }
                                if menu(ui, "Add emoji reaction", "", !self.session.viewing) {
                                    action.reaction = true;
                                }
                                if menu(ui, "Delete image", "Delete", !self.session.viewing) {
                                    action.delete = true;
                                }
                                image_context = Some((*block, action));
                            });
                            if image_response.clicked()
                                || !text_hit
                                    && pointer_gesture::pressed_in(ui, r.shrink(12.0)).is_some()
                                || secondary
                            {
                                image_clicked = Some(*block);
                            }
                            if self.image_block == Some(*block) {
                                image_action = Some((
                                    *block,
                                    image_controls::show(
                                        ui,
                                        *block,
                                        image_controls::SelectionBounds {
                                            rect: r,
                                            moving: self.image_drag.is_some(),
                                        },
                                        image,
                                        &mut self.image_resize,
                                        self.session.viewing,
                                        p,
                                    ),
                                ));
                            }
                        }
                        Object::Border { page: pn, rect, .. } if *pn == page => {
                            ui.painter().rect_stroke(
                                Rect::from_min_size(
                                    o + rect.min.to_vec2() * scale,
                                    rect.size() * scale,
                                ),
                                0.0,
                                Stroke::new(0.7, p.border),
                                egui::StrokeKind::Inside,
                            );
                        }
                        Object::Rule { page: pn, rect } if *pn == page => {
                            ui.painter().line_segment(
                                [
                                    o + rect.min.to_vec2() * scale,
                                    o + rect.max.to_vec2() * scale,
                                ],
                                Stroke::new(1.0, p.border),
                            );
                        }
                        _ => {}
                    }
                }
                let remote_selections = self.session.editor.remote_selections();
                for line in l
                    .lines
                    .iter()
                    .chain(&l.running)
                    .filter(|line| line.page == page)
                {
                    let pos = o + line.rect.min.to_vec2() * scale;
                    let rect = Rect::from_min_size(pos, line.rect.size() * scale);
                    if !ui.clip_rect().intersects(rect) {
                        continue;
                    }
                    let bg = if page_setup.color == [255; 3] {
                        [p.paper.r(), p.paper.g(), p.paper.b()]
                    } else {
                        page_setup.color
                    };
                    let display =
                        layout::display_line(&ctx, &self.session.editor.doc, line, scale, bg);
                    if !self.image_options_open
                        && let Some(para) = self.session.editor.doc.paragraph(line.address)
                    {
                        for note in &para.annotations {
                            if note.comments.iter().all(|c| c.resolved)
                                && note.reactions.is_empty()
                                && !self.comments_resolved
                            {
                                continue;
                            }
                            let begin = note.start.max(line.start);
                            let end = note.end.min(line.end);
                            if end > begin
                                || note.start == note.end
                                    && note.start >= line.start
                                    && note.start <= line.end
                            {
                                let range = Rect::from_min_max(
                                    pos + vec2(layout::display_x(&display, line.start, begin), 0.0),
                                    pos + vec2(
                                        layout::display_x(&display, line.start, end.max(begin)),
                                        rect.height(),
                                    ),
                                );
                                self.annotation_positions
                                    .entry(note.id.clone())
                                    .or_insert(range);
                                let active = self
                                    .active_comment
                                    .as_ref()
                                    .is_some_and(|(id, _)| id == &note.id);
                                ui.painter().rect_filled(
                                    range,
                                    0.0,
                                    comments::highlight(p, active),
                                );
                                if pointer_gesture::clicked_in(ui, range) {
                                    annotation_clicked = note
                                        .comments
                                        .iter()
                                        .find(|c| !c.resolved)
                                        .map(|c| (note.id.clone(), c.id.clone()));
                                }
                            }
                        }
                    }
                    if self.session.editor.selected()
                        && line.address >= s.address
                        && line.address <= e.address
                    {
                        let begin = if line.address == s.address {
                            s.offset.max(line.start)
                        } else {
                            line.start
                        };
                        let end = if line.address == e.address {
                            e.offset.min(line.end)
                        } else {
                            line.end
                        };
                        if end > begin {
                            let selected_rect = Rect::from_min_max(
                                pos + vec2(layout::display_x(&display, line.start, begin), 0.0),
                                pos + vec2(
                                    layout::display_x(&display, line.start, end),
                                    rect.height(),
                                ),
                            );
                            self.selection_annotation_rect = Some(selected_rect);
                            if self.text_comment_range.is_some() {
                                self.annotation_positions
                                    .entry("text-draft".into())
                                    .or_insert(selected_rect);
                            }
                            ui.painter().rect_filled(
                                Rect::from_min_max(
                                    pos + vec2(layout::display_x(&display, line.start, begin), 0.0),
                                    pos + vec2(
                                        layout::display_x(&display, line.start, end),
                                        rect.height(),
                                    ),
                                ),
                                0.0,
                                if proof_range == Some((s, e)) {
                                    comments::highlight(p, true)
                                } else {
                                    p.selection
                                },
                            );
                        }
                    }
                    sharing::paint_presence(
                        ui,
                        &remote_selections,
                        line,
                        &display,
                        pos,
                        rect.height(),
                    );
                    ui.painter().galley(pos, display.clone(), p.text);
                    live_proofing::paint(ui, &live_issues, line, &display, pos, rect.height());
                    if let Some(marker) = &line.marker {
                        ui.painter().text(
                            pos - vec2(18.0 * scale, 0.0),
                            Align2::LEFT_TOP,
                            marker,
                            FontId::proportional(11.0 * scale),
                            p.text,
                        );
                        if (marker == "☐" || marker == "☑")
                            && !self.session.viewing
                            && ui
                                .interact(
                                    Rect::from_min_size(
                                        pos - vec2(20.0 * scale, 0.0),
                                        vec2(18.0 * scale, rect.height()),
                                    ),
                                    Id::new(("check", line.address)),
                                    Sense::click(),
                                )
                                .clicked()
                        {
                            check_change = Some(line.address);
                        }
                    }
                    if self.nonprinting
                        && self
                            .session
                            .editor
                            .doc
                            .paragraph(line.address)
                            .is_some_and(|para| {
                                line.end == para.chars.len()
                                    || para.chars.get(line.end).is_some_and(|c| c.ch == '\n')
                            })
                    {
                        ui.painter().text(
                            pos + vec2(line.rect.width() * scale + 3.0, 0.0),
                            Align2::LEFT_TOP,
                            "¶",
                            FontId::proportional(10.0 * scale),
                            p.muted,
                        );
                    }
                    if self.image_block.is_none()
                        && line.address == self.session.editor.caret.address
                        && self.session.editor.caret.offset >= line.start
                        && self.session.editor.caret.offset <= line.end
                    {
                        caret_rect = Some(Rect::from_min_size(
                            pos + vec2(
                                layout::display_x(
                                    &display,
                                    line.start,
                                    self.session.editor.caret.offset,
                                ),
                                0.0,
                            ),
                            vec2(1.2, rect.height()),
                        ));
                    }
                }
                for (texture, rect, uv) in front_images {
                    ui.painter().image(texture, rect, uv, Color32::WHITE);
                }
                if page_setup.page_numbers {
                    ui.painter().text(
                        o + vec2(w * 0.5, h - page_setup.margins[1] * scale * 0.5),
                        Align2::CENTER_CENTER,
                        (page + 1).to_string(),
                        FontId::proportional(9.0 * scale),
                        p.muted,
                    );
                }
            }
            if let Some(r) = caret_rect {
                if ctx.memory(|m| m.has_focus(canvas_id)) && !self.session.viewing {
                    ctx.output_mut(|o| {
                        o.ime = Some(egui::output::IMEOutput {
                            rect: r,
                            cursor_rect: r,
                            purpose: egui::IMEPurpose::Normal,
                            should_interrupt_composition: false,
                        })
                    });
                    if ctx.input(|i| i.time % 1.1 < 0.65) {
                        ui.painter().rect_filled(r, 0.0, p.text);
                    }
                    if !self.ime_preedit.is_empty() {
                        ui.painter().text(
                            r.right_top(),
                            Align2::LEFT_TOP,
                            &self.ime_preedit,
                            FontId::proportional(self.session.editor.typing.size * scale),
                            p.accent,
                        );
                    }
                    ctx.request_repaint_after(Duration::from_millis(450));
                }
                if self.scroll_caret {
                    if !jumped {
                        ui.scroll_to_rect(r.expand(24.0), None);
                    }
                    self.scroll_caret = false;
                }
            }
            if response.secondary_clicked()
                && let Some(pointer) = response.interact_pointer_pos()
            {
                self.live_proof.context = None;
                let page = (((pointer.y - start_y) / (h + gap)).floor().max(0.0) as usize)
                    .min(l.pages - 1);
                let point = pos2(
                    (pointer.x - left) / scale,
                    (pointer.y - origin(page).y) / scale,
                );
                if let Some(line) = l.hit_line(page, point) {
                    let bg = if page_setup.color == [255; 3] {
                        [p.paper.r(), p.paper.g(), p.paper.b()]
                    } else {
                        page_setup.color
                    };
                    let display =
                        layout::display_line(&ctx, &self.session.editor.doc, line, scale, bg);
                    let pos = Position {
                        address: line.address,
                        offset: layout::display_hit(
                            &display,
                            line.start,
                            line.end,
                            (point.x - line.rect.left()) * scale,
                        ),
                    };
                    // A table command targets the cell under the context click,
                    // even when the insertion caret was in another table.
                    if pos.address != self.session.editor.caret.address {
                        self.session.editor.place(pos, false);
                    }
                    if line.rect.contains(point) {
                        self.live_proof.context = live_issues
                            .iter()
                            .find(|issue| {
                                issue.address == pos.address
                                    && pos.offset >= issue.start
                                    && pos.offset < issue.end
                            })
                            .cloned()
                            .map(|mut issue| {
                                if !issue.grammar {
                                    issue.suggestions =
                                        crate::proofing::suggestions(&issue.original)
                                            .unwrap_or_default();
                                }
                                issue
                            });
                    }
                }
            }
            table_move = table_controls::show(
                ui,
                l,
                &origin,
                scale,
                (
                    self.session.editor.revision,
                    self.session.editor.caret.address.block,
                ),
                &mut self.table_drag,
                self.session.viewing,
            );
            image_move = image_move::show(
                ui,
                l,
                &origin,
                scale,
                &self.session.editor,
                &mut self.image_drag,
                self.session.viewing,
            );
            let text_press = pointer_gesture::pressed_in(ui, response.rect).filter(|pointer| {
                image_move::hit_image(
                    l,
                    &self.session.editor.doc,
                    &origin,
                    scale,
                    *pointer,
                    ui.input(|i| i.modifiers.alt),
                )
                .is_none()
            });
            if (response.clicked()
                || response.drag_started()
                || response.dragged()
                || text_press.is_some())
                && image_clicked.is_none()
                && self.image_resize.is_none()
                && self.image_drag.is_none()
                && image_move.is_none()
                && self.table_drag.is_none()
                && table_move.is_none()
                && let Some(pointer) = text_press.or_else(|| response.interact_pointer_pos())
            {
                let page = (((pointer.y - start_y) / (h + gap)).floor().max(0.0) as usize)
                    .min(l.pages - 1);
                let point = pos2(
                    (pointer.x - left) / scale,
                    (pointer.y - origin(page).y) / scale,
                );
                if let Some(line) = l.hit_line(page, point) {
                    let bg = if page_setup.color == [255; 3] {
                        [p.paper.r(), p.paper.g(), p.paper.b()]
                    } else {
                        page_setup.color
                    };
                    let display =
                        layout::display_line(&ctx, &self.session.editor.doc, line, scale, bg);
                    let pos = Position {
                        address: line.address,
                        offset: layout::display_hit(
                            &display,
                            line.start,
                            line.end,
                            (point.x - line.rect.left()) * scale,
                        ),
                    };
                    self.image_block = None;
                    self.image_selection_id = None;
                    target = Some((
                        pos,
                        response.dragged() && !response.drag_started()
                            || ctx.input(|i| i.modifiers.shift),
                        response.double_clicked(),
                        response.triple_clicked(),
                    ));
                    ctx.memory_mut(|m| m.request_focus(canvas_id));
                    if text_press.is_some() && ui.input(|i| i.pointer.primary_down()) {
                        ctx.set_dragged_id(canvas_id);
                    }
                }
            }
            if response.hovered()
                && self.image_resize.is_none()
                && self.image_drag.is_none()
                && self.image_block.is_none()
            {
                ctx.set_cursor_icon(egui::CursorIcon::Text);
            }
            if response.dragged()
                && let Some(pointer) = response.interact_pointer_pos()
            {
                let delta = if pointer.y < ui.clip_rect().top() + 24.0 {
                    16.0
                } else if pointer.y > ui.clip_rect().bottom() - 24.0 {
                    -16.0
                } else {
                    0.0
                };
                if delta != 0.0 {
                    ui.scroll_with_delta(vec2(0.0, delta));
                    ctx.request_repaint_after(Duration::from_millis(16));
                }
            }
            response.context_menu(|ui| {
                let (corrected, open_proof) = live_proofing::context_menu(
                    ui,
                    &mut self.live_proof,
                    &mut self.session.editor,
                    &mut self.prefs,
                    &mut self.status,
                    self.session.viewing,
                );
                if corrected {
                    self.refocus = true;
                    self.scroll_caret = true;
                }
                if open_proof {
                    self.action = Some(Action::Proofing);
                }
                if matches!(
                    self.session
                        .editor
                        .doc
                        .blocks
                        .get(self.session.editor.caret.address.block),
                    Some(Block::Table(_))
                ) {
                    if table_commands(ui, &mut self.session.editor, self.session.viewing) {
                        self.refocus = true;
                        self.scroll_caret = true;
                    }
                    ui.separator();
                }
                if menu(
                    ui,
                    "Cut",
                    "Ctrl+X",
                    self.session.editor.selected() && !self.session.viewing,
                ) {
                    self.action = Some(Action::Cut);
                }
                if menu(ui, "Copy", "Ctrl+C", self.session.editor.selected()) {
                    self.action = Some(Action::Copy);
                }
                if menu(ui, "Paste", "Ctrl+V", !self.session.viewing) {
                    self.action = Some(Action::Paste);
                }
                ui.separator();
                if menu(ui, "Insert link", "Ctrl+K", !self.session.viewing) {
                    self.action = Some(Action::Link);
                }
                if menu(ui, "Page setup", "", !self.session.viewing) {
                    self.action = Some(Action::Page);
                }
            });
        });
        if let Some((mut pos, extend, double, triple)) = target
            && let Some(para) = self.session.editor.doc.paragraph(pos.address)
        {
            if (self.session.viewing || ctx.input(|i| i.modifiers.command))
                && let Some(url) = para
                    .chars
                    .get(pos.offset)
                    .and_then(|c| c.style.link.clone())
                && (url.starts_with("https://")
                    || url.starts_with("http://")
                    || url.starts_with("mailto:"))
            {
                ctx.open_url(egui::OpenUrl::new_tab(url));
                return;
            }
            let text = para.text();
            let byte = text
                .char_indices()
                .nth(pos.offset)
                .map_or(text.len(), |(i, _)| i);
            if double {
                for (i, word) in text.split_word_bound_indices() {
                    if byte >= i && byte < i + word.len() {
                        self.session.editor.anchor = Position {
                            address: pos.address,
                            offset: text[..i].chars().count(),
                        };
                        pos.offset = self.session.editor.anchor.offset + word.chars().count();
                        self.session.editor.caret = pos;
                        break;
                    }
                }
            } else if triple {
                self.session.editor.anchor = Position {
                    address: pos.address,
                    offset: 0,
                };
                self.session.editor.caret = Position {
                    address: pos.address,
                    offset: para.chars.len(),
                };
            } else {
                let boundaries: Vec<usize> = text
                    .grapheme_indices(true)
                    .map(|(i, _)| text[..i].chars().count())
                    .chain(std::iter::once(para.chars.len()))
                    .collect();
                pos.offset = boundaries
                    .into_iter()
                    .min_by_key(|i| i.abs_diff(pos.offset))
                    .unwrap_or(0);
                self.session.editor.place(pos, extend);
            }
        }
        if let Some(setup) = margin_change {
            self.session
                .editor
                .transaction(|e| e.doc.page = setup.clone());
        }
        if let Some((block, before)) = table_move {
            self.session.editor.move_table(block, before);
            self.refocus = true;
            self.scroll_caret = true;
            ctx.request_repaint();
        }
        if let Some(movement) = image_move {
            match movement {
                image_move::Move::Text(block, target, placement) => {
                    if let Some(destination) = self
                        .session
                        .editor
                        .move_image_to_text(block, target, placement)
                        && let Block::Image(i) = &self.session.editor.doc.blocks[destination]
                    {
                        self.image_block = Some(destination);
                        self.image_selection_id = Some(i.id.clone());
                        image_clicked = None;
                    }
                }
                image_move::Move::Block(block, before, placement) => {
                    if let Some(destination) = self
                        .session
                        .editor
                        .move_image_with_placement(block, before, placement)
                        && let Block::Image(i) = &self.session.editor.doc.blocks[destination]
                    {
                        self.image_block = Some(destination);
                        self.image_selection_id = Some(i.id.clone());
                        image_clicked = None;
                    }
                }
                image_move::Move::Position(block, placement) => {
                    self.session.editor.set_image_placement(block, placement);
                }
            }
            self.refocus = true;
            ctx.request_repaint();
        }
        if let Some(address) = check_change {
            self.session.editor.transaction(|e| {
                if let Some(p) = e.doc.paragraph_mut(address) {
                    p.checked = !p.checked;
                }
            });
        }
        if let Some(thread) = annotation_clicked {
            self.active_comment = Some(thread);
        }
        if let Some(block) = image_clicked
            && let Block::Image(i) = &self.session.editor.doc.blocks[block]
        {
            self.image_effects_preview = None;
            self.image_block = Some(block);
            self.image_selection_id = Some(i.id.clone());
            self.image_resize = None;
            self.session.editor.anchor = self.session.editor.caret;
            ctx.memory_mut(|m| m.request_focus(Id::new("document_canvas")));
            ctx.request_repaint();
        }
        if let Some((block, action)) = image_context.or(image_action) {
            if let Some(size) = action.size {
                self.session.editor.resize_image(block, size.x, size.y);
                ctx.request_repaint();
            }
            if action.options {
                self.image_options_open = true;
                self.comments_open = false;
            }
            if let Some(flow) = action.flow
                && !self.session.viewing
                && let Block::Image(i) = &self.session.editor.doc.blocks[block]
            {
                let mut placement = i.placement;
                placement.flow = flow;
                self.session.editor.set_image_placement(block, placement);
            }
            if action.comment {
                self.begin_comment();
            }
            if action.reaction
                && let Block::Image(i) = &self.session.editor.doc.blocks[block]
            {
                self.emoji_reaction_target = Some(i.id.clone());
                self.emoji_reaction_comment = None;
                self.text_reaction_range = None;
                self.dialog = Some(Dialog::Emojis);
            }
            if action.delete && !self.session.viewing {
                self.session.editor.delete_image(block);
                self.image_block = None;
                self.image_selection_id = None;
                self.image_resize = None;
                self.refocus = true;
                ctx.request_repaint();
            }
        }
    }
}

fn blank_document(title: &str) -> Document {
    let mut doc = Document::new(title);
    if let Some(p) = doc.paragraph_mut(Address::default()) {
        p.style.font = fonts::preferred_body_family().into();
    }
    doc
}
fn template(kind: usize) -> Document {
    let mut d = Document::new(match kind {
        1 => "Resume",
        2 => "Letter",
        3 => "Project proposal",
        4 => "Report",
        5 => "Meeting notes",
        6 => "Essay",
        _ => "Untitled document",
    });
    if kind == 0 {
        return blank_document("Untitled document");
    }
    let content: Vec<(u8, &str)> = match kind {
        1 => vec![
            (7, "Your name"),
            (8, "Your role · email@example.com · City"),
            (2, "Profile"),
            (
                0,
                "Describe your experience and what you bring to the team.",
            ),
            (2, "Experience"),
            (3, "Role · Company"),
            (0, "Describe your responsibilities and achievements."),
            (2, "Education"),
            (0, "Qualification · Institution"),
        ],
        2 => vec![
            (0, "Your name"),
            (0, "Your address"),
            (0, "Date"),
            (0, "Dear recipient,"),
            (0, "Write your letter here."),
            (0, "Kind regards,"),
            (0, "Your name"),
        ],
        3 => vec![
            (7, "Project proposal"),
            (8, "A clear plan for what comes next"),
            (2, "Overview"),
            (0, "Describe the problem and your proposed approach."),
            (2, "Objectives"),
            (0, "Define what success looks like."),
            (2, "Timeline"),
            (0, "List milestones and deliverables."),
            (2, "Next steps"),
            (0, "Capture the decisions and actions to move forward."),
        ],
        4 => vec![
            (7, "Report title"),
            (8, "An overview of findings"),
            (2, "Introduction"),
            (0, "Set the context for your report."),
            (2, "Findings"),
            (0, "Present the evidence and key observations."),
            (2, "Recommendations"),
            (0, "Describe the actions supported by your findings."),
        ],
        6 => vec![
            (7, "Essay title"),
            (8, "Author · Date"),
            (2, "Introduction"),
            (0, "Introduce your topic and state the central argument."),
            (2, "Discussion"),
            (0, "Develop your ideas with evidence and clear reasoning."),
            (2, "Conclusion"),
            (
                0,
                "Bring the argument together and explain its significance.",
            ),
        ],
        _ => vec![
            (7, "Meeting notes"),
            (8, "Topic · Date · Attendees"),
            (2, "Agenda"),
            (0, "Add the topics to discuss."),
            (2, "Discussion"),
            (0, "Capture decisions and context."),
            (2, "Action items"),
            (0, "Write the actions, owners and due dates."),
        ],
    };
    d.blocks = content
        .into_iter()
        .map(|(h, t)| {
            let mut p = Paragraph::from_text(t);
            p.set_heading(h);
            if kind == 1 || kind == 6 {
                p.style.font = "Noto Serif".into();
                for c in &mut p.chars {
                    c.style.font = "Noto Serif".into();
                }
            }
            Block::Paragraph(p)
        })
        .collect();
    d
}

impl AlphaDocs {
    fn update_ui(&mut self, ui: &mut Ui) {
        self.poll_updates(ui.ctx());
        self.poll_export(ui.ctx());
        self.poll_sharing(ui.ctx());
        self.poll_paste(ui.ctx());
        let ctx = ui.ctx().clone();
        if ctx.os() == egui::os::OperatingSystem::Mac
            && ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::COMMAND, Key::F)
            })
        {
            self.fullscreen = !self.fullscreen;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
        }
        if !self.home
            && self.dialog.is_none()
            && !self.feature_search_open
            && ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::F)
            })
        {
            self.toggle_menus(&ctx);
        }
        if ctx.input(|i| i.pointer.any_click() || i.key_pressed(Key::Escape)) {
            self.feature_pulse = None;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, Key::Slash)) {
            self.open_feature_search();
        }
        if self.prepare_fonts(&ctx) {
            self.layout_revision = u64::MAX;
            ctx.request_discard("Register document fonts");
            return;
        }
        self.palette = theme::apply(&ctx, self.prefs.theme);
        ui.painter()
            .rect_filled(ui.max_rect(), 0.0, self.palette.bg);
        ui.set_style(ctx.style_of(ctx.theme()));
        self.tabs.refresh(&self.session);
        if self.dialog.is_none()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::W))
            && let Some(index) = self.tabs.active
        {
            match self.tabs.close(&mut self.session, index) {
                Ok(()) => {
                    self.home = self.tabs.active.is_none();
                    self.reset_document_view();
                    self.persist_session();
                }
                Err(e) => self.status = e,
            }
        }
        let reverse_tab = ctx
            .input_mut(|i| i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, Key::Tab));
        if self.dialog.is_none()
            && (reverse_tab || ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, Key::Tab)))
            && !self.tabs.tabs.is_empty()
        {
            let count = self.tabs.tabs.len();
            let index =
                (self.tabs.active.unwrap_or(0) + if reverse_tab { count - 1 } else { 1 }) % count;
            self.switch_tab(index);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::N))
            && self.dialog.is_none()
        {
            self.execute(Action::New, &ctx);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::F11)) {
            self.fullscreen = !self.fullscreen;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
        }
        if self.home && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::O)) {
            self.execute(Action::Open, &ctx);
        }
        self.sync_image_selection();
        self.input(&ctx);
        if self.view_tab_id != self.session.editor.doc.tabs[self.session.editor.doc.active_tab].id {
            self.reset_document_view();
        }
        self.sync_image_selection();
        self.poll_live_proof(&ctx);
        chrome::show(self, ui);
        if self.home {
            homepage::show(self, ui);
        } else {
            self.editor_ui(ui);
        }
        if !self.home && self.dialog.is_none() && self.proof_state.is_some() {
            self.proofing_dialog(&ctx);
        }
        self.dialogs(&ctx);
        self.nearby_prompt(&ctx);
        self.feature_search(&ctx);
        self.updates_ui(&ctx);
        if let Some(a) = self.action.take() {
            self.execute(a, &ctx);
        }
        self.paint_feature_pulse(&ctx);
        if self.refocus && self.dialog.is_none() {
            ctx.request_repaint();
        }
        if !self.home
            && self.prefs.autosave
            && self.session.editor.revision != self.session.saved_library_revision
            && self.last_autosave.elapsed() > Duration::from_secs(2)
        {
            self.flush();
            self.last_autosave = Instant::now();
            if self.last_history.elapsed() > Duration::from_secs(60) {
                if let Err(e) = persistence::snapshot(&self.session.editor.doc) {
                    self.status = e;
                }
                self.last_history = Instant::now();
            }
        }
        if !self.home
            && self.prefs.autosave
            && self.session.editor.revision != self.session.saved_library_revision
        {
            ctx.request_repaint_after(Duration::from_secs(2));
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            if let Some(job) = &mut self.export_job {
                job.close_requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.status = "Finishing export before closing…".into();
            } else if let Err(error) = self.tabs.save_all(&mut self.session) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.message = format!("Your documents remain open. Saving failed:\n\n{error}");
                self.dialog = Some(Dialog::Message);
            } else {
                self.persist_session();
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(if self.home {
            "AlphaDocs — Documents".into()
        } else {
            format!("{} — AlphaDocs", self.session.editor.doc.title)
        }));
    }
}
impl eframe::App for AlphaDocs {
    fn raw_input_hook(&mut self, ctx: &Context, raw_input: &mut egui::RawInput) {
        self.context_pointer.translate(ctx.os(), raw_input);
    }
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.update_ui(ui);
    }
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        if self.prefs.autosave
            && let Err(e) = self.tabs.save_all(&mut self.session)
        {
            self.status = e;
        }
        self.persist_session();
        let _ = persistence::save_preferences(&self.prefs);
    }
}

fn safe_name(t: &str) -> String {
    let name: String = t
        .chars()
        .filter(|c| !"<>:\"/\\|?*".contains(*c) && !c.is_control())
        .take(100)
        .collect();
    if name.trim().is_empty() {
        "document".into()
    } else {
        name
    }
}
fn truncate(t: &str, n: usize) -> String {
    let mut s: String = t.chars().take(n).collect();
    if t.chars().count() > n {
        s.push('…');
    }
    s
}
fn table_commands(ui: &mut Ui, editor: &mut crate::engine::Editor, viewing: bool) -> bool {
    let mut changed = false;
    for (label, operation) in [
        ("Insert row above", 0),
        ("Insert row below", 1),
        ("Insert column left", 2),
        ("Insert column right", 3),
        ("Delete row", 4),
        ("Delete column", 5),
        ("Delete table", 6),
    ] {
        if menu(ui, label, "", !viewing) {
            match operation {
                0 => editor.insert_table_row(false),
                1 => editor.insert_table_row(true),
                2 => editor.insert_table_column(false),
                3 => editor.insert_table_column(true),
                4 => editor.delete_table_row(),
                5 => editor.delete_table_column(),
                _ => editor.delete_table(),
            }
            changed = true;
        }
    }
    changed
}
fn menu(ui: &mut Ui, label: &str, shortcut: &str, enabled: bool) -> bool {
    menu_width(ui, label, shortcut, enabled, 302.0)
}
fn menu_width(ui: &mut Ui, label: &str, shortcut: &str, enabled: bool, width: f32) -> bool {
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.set_min_width(width);
    let (rect, r) = ui
        .add_enabled_ui(enabled, |ui| {
            ui.allocate_exact_size(vec2(width, 32.0), Sense::click())
        })
        .inner;
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    #[cfg(test)]
    ui.ctx()
        .data_mut(|d| d.insert_temp(Id::new(("menu_bounds", label)), rect));
    if r.hovered() && enabled {
        ui.painter()
            .rect_filled(rect, 2.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let colour = if enabled {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    let icon = if label.contains("(.") {
        Some(Icon::Document)
    } else if label == "Make a copy" {
        Some(Icon::Copy)
    } else if label.starts_with("Share") {
        Some(Icon::Share)
    } else if label.starts_with("Move to trash") {
        Some(Icon::Delete)
    } else if label == "Page setup" {
        Some(Icon::PageSetup)
    } else if label.starts_with("Undo") {
        Some(Icon::Undo)
    } else if label.starts_with("Redo") {
        Some(Icon::Redo)
    } else if label.contains("Print") {
        Some(Icon::Print)
    } else if label.contains("Open") {
        Some(Icon::Folder)
    } else if label.contains("Save") {
        Some(Icon::Save)
    } else if label.contains("Spelling") {
        Some(Icon::Proofing)
    } else if label.contains("Find") {
        Some(Icon::FindReplace)
    } else if label.contains("history") {
        Some(Icon::History)
    } else if label.contains("Image") {
        Some(Icon::Image)
    } else if label.contains("Link") || label.contains("link") {
        Some(Icon::Link)
    } else if label.contains("formatting") {
        Some(Icon::Clear)
    } else if label.contains("document") || label.contains("count") {
        Some(Icon::Document)
    } else if label.contains("Preferences") {
        Some(Icon::Theme)
    } else {
        None
    };
    if let Some(icon) = icon {
        icons::draw(
            ui,
            Rect::from_center_size(pos2(rect.left() + 15.0, rect.center().y), vec2(18.0, 18.0)),
            icon,
            colour,
        );
    }
    ui.painter().text(
        pos2(rect.left() + 34.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        colour,
    );
    ui.painter().text(
        pos2(rect.right() - 10.0, rect.center().y),
        Align2::RIGHT_CENTER,
        super::platform::shortcut_label(ui.ctx(), shortcut),
        FontId::proportional(12.0),
        ui.visuals().weak_text_color(),
    );
    if r.clicked() {
        ui.close();
        true
    } else {
        false
    }
}
fn submenu(ui: &mut Ui, label: &str, icon: Icon, contents: impl FnOnce(&mut Ui)) {
    submenu_width(ui, label, icon, 302.0, contents);
}
fn submenu_width(ui: &mut Ui, label: &str, icon: Icon, width: f32, contents: impl FnOnce(&mut Ui)) {
    ui.set_min_width(width);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 32.0), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 0.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let color = ui.visuals().text_color();
    icons::draw(
        ui,
        Rect::from_center_size(pos2(rect.left() + 15.0, rect.center().y), vec2(18.0, 18.0)),
        icon,
        color,
    );
    ui.painter().text(
        pos2(rect.left() + 34.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        color,
    );
    let c = rect.right_center() - vec2(12.0, 0.0);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            c + vec2(-2.0, -4.0),
            c + vec2(2.0, 0.0),
            c + vec2(-2.0, 4.0),
        ],
        color,
        Stroke::NONE,
    ));
    egui::containers::menu::SubMenu::new().show(ui, &response, contents);
}
fn primary(ui: &mut Ui, label: &str, p: Palette, enabled: bool) -> egui::Response {
    let text_color = if p.dark {
        Color32::from_rgb(15, 33, 58)
    } else {
        Color32::WHITE
    };
    let galley = egui::WidgetText::from(
        RichText::new(label)
            .font(FontId::new(
                14.0,
                egui::FontFamily::Name("Sans-Bold".into()),
            ))
            .color(text_color),
    )
    .into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("")
            .fill(p.accent)
            .corner_radius(22)
            .min_size(vec2((galley.size().x + 32.0).max(68.0), 40.0)),
    );
    ui.painter().galley(
        response.rect.center() - galley.size() * 0.5,
        galley,
        if enabled {
            text_color
        } else {
            text_color.gamma_multiply(0.5)
        },
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    response
}

fn dropdown(ui: &mut Ui, label: &str, contents: impl FnOnce(&mut Ui)) {
    let response = ui.menu_button(format!("{label}    "), contents).response;
    let center = response.rect.right_center() - vec2(11.0, 0.0);
    let stroke = Stroke::new(1.2, ui.visuals().weak_text_color());
    ui.painter()
        .line_segment([center + vec2(-3.0, -1.5), center + vec2(0.0, 1.5)], stroke);
    ui.painter()
        .line_segment([center + vec2(0.0, 1.5), center + vec2(3.0, -1.5)], stroke);
}

fn tonal(ui: &mut Ui, label: &str, p: Palette, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(
            RichText::new(label)
                .font(FontId::new(
                    14.0,
                    egui::FontFamily::Name("Sans-Bold".into()),
                ))
                .color(p.accent),
        )
        .fill(p.selection)
        .corner_radius(24)
        .min_size(vec2(86.0, 40.0)),
    )
}

fn icon_menu(ui: &mut Ui, icon: Icon, label: &str, content: impl FnOnce(&mut Ui)) {
    let response = ui
        .add(
            egui::Button::new("     ")
                .frame(false)
                .min_size(vec2(38.0, 30.0)),
        )
        .on_hover_text(label);
    icons::draw(
        ui,
        Rect::from_center_size(response.rect.center() - vec2(5.0, 0.0), vec2(18.0, 18.0)),
        icon,
        ui.visuals().text_color(),
    );
    let c = response.rect.right_center() - vec2(6.0, 0.0);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            c + vec2(-2.5, -1.5),
            c + vec2(2.5, -1.5),
            c + vec2(0.0, 1.5),
        ],
        ui.visuals().text_color(),
        Stroke::NONE,
    ));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    egui::Popup::menu(&response).show(content);
}
