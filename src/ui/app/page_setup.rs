use super::*;

impl AlphaDocs {
    pub(super) fn page_dialog(&mut self, ui: &mut Ui, close: &mut bool) {
        let p = self.palette;
        ui.label(RichText::new("Page setup").size(24.0));
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            for (pageless, label) in [(false, "Pages"), (true, "Pageless")] {
                let selected = self.page_draft.pageless == pageless;
                let r = ui.add_sized(
                    [168.0, 40.0],
                    egui::Button::new(RichText::new(label).color(if selected {
                        p.accent
                    } else {
                        p.text
                    }))
                    .frame(false),
                );
                tools::bounds(
                    ui,
                    if pageless {
                        "page_pageless"
                    } else {
                        "page_pages"
                    },
                    &r,
                );
                if selected {
                    ui.painter().line_segment(
                        [
                            r.rect.left_bottom() + vec2(60.0, 0.0),
                            r.rect.right_bottom() - vec2(60.0, 0.0),
                        ],
                        Stroke::new(3.0, p.accent),
                    );
                }
                if r.clicked() {
                    self.page_draft.pageless = pageless;
                }
            }
        });
        ui.separator();
        ui.add_space(12.0);
        egui::ScrollArea::vertical().max_height((ui.ctx().content_rect().height() - 300.0).max(200.0)).show(ui, |ui| {
            ui.label("Apply to");
            let scope_response=egui::ComboBox::from_id_salt("page_scope").width(348.0).selected_text("Whole document · all content tabs").show_ui(ui, |ui| {
                ui.label("Whole document · all content tabs");
                ui.add_enabled(false, egui::Button::new("This section")).on_disabled_hover_text("Independent page sections are not supported yet.");
            });
            ui.painter().rect_stroke(scope_response.response.rect,4,Stroke::new(1.0,p.border),egui::StrokeKind::Inside);
            ui.add_space(16.0);
            if self.page_draft.pageless {
                ui.label("Write on one continuous page without page breaks.");
                ui.add_space(8.0);
                ui.label(RichText::new("Paper size, margins, columns and running text are retained for printing and exports. Switch to Pages to change them.").size(12.0).color(p.muted));
                ui.add_space(16.0);
                ui.horizontal(|ui| { ui.label("Background colour"); ui.color_edit_button_srgb(&mut self.page_draft.color); });
            } else {
                ui.label("Orientation");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.page_draft.landscape, false, "Portrait");
                    ui.add_space(20.0);
                    ui.radio_value(&mut self.page_draft.landscape, true, "Landscape");
                });
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Paper size");
                        ui.add_space(6.0);
                        let paper_response=egui::ComboBox::from_id_salt("paper_size").width(250.0).height(360.0).selected_text(self.page_draft.paper.label()).show_ui(ui, |ui| {
                            for paper in Paper::STANDARD {
                                ui.selectable_value(&mut self.page_draft.paper, paper, paper.label());
                            }
                            let custom = matches!(self.page_draft.paper, Paper::Custom { .. });
                            if ui.selectable_label(custom, "Custom size…").clicked() {
                                let (width, height) = self.page_draft.paper.size();
                                self.page_draft.paper = Paper::Custom { width, height };
                            }
                        });
                        tools::bounds(ui,"page_paper",&paper_response.response);
                        ui.painter().rect_stroke(paper_response.response.rect,4,Stroke::new(1.0,p.border),egui::StrokeKind::Inside);
                    });
                    ui.vertical(|ui| { ui.label("Page colour"); ui.add_space(6.0); ui.color_edit_button_srgb(&mut self.page_draft.color); });
                });
                if let Paper::Custom { width, height } = &mut self.page_draft.paper {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        for (label, points) in [("Width", width), ("Height", height)] {
                            ui.label(label);
                            let mut inches = *points / 72.0;
                            if ui.add(egui::DragValue::new(&mut inches).speed(0.05).range(1.0..=200.0).suffix(" in")).changed() { *points = inches * 72.0; }
                        }
                    });
                }
                ui.add_space(16.0);
                ui.label("Margins (inches)");
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    for (i, label) in ["Top", "Bottom", "Left", "Right"].iter().enumerate() {
                        ui.vertical(|ui| {
                            ui.set_width(78.0);
                            ui.label(RichText::new(*label).size(12.0));
                            let mut inches = self.page_draft.margins[i] / 72.0;
                            let r = Frame::new().stroke(Stroke::new(1.0, p.border)).corner_radius(4).inner_margin(8).show(ui, |ui| {
                                ui.add_sized([58.0, 22.0], egui::DragValue::new(&mut inches).speed(0.02).range(0.0..=199.0).max_decimals(2))
                            });
                            if r.inner.changed() { self.page_draft.margins[i] = inches * 72.0; }
                        });
                    }
                });
                ui.add_space(14.0);
                egui::CollapsingHeader::new("Advanced · columns, headers and footers").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Columns"); ui.add(egui::DragValue::new(&mut self.page_draft.columns).range(1..=3));
                        ui.label("Gap (pt)"); ui.add(egui::DragValue::new(&mut self.page_draft.column_gap).range(0.0..=72.0));
                    });
                    for (label, paragraph) in [("Header", &mut self.page_draft.header), ("Footer", &mut self.page_draft.footer)] {
                        ui.label(label);
                        let mut text = paragraph.text();
                        if ui.add(egui::TextEdit::singleline(&mut text).desired_width(320.0).char_limit(512)).changed() {
                            paragraph.chars = text.chars().map(|ch| StyledChar { ch, style: paragraph.style.clone() }).collect();
                        }
                        ui.horizontal(|ui| { for (a, label) in [(Alignment::Left,"Left"),(Alignment::Center,"Centre"),(Alignment::Right,"Right")] { ui.selectable_value(&mut paragraph.alignment,a,label); } });
                    }
                    ui.checkbox(&mut self.page_draft.page_numbers, "Page numbers");
                });
            }
            if !self.page_draft.valid() { ui.colored_label(p.accent, "Choose dimensions and margins that leave at least ½ inch of usable space in each column."); }
        });
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.page_draft.valid(),
                    egui::Button::new(RichText::new("Set as default").color(p.accent)).frame(false),
                )
                .clicked()
            {
                let mut page = self.page_draft.clone();
                // Defaults store geometry, never the current document's running text.
                page.header = Paragraph::default();
                page.footer = Paragraph::default();
                self.prefs.default_page = Some(page);
                self.status = match persistence::save_preferences(&self.prefs) {
                    Ok(()) => "Page defaults saved for new documents".into(),
                    Err(e) => e,
                };
            }
            ui.with_layout(UiLayout::right_to_left(Align::Center), |ui| {
                let ok = primary(
                    ui,
                    "OK",
                    p,
                    self.page_draft.valid() && !self.session.viewing,
                );
                tools::bounds(ui, "page_ok", &ok);
                if ok.clicked() {
                    let page = self.page_draft.clone();
                    self.session
                        .editor
                        .transaction(|e| e.doc.page = page.clone());
                    *close = true;
                }
                let cancel = ui.button("Cancel");
                tools::bounds(ui, "page_cancel", &cancel);
                if cancel.clicked() {
                    *close = true;
                }
            });
        });
    }
}
