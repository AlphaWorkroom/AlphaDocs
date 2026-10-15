use super::*;
#[derive(Default, PartialEq)]
enum Section {
    Pinned,
    Recent,
    #[default]
    All,
}
#[derive(Default)]
pub(super) struct EmojiPicker {
    search: String,
    section: Section,
}

impl EmojiPicker {
    pub fn show(
        &mut self,
        ui: &mut Ui,
        prefs: &mut Preferences,
        heading: &str,
    ) -> (Option<&'static emojis::Emoji>, bool) {
        ui.heading(RichText::new(heading).size(24.0));
        ui.add_space(12.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("Search emojis")
                .desired_width(368.0),
        );
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.section, Section::Pinned, "Pinned");
            ui.selectable_value(&mut self.section, Section::Recent, "Recent");
            ui.selectable_value(&mut self.section, Section::All, "All");
        });
        ui.separator();
        let search = self.search.trim().to_lowercase();
        let candidates: Vec<_> = match self.section {
            Section::All => emojis::iter().collect(),
            Section::Pinned => prefs
                .pinned_emojis
                .iter()
                .filter_map(|s| emojis::get(s))
                .collect(),
            Section::Recent => prefs
                .recent_emojis
                .iter()
                .filter_map(|s| emojis::get(s))
                .collect(),
        };
        let candidates: Vec<_> = candidates
            .into_iter()
            .filter(|e| {
                search.is_empty()
                    || e.name().to_lowercase().contains(&search)
                    || e.as_str().contains(&search)
            })
            .collect();
        let mut chosen = None;
        let mut changed = false;
        if candidates.is_empty() {
            ui.label("No emojis here yet. Right-click an emoji in All to pin it.");
        }
        egui::ScrollArea::vertical()
            .id_salt("emoji_results")
            .max_height(330.0)
            .show_rows(ui, 40.0, candidates.len().div_ceil(9), |ui, rows| {
                for row in rows {
                    ui.horizontal(|ui| {
                        for emoji in candidates.iter().skip(row * 9).take(9) {
                            let (rect, _) =
                                ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
                            let response = ui.interact(
                                rect,
                                Id::new(("emoji_choice", emoji.as_str())),
                                Sense::click(),
                            );
                            if response.hovered() || response.has_focus() {
                                ui.painter().rect_filled(
                                    rect,
                                    6.0,
                                    ui.visuals().widgets.hovered.weak_bg_fill,
                                );
                            }
                            ui.painter().text(
                                rect.center(),
                                Align2::CENTER_CENTER,
                                emoji.as_str(),
                                FontId::new(22.0, egui::FontFamily::Name("Emoji".into())),
                                ui.visuals().text_color(),
                            );
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    true,
                                    emoji.name(),
                                )
                            });
                            if response.clicked() {
                                chosen = Some(*emoji);
                            }
                            response.clone().on_hover_text(emoji.name());
                            response.context_menu(|ui| {
                                let pinned =
                                    prefs.pinned_emojis.iter().any(|s| s == emoji.as_str());
                                if ui
                                    .button(if pinned { "Unpin emoji" } else { "Pin emoji" })
                                    .clicked()
                                {
                                    if pinned {
                                        prefs.pinned_emojis.retain(|s| s != emoji.as_str());
                                    } else if prefs.pinned_emojis.len() < 128 {
                                        prefs.pinned_emojis.push(emoji.as_str().into());
                                    }
                                    changed = true;
                                    ui.close();
                                }
                                if let Some(tones) = emoji.skin_tones() {
                                    ui.separator();
                                    for tone in tones {
                                        if ui
                                            .button(format!("{} {}", tone.as_str(), tone.name()))
                                            .clicked()
                                        {
                                            chosen = Some(tone);
                                            ui.close();
                                        }
                                    }
                                }
                            });
                        }
                    });
                }
            });
        if let Some(emoji) = chosen {
            prefs.recent_emojis.retain(|s| s != emoji.as_str());
            prefs.recent_emojis.insert(0, emoji.as_str().into());
            prefs.recent_emojis.truncate(32);
            changed = true;
        }
        ui.label(RichText::new("Right-click to pin or choose a skin tone.").size(12.0));
        (chosen, changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_and_keyboard_choice_populate_recent_and_preferences_roundtrip() {
        let ctx = Context::default();
        fonts::install(&ctx);
        let mut picker = EmojiPicker {
            search: "rocket".into(),
            ..Default::default()
        };
        let mut prefs = Preferences::default();
        let mut choice = None;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                choice = picker.show(ui, &mut prefs, "Emojis").0;
            });
        });
        output.textures_delta.clear();
        let id = Id::new(("emoji_choice", "🚀"));
        assert!(ctx.read_response(id).is_some());
        ctx.memory_mut(|m| m.request_focus(id));
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    choice = picker.show(ui, &mut prefs, "Emojis").0;
                });
            },
        );
        output.textures_delta.clear();
        assert_eq!(choice.unwrap().as_str(), "🚀");
        assert_eq!(prefs.recent_emojis, ["🚀"]);
        prefs.pinned_emojis.push("👩🏽‍💻".into());
        let loaded: Preferences =
            serde_json::from_str(&serde_json::to_string(&prefs).unwrap()).unwrap();
        assert_eq!(loaded.recent_emojis, prefs.recent_emojis);
        assert_eq!(loaded.pinned_emojis, prefs.pinned_emojis);
        assert!(emojis::iter().count() > 1800);
    }
}
