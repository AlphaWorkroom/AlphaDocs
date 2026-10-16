use super::*;

pub(super) fn checkbox(ui: &mut Ui, value: &mut bool, label: &str, p: Palette) -> egui::Response {
    ui.scope(|ui| {
        let v = ui.visuals_mut();
        for w in [
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
        ] {
            w.corner_radius = egui::CornerRadius::same(2);
            w.bg_stroke = Stroke::new(1.0, p.border);
        }
        ui.checkbox(value, label)
    })
    .inner
}

impl AlphaDocs {
    pub(super) fn preferences_dialog(&mut self, ui: &mut Ui, close: &mut bool) {
        let p = self.palette;
        ui.label(RichText::new("Preferences").size(24.0));
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tools.substitutions_tab, false, "General");
            ui.selectable_value(&mut self.tools.substitutions_tab, true, "Substitutions");
        });
        ui.separator();
        ui.add_space(10.0);
        egui::ScrollArea::vertical().max_height((ui.ctx().content_rect().height()-260.0).clamp(240.0,560.0)).show(ui,|ui|{
            if self.tools.substitutions_tab {
                checkbox(ui,&mut self.prefs.typing.substitutions,"Automatic substitution",p);
                ui.label(RichText::new("Replaces whole tokens as you type; Undo restores the keystroke and replacement together. Paste is unchanged. Changes save when you close Preferences.").size(12.0).color(p.muted));
                let mut remove=None;
                egui::Grid::new("substitution_rows").num_columns(4).spacing([8.0,12.0]).show(ui,|ui|{
                    ui.label("");ui.label("Replace");ui.label("With");ui.label("");ui.end_row();
                    for (i,s) in self.prefs.typing.replacements.iter_mut().enumerate(){
                        ui.push_id(i,|ui|{
                            checkbox(ui,&mut s.enabled,"",p);
                            ui.add(egui::TextEdit::singleline(&mut s.from).desired_width(118.0).char_limit(128));
                            ui.add(egui::TextEdit::singleline(&mut s.to).desired_width(118.0).char_limit(512));
                            if icons::button(ui,Icon::Close,"Remove substitution",false,true).clicked(){remove=Some(i);}ui.end_row();
                        });
                    }
                });
                if let Some(i)=remove{self.prefs.typing.replacements.remove(i);}
                if ui.add_enabled(self.prefs.typing.replacements.len()<100,egui::Button::new("+ Add substitution")).clicked(){self.prefs.typing.replacements.push(crate::writing::Substitution{enabled:true,from:String::new(),to:String::new()});}
            }else{
                ui.label("Appearance");ui.horizontal(|ui|{for (t,label) in [(Theme::System,"System"),(Theme::Light,"Light"),(Theme::Dark,"Dark")] {ui.radio_value(&mut self.prefs.theme,t,label);}});
                checkbox(ui,&mut self.prefs.outline,"Show document outline",p);
                checkbox(ui,&mut self.prefs.ruler,"Show ruler",p);
                checkbox(ui,&mut self.prefs.counts,"Display word count while typing",p);
                let r=checkbox(ui,&mut self.prefs.autosave,"Automatically save every 2 seconds",p);
                #[cfg(test)] ui.ctx().data_mut(|d|d.insert_temp(Id::new("autosave_preference_bounds"),r.rect));
                #[cfg(not(test))] let _=r;
                ui.label(RichText::new(if self.prefs.autosave {"Documents are saved locally every 2 seconds. Save and closing also keep your work."}else{"Automatic saving is off. Use Save to keep your work. Closing documents still saves them locally."}).size(12.0).color(p.muted));
                ui.add_space(14.0);ui.separator();ui.label("While typing · off until you enable them");
                checkbox(ui,&mut self.prefs.typing.capitalize,"Automatically capitalize sentence starts",p);
                checkbox(ui,&mut self.prefs.typing.smart_quotes,"Use smart quotes",p);
                checkbox(ui,&mut self.prefs.typing.detect_links,"Automatically detect http(s) and www links",p);
                checkbox(ui,&mut self.prefs.typing.detect_lists,"Automatically detect - / * / 1. lists",p);
                ui.add_enabled(false,egui::Checkbox::new(&mut false,"Enable Markdown")).on_disabled_hover_text("Markdown export is available. Full Markdown typing shortcuts are not implemented.");
                ui.add_enabled(false,egui::Checkbox::new(&mut false,"Automatically correct spelling")).on_disabled_hover_text("Offline suggestions and right-click corrections are available. Automatic replacement is not enabled.");
                ui.add_enabled(false,egui::Checkbox::new(&mut false,"Smart Reply suggestions")).on_disabled_hover_text("AlphaDocs does not include a generative writing assistant.");
                ui.label(RichText::new("Use Insert → Emoji for the complete offline emoji picker.").size(12.0).color(p.muted));
                ui.add_space(14.0);ui.separator();
                let r=checkbox(ui,&mut self.prefs.online_translation,"Enable online document translation (MyMemory)",p);
                #[cfg(test)] ui.ctx().data_mut(|d|d.insert_temp(Id::new("translation_preference_bounds"),r.rect));
                #[cfg(not(test))] let _=r;
                ui.label(RichText::new("Opt-in: adds Tools → Translate document. Nothing is sent when you enable this. Clicking Translate sends document text to Translated's MyMemory service, subject to its terms and a free 5,000-character daily quota.").size(12.0).color(p.muted));
                ui.hyperlink_to("MyMemory terms and privacy","https://mymemory.translated.net/doc/tos.php");
                checkbox(ui,&mut self.prefs.check_updates,"Automatically check GitHub for updates daily",p);
            }
        });
        ui.add_space(18.0);
        ui.with_layout(UiLayout::right_to_left(Align::Center), |ui| {
            if primary(ui, "Done", p, true).clicked() {
                *close = true;
            }
        });
    }
}
