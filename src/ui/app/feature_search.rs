use super::*;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Target {
    Dialog,
    Images,
    Tabs,
    Canvas,
}
#[derive(Clone, Copy)]
enum Command {
    Action(Action),
    Images,
    Tabs,
    Rename,
}
fn commands() -> Vec<(&'static str, &'static str, Command)> {
    use Command::Action as A;
    vec![
        ("New document", "File · Ctrl+N", A(Action::New)),
        ("Open document", "File · Ctrl+O", A(Action::Open)),
        (
            "Share or join a document",
            "File · Encrypted LAN/VPN collaboration",
            A(Action::Share),
        ),
        ("Save", "File · Ctrl+S", A(Action::Save)),
        ("Save as", "File · Ctrl+Shift+S", A(Action::SaveAs)),
        (
            "Rename document",
            "File · Double-click an open-file tab",
            A(Action::Rename),
        ),
        ("Find and replace", "Edit · Ctrl+H", A(Action::Find)),
        ("Undo", "Edit · Ctrl+Z", A(Action::Undo)),
        ("Redo", "Edit · Ctrl+Shift+Z", A(Action::Redo)),
        (
            "Insert image",
            "Insert · Choose an image file",
            A(Action::Image),
        ),
        (
            "Image options: padding, transparency, wrapping, snapping and adjustments",
            "Right sidebar",
            Command::Images,
        ),
        (
            "Insert table (2 × 2)",
            "Insert · Table",
            A(Action::Table(2, 2)),
        ),
        (
            "Insert emoji",
            "Insert · Pinned, recent and all",
            A(Action::Emojis),
        ),
        ("Insert symbol", "Insert · Symbols", A(Action::Symbols)),
        ("Insert hyperlink", "Insert · Ctrl+K", A(Action::Link)),
        ("Insert page break", "Insert · Ctrl+Enter", A(Action::Break)),
        ("Insert horizontal line", "Insert", A(Action::Rule)),
        (
            "Add comment",
            "Insert · Select text or an image · Ctrl+Alt+M",
            A(Action::Comment),
        ),
        (
            "Add emoji reaction",
            "Insert · Select text or an image",
            A(Action::Reaction),
        ),
        (
            "Add document tab",
            "Document tabs · Separate content in this file",
            Command::Tabs,
        ),
        (
            "Rename document tab",
            "Document tabs · Also double-click its name",
            Command::Rename,
        ),
        (
            "Page setup: paper, margins and columns",
            "File",
            A(Action::Page),
        ),
        ("Word count", "Tools", A(Action::Count)),
        (
            "Hide or show the menus",
            "View · Ctrl+Shift+F · Compact header",
            A(Action::ToggleMenus),
        ),
        (
            "Spelling and grammar check",
            "Tools · Ctrl+Alt+X · Offline English",
            A(Action::Proofing),
        ),
        ("Version history", "File", A(Action::History)),
        (
            "Check for updates",
            "Help · GitHub releases",
            A(Action::CheckUpdates),
        ),
        (
            "Download AlphaDocs ADOCS",
            "File · Complete editable native copy",
            A(Action::Export(compat::export::Format::Native)),
        ),
        (
            "Export PDF",
            "File · All document tabs",
            A(Action::ExportPdf),
        ),
        (
            "Export DOCX",
            "File · All document tabs",
            A(Action::ExportDocx),
        ),
        (
            "Export OpenDocument ODT",
            "File · All document tabs",
            A(Action::Export(compat::export::Format::Odt)),
        ),
        (
            "Export Plain Text TXT",
            "File · All document tabs",
            A(Action::Export(compat::export::Format::Txt)),
        ),
        (
            "Export Rich Text RTF",
            "File · All document tabs",
            A(Action::Export(compat::export::Format::Rtf)),
        ),
        (
            "Export Web Page HTML ZIP",
            "File · Offline document and images",
            A(Action::Export(compat::export::Format::Html)),
        ),
        (
            "Export EPUB Publication",
            "File · Reflowable book and navigation",
            A(Action::Export(compat::export::Format::Epub)),
        ),
        (
            "Export Markdown MD",
            "File · Text and companion picture folder",
            A(Action::Export(compat::export::Format::Markdown)),
        ),
        (
            "Print: printers, paper, scale and preview",
            "File · Native Windows printing and Save as PDF",
            A(Action::Print),
        ),
        (
            "Preferences: autosave, appearance, ruler and outline",
            "Tools",
            A(Action::Preferences),
        ),
        ("Help and keyboard shortcuts", "Help", A(Action::Help)),
        (
            "About AlphaDocs · Alpha Workroom · licence",
            "Help",
            A(Action::About),
        ),
        ("Clear formatting", "Format", A(Action::Clear)),
    ]
}
impl AlphaDocs {
    pub(super) fn open_feature_search(&mut self) {
        self.feature_search_open = true;
        self.feature_focus = true;
        self.feature_index = 0;
        self.refocus = false;
    }
    pub(super) fn feature_destination(&mut self, target: Target, rect: Rect) {
        if self.feature_pulse.is_some_and(|(_, t)| t == target) {
            self.feature_rect = Some(rect);
        }
    }
    fn available_feature(&self, command: Command) -> bool {
        match command {
            Command::Action(
                Action::New
                | Action::Open
                | Action::Preferences
                | Action::Help
                | Action::About
                | Action::CheckUpdates
                | Action::Share,
            ) => true,
            _ if self.home => false,
            Command::Action(Action::Comment | Action::Reaction) => {
                !self.session.viewing
                    && (self.image_block.is_some() || self.session.editor.selected())
            }
            Command::Action(Action::Undo) => {
                !self.session.viewing && self.session.editor.can_undo()
            }
            Command::Action(Action::Redo) => {
                !self.session.viewing && self.session.editor.can_redo()
            }
            Command::Action(
                Action::Find
                | Action::Count
                | Action::History
                | Action::ExportPdf
                | Action::ExportDocx
                | Action::Export(_)
                | Action::Print
                | Action::ToggleMenus,
            )
            | Command::Images => true,
            _ => !self.session.viewing,
        }
    }
    pub(super) fn feature_search(&mut self, ctx: &Context) {
        if !self.feature_search_open {
            return;
        }
        let p = self.palette;
        let mut selected = None;
        let modal = egui::Modal::new(Id::new("feature_finder"))
            .frame(
                Frame::new()
                    .fill(p.chrome)
                    .corner_radius(24)
                    .inner_margin(24),
            )
            .show(ctx, |ui| {
                ui.set_width(540.0);
                ui.horizontal(|ui| {
                    ui.heading("Search features");
                    ui.with_layout(UiLayout::right_to_left(Align::Center), |ui| {
                        if icons::button(ui, Icon::Close, "Close feature search", false, true)
                            .clicked()
                        {
                            self.feature_search_open = false;
                        }
                    });
                });
                ui.add_space(12.0);
                let input = ui.add(
                    egui::TextEdit::singleline(&mut self.feature_query)
                        .id(Id::new("feature_query"))
                        .hint_text("Try image padding, autosave, comments…")
                        .margin(vec2(10.0, 8.0))
                        .desired_width(f32::INFINITY),
                );
                if self.feature_focus {
                    input.request_focus();
                    self.feature_focus = false;
                }
                if input.changed() {
                    self.feature_index = 0;
                }
                let query = self.feature_query.to_lowercase();
                let matches: Vec<_> = commands()
                    .into_iter()
                    .filter(|(label, hint, _)| {
                        query
                            .split_whitespace()
                            .all(|word| format!("{label} {hint}").to_lowercase().contains(word))
                    })
                    .collect();
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowDown)) {
                    self.feature_index =
                        (self.feature_index + 1).min(matches.len().saturating_sub(1));
                }
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowUp)) {
                    self.feature_index = self.feature_index.saturating_sub(1);
                }
                let enter = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
                ui.add_space(12.0);
                egui::ScrollArea::vertical()
                    .max_height(400.0)
                    .show(ui, |ui| {
                        if matches.is_empty() {
                            ui.label("No matching features. Try a shorter name.");
                        }
                        for (index, (label, hint, command)) in matches.iter().enumerate() {
                            let enabled = self.available_feature(*command);
                            let row = ui.add_enabled(
                                enabled,
                                egui::Button::new(
                                    RichText::new(format!("{label}\n{hint}")).size(14.0),
                                )
                                .selected(self.feature_index == index)
                                .corner_radius(10)
                                .min_size(vec2(ui.available_width(), 48.0)),
                            );
                            if index == self.feature_index
                                && (enter
                                    || ctx.input(|i| {
                                        i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::ArrowUp)
                                    }))
                            {
                                row.scroll_to_me(Some(Align::Center));
                            }
                            if enabled && (row.clicked() || (enter && index == self.feature_index))
                            {
                                selected = Some(*command);
                            }
                        }
                    });
                ui.add_space(8.0);
                ui.label(
                    RichText::new("↑ ↓ to choose · Enter to open · Esc to close")
                        .size(12.0)
                        .color(p.muted),
                );
            });
        if modal.should_close() {
            self.feature_search_open = false;
            self.refocus = !self.home;
        }
        if let Some(command) = selected {
            self.feature_search_open = false;
            self.feature_rect = None;
            self.dialog = None;
            let target = match command {
                Command::Action(action) => {
                    self.execute(action, ctx);
                    if self.dialog.is_some() {
                        Target::Dialog
                    } else {
                        Target::Canvas
                    }
                }
                Command::Images => {
                    self.image_options_open = true;
                    Target::Images
                }
                Command::Tabs => {
                    self.prefs.outline = true;
                    self.session.editor.add_tab();
                    self.reset_document_view();
                    self.refresh_content_cache();
                    Target::Tabs
                }
                Command::Rename => {
                    self.prefs.outline = true;
                    let tab = &self.session.editor.doc.tabs[self.session.editor.doc.active_tab];
                    self.tab_rename = Some((tab.id.clone(), tab.title.clone()));
                    self.tab_rename_focus = true;
                    Target::Tabs
                }
            };
            self.feature_pulse = Some((Instant::now(), target));
            ctx.request_repaint();
        }
    }
    pub(super) fn paint_feature_pulse(&self, ctx: &Context) {
        if let Some((start, _)) = self.feature_pulse
            && let Some(rect) = self.feature_rect
        {
            let alpha = (130.0 + 90.0 * (start.elapsed().as_secs_f32() * 4.0).sin()) as u8;
            ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                Id::new("feature_pulse"),
            ))
            .rect_stroke(
                rect.expand(3.0),
                12,
                Stroke::new(
                    2.5,
                    self.palette.accent.gamma_multiply(alpha as f32 / 255.0),
                ),
                egui::StrokeKind::Outside,
            );
            ctx.request_repaint_after(Duration::from_millis(30));
        }
    }
}
