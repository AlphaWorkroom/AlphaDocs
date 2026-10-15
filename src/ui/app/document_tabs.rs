use super::*;

impl AlphaDocs {
    pub(super) fn document_tabs_panel(&mut self, ui: &mut Ui) {
        if !self.prefs.outline {
            return;
        }
        let palette = self.palette;
        let mut switch = None;
        let mut duplicate = None;
        let mut delete = None;
        let mut add = false;
        let mut rename = None;
        let mut heading_target = None;
        let panel = egui::Panel::left("outline")
            .exact_size(280.0)
            .resizable(false)
            .frame(Frame::new().fill(palette.bg).inner_margin(Margin::same(24)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Document tabs").size(14.0));
                    ui.with_layout(UiLayout::right_to_left(Align::Center), |ui| {
                        if icons::button_with_id(
                            ui,
                            Id::new("add_document_tab"),
                            Icon::Plus,
                            "Add document tab",
                            false,
                            !self.session.viewing,
                        )
                        .clicked()
                        {
                            add = true;
                        }
                        if icons::button(ui, Icon::Close, "Hide document tabs", false, true)
                            .clicked()
                        {
                            self.prefs.outline = false;
                        }
                    });
                });
                ui.add_space(16.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (index, tab) in self.session.editor.doc.tabs.iter().enumerate() {
                        let active = index == self.session.editor.doc.active_tab;
                        ui.push_id((&tab.id, "content_tab"), |ui| {
                            let row_width = ui.available_width();
                            Frame::new()
                                .fill(if active {
                                    palette.selection
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .corner_radius(22)
                                .inner_margin(Margin::symmetric(8, 4))
                                .show(ui, |ui| {
                                    ui.set_width((row_width - 16.0).max(120.0));
                                    ui.horizontal(|ui| {
                                        let (rect, _) = ui
                                            .allocate_exact_size(vec2(20.0, 24.0), Sense::hover());
                                        icons::draw(
                                            ui,
                                            rect,
                                            Icon::Document,
                                            if active { palette.accent } else { palette.text },
                                        );
                                        let (_, rect) = ui.allocate_space(vec2(
                                            (ui.available_width()
                                                - 30.0
                                                - ui.spacing().item_spacing.x)
                                                .max(60.0),
                                            30.0,
                                        ));
                                        let response = ui.interact(
                                            rect,
                                            Id::new(("content_tab", &tab.id)),
                                            Sense::click(),
                                        );
                                        response.widget_info(|| {
                                            egui::WidgetInfo::selected(
                                                egui::WidgetType::Button,
                                                true,
                                                active,
                                                &tab.title,
                                            )
                                        });
                                        if response.hovered() {
                                            ui.painter().rect_filled(
                                                rect,
                                                4.0,
                                                ui.visuals().widgets.hovered.weak_bg_fill,
                                            );
                                        }
                                        ui.painter().with_clip_rect(rect).text(
                                            rect.left_center(),
                                            Align2::LEFT_CENTER,
                                            truncate(&tab.title, 23),
                                            FontId::proportional(14.0),
                                            if active { palette.accent } else { palette.text },
                                        );
                                        if response.clicked() {
                                            switch = Some(index);
                                        }
                                        if response.double_clicked() && !self.session.viewing {
                                            self.tab_rename =
                                                Some((tab.id.clone(), tab.title.clone()));
                                            self.tab_rename_focus = true;
                                            switch = None;
                                        }
                                        let menu = icons::button(
                                            ui,
                                            Icon::More,
                                            &format!("Options for {}", tab.title),
                                            false,
                                            true,
                                        );
                                        egui::Popup::menu(&menu).show(|ui| {
                                            if ui
                                                .add_enabled(
                                                    !self.session.viewing,
                                                    egui::Button::new("Rename"),
                                                )
                                                .clicked()
                                            {
                                                self.tab_rename =
                                                    Some((tab.id.clone(), tab.title.clone()));
                                                self.tab_rename_focus = true;
                                                ui.close();
                                            }
                                            if ui
                                                .add_enabled(
                                                    !self.session.viewing,
                                                    egui::Button::new("Duplicate"),
                                                )
                                                .clicked()
                                            {
                                                duplicate = Some(index);
                                                ui.close();
                                            }
                                            if ui
                                                .add_enabled(
                                                    !self.session.viewing
                                                        && self.session.editor.doc.tabs.len() > 1,
                                                    egui::Button::new("Delete tab"),
                                                )
                                                .clicked()
                                            {
                                                delete = Some(index);
                                                ui.close();
                                            }
                                        });
                                    });
                                });
                            if let Some((id, title)) =
                                self.tab_rename.as_mut().filter(|(id, _)| *id == tab.id)
                            {
                                ui.horizontal(|ui| {
                                    let field_id = Id::new(("content_tab_rename", id.as_str()));
                                    let submit = ui.ctx().memory(|m| m.has_focus(field_id))
                                        && ui.input_mut(|i| {
                                            i.consume_key(egui::Modifiers::NONE, Key::Enter)
                                        });
                                    let field = ui.add_enabled(
                                        !self.session.viewing,
                                        egui::TextEdit::singleline(title)
                                            .id(field_id)
                                            .desired_width(150.0)
                                            .hint_text("Tab name"),
                                    );
                                    if self.tab_rename_focus {
                                        field.request_focus();
                                        self.tab_rename_focus = false;
                                    }
                                    if !self.session.viewing
                                        && (ui.button("Save").clicked() || submit)
                                    {
                                        rename = Some((id.clone(), title.clone()));
                                    }
                                });
                            }
                            if active {
                                ui.add_space(8.0);
                                if self.content_cache.headings.is_empty() {
                                    ui.label(
                                        RichText::new(
                                            "Headings you add to this tab will appear here.",
                                        )
                                        .italics()
                                        .size(12.0)
                                        .color(palette.muted),
                                    );
                                }
                                for (address, heading, title) in &self.content_cache.headings {
                                    ui.horizontal(|ui| {
                                        ui.add_space(
                                            8.0 + heading.saturating_sub(1).min(5) as f32 * 10.0,
                                        );
                                        let (rect, _) = ui.allocate_exact_size(
                                            vec2(ui.available_width().max(1.0), 30.0),
                                            Sense::hover(),
                                        );
                                        let response = ui.interact(
                                            rect,
                                            Id::new(("outline_heading", address)),
                                            Sense::click(),
                                        );
                                        response.widget_info(|| {
                                            egui::WidgetInfo::labeled(
                                                egui::WidgetType::Button,
                                                true,
                                                title,
                                            )
                                        });
                                        if response.hovered() {
                                            ui.painter().rect_filled(rect, 4.0, palette.hover);
                                        }
                                        ui.painter().text(
                                            rect.left_center() + vec2(4.0, 0.0),
                                            Align2::LEFT_CENTER,
                                            truncate(title, 24),
                                            FontId::proportional(14.0),
                                            palette.text,
                                        );
                                        if response.clicked() {
                                            heading_target = Some(*address);
                                        }
                                    });
                                }
                                ui.add_space(12.0);
                            }
                        });
                    }
                });
            });
        self.feature_destination(feature_search::Target::Tabs, panel.response.rect);
        let changed = add || duplicate.is_some() || delete.is_some() || switch.is_some();
        if add {
            self.session.editor.add_tab();
        }
        if let Some(index) = duplicate {
            self.session.editor.duplicate_tab(index);
        }
        if let Some(index) = delete {
            self.session.editor.delete_tab(index);
        }
        if let Some(index) = switch {
            self.session.editor.switch_tab(index);
        }
        if let Some((id, title)) = rename {
            if let Some(index) = self
                .session
                .editor
                .doc
                .tabs
                .iter()
                .position(|tab| tab.id == id)
            {
                self.session.editor.rename_tab(index, &title);
            }
            self.tab_rename = None;
            self.refocus = true;
        }
        if changed {
            self.reset_document_view();
            self.refresh_content_cache();
        }
        if self.tab_rename.is_some() {
            self.refocus = false;
        }
        if let Some(address) = heading_target {
            self.session
                .editor
                .place(Position { address, offset: 0 }, false);
            self.scroll_caret = true;
            self.refocus = true;
        }
    }
}
