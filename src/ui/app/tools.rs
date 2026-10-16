use super::*;
use crate::writing::{self, Citation, CitationStyle};
use std::sync::mpsc::{self, Receiver};

pub(super) fn bounds(_ui: &Ui, _id: &str, _response: &egui::Response) {
    #[cfg(test)]
    _ui.ctx()
        .data_mut(|d| d.insert_temp(Id::new(_id), _response.rect));
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Tool {
    Compare,
    Citations,
    Lines,
    Dictionary,
    Translate,
    PersonalDictionary,
    Voice,
    Notifications,
    Accessibility,
}
impl Tool {
    fn title(self) -> &'static str {
        match self {
            Self::Compare => "Compare documents",
            Self::Citations => "Citations",
            Self::Lines => "Line numbers",
            Self::Dictionary => "Dictionary",
            Self::Translate => "Translate document",
            Self::PersonalDictionary => "Personal dictionary",
            Self::Voice => "Voice typing",
            Self::Notifications => "Notification settings",
            Self::Accessibility => "Accessibility",
        }
    }
}
enum Output {
    Document(Box<Document>),
    Definition(String),
}
struct Job {
    source: (String, String, u64),
    receiver: Receiver<Result<Output, String>>,
}
#[derive(Default)]
pub(super) struct Tools {
    panel: Option<Tool>,
    job: Option<Job>,
    error: String,
    query: String,
    definition: String,
    comparison: Option<Document>,
    attribution: String,
    source: usize,
    target: usize,
    title: String,
    citation: Citation,
    adding: bool,
    style: CitationStyle,
    page: String,
    pub substitutions_tab: bool,
}
#[cfg(test)]
impl Tools {
    pub(super) fn query_for_test(&self) -> &str {
        &self.query
    }
}
impl AlphaDocs {
    pub(super) fn tools_menu(&mut self, ui: &mut Ui) {
        ui.menu_button("Spelling and grammar", |ui| {
            if menu(ui, "Spelling and grammar check", "Ctrl+Alt+X", true) {
                self.queue(Action::Proofing);
            }
            let a = ui
                .checkbox(
                    &mut self.prefs.spelling_suggestions,
                    "Show spelling suggestions",
                )
                .changed();
            let b = ui
                .checkbox(
                    &mut self.prefs.grammar_suggestions,
                    "Show grammar suggestions",
                )
                .changed();
            if a || b {
                self.live_proof.context = None;
                self.save_tool_preferences();
            }
            if menu(ui, "Personal dictionary", "", true) {
                self.queue(Action::Tool(Tool::PersonalDictionary));
            }
        });
        if menu(ui, "Word count", "Ctrl+Shift+C", true) {
            self.queue(Action::Count);
        }
        ui.scope(|ui| {menu(ui,"Review suggested edits","",false);}).response.on_hover_text("Tracked changes are not implemented. Local comments are available from Insert → Comment.");
        for tool in [Tool::Compare, Tool::Citations] {
            if menu(ui, tool.title(), "", true) {
                self.queue(Action::Tool(tool));
            }
        }
        ui.scope(|ui| {menu(ui,"eSignature","",false);}).response.on_hover_text("Verified signature requests and audit trails are not implemented. Insert an existing signature image using Insert → Image.");
        if menu(ui, "Line numbers", "", true) {
            self.queue(Action::Tool(Tool::Lines));
        }
        ui.scope(|ui| {menu(ui,"Linked objects","",false);}).response.on_hover_text("Images and tables are embedded copies. Refreshing external charts, tables or slides is not supported yet.");
        if menu(ui, "Dictionary", "Ctrl+Shift+Y", true) {
            self.queue(Action::Tool(Tool::Dictionary));
        }
        ui.separator();
        if self.prefs.online_translation && menu(ui, "Translate document…", "", true) {
            self.queue(Action::Tool(Tool::Translate));
        }
        if menu(ui, "Voice typing…", "", true) {
            self.queue(Action::Tool(Tool::Voice));
        }
        ui.separator();
        if menu(ui, "Notification settings", "", true) {
            self.queue(Action::Tool(Tool::Notifications));
        }
        if menu(ui, "Preferences", "", true) {
            self.queue(Action::Preferences);
        }
        if menu(ui, "Accessibility", "", true) {
            self.queue(Action::Tool(Tool::Accessibility));
        }
    }
    pub(super) fn open_tool(&mut self, tool: Tool) {
        if tool == Tool::Translate && !self.prefs.online_translation {
            return;
        }
        self.tools.error.clear();
        match tool {
            Tool::Citations | Tool::Lines | Tool::Dictionary => {
                self.tools.panel = Some(tool);
                self.image_options_open = false;
                self.comments_open = false;
                if tool == Tool::Dictionary {
                    let q = self.session.editor.selected_text();
                    if !q.trim().is_empty() {
                        self.tools.query = q.trim().into();
                    }
                }
            }
            Tool::Translate => {
                self.tools.title = format!("Translated copy of {}", self.session.editor.doc.title);
                self.tools.target = 1;
                self.dialog = Some(Dialog::Tool(tool));
            }
            _ => self.dialog = Some(Dialog::Tool(tool)),
        }
    }
    fn save_tool_preferences(&mut self) {
        #[cfg(not(test))]
        if let Err(e) = persistence::save_preferences(&self.prefs) {
            self.status = e;
        }
    }
    fn tool_source(&self) -> (String, String, u64) {
        (
            self.session.editor.doc.id.clone(),
            self.session.editor.doc.tabs[self.session.editor.doc.active_tab]
                .id
                .clone(),
            self.session.editor.revision,
        )
    }
    fn tool_job(
        &mut self,
        ctx: &Context,
        task: impl FnOnce() -> Result<Output, String> + Send + 'static,
    ) {
        let (tx, receiver) = mpsc::channel();
        let repaint = ctx.clone();
        self.tools.error.clear();
        self.tools.job = Some(Job {
            source: self.tool_source(),
            receiver,
        });
        std::thread::spawn(move || {
            let _ = tx.send(task());
            repaint.request_repaint();
        });
    }
    pub(super) fn poll_tools(&mut self, ctx: &Context) {
        let Some(job) = self.tools.job.take() else {
            return;
        };
        match job.receiver.try_recv() {
            Ok(Ok(Output::Definition(text))) => self.tools.definition = text,
            Ok(Ok(Output::Document(doc))) => {
                if job.source != self.tool_source() {
                    self.tools.error="The source document changed while the tool was working. Run it again; the source has been retained.".into();
                } else {
                    self.flush();
                    self.load(*doc, None);
                    self.dialog = None;
                    self.tools.panel = None;
                    self.status = "Created a separate document copy".into();
                }
            }
            Ok(Err(e)) => self.tools.error = e,
            Err(mpsc::TryRecvError::Empty) => {
                self.tools.job = Some(job);
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.tools.error =
                    "The writing tool ended unexpectedly. Your document is unchanged.".into()
            }
        }
    }
    pub(super) fn tools_panel(&mut self, ui: &mut Ui) {
        let Some(tool) = self.tools.panel else {
            return;
        };
        let p = self.palette;
        egui::Panel::right("writing_tools").exact_size(320.0).resizable(false).frame(Frame::new().fill(p.toolbar).corner_radius(16).inner_margin(20)).show(ui,|ui|{
            ui.horizontal(|ui|{ui.label(RichText::new(tool.title()).strong().size(16.0));ui.with_layout(UiLayout::right_to_left(Align::Center),|ui|{if icons::button(ui,Icon::Close,"Close writing tool",false,true).clicked(){self.tools.panel=None;self.refocus=true;}});});
            ui.add_space(18.0);
            egui::ScrollArea::vertical().show(ui,|ui|{
                match tool {
                    Tool::Citations=>self.citations_panel(ui),
                    Tool::Lines=>{
                        let mut mode=self.session.editor.doc.page.line_numbers;
                        let mut enabled=mode!=LineNumbering::Off;
                        ui.add_enabled_ui(!self.session.editor.doc.page.pageless && !self.session.viewing,|ui|{
                            if ui.checkbox(&mut enabled,"Show line numbers").changed(){mode=if enabled{LineNumbering::Continuous}else{LineNumbering::Off};}
                            ui.add_space(20.0);ui.label("Line numbering mode");
                            ui.add_enabled_ui(enabled,|ui|{
                                ui.radio_value(&mut mode,LineNumbering::Continuous,"Continue throughout document");
                                ui.radio_value(&mut mode,LineNumbering::EachPage,"Restart on each page");
                                ui.add_enabled(false,egui::RadioButton::new(false,"Restart on each section")).on_disabled_hover_text("Independent page sections are not supported.");
                            });
                        });
                        if mode!=self.session.editor.doc.page.line_numbers {self.session.editor.transaction(|e|e.doc.page.line_numbers=mode);}
                        ui.add_space(16.0);ui.label("Applies to the whole document, including all content tabs. Counts measured body text lines; table cells, images and running text are excluded. A left gutter of at least ¼ inch is needed for visible numbers.");
                        if self.session.editor.doc.page.pageless {ui.label(RichText::new("Switch to Pages in Page setup to display line numbers.").color(p.accent));}
                    },
                    Tool::Dictionary=>{
                        ui.label("English dictionary · online");
                        let r=ui.add(egui::TextEdit::singleline(&mut self.tools.query).hint_text("Search for a word").char_limit(80).desired_width(280.0));
                        let enter=r.lost_focus()&&ui.input(|i|i.key_pressed(Key::Enter));
                        if (ui.add_enabled(self.tools.job.is_none(),egui::Button::new("Search")).clicked()||enter)&&self.tools.job.is_none(){let q=self.tools.query.trim().to_owned();self.tool_job(ui.ctx(),move||writing::dictionary(&q).map(Output::Definition));}
                        ui.add_space(8.0);ui.label(RichText::new("Search sends this word to Free Dictionary API (dictionaryapi.dev). No background lookups.").size(12.0).color(p.muted));
                        ui.separator();ui.label(&self.tools.definition);
                    },_=>{}
                }
                if self.tools.job.is_some(){ui.spinner();ui.label("Working…");}
                if !self.tools.error.is_empty(){ui.colored_label(p.accent,&self.tools.error);}
            });
        });
    }
    fn citations_panel(&mut self, ui: &mut Ui) {
        egui::ComboBox::from_id_salt("citation_style")
            .width(280.0)
            .selected_text(match self.tools.style {
                CitationStyle::Apa => "APA (7th ed.) · basic",
                CitationStyle::Mla => "MLA (8th ed.) · basic",
                CitationStyle::Chicago => "Chicago author-date · basic",
            })
            .show_ui(ui, |ui| {
                for (s, l) in [
                    (CitationStyle::Apa, "APA (7th ed.)"),
                    (CitationStyle::Mla, "MLA (8th ed.)"),
                    (CitationStyle::Chicago, "Chicago author-date"),
                ] {
                    ui.selectable_value(&mut self.tools.style, s, l);
                }
            });
        ui.label(RichText::new("Manual sources with a single author/organisation. Check the generated text against your style guide; complex sources and live citation fields are not supported.").size(12.0).color(self.palette.muted));
        ui.add_space(10.0);
        let add = tonal(
            ui,
            "+ Add citation source",
            self.palette,
            !self.session.viewing && self.session.editor.doc.citations.len() < 1000,
        );
        bounds(ui, "citations_add", &add);
        if add.clicked() {
            self.tools.adding = true;
        }
        if self.tools.adding {
            for (label, id, field) in [
                (
                    "Author / organisation",
                    "citation_author",
                    &mut self.tools.citation.author,
                ),
                ("Title *", "citation_title", &mut self.tools.citation.title),
                ("Year", "citation_year", &mut self.tools.citation.year),
                (
                    "Publisher / website",
                    "citation_publisher",
                    &mut self.tools.citation.publisher,
                ),
                ("URL", "citation_url", &mut self.tools.citation.url),
            ] {
                ui.label(label);
                let input = ui.add(
                    egui::TextEdit::singleline(field)
                        .id(Id::new(id))
                        .char_limit(4096)
                        .desired_width(280.0),
                );
                bounds(ui, id, &input);
            }
            ui.horizontal(|ui| {
                let save = tonal(
                    ui,
                    "Save source",
                    self.palette,
                    self.tools.citation.valid() && !self.session.viewing,
                );
                bounds(ui, "citations_save", &save);
                if save.clicked() {
                    let c = self.tools.citation.clone();
                    self.session.editor.transaction(|e| e.doc.citations.push(c));
                    self.tools.citation = Citation::default();
                    self.tools.adding = false;
                }
                if ui.button("Cancel").clicked() {
                    self.tools.adding = false;
                }
            });
        }
        ui.add_space(12.0);
        ui.label("Page for in-text citation (optional)");
        let page = ui.add(egui::TextEdit::singleline(&mut self.tools.page).char_limit(32));
        bounds(ui, "citation_page", &page);
        let mut remove = None;
        let mut cite = None;
        for (i, c) in self.session.editor.doc.citations.iter().enumerate() {
            ui.separator();
            ui.label(RichText::new(&c.title).strong());
            ui.label(format!("{} · {}", c.author, c.year));
            ui.horizontal(|ui| {
                let cite_response =
                    ui.add_enabled(!self.session.viewing, egui::Button::new("Cite"));
                bounds(ui, &format!("citations_cite_{i}"), &cite_response);
                if cite_response.clicked() {
                    cite = Some(c.inline(self.tools.style, &self.tools.page));
                }
                if ui
                    .add_enabled(!self.session.viewing, egui::Button::new("Remove"))
                    .clicked()
                {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            self.session.editor.transaction(|e| {
                e.doc.citations.remove(i);
            });
        }
        if let Some(text) = cite {
            self.session.editor.insert(&text);
            self.refocus = true;
            self.scroll_caret = true;
        }
        ui.add_space(14.0);
        let bibliography = tonal(
            ui,
            "Insert bibliography",
            self.palette,
            !self.session.viewing && !self.session.editor.doc.citations.is_empty(),
        );
        bounds(ui, "citations_bibliography", &bibliography);
        if bibliography.clicked() {
            let mut entries: Vec<_> = self
                .session
                .editor
                .doc
                .citations
                .iter()
                .map(|c| c.bibliography(self.tools.style))
                .collect();
            entries.sort_by_key(|s| s.to_lowercase());
            self.session.editor.insert(&format!(
                "\n{}\n{}\n",
                if self.tools.style == CitationStyle::Mla {
                    "Works cited"
                } else {
                    "References"
                },
                entries.join("\n")
            ));
            self.refocus = true;
            self.scroll_caret = true;
        }
    }
    pub(super) fn tool_dialog(&mut self, tool: Tool, ui: &mut Ui, close: &mut bool) {
        ui.label(RichText::new(tool.title()).size(24.0));
        ui.add_space(20.0);
        match tool {
            Tool::Compare => {
                ui.label("Create a separate text comparison report.");
                ui.label("Choose the original document; the current document is the revised version. All content tabs and table text are included. Formatting, images, comments and tracked revisions are not compared.");
                ui.add_space(14.0);
                if ui.button("Choose document…").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("Documents", &["adocs", "docx", "txt"])
                        .pick_file()
                {
                    let result = if path
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("docx"))
                    {
                        compat::docx::import(&path).map(|(d, _)| d)
                    } else if path
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("txt"))
                    {
                        std::fs::read_to_string(&path)
                            .map_err(|e| e.to_string())
                            .map(|s| {
                                let mut d = Document::new(
                                    path.file_stem()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .as_ref(),
                                );
                                d.blocks = s
                                    .lines()
                                    .map(|s| Block::Paragraph(Paragraph::from_text(s)))
                                    .collect();
                                if d.blocks.is_empty() {
                                    d.blocks.push(Block::Paragraph(Paragraph::default()));
                                }
                                d
                            })
                    } else {
                        persistence::load_document(&path)
                    };
                    match result {
                        Ok(d) => {
                            self.tools.comparison = Some(d);
                            self.tools.error.clear();
                        }
                        Err(e) => self.tools.error = e,
                    }
                }
                if let Some(doc) = &self.tools.comparison {
                    ui.label(&doc.title);
                }
                ui.label("Attribute changes to");
                ui.add(
                    egui::TextEdit::singleline(&mut self.tools.attribution)
                        .hint_text("Author name")
                        .char_limit(100),
                );
                ui.add_space(18.0);
                if primary(
                    ui,
                    "Compare",
                    self.palette,
                    self.tools.job.is_none() && self.tools.comparison.is_some(),
                )
                .clicked()
                {
                    let original = self.tools.comparison.clone().unwrap();
                    let revised = self.session.editor.doc.clone();
                    let author = self.tools.attribution.clone();
                    self.tool_job(ui.ctx(), move || {
                        writing::compare(&original, &revised, &author)
                            .map(|d| Output::Document(Box::new(d)))
                    });
                }
            }
            Tool::Translate => {
                ui.label("Create a translated copy using MyMemory (Translated). Clicking Translate sends all text, including table cells and headers/footers, to api.mymemory.translated.net. Images are not uploaded.");
                ui.hyperlink_to(
                    "Provider terms and privacy",
                    "https://mymemory.translated.net/doc/tos.php",
                );
                ui.add_space(12.0);
                ui.label("New document title");
                ui.add(
                    egui::TextEdit::singleline(&mut self.tools.title)
                        .char_limit(200)
                        .desired_width(350.0),
                );
                for (label, index) in [
                    ("From", &mut self.tools.source),
                    ("To", &mut self.tools.target),
                ] {
                    egui::ComboBox::from_id_salt(label)
                        .width(350.0)
                        .selected_text(format!("{label}: {}", writing::LANGUAGES[*index].1))
                        .show_ui(ui, |ui| {
                            for (i, (_, name)) in writing::LANGUAGES.iter().enumerate() {
                                ui.selectable_value(index, i, *name);
                            }
                        });
                }
                ui.label(RichText::new("Free anonymous limit: 5,000 characters/day, shared by your public IP. Availability and translation quality vary. Paragraph styling and objects are retained; inline formatting, hyperlinks and text annotations are reset in the copy. Original remains unchanged.").size(12.0).color(self.palette.muted));
                ui.add_space(18.0);
                if primary(
                    ui,
                    "Translate",
                    self.palette,
                    self.prefs.online_translation
                        && self.tools.job.is_none()
                        && !self.tools.title.trim().is_empty()
                        && self.tools.source != self.tools.target,
                )
                .clicked()
                {
                    let doc = self.session.editor.doc.clone();
                    let title = self.tools.title.clone();
                    let source = writing::LANGUAGES[self.tools.source].0;
                    let target = writing::LANGUAGES[self.tools.target].0;
                    self.tool_job(ui.ctx(), move || {
                        writing::translate(doc, &title, source, target)
                            .map(|d| Output::Document(Box::new(d)))
                    });
                }
            }
            Tool::PersonalDictionary => {
                ui.label("Words saved on this computer are excluded from offline spelling checks.");
                ui.horizontal(|ui| {
                    let input = ui.add(
                        egui::TextEdit::singleline(&mut self.tools.query)
                            .hint_text("Add a word")
                            .char_limit(80),
                    );
                    bounds(ui, "personal_dictionary_input", &input);
                    if ui.button("Add").clicked() {
                        let word = self.tools.query.trim();
                        if !word.is_empty()
                            && !word.chars().any(char::is_whitespace)
                            && !self
                                .prefs
                                .personal_dictionary
                                .iter()
                                .any(|s| s.eq_ignore_ascii_case(word))
                        {
                            self.prefs.personal_dictionary.push(word.into());
                            self.tools.query.clear();
                            self.save_tool_preferences();
                        }
                    }
                });
                let mut remove = None;
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for (i, w) in self.prefs.personal_dictionary.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(w);
                                if ui.small_button("Remove").clicked() {
                                    remove = Some(i);
                                }
                            });
                        }
                    });
                if let Some(i) = remove {
                    self.prefs.personal_dictionary.remove(i);
                    self.save_tool_preferences();
                }
            }
            Tool::Voice => {
                ui.label("Use your operating system's dictation while the document caret is active. Recognised text enters AlphaDocs through the normal, undoable text input.");
                ui.add_space(12.0);
                ui.label(if cfg!(target_os="windows"){ "Windows: click inside the document, then press Windows + H. Configure Speech in Windows Settings first." }else if cfg!(target_os="macos"){"macOS: enable Dictation in System Settings → Keyboard, then use the configured Dictation shortcut while editing."}else{"Linux: use a dictation/input-method tool provided by your desktop. AlphaDocs does not include a speech-recognition engine."});
                ui.add_space(12.0);
                ui.label("AlphaDocs has no built-in microphone capture, language selector or voice editing commands. Your operating system controls microphone permission and whether speech processing uses an online service.");
            }
            Tool::Notifications => {
                ui.label("Notifications on this computer");
                let r = ui.checkbox(
                    &mut self.prefs.nearby_invites,
                    "Show incoming nearby invitations",
                );
                bounds(ui, "notifications_nearby", &r);
                ui.label(RichText::new("Requires nearby visibility in Share. When off, incoming offers are dismissed; no session is joined automatically.").size(12.0).color(self.palette.muted));
                let r = ui.checkbox(
                    &mut self.prefs.check_updates,
                    "Check GitHub daily for app updates",
                );
                bounds(ui, "notifications_updates", &r);
                ui.label("AlphaDocs does not send email or hosted comment notifications. Collaboration status remains visible in the document.");
            }
            Tool::Accessibility => {
                let r = ui.checkbox(
                    &mut self.prefs.accessible_text,
                    "Expose document text to assistive technology",
                );
                bounds(ui, "accessibility_text", &r);
                ui.label(RichText::new("Provides the canvas text through the native accessibility bridge. Full rich-text navigation and braille have not been verified.").size(12.0).color(self.palette.muted));
                ui.add_space(12.0);
                ui.label("Interface scale");
                let r = ui.add(
                    egui::Slider::new(&mut self.prefs.interface_scale, 1.0..=1.5)
                        .step_by(0.05)
                        .suffix("×"),
                );
                bounds(ui, "accessibility_scale", &r);
                ui.label("The interface scale changes menus, dialogs and controls. Use the toolbar zoom for document text. System magnifier settings remain under your operating system's control.");
                ui.add_enabled(false, egui::Checkbox::new(&mut false, "Braille support"))
                    .on_disabled_hover_text(
                        "Dedicated braille navigation has not been implemented or verified.",
                    );
            }
            _ => {}
        }
        if self.tools.job.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Working… You can close this dialog.");
            });
        }
        if !self.tools.error.is_empty() {
            ui.colored_label(self.palette.accent, &self.tools.error);
        }
        ui.add_space(18.0);
        if ui.button("Done").clicked() {
            *close = true;
            ui.ctx().set_zoom_factor(self.prefs.interface_scale);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_worker_result_with_stale_revision_is_rejected_in_full_ui_frame() {
        let ctx = Context::default();
        fonts::install(&ctx);
        let mut app = AlphaDocs::fresh(&ctx, Preferences::default(), vec![]);
        app.home = false;
        let source = app.tool_source();
        let mut result = Document::new("Generated report");
        result.blocks = vec![Block::Paragraph(Paragraph::from_text("stale result"))];
        let (tx, receiver) = mpsc::channel();
        tx.send(Ok(Output::Document(Box::new(result)))).unwrap();
        app.tools.job = Some(Job { source, receiver });
        app.session
            .editor
            .insert("Edited while the tool was running.");
        let original_id = app.session.editor.doc.id.clone();

        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0))),
                ..Default::default()
            },
            |ui| app.update_ui(ui),
        );
        output.textures_delta.clear();

        assert_eq!(app.session.editor.doc.id, original_id);
        assert_eq!(
            app.session.editor.doc.text(),
            "Edited while the tool was running."
        );
        assert!(app.tools.error.contains("source document changed"));
    }
}
