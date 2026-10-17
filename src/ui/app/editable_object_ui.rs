use super::*;

pub(super) fn outlined<R>(ui: &mut Ui, palette: Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .stroke(Stroke::new(1.0, palette.border))
        .corner_radius(4)
        .inner_margin(8)
        .show(ui, add)
        .inner
}

pub(super) struct Draft {
    document: String,
    tab: usize,
    revision: u64,
    caret: Position,
    anchor: Position,
    target: Option<String>,
    source: ObjectSource,
    text: String,
    width: f32,
    height: f32,
    offset: [f32; 2],
    error: String,
    preview: Option<(EmbeddedImage, egui::TextureHandle)>,
}

impl AlphaDocs {
    pub(super) fn open_editable_object(
        &mut self,
        kind: Option<ShapeKind>,
        existing: Option<usize>,
    ) {
        let (source, target, width, height, offset) = if let Some(block) = existing {
            let Some(Block::Image(image)) = self.session.editor.doc.blocks.get(block) else {
                return;
            };
            let Some(object) = &image.editable else {
                return;
            };
            (
                object.source.clone(),
                Some(image.id.clone()),
                image.width,
                image.height,
                image.placement.offset,
            )
        } else {
            if !matches!(
                self.session
                    .editor
                    .doc
                    .blocks
                    .get(self.session.editor.caret.address.block),
                Some(Block::Paragraph(_))
            ) {
                self.status = "Place the cursor in a body paragraph to insert an object".into();
                return;
            }
            (
                if let Some(kind) = kind {
                    ObjectSource::Drawing {
                        kind,
                        text: Box::new(Paragraph::from_text(
                            if matches!(kind, ShapeKind::Line | ShapeKind::Arrow) {
                                ""
                            } else {
                                "Your text here"
                            },
                        )),
                        fill: None,
                        border: CellBorder {
                            colour: [95; 3],
                            width: 1.0,
                        },
                    }
                } else {
                    ObjectSource::Equation {
                        latex: r"\frac{-b \pm \sqrt{b^2-4ac}}{2a}".into(),
                        size: 18.0,
                        colour: [31; 3],
                    }
                },
                None,
                240.0,
                100.0,
                [0.0; 2],
            )
        };
        let text = match &source {
            ObjectSource::Drawing { text, .. } => text.text(),
            _ => String::new(),
        };
        self.object_draft = Some(Draft {
            document: self.session.editor.doc.id.clone(),
            tab: self.session.editor.doc.active_tab,
            revision: self.session.editor.revision,
            caret: self.session.editor.caret,
            anchor: self.session.editor.anchor,
            target,
            source,
            text,
            width,
            height,
            offset,
            error: String::new(),
            preview: None,
        });
        self.dialog = Some(Dialog::EditableObject);
    }
    pub(super) fn editable_object_dialog(&mut self, ui: &mut Ui, close: &mut bool) {
        let Some(mut draft) = self.object_draft.take() else {
            *close = true;
            return;
        };
        let title = if matches!(draft.source, ObjectSource::Equation { .. }) {
            "Equation"
        } else {
            "Text box and shape"
        };
        ui.label(RichText::new(title).size(24.0));
        ui.add_space(12.0);
        let valid = draft.document == self.session.editor.doc.id
            && draft.tab == self.session.editor.doc.active_tab
            && draft.revision == self.session.editor.revision
            && draft.caret == self.session.editor.caret
            && draft.anchor == self.session.editor.anchor;
        let mut apply = false;
        let mut generate = false;
        egui::ScrollArea::vertical().id_salt("editable_object_fields").max_height((ui.ctx().content_rect().height()-180.0).max(160.0)).show(ui,|ui| {
        match &mut draft.source {
            ObjectSource::Equation {
                latex,
                size,
                colour,
            } => {
                ui.label("LaTeX equation");
                outlined(ui, self.palette, |ui| ui.add(
                    egui::TextEdit::multiline(latex)
                        .frame(Frame::NONE)
                        .desired_width(f32::INFINITY)
                        .desired_rows(3),
                ));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Size");
                    ui.add(egui::DragValue::new(size).range(6.0..=100.0).suffix(" pt"));
                    ui.label("Colour");
                    ui.color_edit_button_srgb(colour);
                });
                ui.add_space(8.0);
                ui.label(
                    RichText::new(r"Fractions: \frac{a}{b}   Powers: x^{2}   Roots: \sqrt{x}")
                        .size(12.0)
                        .color(self.palette.muted),
                );
            }
            ObjectSource::Drawing {
                kind,
                text,
                fill,
                border,
            } => {
                egui::ComboBox::from_label("Shape")
                    .selected_text(kind.label())
                    .show_ui(ui, |ui| {
                        for value in ShapeKind::ALL {
                            ui.selectable_value(kind, value, value.label());
                        }
                    });
                if !matches!(kind, ShapeKind::Line | ShapeKind::Arrow) {
                    ui.add_space(8.0);
                    ui.label("Text");
                    if outlined(ui, self.palette, |ui| ui.add(
                            egui::TextEdit::multiline(&mut draft.text)
                                .frame(Frame::NONE)
                                .desired_width(f32::INFINITY)
                                .desired_rows(4),
                        ))
                        .changed()
                    {
                        **text = crate::engine::edit_drawing_text(text, &draft.text);
                    }
                    ui.horizontal(|ui| {
                        let mut font = text.style.font.clone();
                        egui::ComboBox::from_id_salt("drawing_font")
                            .selected_text(&font)
                            .height(220.0)
                            .show_ui(ui, |ui| {
                                for name in fonts::families() {
                                    ui.selectable_value(&mut font, name.clone(), name);
                                }
                            });
                        if font != text.style.font {
                            text.style.font = font.clone();
                            for ch in &mut text.chars { ch.style.font = font.clone(); }
                        }
                        for (label, property) in [("Italic", 0), ("Underline", 1)] {
                            let value = if property == 0 { text.style.italic } else { text.style.underline };
                            if ui.selectable_label(value, label).clicked() {
                                if property == 0 { text.style.italic = !value; } else { text.style.underline = !value; }
                                for ch in &mut text.chars {
                                    if property == 0 { ch.style.italic = !value; } else { ch.style.underline = !value; }
                                }
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        let mut size = text.style.size;
                        if ui
                            .add(
                                egui::DragValue::new(&mut size)
                                    .range(6.0..=100.0)
                                    .suffix(" pt"),
                            )
                            .changed()
                        {
                            text.style.size = size;
                            for ch in &mut text.chars {
                                ch.style.size = size;
                            }
                        }
                        let mut bold = text.style.bold;
                        if ui.selectable_label(bold, "Bold").clicked() {
                            bold = !bold;
                            text.style.bold = bold;
                            for ch in &mut text.chars {
                                ch.style.bold = bold;
                            }
                        }
                        let mut colour = text.style.color;
                        if ui.color_edit_button_srgb(&mut colour).changed() {
                            text.style.color = colour;
                            for ch in &mut text.chars {
                                ch.style.color = colour;
                            }
                        }
                        egui::ComboBox::from_id_salt("drawing_alignment")
                            .selected_text(match text.alignment {
                                Alignment::Center => "Centre",
                                Alignment::Right => "Right",
                                _ => "Left",
                            })
                            .show_ui(ui, |ui| {
                                for (value, name) in [
                                    (Alignment::Left, "Left"),
                                    (Alignment::Center, "Centre"),
                                    (Alignment::Right, "Right"),
                                ] {
                                    ui.selectable_value(&mut text.alignment, value, name);
                                }
                            });
                    });
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let mut enabled = fill.is_some();
                    if preferences::checkbox(ui, &mut enabled, "Fill", self.palette).changed() {
                        *fill = enabled.then_some([211, 227, 253]);
                    }
                    if let Some(colour) = fill {
                        ui.color_edit_button_srgb(colour);
                    }
                    ui.label("Border");
                    ui.color_edit_button_srgb(&mut border.colour);
                    ui.add(
                        egui::DragValue::new(&mut border.width)
                            .range(0.0..=12.0)
                            .suffix(" pt"),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Width");
                    ui.add(
                        egui::DragValue::new(&mut draft.width)
                            .range(24.0..=1500.0)
                            .suffix(" pt"),
                    );
                    ui.label("Height");
                    ui.add(
                        egui::DragValue::new(&mut draft.height)
                            .range(12.0..=1500.0)
                            .suffix(" pt"),
                    );
                });
                ui.label(RichText::new("Height grows to fit text. Drag the object or its handles on the page to position or resize it.").size(12.0).color(self.palette.muted));
            }
        }
        ui.add_space(12.0);
        generate = ui.button("Preview").clicked() || draft.preview.is_none() && draft.error.is_empty();
        if let Some((_, texture)) = &draft.preview {
            let size = texture.size_vec2();
            let ratio = (ui.available_width() / size.x).min(130.0 / size.y).min(1.0);
            ui.add_space(8.0);
            Frame::new()
                .fill(Color32::WHITE)
                .inner_margin(12)
                .corner_radius(8)
                .show(ui, |ui| {
                    ui.add(egui::Image::new((texture.id(), size * ratio)));
                });
        }
        if !draft.error.is_empty() {
            ui.add_space(8.0);
            ui.colored_label(self.palette.text, &draft.error);
        }
        if !valid {
            ui.label("The document changed. Reopen this editor before applying.");
        }
        });
        ui.add_space(16.0);
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), 40.0),
            UiLayout::right_to_left(Align::Center),
            |ui| {
                let button = primary(
                    ui,
                    if draft.target.is_some() {
                        "Apply"
                    } else {
                        "Insert"
                    },
                    self.palette,
                    valid && !self.session.viewing,
                );
                tools::bounds(ui, "editable_object_apply", &button);
                if button.clicked() {
                    generate = true;
                    apply = true;
                }
                let cancel = ui.button("Cancel");
                tools::bounds(ui, "editable_object_cancel", &cancel);
                if cancel.clicked() {
                    *close = true;
                }
            },
        );
        if generate {
            match crate::editable_objects::prepare(
                ui.ctx(),
                draft.source.clone(),
                draft.width,
                draft.height,
            ) {
                Ok(mut image) => {
                    let generated = image::load_from_memory(&image.data).map(|im| im.to_rgba8());
                    if let Ok(decoded) = generated {
                        let texture = ui.ctx().load_texture(
                            "editable_object_preview",
                            egui::ColorImage::from_rgba_unmultiplied(
                                [decoded.width() as usize, decoded.height() as usize],
                                decoded.as_raw(),
                            ),
                            egui::TextureOptions::LINEAR,
                        );
                        draft.preview = Some((image.clone(), texture));
                        draft.error.clear();
                    }
                    if apply && valid && !self.session.viewing {
                        image.placement.offset = draft.offset;
                        if let Some(id) = &draft.target {
                            let target = self
                                .session
                                .editor
                                .doc
                                .blocks
                                .iter()
                                .position(|b| matches!(b,Block::Image(i) if &i.id==id));
                            if let Some(block) = target {
                                self.session.editor.transaction(|editor| {
                                    if let Block::Image(old) = &mut editor.doc.blocks[block] {
                                        old.data = image.data.clone();
                                        old.editable = image.editable.clone();
                                        old.width = image.width;
                                        old.height = image.height;
                                    }
                                });
                            }
                        } else {
                            self.session.editor.insert_block(Block::Image(image));
                            self.scroll_caret = true;
                        }
                        self.textures.clear();
                        *close = true;
                    }
                }
                Err(error) => draft.error = error,
            }
        }
        if !*close {
            self.object_draft = Some(draft);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(ctx: &Context, app: &mut AlphaDocs, events: Vec<Event>, size: egui::Vec2) {
        app.last_autosave = Instant::now();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                events,
                ..Default::default()
            },
            |ui| app.update_ui(ui),
        );
        output.textures_delta.clear();
    }
    fn click(ctx: &Context, app: &mut AlphaDocs, name: &str, size: egui::Vec2) {
        let bounds = ctx.data(|d| d.get_temp::<Rect>(Id::new(name))).unwrap();
        assert!(
            bounds.min.y >= 0.0 && bounds.max.y <= size.y,
            "Footer outside window: {bounds:?}"
        );
        let pos = bounds.center();
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            size,
        );
    }
    #[test]
    fn source_dialog_insert_edit_cancel_stale_and_readonly_are_transactional_in_both_themes() {
        for theme in [Theme::Light, Theme::Dark] {
            let size = vec2(960.0, 640.0);
            let ctx = Context::default();
            fonts::install(&ctx);
            let mut app = AlphaDocs::fresh(
                &ctx,
                Preferences {
                    theme,
                    ..Default::default()
                },
                vec![],
            );
            app.home = false;
            app.session.editor.insert("Original writing");
            let original = app.session.editor.doc.clone();
            app.open_editable_object(Some(ShapeKind::TextBox), None);
            frame(&ctx, &mut app, vec![], size);
            frame(&ctx, &mut app, vec![], size);
            click(&ctx, &mut app, "editable_object_cancel", size);
            assert!(app.dialog.is_none());
            assert_eq!(original, app.session.editor.doc);
            app.open_editable_object(Some(ShapeKind::TextBox), None);
            frame(&ctx, &mut app, vec![], size);
            frame(&ctx, &mut app, vec![], size);
            click(&ctx, &mut app, "editable_object_apply", size);
            assert!(app.dialog.is_none());
            let block = app
                .session
                .editor
                .doc
                .blocks
                .iter()
                .position(|b| matches!(b, Block::Image(_)))
                .unwrap();
            let inserted = app.session.editor.doc.clone();
            app.open_editable_object(None, Some(block));
            if let ObjectSource::Drawing { text, .. } =
                &mut app.object_draft.as_mut().unwrap().source
            {
                **text = crate::engine::edit_drawing_text(text, "Edited café\nSecond line");
            }
            frame(&ctx, &mut app, vec![], size);
            frame(&ctx, &mut app, vec![], size);
            click(&ctx, &mut app, "editable_object_apply", size);
            let Block::Image(image) = &app.session.editor.doc.blocks[block] else {
                panic!()
            };
            assert!(
                matches!(&image.editable.as_ref().unwrap().source, ObjectSource::Drawing { text, .. } if text.text() == "Edited café\nSecond line")
            );
            app.session.editor.undo();
            assert_eq!(inserted, app.session.editor.doc);
            app.session.editor.undo();
            assert_eq!(original, app.session.editor.doc);
            app.open_editable_object(None, None);
            app.session.editor.insert(" changed");
            let changed = app.session.editor.doc.clone();
            frame(&ctx, &mut app, vec![], size);
            frame(&ctx, &mut app, vec![], size);
            click(&ctx, &mut app, "editable_object_apply", size);
            assert_eq!(changed, app.session.editor.doc);
            app.dialog = None;
            app.session.viewing = true;
            app.open_editable_object(None, None);
            frame(&ctx, &mut app, vec![], size);
            frame(&ctx, &mut app, vec![], size);
            click(&ctx, &mut app, "editable_object_apply", size);
            assert_eq!(changed, app.session.editor.doc);
        }
    }
}
