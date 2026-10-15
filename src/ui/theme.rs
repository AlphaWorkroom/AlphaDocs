use crate::persistence::Theme;
use eframe::egui::{self, Color32, Context, Stroke};
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub chrome: Color32,
    pub toolbar: Color32,
    pub paper: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub border: Color32,
    pub accent: Color32,
    pub selection: Color32,
    pub hover: Color32,
    pub dark: bool,
}
fn c(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}
impl Palette {
    pub fn get(dark: bool) -> Self {
        if dark {
            Self {
                bg: c(23, 25, 30),
                chrome: c(32, 35, 41),
                toolbar: c(41, 45, 53),
                paper: c(34, 37, 43),
                text: c(223, 227, 235),
                muted: c(157, 164, 177),
                border: c(71, 77, 88),
                accent: c(168, 199, 250),
                selection: c(48, 74, 112),
                hover: c(52, 57, 66),
                dark,
            }
        } else {
            Self {
                bg: c(248, 250, 253),
                chrome: Color32::WHITE,
                toolbar: c(240, 244, 249),
                paper: Color32::WHITE,
                text: c(31, 31, 31),
                muted: c(95, 99, 104),
                border: c(196, 199, 197),
                accent: c(11, 87, 208),
                selection: c(211, 227, 253),
                hover: c(229, 234, 240),
                dark,
            }
        }
    }
}
pub fn apply(ctx: &Context, theme: Theme) -> Palette {
    ctx.set_theme(match theme {
        Theme::System => egui::ThemePreference::System,
        Theme::Light => egui::ThemePreference::Light,
        Theme::Dark => egui::ThemePreference::Dark,
    });
    let p = Palette::get(ctx.theme() == egui::Theme::Dark);
    ctx.style_mut_of(ctx.theme(), |s| {
        s.visuals.override_text_color = Some(p.text);
        s.visuals.panel_fill = p.bg;
        s.visuals.window_fill = p.chrome;
        s.visuals.extreme_bg_color = p.chrome;
        s.visuals.faint_bg_color = p.toolbar;
        s.visuals.window_corner_radius = egui::CornerRadius::same(20);
        s.visuals.menu_corner_radius = egui::CornerRadius::same(5);
        s.visuals.selection.bg_fill = p.selection;
        s.visuals.selection.stroke = Stroke::new(1.0, p.accent);
        s.visuals.widgets.inactive.bg_fill = p.toolbar;
        s.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(16);
        s.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(16);
        s.visuals.widgets.active.corner_radius = egui::CornerRadius::same(16);
        s.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        s.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
        s.visuals.widgets.hovered.bg_fill = p.hover;
        s.visuals.widgets.hovered.weak_bg_fill = p.hover;
        s.visuals.widgets.active.bg_fill = p.selection;
        s.visuals.widgets.active.weak_bg_fill = p.selection;
        s.visuals.widgets.active.fg_stroke = Stroke::new(1.0, p.accent);
        s.spacing.item_spacing = egui::vec2(6.0, 6.0);
        s.spacing.button_padding = egui::vec2(9.0, 5.0);
        s.spacing.interact_size.y = 30.0;
        s.text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        s.text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
        s.text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
    });
    p
}
