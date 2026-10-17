use super::*;

impl AlphaDocs {
    pub(super) fn image_toolbar(&mut self, ui: &mut Ui) {
        self.toolbar_shell(ui, "image_context_toolbar", |app, ui| {
            for (icon, label, action, enabled) in [
                (Icon::FindReplace, "Find", Action::Find, true),
                (
                    Icon::Undo,
                    "Undo",
                    Action::Undo,
                    app.session.editor.can_undo() && !app.session.viewing,
                ),
                (
                    Icon::Redo,
                    "Redo",
                    Action::Redo,
                    app.session.editor.can_redo() && !app.session.viewing,
                ),
                (Icon::Print, "Print", Action::Print, true),
                (
                    Icon::Proofing,
                    "Spelling and grammar check (Ctrl+Alt+X)",
                    Action::Proofing,
                    true,
                ),
            ] {
                if icons::button(ui, icon, label, false, enabled).clicked() {
                    app.queue(action);
                }
            }
            ui.separator();
            egui::ComboBox::from_id_salt("image_zoom")
                .selected_text(format!("{}%", (app.prefs.zoom * 100.0).round()))
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
            if let Some(block) = app.image_block
                && let Block::Image(image) = &app.session.editor.doc.blocks[block]
            {
                let mut action = image_controls::ImageAction::default();
                let wrap = icons::button(
                    ui,
                    Icon::Wrap,
                    image.placement.flow.label(),
                    false,
                    !app.session.viewing,
                );
                egui::Popup::menu(&wrap).show(|ui| {
                    image_controls::flow_menu(
                        ui,
                        image.placement.flow,
                        &mut action,
                        !app.session.viewing,
                    );
                });
                if let Some(flow) = action.flow {
                    let mut placement = image.placement;
                    placement.flow = flow;
                    app.session.editor.set_image_placement(block, placement);
                }
            }
            if icons::button(
                ui,
                Icon::Comment,
                "Add comment",
                false,
                !app.session.viewing,
            )
            .clicked()
            {
                app.queue(Action::Comment);
            }
            if icons::button(
                ui,
                Icon::Reaction,
                "Add emoji reaction",
                false,
                !app.session.viewing,
            )
            .clicked()
            {
                app.queue(Action::Reaction);
            }
            ui.separator();
            let editable = app.image_block.is_some_and(|block| matches!(app.session.editor.doc.blocks.get(block), Some(Block::Image(i)) if i.editable.is_some()));
            if ui
                .add(
                    egui::Button::new(if editable { "Edit object…" } else { "Image options" })
                        .frame(false)
                        .min_size(vec2(106.0, 30.0)),
                )
                .clicked()
            {
                if editable { app.open_editable_object(None, app.image_block); }
                else { app.image_options_open = true; app.comments_open = false; }
            }
            if ui
                .add_enabled(
                    !app.session.viewing && !editable,
                    egui::Button::new("Original size")
                        .frame(false)
                        .min_size(vec2(100.0, 30.0)),
                )
                .clicked()
                && let Some(block) = app.image_block
                && let Some((_, _, texture)) = app.textures.get(&block)
            {
                let [width, height] = texture.size();
                app.session
                    .editor
                    .resize_image(block, width as f32 * 0.75, height as f32 * 0.75);
            }
        });
    }
    pub(super) fn image_sidebar(&mut self, ui: &mut Ui) {
        if !self.image_options_open {
            return;
        }
        self.comments_open = false;
        let p = self.palette;
        let panel = egui::Panel::right("image_options_sidebar").exact_size(320.0).resizable(false)
            .frame(Frame::new().fill(p.toolbar).corner_radius(18).inner_margin(Margin::same(16))).show(ui, |ui| {
                ui.horizontal(|ui| {
                    icons::draw(ui, Rect::from_center_size(ui.cursor().left_top() + vec2(9.0, 9.0), vec2(18.0, 18.0)), Icon::Image, p.text);
                    ui.add_space(24.0);
                    ui.heading(RichText::new("Image options").size(16.0));
                    ui.with_layout(UiLayout::right_to_left(Align::Center), |ui| {
                        if icons::button(ui, Icon::Close, "Close image options", false, true).clicked() { self.image_options_open = false; }
                    });
                });
                ui.add_space(20.0);
                let Some(block) = self.image_block else { ui.label("Select an image to edit its settings."); return; };
                let Some(Block::Image(image)) = self.session.editor.doc.blocks.get(block).cloned() else { return; };
                let (width, height) = image.visible_size();
                let (mut width_in, mut height_in) = (width / 72.0, height / 72.0);
                egui::ScrollArea::vertical().id_salt("image_settings_scroll").show(ui, |ui| {
                    ui.add_enabled_ui(!self.session.viewing, |ui| {
                        egui::CollapsingHeader::new("Size & rotation").default_open(true).show_unindented(ui, |ui| {
                            Frame::new().fill(image_card(p)).corner_radius(12).inner_margin(Margin::same(12)).show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                egui::Grid::new("image_sidebar_size").num_columns(2).min_col_width(100.0).spacing(vec2(20.0, 12.0)).show(ui, |ui| {
                                    ui.label("Width"); ui.label("Height"); ui.end_row();
                                    let w = dimension_input(ui, &mut width_in, p).changed();
                                    let h = dimension_input(ui, &mut height_in, p).changed();
                                    ui.end_row();
                                    if w || h {
                                        if self.image_lock_ratio {
                                            if w { height_in = width_in * height / width; } else { width_in = height_in * width / height; }
                                        }
                                        let (source_w,source_h)=image.source_size_for_visible(width_in*72.0,height_in*72.0);
                                        self.session.editor.resize_image(block, source_w,source_h);
                                    }
                                });
                                ui.add_space(12.0);
                                ui.checkbox(&mut self.image_lock_ratio, "Lock aspect ratio");
                                if let Some((_, _, texture)) = self.textures.get(&block) {
                                    let natural = vec2(texture.size()[0] as f32, texture.size()[1] as f32) * 0.75;
                                    let mut width_percent = image.width / natural.x * 100.0;
                                    let mut height_percent = image.height / natural.y * 100.0;
                                    ui.add_space(12.0);
                                    egui::Grid::new("image_scale_percent").num_columns(2).min_col_width(100.0).spacing(vec2(20.0, 8.0)).show(ui, |ui| {
                                        ui.label("Width scale"); ui.label("Height scale"); ui.end_row();
                                        let w = ui.add(egui::DragValue::new(&mut width_percent).range(1.0..=1000.0).suffix(" %").speed(1.0)).changed();
                                        let h = ui.add(egui::DragValue::new(&mut height_percent).range(1.0..=1000.0).suffix(" %").speed(1.0)).changed(); ui.end_row();
                                        if w || h {
                                            let mut size = vec2(natural.x * width_percent / 100.0, natural.y * height_percent / 100.0);
                                            if self.image_lock_ratio { if w { size.y = size.x * image.height / image.width; } else { size.x = size.y * image.width / image.height; } }
                                            self.session.editor.resize_image(block, size.x, size.y);
                                        }
                                    });
                                }
                                ui.add_space(20.0);
                                ui.label("Rotate");
                                ui.add_enabled_ui(false, |ui| {
                                    ui.horizontal(|ui| { let mut angle = 0.0; ui.add(egui::DragValue::new(&mut angle).suffix(" °")); let _ = ui.button("90°"); });
                                }).response.on_disabled_hover_text("Image rotation is not available yet.");
                            });
                        });
                        ui.add_space(12.0);
                        let mut placement = image.placement;
                        image_flow_sections(ui, p, &mut placement);
                        ui.add_space(12.0);
                        image_snapping_section(ui, p, &mut placement);
                        if placement != image.placement { self.session.editor.set_image_placement(block, placement); }
                        ui.add_space(12.0);
                        egui::CollapsingHeader::new("Transparency").default_open(true).show_unindented(ui, |ui| {
                            let mut trim = image.trim_transparency;
                            if ui.checkbox(&mut trim, "Trim transparent edges").changed()
                                && let Err(error) = self.session.editor.trim_image_transparency(block, trim) { self.status = error; }
                            ui.label(RichText::new("Keeps the original image. Turn this off to restore its transparent borders.").size(12.0).color(p.muted));
                            let mut padding=image.padding;
                            let toggled=ui.checkbox(&mut padding.enabled,"Add transparent border").changed();
                            let id=Id::new(("image_padding_slider",&image.id));
                            let mut percent=ui.ctx().data_mut(|d|d.get_temp::<f32>(id)).unwrap_or(padding.fraction*100.0);
                            ui.label("Padding on each side");
                            let before=percent;
                            let response=ui.add_enabled_ui(padding.enabled,|ui|adjustment_control(ui,&mut percent,0.0..=25.0,2.0,p)).inner;
                            let changed=before!=percent;
                            if changed{ui.ctx().data_mut(|d|d.insert_temp(id,percent));}
                            if response.drag_stopped() || (changed && !response.dragged()) || toggled {
                                padding.fraction=percent/100.0;
                                self.session.editor.transaction(|e|{if let Some(Block::Image(i))=e.doc.blocks.get_mut(block){i.padding=padding;}});
                                ui.ctx().data_mut(|d|d.remove::<f32>(id));
                            }
                            ui.label(RichText::new("Border width on each side, relative to the shorter image edge. Original pixels are retained.").size(12.0).color(p.muted));
                        });
                        ui.add_space(12.0);
                        self.image_adjustments(ui, block, &image);
                        ui.add_space(12.0);
                        egui::CollapsingHeader::new("Alt text").default_open(true).show_unindented(ui, |ui| {
                            let mut alt = image.alt_text.clone();
                            ui.label(RichText::new("Describe the image for screen readers.").size(12.0).color(p.muted));
                            if ui.add(egui::TextEdit::multiline(&mut alt).hint_text("Describe the image").desired_rows(3).desired_width(f32::INFINITY)).changed() {
                                self.session.editor.transaction(|e| { if let Some(Block::Image(i)) = e.doc.blocks.get_mut(block) { i.alt_text = alt.clone(); } });
                            }
                        });
                        egui::CollapsingHeader::new("Advanced options").show_unindented(ui, |ui| {
                            let mut title = image.title.clone();
                            ui.label(RichText::new("Add a title to your image.").size(12.0).color(p.muted));
                            if ui.add(egui::TextEdit::singleline(&mut title).hint_text("Image title")).changed() {
                                self.session.editor.transaction(|e| { if let Some(Block::Image(i)) = e.doc.blocks.get_mut(block) { i.title = title.clone(); } });
                            }
                        });
                    });
                });
            });
        self.feature_destination(feature_search::Target::Images, panel.response.rect);
    }
    fn image_adjustments(&mut self, ui: &mut Ui, block: usize, image: &EmbeddedImage) {
        let p = self.palette;
        let mut effects = self
            .image_effects_preview
            .filter(|(b, _)| *b == block)
            .map(|(_, e)| e)
            .unwrap_or(image.effects);
        egui::CollapsingHeader::new("Recolor").show_unindented(ui, |ui| {
            let before = effects;
            egui::ComboBox::from_id_salt("image_recolor")
                .selected_text(if effects.grayscale {
                    "Grayscale"
                } else {
                    "Original colors"
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut effects.grayscale, false, "Original colors");
                    ui.selectable_value(&mut effects.grayscale, true, "Grayscale");
                });
            if before != effects {
                self.session.editor.set_image_effects(block, effects);
            }
        });
        egui::CollapsingHeader::new("Adjustments")
            .default_open(true)
            .show_unindented(ui, |ui| {
                Frame::new()
                    .fill(image_card(p))
                    .corner_radius(12)
                    .inner_margin(Margin::same(12))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        let before = effects;
                        let mut dragging = false;
                        for (label, value, range) in [
                            ("Opacity", &mut effects.opacity, 0.0..=100.0),
                            ("Brightness", &mut effects.brightness, -100.0..=100.0),
                            ("Contrast", &mut effects.contrast, -100.0..=100.0),
                        ] {
                            ui.label(label);
                            let mut percent = *value * 100.0;
                            let response = adjustment_control(
                                ui,
                                &mut percent,
                                range,
                                if label == "Opacity" { 100.0 } else { 0.0 },
                                p,
                            );
                            *value = percent / 100.0;
                            dragging |= response.dragged() || response.drag_started();
                            ui.add_space(10.0);
                        }
                        if before != effects {
                            if dragging {
                                self.image_effects_preview = Some((block, effects));
                            } else {
                                self.image_effects_preview = None;
                                self.session.editor.set_image_effects(block, effects);
                            }
                        }
                        if ui
                            .add(
                                egui::Button::new(RichText::new("Reset").color(p.accent))
                                    .corner_radius(20)
                                    .min_size(vec2(82.0, 34.0)),
                            )
                            .clicked()
                        {
                            self.image_effects_preview = None;
                            self.session
                                .editor
                                .set_image_effects(block, ImageEffects::default());
                        }
                    });
            });
        if self.image_effects_preview.is_some_and(|(b, _)| b == block)
            && ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary))
            && let Some((_, effects)) = self.image_effects_preview.take()
        {
            self.session.editor.set_image_effects(block, effects);
        }
    }
}
fn dimension_input(ui: &mut Ui, value: &mut f32, p: Palette) -> egui::Response {
    Frame::new()
        .fill(p.chrome)
        .stroke(Stroke::new(0.7, p.border))
        .corner_radius(4)
        .inner_margin(Margin::same(6))
        .show(ui, |ui| {
            ui.add_sized(
                vec2(88.0, 24.0),
                egui::DragValue::new(value)
                    .range(0.1..=16.0)
                    .speed(0.01)
                    .suffix(" in")
                    .max_decimals(2),
            )
        })
        .inner
}

fn image_card(p: Palette) -> Color32 {
    if p.dark {
        p.chrome
    } else {
        Color32::from_rgb(233, 238, 246)
    }
}

pub(super) fn adjustment_control(
    ui: &mut Ui,
    percent: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    original: f32,
    p: Palette,
) -> egui::Response {
    ui.horizontal(|ui| {
        let slider = ui
            .scope(|ui| {
                ui.spacing_mut().slider_width = 108.0;
                ui.spacing_mut().slider_rail_height = 4.0;
                ui.visuals_mut().widgets.inactive.bg_fill = p.selection;
                ui.visuals_mut().widgets.inactive.fg_stroke = Stroke::new(1.5, p.accent);
                ui.visuals_mut().widgets.hovered.bg_fill = p.accent;
                ui.visuals_mut().widgets.active.bg_fill = p.accent;
                ui.visuals_mut().selection.bg_fill = p.accent;
                ui.add(
                    egui::Slider::new(percent, range.clone())
                        .show_value(false)
                        .trailing_fill(true)
                        .handle_shape(egui::style::HandleShape::Rect { aspect_ratio: 0.2 }),
                )
            })
            .inner;
        if slider.double_clicked()
            || ui.input(|i| {
                i.pointer
                    .button_double_clicked(egui::PointerButton::Primary)
                    && i.pointer.interact_pos().is_some_and(|pos| {
                        slider.rect.contains(pos) && ui.clip_rect().contains(pos)
                    })
            })
        {
            *percent = original;
        }
        let slider = slider.on_hover_text("Double-click to restore the original value");
        let numeric = Frame::new()
            .fill(p.chrome)
            .stroke(Stroke::new(0.7, p.border))
            .corner_radius(4)
            .inner_margin(Margin::same(4))
            .show(ui, |ui| {
                ui.visuals_mut().button_frame = false;
                ui.add_sized(
                    vec2(76.0, 24.0),
                    egui::DragValue::new(percent)
                        .range(range)
                        .suffix(" %")
                        .max_decimals(0)
                        .speed(1.0),
                )
            })
            .inner;
        slider.union(numeric)
    })
    .inner
}

fn image_snapping_section(ui: &mut Ui, p: Palette, placement: &mut ImagePlacement) {
    egui::CollapsingHeader::new("Snapping & guides").default_open(true).show_unindented(ui, |ui| {
        Frame::new().fill(image_card(p)).corner_radius(12).inner_margin(Margin::same(12)).show(ui, |ui| {
            ui.add_enabled_ui(placement.flow != ImageFlow::Break, |ui| {
                ui.label("Movement");
                egui::ComboBox::from_id_salt("image_snap_mode")
                    .selected_text(match placement.snap {
                        ImageSnap::Freeform => "Freeform",
                        ImageSnap::TextLines => "Snap to text lines",
                        ImageSnap::NearTextLines => "Snap near text lines",
                    }).width(236.0).show_ui(ui, |ui| {
                        ui.selectable_value(&mut placement.snap, ImageSnap::Freeform, "Freeform");
                        ui.selectable_value(&mut placement.snap, ImageSnap::TextLines, "Snap to text lines");
                        ui.selectable_value(&mut placement.snap, ImageSnap::NearTextLines, "Snap near text lines");
                    });
                ui.label(RichText::new(match placement.snap {
                    ImageSnap::Freeform => "Place freely. Guides can help you line things up.",
                    ImageSnap::TextLines => "Snap the top edge to a text line; horizontal movement stays free.",
                    ImageSnap::NearTextLines => "Snap the top edge when close to a line. Place freely between lines.",
                }).size(12.0).color(p.muted));
                ui.add_space(8.0);
                ui.checkbox(&mut placement.guides, "Show alignment guides");
                ui.checkbox(&mut placement.snap_guides, "Snap to alignment guides");
                ui.label(RichText::new("Page centres, margins, images, text blocks and table cells.").size(12.0).color(p.muted));
            }).response.on_disabled_hover_text("Choose Wrap text, Behind text or In front of text for floating placement. Break text drops between measured text lines and splits a paragraph when dropped inside it.");
        });
    });
}

fn image_flow_sections(ui: &mut Ui, p: Palette, placement: &mut ImagePlacement) {
    egui::CollapsingHeader::new("Text wrapping").default_open(true).show_unindented(ui, |ui| {
        Frame::new().fill(image_card(p)).corner_radius(12).inner_margin(Margin::same(10)).show(ui, |ui| {
            egui::Grid::new("image_flow_choices").num_columns(3).spacing(vec2(8.0, 12.0)).show(ui, |ui| {
                for (index, flow) in [None, Some(ImageFlow::Wrap), Some(ImageFlow::Break), Some(ImageFlow::Behind), Some(ImageFlow::Front)].into_iter().enumerate() {
                    ui.vertical(|ui| {
                        let selected = flow == Some(placement.flow);
                        let enabled = flow.is_some();
                        let name = flow.map_or("Inline with text", ImageFlow::label);
                        let (_, rect) = ui.allocate_space(vec2(80.0,80.0));
                        let response = ui.interact(rect, Id::new(("image_flow", index)), if enabled { Sense::click() } else { Sense::hover() });
                        ui.painter().rect_filled(rect, 10.0, p.chrome);
                        if selected || response.hovered() && enabled { ui.painter().rect_stroke(rect, 10.0, Stroke::new(1.5, p.accent), egui::StrokeKind::Inside); }
                        let ink = if enabled { p.accent } else { p.muted.gamma_multiply(0.45) };
                        let object = Rect::from_center_size(rect.center(), vec2(18.0,22.0));
                        if index == 3 { ui.painter().rect_filled(object, 0.0, ink.gamma_multiply(0.5)); }
                        for row in 0..5 {
                            let y = rect.top()+12.0+row as f32*10.0;
                            if index == 2 && (1..=3).contains(&row) { continue; }
                            let spans: &[(f32,f32)] = if index == 1 && (1..=3).contains(&row) { &[(10.0,12.0),(58.0,12.0)] } else { &[(10.0,60.0)] };
                            for (x,width) in spans { ui.painter().rect_filled(Rect::from_min_size(pos2(rect.left()+x,y),vec2(*width,4.0)),0.0,p.border); }
                        }
                        if index != 3 { ui.painter().rect_filled(object,0.0,ink); }
                        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton,enabled,selected,name));
                        if response.clicked() && let Some(flow) = flow { placement.flow = flow; }
                        if !enabled { response.on_hover_text("Inline images within a text run are not available yet."); }
                        ui.add_sized([80.0,30.0],egui::Label::new(RichText::new(name).size(11.0).color(if enabled {p.text} else {p.muted.gamma_multiply(0.6)})).wrap());
                    });
                    if index % 3 == 2 { ui.end_row(); }
                }
            });
            ui.add_space(12.0);
            ui.add_enabled_ui(placement.flow == ImageFlow::Wrap, |ui| {
                egui::ComboBox::from_id_salt("image_wrap_side").selected_text(match placement.wrap_side { WrapSide::Both=>"Both sides", WrapSide::Left=>"Left only", WrapSide::Right=>"Right only", WrapSide::Largest=>"Largest side" }).width(226.0).show_ui(ui, |ui| {
                    for (value,label) in [(WrapSide::Both,"Both sides"),(WrapSide::Left,"Left only"),(WrapSide::Right,"Right only"),(WrapSide::Largest,"Largest side")] { ui.selectable_value(&mut placement.wrap_side,value,label); }
                });
            });
            ui.add_space(10.0);
            ui.label("Margins from text");
            ui.add_enabled_ui(matches!(placement.flow,ImageFlow::Wrap|ImageFlow::Break), |ui| {
                egui::Grid::new("image_wrap_margins").num_columns(2).min_col_width(106.0).show(ui,|ui| {
                    for (index,name) in ["Top","Bottom","Left","Right"].into_iter().enumerate() {
                        ui.vertical(|ui| { ui.label(name); let mut inches=placement.margins[index]/72.0; if Frame::new().fill(p.chrome).stroke(Stroke::new(0.7,p.border)).corner_radius(4).inner_margin(Margin::same(4)).show(ui, |ui| ui.add_sized(vec2(76.0,24.0),egui::DragValue::new(&mut inches).range(0.0..=10.0).suffix(" in").max_decimals(2).speed(0.01))).inner.changed() { placement.margins[index]=inches.clamp(0.0,10.0)*72.0; } });
                        if index%2==1 {ui.end_row();}
                    }
                });
            });
        });
    });
    ui.add_space(12.0);
    egui::CollapsingHeader::new("Position").show_unindented(ui, |ui| {
        egui::ComboBox::from_id_salt("image_anchor_mode")
            .selected_text("Move with text")
            .show_ui(ui, |ui| {
                if ui
                    .add(egui::Button::new("Move with text").selected(true))
                    .clicked()
                {
                    ui.close();
                }
                ui.add_enabled(false, egui::Button::new("Fix position on page"))
                    .on_disabled_hover_text("Page-relative anchoring is not implemented yet.");
            });
        ui.add_enabled_ui(true, |ui| {
            ui.label(
                RichText::new("Drag the image, or set its offset from the text anchor.")
                    .size(12.0)
                    .color(p.muted),
            );
            ui.horizontal(|ui| {
                ui.label("Horizontal");
                ui.add(
                    egui::DragValue::new(&mut placement.offset[0])
                        .range(0.0..=2400.0)
                        .suffix(" pt"),
                );
            });
            ui.add_enabled_ui(placement.flow != ImageFlow::Break, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Vertical");
                    ui.add(
                        egui::DragValue::new(&mut placement.offset[1])
                            .range(-2400.0..=2400.0)
                            .suffix(" pt"),
                    );
                });
            });
        });
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn double_click_restores_opacity_and_adjustment_defaults() {
        for (initial, range, original) in [
            (70.0, 0.0..=100.0, 100.0),
            (70.0, -100.0..=100.0, 0.0),
            (55.0, 10.0..=200.0, 100.0),
        ] {
            let ctx = Context::default();
            let p = theme::apply(&ctx, Theme::Light);
            let mut percent = initial;
            let mut rect = Rect::NOTHING;
            let mut frame = |time, events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 150.0))),
                        time: Some(time),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        rect =
                            adjustment_control(ui, &mut percent, range.clone(), original, p).rect;
                    },
                );
                output.textures_delta.clear();
                (rect, percent)
            };
            let (rect, _) = frame(0.0, vec![]);
            let point = pos2(rect.left() + 20.0, rect.center().y);
            let button = |pressed| Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            frame(0.1, vec![Event::PointerMoved(point), button(true)]);
            let (_, first) = frame(0.11, vec![button(false)]);
            assert!((first - original).abs() > 1.0);
            frame(0.2, vec![button(true)]);
            let (_, reset) = frame(0.21, vec![button(false)]);
            assert_eq!(reset, original);
        }
    }
}
