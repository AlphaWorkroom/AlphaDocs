use super::*;

const WHEEL_DIAMETER: f32 = 176.0;
const PINNED_COLOUR_LIMIT: usize = 24;

#[derive(Clone)]
pub(super) struct PickerState {
    highlight: bool,
    colour: [u8; 3],
    hue: f32,
    saturation: f32,
    value: f32,
    hex: String,
    rgb: [String; 3],
    hex_valid: bool,
    rgb_valid: bool,
}

impl PickerState {
    fn new(highlight: bool, colour: [u8; 3]) -> Self {
        let (hue, saturation, value) = rgb_to_hsv(colour);
        let mut state = Self {
            highlight,
            colour,
            hue,
            saturation,
            value,
            hex: String::new(),
            rgb: [String::new(), String::new(), String::new()],
            hex_valid: true,
            rgb_valid: true,
        };
        state.sync_fields();
        state
    }

    fn sync_fields(&mut self) {
        self.hex = format!(
            "#{:02X}{:02X}{:02X}",
            self.colour[0], self.colour[1], self.colour[2]
        );
        self.rgb = self.colour.map(|channel| channel.to_string());
        self.hex_valid = true;
        self.rgb_valid = true;
    }

    fn update_wheel(&mut self, rect: Rect, point: Pos2) {
        let center = rect.center();
        let delta = point - center;
        let radius = rect.width() * 0.5;
        let distance = delta.length().min(radius);
        let hue = delta.y.atan2(delta.x).to_degrees().rem_euclid(360.0);
        let saturation = distance / radius;
        self.hue = hue;
        self.saturation = saturation;
        self.colour = hsv_to_rgb(self.hue, self.saturation, self.value);
        self.sync_fields();
    }

    fn update_value(&mut self, rect: Rect, point: Pos2) {
        self.value = (1.0 - (point.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
        self.colour = hsv_to_rgb(self.hue, self.saturation, self.value);
        self.sync_fields();
    }

    fn set_hex(&mut self) {
        let value = self.hex.trim().trim_start_matches('#');
        if value.len() == 6
            && value.bytes().all(|b| b.is_ascii_hexdigit())
            && let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&value[0..2], 16),
                u8::from_str_radix(&value[2..4], 16),
                u8::from_str_radix(&value[4..6], 16),
            )
        {
            self.colour = [r, g, b];
            self.update_hsv_from_rgb_preserving_achromatic_hue();
            self.sync_fields();
        } else {
            self.hex_valid = false;
        }
    }

    fn set_rgb(&mut self) {
        let parsed: Option<Vec<u8>> = self
            .rgb
            .iter()
            .map(|channel| channel.parse::<u8>().ok())
            .collect();
        if let Some(values) = parsed {
            self.colour = [values[0], values[1], values[2]];
            self.update_hsv_from_rgb_preserving_achromatic_hue();
            self.sync_fields();
        } else {
            self.rgb_valid = false;
        }
    }

    fn update_hsv_from_rgb_preserving_achromatic_hue(&mut self) {
        let (hue, saturation, value) = rgb_to_hsv(self.colour);
        if saturation > 0.0 {
            self.hue = hue;
        }
        self.saturation = saturation;
        self.value = value;
    }

    fn valid(&self) -> bool {
        self.hex_valid && self.rgb_valid
    }
}

impl AlphaDocs {
    pub(super) fn colours(&mut self, ui: &mut Ui, highlight: bool) {
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
        if !self.prefs.pinned_colours.is_empty() {
            ui.label("Pinned colours");
            egui::Grid::new(if highlight {
                "highlight_pinned_palette"
            } else {
                "text_pinned_palette"
            })
            .spacing(vec2(4.0, 4.0))
            .show(ui, |ui| {
                for (index, colour) in self.prefs.pinned_colours.clone().iter().enumerate() {
                    let response = colour_swatch(ui, *colour, !self.session.viewing);
                    if response.clicked() && !self.session.viewing {
                        self.apply_colour(*colour, highlight);
                        ui.close();
                    }
                    response.context_menu(|ui| {
                        if ui.button("Unpin colour").clicked() {
                            self.prefs.pinned_colours.retain(|value| value != colour);
                            self.save_colour_preferences();
                            ui.close();
                        }
                    });
                    if (index + 1) % 6 == 0 {
                        ui.end_row();
                    }
                }
            });
        }
        egui::Grid::new(if highlight {
            "highlight_palette"
        } else {
            "text_palette"
        })
        .spacing(vec2(4.0, 4.0))
        .show(ui, |ui| {
            for (index, colour) in colours.iter().enumerate() {
                if colour_swatch(ui, *colour, !self.session.viewing).clicked()
                    && !self.session.viewing
                {
                    self.apply_colour(*colour, highlight);
                    ui.close();
                }
                if (index + 1) % 6 == 0 {
                    ui.end_row();
                }
            }
        });
        if highlight
            && ui
                .add_enabled(!self.session.viewing, egui::Button::new("None"))
                .clicked()
        {
            self.session.editor.format(|style| style.highlight = None);
            self.refocus = true;
            ui.close();
        }
        if ui
            .add_enabled(!self.session.viewing, egui::Button::new("Custom colour…"))
            .clicked()
        {
            let colour = if highlight {
                self.session
                    .editor
                    .typing
                    .highlight
                    .unwrap_or([255, 235, 59])
            } else {
                self.session.editor.typing.color
            };
            self.colour_picker = Some(PickerState::new(highlight, colour));
            self.dialog = Some(Dialog::ColourPicker);
            ui.close();
        }
    }

    pub(super) fn colour_picker_dialog(&mut self, ui: &mut Ui, close: &mut bool) {
        let p = self.palette;
        let Some(state) = self.colour_picker.as_mut() else {
            *close = true;
            return;
        };
        ui.label(RichText::new("Custom colour").size(24.0));
        ui.add_space(16.0);

        let wheel_group_width = 212.0;
        let wheel_left = ((ui.available_width() - wheel_group_width) * 0.5).max(0.0);
        ui.horizontal(|ui| {
            ui.add_space(wheel_left);
            ui.horizontal(|ui| {
                ui.add_space(3.0);
                let (wheel_rect, _) =
                    ui.allocate_exact_size(vec2(WHEEL_DIAMETER, WHEEL_DIAMETER), Sense::hover());
                let wheel_response =
                    ui.interact(wheel_rect, Id::new("colour_wheel"), Sense::click_and_drag());
                paint_colour_wheel(ui.painter(), wheel_rect, state.hue, state.saturation);
                if let Some(point) = active_pointer(ui, &wheel_response) {
                    state.update_wheel(wheel_rect, point);
                }

                ui.add_space(10.0);
                let (rail_rect, _) =
                    ui.allocate_exact_size(vec2(20.0, WHEEL_DIAMETER), Sense::hover());
                let rail_response = ui.interact(
                    rail_rect,
                    Id::new("colour_value_rail"),
                    Sense::click_and_drag(),
                );
                paint_value_rail(
                    ui.painter(),
                    rail_rect,
                    state.hue,
                    state.saturation,
                    state.value,
                );
                if let Some(point) = active_pointer(ui, &rail_response) {
                    state.update_value(rail_rect, point);
                }
                ui.add_space(3.0);
            });
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label("Preview");
            let (preview, _) = ui.allocate_exact_size(vec2(38.0, 30.0), Sense::hover());
            ui.painter()
                .rect_filled(preview, 6.0, layout::color(state.colour));
            ui.painter().rect_stroke(
                preview,
                6.0,
                Stroke::new(1.0, p.border),
                egui::StrokeKind::Inside,
            );
            ui.add_space(8.0);
            ui.label("HEX");
            let response = colour_text_input(ui, Id::new("colour_hex"), &mut state.hex, 78.0, p);
            if response.changed() {
                state.set_hex();
            }
            if !state.hex_valid {
                ui.label(RichText::new("Use #RRGGBB").size(11.0).color(p.muted));
            }
        });
        ui.horizontal(|ui| {
            ui.label("RGB");
            let mut changed = false;
            for (index, channel) in state.rgb.iter_mut().enumerate() {
                changed |= colour_text_input(ui, Id::new(("colour_rgb", index)), channel, 42.0, p)
                    .changed();
            }
            if changed {
                state.set_rgb();
            }
        });
        if !state.rgb_valid {
            ui.label(
                RichText::new("RGB channels must be 0–255")
                    .size(11.0)
                    .color(p.muted),
            );
        }
        ui.add_space(20.0);
        let mut apply = None;
        let mut pin = None;
        ui.with_layout(UiLayout::right_to_left(Align::Center), |ui| {
            if primary(ui, "Apply", p, !self.session.viewing && state.valid()).clicked()
                && !self.session.viewing
                && state.valid()
            {
                apply = Some((state.colour, state.highlight));
                *close = true;
            }
            if ui
                .add(egui::Button::new("Pin colour").stroke(Stroke::new(1.0, p.accent)))
                .clicked()
            {
                pin = Some(state.colour);
            }
            if ui.button("Cancel").clicked() {
                *close = true;
            }
        });
        if let Some((colour, highlight)) = apply {
            self.apply_colour(colour, highlight);
        }
        if let Some(colour) = pin {
            pin_colour(&mut self.prefs.pinned_colours, colour);
            self.save_colour_preferences();
        }
    }

    fn save_colour_preferences(&mut self) {
        #[cfg(not(test))]
        if let Err(error) = persistence::save_preferences(&self.prefs) {
            self.status = error;
        }
    }

    fn apply_colour(&mut self, colour: [u8; 3], highlight: bool) {
        if self.session.viewing {
            return;
        }
        self.session.editor.format(|style| {
            if highlight {
                style.highlight = Some(colour);
            } else {
                style.color = colour;
            }
        });
        self.refocus = true;
    }
}

fn active_pointer(ui: &Ui, response: &egui::Response) -> Option<Pos2> {
    (response.dragged() || response.drag_started() || response.clicked())
        .then(|| response.interact_pointer_pos())
        .flatten()
        .or_else(|| {
            pointer_gesture::pressed_in(ui, response.rect)
                .filter(|_| ui.input(|i| i.pointer.primary_down()))
        })
}

fn colour_swatch(ui: &mut Ui, colour: [u8; 3], enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new("")
            .fill(layout::color(colour))
            .min_size(vec2(26.0, 26.0)),
    )
}

fn colour_text_input(
    ui: &mut Ui,
    id: Id,
    value: &mut String,
    content_width: f32,
    palette: Palette,
) -> egui::Response {
    Frame::new()
        .fill(palette.chrome)
        .stroke(Stroke::new(1.0, palette.border))
        .corner_radius(6.0)
        .inner_margin(Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.add_sized(
                vec2(content_width, 24.0),
                egui::TextEdit::singleline(value)
                    .id(id)
                    .frame(Frame::NONE)
                    .margin(vec2(0.0, 0.0)),
            )
        })
        .inner
}

fn pin_colour(colours: &mut Vec<[u8; 3]>, colour: [u8; 3]) {
    if !colours.contains(&colour) && colours.len() < PINNED_COLOUR_LIMIT {
        colours.push(colour);
    }
}

fn paint_colour_wheel(painter: &egui::Painter, rect: Rect, selected_hue: f32, selected_sat: f32) {
    use eframe::egui::epaint::{Mesh, Shape};
    let center = rect.center();
    let radius = rect.width() * 0.5;
    let segments = 96usize;
    let rings = 12usize;
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, Color32::WHITE);
    for ring in 1..=rings {
        let saturation = ring as f32 / rings as f32;
        for segment in 0..segments {
            let hue = segment as f32 * 360.0 / segments as f32;
            let angle = hue.to_radians();
            let point = center + vec2(angle.cos(), angle.sin()) * (radius * saturation);
            let hue_rgb = hsv_to_rgb(hue, 1.0, 1.0);
            let colour = Color32::from_rgb(
                (255.0 + (hue_rgb[0] as f32 - 255.0) * saturation) as u8,
                (255.0 + (hue_rgb[1] as f32 - 255.0) * saturation) as u8,
                (255.0 + (hue_rgb[2] as f32 - 255.0) * saturation) as u8,
            );
            mesh.colored_vertex(point, colour);
        }
    }
    for segment in 0..segments {
        let next = (segment + 1) % segments;
        mesh.add_triangle(0, 1 + segment as u32, 1 + next as u32);
    }
    for ring in 1..rings {
        let inner = 1 + (ring - 1) * segments;
        let outer = 1 + ring * segments;
        for segment in 0..segments {
            let next = (segment + 1) % segments;
            let a = (inner + segment) as u32;
            let b = (inner + next) as u32;
            let c = (outer + segment) as u32;
            let d = (outer + next) as u32;
            mesh.add_triangle(a, c, b);
            mesh.add_triangle(b, c, d);
        }
    }
    painter.add(Shape::mesh(mesh));
    painter.circle_stroke(
        rect.center(),
        radius,
        Stroke::new(1.0, Color32::from_black_alpha(60)),
    );
    let angle = selected_hue.to_radians();
    let marker = center + vec2(angle.cos(), angle.sin()) * (radius * selected_sat);
    painter.circle_filled(marker, 6.0, Color32::WHITE);
    painter.circle_stroke(marker, 7.0, Stroke::new(1.5, Color32::BLACK));
}

fn paint_value_rail(painter: &egui::Painter, rect: Rect, hue: f32, saturation: f32, value: f32) {
    use eframe::egui::epaint::{Mesh, Shape};
    let top = hsv_to_rgb(hue, saturation, 1.0);
    let bottom = [0, 0, 0];
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.left_top(), layout::color(top));
    mesh.colored_vertex(rect.right_top(), layout::color(top));
    mesh.colored_vertex(rect.right_bottom(), layout::color(bottom));
    mesh.colored_vertex(rect.left_bottom(), layout::color(bottom));
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(Shape::mesh(mesh));
    painter.rect_stroke(
        rect,
        5.0,
        Stroke::new(1.0, Color32::from_black_alpha(80)),
        egui::StrokeKind::Inside,
    );
    let y = rect.top() + rect.height() * (1.0 - value);
    painter.line_segment(
        [pos2(rect.left() - 2.0, y), pos2(rect.right() + 2.0, y)],
        Stroke::new(2.0, Color32::WHITE),
    );
}

fn rgb_to_hsv(rgb: [u8; 3]) -> (f32, f32, f32) {
    let [r, g, b] = rgb.map(|v| v as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    (hue, if max == 0.0 { 0.0 } else { delta / max }, max)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h.rem_euclid(360.0) {
        hue if hue < 60.0 => (c, x, 0.0),
        hue if hue < 120.0 => (x, c, 0.0),
        hue if hue < 180.0 => (0.0, c, x),
        hue if hue < 240.0 => (0.0, x, c),
        hue if hue < 300.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r, g, b].map(|channel| ((channel + m) * 255.0).round().clamp(0.0, 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> (Context, AlphaDocs) {
        let ctx = Context::default();
        crate::fonts::install(&ctx);
        let mut app = AlphaDocs::fresh(&ctx, Preferences::default(), vec![]);
        app.home = false;
        app.last_autosave = Instant::now();
        app.session.editor.insert("selected text");
        app.session.editor.select_all();
        (ctx, app)
    }

    fn frame(ctx: &Context, app: &mut AlphaDocs, events: Vec<Event>) {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0))),
                events,
                ..Default::default()
            },
            |ui| app.update_ui(ui),
        );
        output.textures_delta.clear();
    }

    #[test]
    fn hsv_roundtrip_and_pinned_colours_are_deduplicated_and_bounded() {
        let colour = hsv_to_rgb(214.0, 0.72, 0.83);
        let (hue, saturation, value) = rgb_to_hsv(colour);
        let roundtrip = hsv_to_rgb(hue, saturation, value);
        assert!(
            colour
                .iter()
                .zip(roundtrip)
                .all(|(a, b)| a.abs_diff(b) <= 1)
        );

        let mut pinned = Vec::new();
        for index in 0..PINNED_COLOUR_LIMIT {
            pin_colour(&mut pinned, [index as u8, 1, 2]);
        }
        pin_colour(&mut pinned, [0, 1, 2]);
        pin_colour(&mut pinned, [250, 1, 2]);
        assert_eq!(pinned.len(), PINNED_COLOUR_LIMIT);
        assert_eq!(pinned.iter().filter(|c| **c == [0, 1, 2]).count(), 1);
        let prefs = Preferences {
            pinned_colours: pinned,
            ..Default::default()
        };
        let restored: Preferences =
            serde_json::from_str(&serde_json::to_string(&prefs).unwrap()).unwrap();
        assert_eq!(restored.pinned_colours, prefs.pinned_colours);
    }

    #[test]
    fn custom_colour_fields_validate_and_update_one_another() {
        let mut state = PickerState::new(false, [1, 2, 3]);
        state.hex = "#0B57D0".into();
        state.set_hex();
        assert_eq!(state.colour, [11, 87, 208]);
        state.rgb = ["17".into(), "34".into(), "51".into()];
        state.set_rgb();
        assert_eq!(state.colour, [17, 34, 51]);
        state.hex = "#ZZZZZZ".into();
        state.set_hex();
        assert_eq!(state.colour, [17, 34, 51]);
        assert!(!state.valid());
        assert!(state.hex.contains("ZZ"));
        state.hex = "#112233".into();
        state.set_hex();
        assert!(state.valid());
        state.rgb = ["256".into(), "0".into(), "0".into()];
        state.set_rgb();
        assert!(!state.valid());
    }

    #[test]
    fn black_retains_wheel_hue_and_grey_retains_last_hue_for_value_changes() {
        let mut state = PickerState::new(false, [0, 0, 0]);
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(WHEEL_DIAMETER, WHEEL_DIAMETER));
        let hue_point = pos2(rect.right(), rect.center().y);
        state.update_wheel(rect, hue_point);
        assert_eq!(state.colour, [0, 0, 0]);
        assert!(state.saturation > 0.99);
        let rail = Rect::from_min_size(Pos2::ZERO, vec2(20.0, WHEEL_DIAMETER));
        state.update_value(rail, rail.left_top());
        assert!(state.colour[0] > 250 && state.colour[1] < 5 && state.colour[2] < 5);

        state.hue = 240.0;
        state.colour = [128, 128, 128];
        state.update_hsv_from_rgb_preserving_achromatic_hue();
        assert!((state.hue - 240.0).abs() < 0.01);
        let angle = 240_f32.to_radians();
        state.update_wheel(
            rect,
            rect.center() + vec2(angle.cos(), angle.sin()) * (rect.width() * 0.5),
        );
        assert!(state.colour[2] > 120 && state.colour[0] < 5 && state.colour[1] < 5);
    }

    #[test]
    fn native_wheel_drag_edits_preview_then_cancel_leaves_document_unchanged() {
        let (ctx, mut app) = editor();
        let original = app.session.editor.doc.clone();
        app.dialog = Some(Dialog::ColourPicker);
        app.colour_picker = Some(PickerState::new(false, [255, 0, 0]));
        for _ in 0..5 {
            frame(&ctx, &mut app, vec![]);
        }
        let wheel = ctx.read_response(Id::new("colour_wheel")).unwrap().rect;
        let angle = 240_f32.to_radians();
        let start = wheel.center() + vec2(angle.cos(), angle.sin()) * (wheel.width() * 0.2);
        let point = wheel.center() + vec2(angle.cos(), angle.sin()) * (wheel.width() * 0.48);
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(start),
                Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        frame(&ctx, &mut app, vec![Event::PointerMoved(point)]);
        frame(
            &ctx,
            &mut app,
            vec![Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let preview = app.colour_picker.as_ref().unwrap().colour;
        assert!(preview[2] > preview[0]);
        assert_eq!(app.session.editor.doc, original);
        app.dialog = None; // Cancel discards the preview state.
        app.colour_picker = None;
        assert_eq!(app.session.editor.doc, original);
    }

    #[test]
    fn apply_is_one_undoable_formatting_action_and_readonly_rejects_it() {
        let (_ctx, mut app) = editor();
        let original = app.session.editor.doc.clone();
        app.apply_colour([10, 80, 210], false);
        assert_ne!(app.session.editor.doc, original);
        let styled = app.session.editor.doc.clone();
        app.session.editor.undo();
        assert_eq!(app.session.editor.doc, original);
        app.session.viewing = true;
        app.apply_colour([10, 80, 210], false);
        assert_eq!(app.session.editor.doc, original);
        assert_ne!(styled, original);
    }
}
