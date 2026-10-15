use super::*;
impl AlphaDocs {
    pub(super) fn about_contents(&mut self, ui: &mut Ui) -> bool {
        let p = self.palette;
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
            icons::brand(ui, rect, p.accent);
            ui.vertical(|ui| {
                ui.label(RichText::new("AlphaDocs").size(28.0).strong());
                ui.label("AlphaDocs by Alpha Workroom");
                ui.label(
                    RichText::new(concat!("Version ", env!("CARGO_PKG_VERSION")))
                        .size(12.0)
                        .color(p.muted),
                );
                if ui.link("Check for updates").clicked() {
                    self.queue(Action::CheckUpdates);
                }
            });
        });
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space((ui.available_width() - 256.0).max(0.0) * 0.5);
            let (rect, _) = ui.allocate_exact_size(vec2(256.0, 132.0), Sense::hover());
            icons::workroom_brand(ui, rect, if p.dark { p.accent } else { Color32::BLACK });
        });
        ui.add_space(10.0);
        ui.label("Copyright (c) 2026 Alpha Workroom");
        ui.label(
            RichText::new("AlphaDocs Free Use & Non-Monetisation Licence · Version 1.1").strong(),
        );
        ui.label("The complete supplied application licence follows. Dependency, font and dictionary licences accompany the package separately.");
        ui.add_space(8.0);
        egui::ScrollArea::vertical()
            .id_salt("application_licence")
            .max_height((ui.ctx().content_rect().height() - 620.0).clamp(45.0, 280.0))
            .show(ui, |ui| {
                ui.label(RichText::new(include_str!("../../../LICENSE")).size(12.0));
            });
        ui.add_space(16.0);
        primary(ui, "Done", p, true).clicked()
    }
}
