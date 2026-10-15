use super::*;
use std::sync::mpsc::{self, Receiver};
pub(super) struct ProofState {
    receiver: Receiver<Result<Vec<crate::proofing::Issue>, String>>,
    source: (String, String, u64),
    issues: Option<Vec<crate::proofing::Issue>>,
    error: Option<String>,
    index: usize,
    presented: Option<usize>,
    suggestion: usize,
    ignored: Vec<(Address, usize, String)>,
}
impl AlphaDocs {
    pub(super) fn proofing_range(&self) -> Option<(Position, Position)> {
        let state = self.proof_state.as_ref()?;
        if state.source.0 != self.session.editor.doc.id
            || state.source.1 != self.session.editor.doc.tabs[self.session.editor.doc.active_tab].id
            || state.source.2 != self.session.editor.revision
        {
            return None;
        }
        let issue = state.issues.as_ref()?.get(state.index)?;
        Some((
            Position {
                address: issue.address,
                offset: issue.start,
            },
            Position {
                address: issue.address,
                offset: issue.end,
            },
        ))
    }
    #[cfg(not(test))]
    pub(super) fn paste_native(&mut self) -> bool {
        match crate::clipboard::capture() {
            Ok(Some(capture)) => {
                let width = self.session.editor.doc.page.size().0
                    - self.session.editor.doc.page.margins[2]
                    - self.session.editor.doc.page.margins[3];
                let style = self.session.editor.typing.clone();
                let (tx, receiver) = mpsc::channel();
                std::thread::spawn(move || {
                    let _ = tx.send(capture.import(&style, width));
                });
                self.paste_state = Some(PasteState {
                    receiver,
                    image: self.image_block,
                    source: (
                        self.session.editor.doc.id.clone(),
                        self.session.editor.doc.tabs[self.session.editor.doc.active_tab]
                            .id
                            .clone(),
                        self.session.editor.revision,
                        self.session.editor.anchor,
                        self.session.editor.caret,
                    ),
                });
                self.status = "Pasting rich content…".into();
                true
            }
            Err(e) => {
                self.status = format!("Rich paste unavailable: {e}");
                false
            }
            _ => false,
        }
    }
    #[cfg(test)]
    pub(super) fn paste_native(&mut self) -> bool {
        false
    }
    pub(super) fn poll_paste(&mut self, ctx: &Context) {
        let Some(state) = self.paste_state.take() else {
            return;
        };
        // Let same-frame pointer/navigation input update the selection first.
        if ctx.input(|i| {
            i.pointer.any_pressed()
                || i.events
                    .iter()
                    .any(|e| matches!(e, Event::Text(_) | Event::Key { pressed: true, .. }))
        }) {
            self.paste_state = Some(state);
            ctx.request_repaint();
            return;
        }
        match state.receiver.try_recv() {
            Ok(Ok(import)) => {
                let current = (
                    self.session.editor.doc.id.clone(),
                    self.session.editor.doc.tabs[self.session.editor.doc.active_tab]
                        .id
                        .clone(),
                    self.session.editor.revision,
                    self.session.editor.anchor,
                    self.session.editor.caret,
                );
                let field_focused = ctx
                    .memory(|m| m.focused())
                    .is_some_and(|id| egui::TextEdit::load_state(ctx, id).is_some());
                if current != state.source
                    || self.image_block != state.image
                    || self.session.viewing
                    || self.home
                    || field_focused
                {
                    self.status="Paste cancelled because the selection changed; paste again at the intended position".into();
                    return;
                }
                if import.content.blocks.is_empty() {
                    self.status = "The clipboard has no supported content".into();
                    return;
                }
                let cell_only = matches!(
                    self.session
                        .editor
                        .doc
                        .blocks
                        .get(self.session.editor.caret.address.block),
                    Some(Block::Table(_))
                ) && !self.session.editor.selection_covers_body();
                if state.image.is_none()
                    && cell_only
                    && import
                        .content
                        .blocks
                        .iter()
                        .any(|b| !matches!(b, Block::Paragraph(_)))
                {
                    self.status =
                        "Paste tables or images in the document body; table cells accept text only"
                            .into();
                    return;
                }
                self.session
                    .editor
                    .paste_fragment_replacing_image(&import.content, state.image);
                self.image_block = None;
                self.image_selection_id = None;
                self.status = if import.warnings.is_empty() {
                    "Pasted rich content".into()
                } else {
                    format!("Pasted with notices: {}", import.warnings.join(" · "))
                };
                self.textures.clear();
                self.scroll_caret = true;
                self.refocus = true;
            }
            Ok(Err(e)) => self.status = format!("Could not paste: {e}"),
            Err(mpsc::TryRecvError::Empty) => {
                self.paste_state = Some(state);
                ctx.request_repaint_after(Duration::from_millis(30));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.status = "Clipboard import stopped unexpectedly".into()
            }
        }
    }
    pub(super) fn start_proofing(&mut self, ctx: &Context) {
        let doc = self.session.editor.doc.clone();
        let source = (
            doc.id.clone(),
            doc.tabs[doc.active_tab].id.clone(),
            self.session.editor.revision,
        );
        let skipped = self
            .proof_state
            .take()
            .filter(|old| old.source == source)
            .map(|old| old.ignored)
            .unwrap_or_default();
        let ignored = self.prefs.personal_dictionary.clone();
        let (tx, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(crate::proofing::check(&doc, &ignored));
            ctx.request_repaint();
        });
        self.proof_state = Some(ProofState {
            receiver,
            source,
            issues: None,
            error: None,
            index: 0,
            presented: None,
            suggestion: 0,
            ignored: skipped,
        });
        self.dialog = None;
        self.refocus = false;
    }
    pub(super) fn proofing_dialog(&mut self, ctx: &Context) {
        let Some(mut state) = self.proof_state.take() else {
            return;
        };
        if let Ok(result) = state.receiver.try_recv() {
            match result {
                Ok(mut issues) => {
                    issues.retain(|i| {
                        !state.ignored.iter().any(|(address, start, word)| {
                            *address == i.address && *start == i.start && *word == i.original
                        })
                    });
                    state.issues = Some(issues);
                }
                Err(e) => state.error = Some(e),
            }
        }
        let current = (
            self.session.editor.doc.id.clone(),
            self.session.editor.doc.tabs[self.session.editor.doc.active_tab]
                .id
                .clone(),
            self.session.editor.revision,
        );
        if current != state.source {
            self.start_proofing(ctx);
            return;
        }
        if let Some(issue) = state
            .issues
            .as_ref()
            .and_then(|issues| issues.get(state.index))
        {
            let anchor = Position {
                address: issue.address,
                offset: issue.start,
            };
            let caret = Position {
                address: issue.address,
                offset: issue.end,
            };
            if state.presented != Some(state.index) {
                self.session.editor.anchor = anchor;
                self.session.editor.caret = caret;
                self.session.editor.sync_style();
                self.scroll_caret = true;
                state.presented = Some(state.index);
                state.suggestion = 0;
                ctx.request_repaint();
            }
        }
        let p = self.palette;
        let mut close = false;
        let mut replace = None;
        let mut add = None;
        let screen = ctx.content_rect();
        let top = if self.prefs.compact_menus {
            74.0
        } else {
            170.0
        };
        egui::Area::new(Id::new("proofing_card"))
            .fixed_pos(pos2(
                (screen.right() - 360.0).max(screen.left() + 8.0),
                screen.top() + top,
            ))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                Frame::popup(ui.style())
                    .fill(p.chrome)
                    .corner_radius(14)
                    .inner_margin(16)
                    .show(ui, |ui| {
                        ui.set_width(312.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Spelling and grammar").size(14.0));
                            ui.with_layout(UiLayout::right_to_left(Align::Center), |ui| {
                                close |= icons::button(
                                    ui,
                                    Icon::Close,
                                    "Close spelling and grammar",
                                    false,
                                    true,
                                )
                                .clicked();
                                let more =
                                    icons::button(ui, Icon::More, "Proofing options", false, true);
                                egui::Popup::menu(&more).show(|ui| {
                                    ui.label("English (Australia) · On-device checks");
                                    ui.label("Grammar: repeated words, articles, agreement and I.");
                                    if let Some(issue) =
                                        state.issues.as_ref().and_then(|v| v.get(state.index))
                                        && !issue.grammar
                                        && menu(ui, "Add to dictionary", "", !self.session.viewing)
                                    {
                                        add = Some(issue.original.clone());
                                    }
                                });
                                let count = state.issues.as_ref().map_or(0, Vec::len);
                                if icons::button(
                                    ui,
                                    Icon::ChevronDown,
                                    "Next issue",
                                    false,
                                    state.index + 1 < count,
                                )
                                .clicked()
                                {
                                    state.index += 1;
                                }
                                if icons::button(
                                    ui,
                                    Icon::ChevronUp,
                                    "Previous issue",
                                    false,
                                    state.index > 0,
                                )
                                .clicked()
                                {
                                    state.index -= 1;
                                }
                            });
                        });
                        ui.add_space(16.0);
                        if let Some(error) = &state.error {
                            ui.label(error);
                        } else if let Some(issues) = &state.issues {
                            if let Some(issue) = issues.get(state.index) {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label("Change");
                                    ui.label(RichText::new(&issue.original).font(FontId::new(
                                        14.0,
                                        egui::FontFamily::Name("Sans-Bold".into()),
                                    )));
                                    ui.label("to:");
                                });
                                ui.add_space(4.0);
                                ui.horizontal_wrapped(|ui| {
                                    for (index, suggestion) in issue.suggestions.iter().enumerate()
                                    {
                                        let response = ui.add_enabled(
                                            !self.session.viewing,
                                            egui::Button::new(
                                                RichText::new(suggestion).color(p.accent),
                                            )
                                            .fill(p.chrome)
                                            .stroke(Stroke::new(
                                                if state.suggestion == index { 1.0 } else { 0.5 },
                                                p.border,
                                            ))
                                            .corner_radius(20)
                                            .min_size(vec2(64.0, 36.0)),
                                        );
                                        if response.clicked() {
                                            state.suggestion = index;
                                        }
                                        if response.double_clicked() {
                                            replace = Some((issue.clone(), suggestion.clone()));
                                        }
                                    }
                                });
                                if issue.suggestions.is_empty() {
                                    ui.label(
                                        RichText::new("No dictionary suggestion").color(p.muted),
                                    );
                                }
                                ui.add_space(16.0);
                                ui.with_layout(UiLayout::right_to_left(Align::Center), |ui| {
                                    if primary(
                                        ui,
                                        "Accept",
                                        p,
                                        !self.session.viewing && !issue.suggestions.is_empty(),
                                    )
                                    .clicked()
                                    {
                                        replace = issue
                                            .suggestions
                                            .get(state.suggestion)
                                            .map(|s| (issue.clone(), s.clone()));
                                    }
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new("Ignore").color(p.accent),
                                            )
                                            .frame(false)
                                            .min_size(vec2(72.0, 40.0)),
                                        )
                                        .clicked()
                                    {
                                        state.ignored.push((
                                            issue.address,
                                            issue.start,
                                            issue.original.clone(),
                                        ));
                                        state.index += 1;
                                    }
                                });
                            } else {
                                ui.label("Check complete");
                                ui.label(
                                    RichText::new("No further issues found by these checks.")
                                        .color(p.muted),
                                );
                            }
                        } else {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Checking…");
                            });
                            ctx.request_repaint_after(Duration::from_millis(50));
                        }
                    });
            });
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            close = true;
        }
        if let Some(word) = add {
            self.prefs.personal_dictionary.push(word);
            if let Err(e) = persistence::save_preferences(&self.prefs) {
                self.status = format!("Could not save dictionary: {e}");
            }
            self.proof_state = Some(state);
            self.start_proofing(ctx);
        } else if let Some((issue, suggestion)) = replace {
            if !self.session.viewing {
                crate::proofing::replace(&mut self.session.editor, &issue, &suggestion);
            }
            let removed = issue.end - issue.start;
            let inserted = suggestion.chars().count();
            state
                .ignored
                .retain(|(a, start, _)| !(*a == issue.address && *start == issue.start));
            for (address, start, _) in &mut state.ignored {
                if *address == issue.address && *start >= issue.end {
                    *start = start.saturating_sub(removed) + inserted;
                }
            }
            state.source.2 = self.session.editor.revision;
            self.proof_state = Some(state);
            self.scroll_caret = true;
            self.start_proofing(ctx);
        } else {
            self.proof_state = Some(state);
        }
        if close {
            self.dialog = None;
            self.proof_state = None;
            self.refocus = true;
        }
    }
}

pub(super) struct PasteState {
    pub(super) receiver: Receiver<Result<crate::clipboard::Import, String>>,
    pub(super) image: Option<usize>,
    pub(super) source: (String, String, u64, Position, Position),
}
