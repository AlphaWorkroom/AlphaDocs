use super::*;

impl AlphaDocs {
    pub(super) fn input(&mut self, ctx: &Context) {
        if self.home || self.dialog.is_some() || self.feature_search_open {
            return;
        }
        if ctx
            .input_mut(|i| i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::S))
        {
            self.execute(Action::SaveAs, ctx);
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::ALT, Key::X))
        {
            self.execute(Action::Proofing, ctx);
            return;
        }
        let mac = ctx.os() == egui::os::OperatingSystem::Mac;
        // Cmd+H belongs to macOS Hide. Find/replace uses Cmd+Shift+H.
        if mac
            && ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::H)
            })
        {
            self.execute(Action::Find, ctx);
            return;
        }
        for (key, action) in [
            (Key::S, Action::Save),
            (Key::O, Action::Open),
            (Key::F, Action::Find),
            (Key::H, Action::Find),
            (Key::P, Action::Print),
        ] {
            if mac && key == Key::H {
                continue;
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, key)) {
                self.execute(action, ctx);
                return;
            }
        }
        let canvas_focused = ctx.memory(|m| m.has_focus(Id::new("document_canvas")));
        // A comment/numeric/title field owns typing, even while an image remains
        // selected. Otherwise typing into the sidebar could delete the image.
        if !canvas_focused
            && ctx
                .memory(|m| m.focused())
                .is_some_and(|id| egui::TextEdit::load_state(ctx, id).is_some())
        {
            return;
        }
        // An image is a document selection even if a non-text toolbar button
        // currently owns toolkit focus. Sidebar/title text fields returned above.
        let canvas_focused = canvas_focused || self.image_block.is_some();
        if !self.session.viewing
            && (self.image_block.is_some() || self.session.editor.selected())
            && ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::ALT, Key::M)
            })
        {
            self.begin_comment();
            return;
        }
        if canvas_focused && let Some(block) = self.image_block {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
                self.image_block = None;
                self.image_selection_id = None;
                self.image_resize = None;
                self.image_drag = None;
                ctx.request_repaint();
                return;
            }
            let events = ctx.input(|i| i.events.clone());
            let copy = events.iter().any(|e| matches!(e, Event::Copy | Event::Cut));
            let cut = events.iter().any(|e| matches!(e, Event::Cut)) && !self.session.viewing;
            if copy && let Some(Block::Image(image)) = self.session.editor.doc.blocks.get(block) {
                let text = image.name.clone();
                self.clipboard = Some(ClipboardContent {
                    text: text.clone(),
                    blocks: vec![Block::Image(image.clone())],
                });
                if self
                    .clipboard
                    .as_ref()
                    .is_none_or(|f| crate::clipboard::write(f).is_err())
                {
                    ctx.copy_text(text);
                }
                ctx.input_mut(|i| i.events.retain(|e| !matches!(e, Event::Copy | Event::Cut)));
            }
            let delete = !self.session.viewing
                && (cut
                    || ctx.input_mut(|i| {
                        i.consume_key(egui::Modifiers::NONE, Key::Delete)
                            || i.consume_key(egui::Modifiers::NONE, Key::Backspace)
                    }));
            let replace =
                !self.session.viewing && events.iter().any(|e| matches!(e, Event::Text(_)));
            if delete || replace {
                self.session.editor.delete_image(block);
                self.image_block = None;
                self.image_selection_id = None;
                self.image_resize = None;
                self.refocus = true;
                ctx.request_repaint();
                if delete {
                    return;
                }
            }
        }
        let field_focused = ctx
            .memory(|m| m.focused())
            .is_some_and(|id| egui::TextEdit::load_state(ctx, id).is_some());
        if field_focused && !canvas_focused {
            return;
        }
        if !canvas_focused {
            if ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::Z)
            }) {
                if !self.session.viewing {
                    self.execute(Action::Redo, ctx);
                }
                return;
            }
            // Toolbar buttons must not swallow document commands. Editable title,
            // search and numeric fields retain their normal text shortcuts.
            for (key, action) in [
                (Key::Z, Action::Undo),
                (Key::Y, Action::Redo),
                (Key::A, Action::SelectAll),
                (Key::K, Action::Link),
            ] {
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, key)) {
                    if !self.session.viewing || matches!(action, Action::SelectAll) {
                        self.execute(action, ctx);
                    }
                    return;
                }
            }
            if !self.session.viewing {
                for (key, field) in [(Key::B, 0), (Key::I, 1), (Key::U, 2)] {
                    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, key)) {
                        self.toggle_style(field);
                        return;
                    }
                }
            }
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        let mut edited = false;
        for event in events {
            match event {
                Event::Text(t) if !self.session.viewing => {
                    if self.ime_preedit.is_empty() {
                        self.session.editor.insert(&t);
                        edited = true;
                    }
                }
                Event::Paste(t) if !self.session.viewing => {
                    self.paste_plain |= ctx.input(|i| i.modifiers.shift);
                    self.paste_text(&t);
                    edited = true;
                }
                Event::Copy => self.copy_selection(ctx, false),
                Event::Cut => {
                    self.copy_selection(ctx, true);
                }
                Event::Ime(egui::ImeEvent::Commit(t)) if !self.session.viewing => {
                    self.ime_preedit.clear();
                    self.session.editor.insert(&t);
                    edited = true;
                }
                Event::Ime(egui::ImeEvent::Preedit { text, .. }) => self.ime_preedit = text,
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    let cmd = modifiers.command;
                    let shift = modifiers.shift;
                    if self.session.viewing
                        && !(matches!(
                            key,
                            Key::A
                                | Key::ArrowLeft
                                | Key::ArrowRight
                                | Key::ArrowUp
                                | Key::ArrowDown
                                | Key::Home
                                | Key::End
                                | Key::PageUp
                                | Key::PageDown
                        ))
                    {
                        continue;
                    }
                    if mac && self.mac_navigation(key, modifiers) {
                        edited = true;
                        continue;
                    }
                    if cmd {
                        if modifiers.alt {
                            let heading = match key {
                                Key::Num0 => Some(0),
                                Key::Num1 => Some(1),
                                Key::Num2 => Some(2),
                                Key::Num3 => Some(3),
                                Key::Num4 => Some(4),
                                Key::Num5 => Some(5),
                                Key::Num6 => Some(6),
                                _ => None,
                            };
                            if let Some(heading) = heading {
                                self.session
                                    .editor
                                    .paragraph_format(|p| p.set_heading(heading));
                                continue;
                            }
                        }
                        match key {
                            Key::A => self.session.editor.select_all(),
                            Key::Z => {
                                if shift {
                                    self.session.editor.redo();
                                } else {
                                    self.session.editor.undo();
                                }
                            }
                            Key::Y => self.session.editor.redo(),
                            Key::B => {
                                let v = !self.session.editor.typing.bold;
                                self.session.editor.format(|s| s.bold = v);
                            }
                            Key::I => {
                                let v = !self.session.editor.typing.italic;
                                self.session.editor.format(|s| s.italic = v);
                            }
                            Key::U => {
                                let v = !self.session.editor.typing.underline;
                                self.session.editor.format(|s| s.underline = v);
                            }
                            Key::Period if shift => self
                                .session
                                .editor
                                .format(|s| s.size = (s.size + 1.0).min(400.0)),
                            Key::Comma if shift => self
                                .session
                                .editor
                                .format(|s| s.size = (s.size - 1.0).max(1.0)),
                            Key::Period => self.toggle_baseline(Baseline::Superscript),
                            Key::Comma => self.toggle_baseline(Baseline::Subscript),
                            Key::X if shift => self.toggle_style(3),
                            Key::Num7 if shift => self
                                .session
                                .editor
                                .paragraph_format(|p| p.list = ListKind::Number),
                            Key::Num8 if shift => self
                                .session
                                .editor
                                .paragraph_format(|p| p.list = ListKind::Bullet),
                            Key::Num9 if shift => self
                                .session
                                .editor
                                .paragraph_format(|p| p.list = ListKind::Check),
                            Key::Backslash => self.execute(Action::Clear, ctx),
                            Key::K => self.execute(Action::Link, ctx),
                            Key::V => self.paste_plain = shift,
                            Key::Enter => self.execute(Action::Break, ctx),
                            Key::L if shift => self
                                .session
                                .editor
                                .paragraph_format(|p| p.alignment = Alignment::Left),
                            Key::E if shift => self
                                .session
                                .editor
                                .paragraph_format(|p| p.alignment = Alignment::Center),
                            Key::R if shift => self
                                .session
                                .editor
                                .paragraph_format(|p| p.alignment = Alignment::Right),
                            Key::J if shift => self
                                .session
                                .editor
                                .paragraph_format(|p| p.alignment = Alignment::Justified),
                            _ => {}
                        }
                    }
                    match key {
                        Key::ArrowLeft => self.session.editor.move_horizontal(false, shift, cmd),
                        Key::ArrowRight => self.session.editor.move_horizontal(true, shift, cmd),
                        Key::ArrowUp | Key::ArrowDown => {
                            if let Some(l) = &self.layout {
                                self.session.editor.place(
                                    l.vertical(self.session.editor.caret, key == Key::ArrowDown),
                                    shift,
                                );
                            }
                        }
                        Key::Home | Key::End => {
                            if cmd {
                                let a = self.session.editor.doc.addresses();
                                let address = if key == Key::Home {
                                    a[0]
                                } else {
                                    *a.last().expect("paragraph")
                                };
                                let offset = if key == Key::Home {
                                    0
                                } else {
                                    self.session
                                        .editor
                                        .doc
                                        .paragraph(address)
                                        .expect("paragraph")
                                        .chars
                                        .len()
                                };
                                self.session
                                    .editor
                                    .place(Position { address, offset }, shift);
                            } else if let Some(l) = &self.layout {
                                self.session.editor.place(
                                    l.boundary(self.session.editor.caret, key == Key::End),
                                    shift,
                                );
                            }
                        }
                        Key::PageUp | Key::PageDown => {
                            if let Some(l) = &self.layout {
                                let mut pos = self.session.editor.caret;
                                for _ in 0..25 {
                                    pos = l.vertical(pos, key == Key::PageDown);
                                }
                                self.session.editor.place(pos, shift);
                            }
                        }
                        Key::Enter if !cmd => {
                            if shift {
                                self.session.editor.soft_break();
                            } else {
                                self.session.editor.enter();
                            }
                        }
                        Key::Backspace => {
                            if cmd {
                                self.session.editor.delete_word(true);
                            } else {
                                self.session.editor.delete(true);
                            }
                        }
                        Key::Delete => {
                            if cmd {
                                self.session.editor.delete_word(false);
                            } else {
                                self.session.editor.delete(false);
                            }
                        }
                        Key::Tab if !self.session.viewing => self.session.editor.tab(shift),
                        _ => {}
                    }
                    edited = true;
                }
                _ => {}
            }
        }
        ctx.input_mut(|i| {
            i.events.retain(|e| {
                !matches!(
                    e,
                    Event::Text(_)
                        | Event::Paste(_)
                        | Event::Copy
                        | Event::Cut
                        | Event::Ime(_)
                        | Event::Key { .. }
                )
            })
        });
        if edited {
            self.scroll_caret = true;
            ctx.request_repaint();
        }
    }
}

impl AlphaDocs {
    /// Mac Command arrows use measured visual lines/document bounds; Option
    /// arrows and deletion use the engine's Unicode word boundaries.
    fn mac_navigation(&mut self, key: Key, modifiers: egui::Modifiers) -> bool {
        let cmd = modifiers.command;
        let shift = modifiers.shift;
        if cmd && matches!(key, Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End) {
            let addresses = self.session.editor.doc.addresses();
            let end = matches!(key, Key::ArrowDown | Key::End);
            let address = if end {
                *addresses.last().expect("paragraph")
            } else {
                addresses[0]
            };
            let offset = if end {
                self.session
                    .editor
                    .doc
                    .paragraph(address)
                    .expect("paragraph")
                    .chars
                    .len()
            } else {
                0
            };
            self.session
                .editor
                .place(Position { address, offset }, shift);
            return true;
        }
        if cmd
            && matches!(
                key,
                Key::ArrowLeft | Key::ArrowRight | Key::Backspace | Key::Delete
            )
        {
            let end = matches!(key, Key::ArrowRight | Key::Delete);
            if let Some(layout) = &self.layout {
                let destination = layout.boundary(self.session.editor.caret, end);
                if matches!(key, Key::Backspace | Key::Delete) {
                    if !self.session.editor.selected() {
                        self.session.editor.place(destination, true);
                    }
                    if self.session.editor.selected() {
                        self.session.editor.delete(!end);
                    }
                } else {
                    self.session.editor.place(destination, shift);
                }
            }
            return true;
        }
        if modifiers.alt && !cmd {
            match key {
                Key::ArrowLeft | Key::ArrowRight => {
                    self.session
                        .editor
                        .move_horizontal(key == Key::ArrowRight, shift, true)
                }
                Key::Backspace | Key::Delete => {
                    self.session.editor.delete_word(key == Key::Backspace)
                }
                _ => return false,
            }
            return true;
        }
        false
    }
}
