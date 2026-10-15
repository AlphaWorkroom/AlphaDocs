use super::*;

impl AlphaDocs {
    pub(super) fn toggle_menus(&mut self, ctx: &Context) {
        self.prefs.compact_menus = !self.prefs.compact_menus;
        self.refocus = true;
        #[cfg(not(test))]
        if let Err(error) = persistence::save_preferences(&self.prefs) {
            self.status = format!("Could not save menu preference: {error}");
        }
        ctx.request_repaint();
    }
    pub(super) fn toolbar_shell(
        &mut self,
        ui: &mut Ui,
        id: &str,
        contents: impl FnOnce(&mut Self, &mut Ui),
    ) {
        Frame::new()
            .fill(self.palette.toolbar)
            .corner_radius(22)
            .inner_margin(Margin::symmetric(12, 4))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    let tools_width = (ui.available_width() - 206.0).max(32.0);
                    ui.allocate_ui_with_layout(
                        vec2(tools_width, 32.0),
                        UiLayout::left_to_right(Align::Center),
                        |ui| {
                            egui::ScrollArea::horizontal()
                                .id_salt(id)
                                .max_width(tools_width)
                                .scroll_bar_visibility(
                                    egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                                )
                                .auto_shrink([false, true])
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| contents(self, ui));
                                });
                        },
                    );
                    ui.with_layout(UiLayout::right_to_left(Align::Center), |ui| {
                        self.mode_controls(ui);
                    });
                });
            });
    }
    fn mode_controls(&mut self, ui: &mut Ui) {
        let p = self.palette;
        let label = if self.prefs.compact_menus {
            "Show the menus (Ctrl+Shift+F)"
        } else {
            "Hide the menus (Ctrl+Shift+F)"
        };
        if icons::button_with_id(
            ui,
            Id::new("toggle_menus_button"),
            if self.prefs.compact_menus {
                Icon::ChevronDown
            } else {
                Icon::ChevronUp
            },
            label,
            false,
            true,
        )
        .clicked()
        {
            self.toggle_menus(ui.ctx());
        }
        let (separator, _) = ui.allocate_exact_size(vec2(8.0, 24.0), Sense::hover());
        ui.painter().line_segment(
            [
                separator.center_top() + vec2(0.0, 2.0),
                separator.center_bottom() - vec2(0.0, 2.0),
            ],
            Stroke::new(1.0, p.border),
        );
        let (_, rect) = ui.allocate_space(vec2(148.0, 30.0));
        let response = ui
            .interact(rect, Id::new("mode_selector"), Sense::click())
            .on_hover_text("Change document mode");
        let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&response));
        if response.hovered() || response.has_focus() || open {
            ui.painter().rect_filled(rect, 18, p.hover);
        }
        let label = if self.session.viewing {
            "Viewing"
        } else {
            "Editing"
        };
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
        icons::draw(
            ui,
            Rect::from_center_size(rect.left_center() + vec2(21.0, 0.0), vec2(18.0, 18.0)),
            if self.session.viewing {
                Icon::View
            } else {
                Icon::Edit
            },
            p.text,
        );
        ui.painter().text(
            rect.left_center() + vec2(40.0, 0.0),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(14.0),
            p.text,
        );
        let c = rect.right_center() - vec2(17.0, 0.0);
        let direction = if open { -1.0 } else { 1.0 };
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                c + vec2(-3.5, -1.5 * direction),
                c + vec2(3.5, -1.5 * direction),
                c + vec2(0.0, 2.0 * direction),
            ],
            p.muted,
            Stroke::NONE,
        ));
        egui::Popup::menu(&response)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| self.mode_choices(ui));
    }
    pub(super) fn mode_choices(&mut self, ui: &mut Ui) {
        let p = self.palette;
        ui.set_width(242.0);
        ui.spacing_mut().item_spacing.y = 0.0;
        for (index, icon, label, description, enabled) in [
            (
                0,
                Icon::Edit,
                "Editing",
                "Edit document directly",
                self.session.editor.collaboration_writable(),
            ),
            (
                1,
                Icon::Suggest,
                "Suggesting",
                "Tracked suggestions unavailable",
                false,
            ),
            (
                2,
                Icon::View,
                "Viewing",
                "Read or print final document",
                true,
            ),
        ] {
            let selected =
                (index == 0 && !self.session.viewing) || (index == 2 && self.session.viewing);
            let (_, rect) = ui.allocate_space(vec2(ui.available_width(), 50.0));
            let response = ui.interact(
                rect,
                Id::new(("document_mode", index)),
                if enabled {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            response.widget_info(|| {
                egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, label)
            });
            if enabled && (response.hovered() || response.has_focus()) {
                ui.painter().rect_filled(rect, 4, p.hover);
            }
            let color = if enabled {
                p.text
            } else {
                p.muted.gamma_multiply(0.65)
            };
            icons::draw(
                ui,
                Rect::from_center_size(rect.left_top() + vec2(17.0, 17.0), vec2(18.0, 18.0)),
                icon,
                color,
            );
            ui.painter().text(
                rect.left_top() + vec2(34.0, 15.0),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(14.0),
                color,
            );
            ui.painter().text(
                rect.left_top() + vec2(34.0, 35.0),
                Align2::LEFT_CENTER,
                description,
                FontId::proportional(12.0),
                if enabled { p.muted } else { color },
            );
            if selected {
                icons::draw(
                    ui,
                    Rect::from_center_size(rect.right_top() + vec2(-15.0, 17.0), vec2(18.0, 18.0)),
                    Icon::Resolve,
                    p.text,
                );
            }
            if !enabled {
                response.on_hover_text("Suggesting requires tracked edits and accept/reject support. It cannot be selected in this version.");
                continue;
            }
            if response.clicked()
                || (response.has_focus()
                    && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter)))
            {
                self.session.viewing = index == 2;
                self.refocus = true;
                ui.close();
            }
        }
    }
}
