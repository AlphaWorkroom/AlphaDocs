use super::*;

impl AlphaDocs {
    pub(super) fn dialogs(&mut self, ctx: &Context) {
        let Some(dialog) = self.dialog else {
            return;
        };

        if matches!(dialog, Dialog::Find) {
            self.find_window(ctx);
            return;
        }
        if matches!(dialog, Dialog::Print) {
            self.print_screen(ctx);
            return;
        }
        if matches!(dialog, Dialog::Share) {
            self.sharing_dialog(ctx);
            return;
        }

        let p = self.palette;

        let mut close = false;

        let mut restore = None;

        let response=egui::Modal::new(Id::new("alphadocs_dialog")).frame(Frame::new().fill(p.chrome).corner_radius(24).inner_margin(Margin::same(24)).shadow(egui::epaint::Shadow{
offset:[0,
6],
blur:24,
spread:0,
color:Color32::from_black_alpha(50)}
)).show(ctx,
|ui|{
ui.set_width(if matches!(dialog,
Dialog::Help | Dialog::About){
540.0}
else{
if matches!(dialog, Dialog::Page) {348.0} else if matches!(dialog, Dialog::ColourPicker) {360.0} else if matches!(dialog, Dialog::EditableObject) {480.0} else {368.0}}
);
match dialog{
        Dialog::Tool(tool) => self.tool_dialog(tool,ui,&mut close),
        Dialog::Share => {},
        Dialog::Rename => {
            ui.label(RichText::new("Rename document").size(24.0));ui.add_space(16.0);
            let field_id=Id::new("document_rename_field");
            let submit=ui.ctx().memory(|m|m.has_focus(field_id)) && ui.input_mut(|i|i.consume_key(egui::Modifiers::NONE,Key::Enter));
            let field=ui.add_enabled(!self.session.viewing,egui::TextEdit::singleline(&mut self.title_draft).id(field_id).desired_width(f32::INFINITY).margin(vec2(10.0,8.0)));
            if self.rename_focus {field.request_focus();self.rename_focus=false;}
            ui.add_space(20.0);
            ui.horizontal(|ui| {
                if primary(ui,"Save",p,!self.session.viewing && !self.title_draft.trim().is_empty()).clicked() || (!self.session.viewing && !self.title_draft.trim().is_empty() && submit) {
                    let title=self.title_draft.trim().to_owned();self.session.editor.transaction(|e|e.doc.title=title.clone());close=true;
                }
                if ui.button("Cancel").clicked(){close=true;}
            });
        },

        Dialog::Page=>self.page_dialog(ui,&mut close),

        Dialog::Count=>{
ui.label(RichText::new("Word count").size(24.0));
ui.add_space(24.0);
let(w,
c,
n)=self.session.editor.doc.counts();
let pages=self.layout.as_ref().map_or(1,
|l|l.pages);
for(label,
value)in [("Pages",
pages),
("Words",
w),
("Characters",
c),
("Characters excluding spaces",
n)]{
ui.horizontal(|ui|{
ui.label(label);
ui.with_layout(UiLayout::right_to_left(Align::Center),
|ui|{
ui.label(value.to_string());
}
);
}
);
ui.add_space(10.0);
ui.separator();
ui.add_space(10.0);
}
ui.checkbox(&mut self.prefs.counts,
"Display word count while typing");
ui.checkbox(&mut self.prefs.check_updates, "Automatically check GitHub for updates daily");
ui.add_space(18.0);
ui.with_layout(UiLayout::right_to_left(Align::Center),
|ui|{
if primary(ui,
"OK",
p,
true).clicked(){
close=true;
}
}
);
}
,

        Dialog::Link=>{
ui.label(RichText::new("Insert link").size(24.0));
ui.add_space(16.0);
ui.label("Text");
ui.text_edit_singleline(&mut self.link_text);
ui.label("Link");
ui.add(egui::TextEdit::singleline(&mut self.link_url).hint_text("https://example.com"));
ui.add_space(20.0);
ui.with_layout(UiLayout::right_to_left(Align::Center),
|ui|{
let valid=self.link_url.starts_with("https://")||self.link_url.starts_with("http://")||self.link_url.starts_with("mailto:");

if primary(ui,
"Apply",
p,
valid&&!self.link_text.is_empty()).clicked(){
let link=self.link_url.clone();

if self.session.editor.selected(){
self.session.editor.format(|s|{
s.link=Some(link.clone());
s.underline=true;
s.color=[11,
87,
208];
}
);
}
else{
let old=self.session.editor.typing.clone();
self.session.editor.typing.link=Some(link);
self.session.editor.typing.underline=true;
self.session.editor.typing.color=[11,
87,
208];
self.session.editor.insert(&self.link_text);
self.session.editor.typing=old;
}
close=true;
}

if ui.button("Cancel").clicked(){
close=true;
}

if ui.button("Remove link").clicked(){
self.session.editor.format(|s|{
s.link=None;
s.underline=false;
}
);
close=true;
}
}
);
}
,

        Dialog::Preferences=>self.preferences_dialog(ui,&mut close),
        Dialog::History=>{
ui.label(RichText::new("Version history").size(24.0));
ui.add_space(16.0);
ui.label("Snapshots are kept on this computer.");

if self.history_files.is_empty(){
ui.label("Save the document to create its first snapshot.");
}
egui::ScrollArea::vertical().max_height(300.0).show(ui,
|ui|{
for path in &self.history_files{
ui.horizontal(|ui|{
let seconds=path.file_stem().and_then(|s|s.to_str()).and_then(|s|s.parse::<u64>().ok()).unwrap_or(0)/1000;
ui.label(persistence::readable_time(seconds));

if ui.button("Restore").clicked(){
restore=Some(path.clone());
}
}
);
}
}
);
ui.add_space(20.0);

if ui.button("Close").clicked(){
close=true;
}
}
,

        Dialog::Emojis=>{
let heading = if self.emoji_reaction_target.is_some() || self.text_reaction_range.is_some() { "Emoji reactions" } else { "Emojis" };
let (chosen, changed) = self.emoji_picker.show(ui, &mut self.prefs, heading);
if changed && let Err(error) = persistence::save_preferences(&self.prefs) { self.status = error; }
if let Some(emoji) = chosen {
if let Some(target) = self.emoji_reaction_target.take() {
let comment = self.emoji_reaction_comment.take();
self.session.editor.toggle_annotation_reaction(&target, comment.as_deref(), emoji.as_str());
self.image_options_open = false;
} else if let Some((range,revision)) = self.text_reaction_range.take() {
if revision == self.session.editor.revision { self.session.editor.annotate_text(range, None, Some(emoji.as_str())); }
} else {
let mut style = self.session.editor.typing.clone();
style.font = "Noto Emoji".into();
self.session.editor.insert_rich(vec![emoji.as_str().chars().map(|ch| StyledChar { ch, style: style.clone() }).collect()]);
}
close = true;
}
if ui.button("Done").clicked() { close = true; }
},
        Dialog::ColourPicker => self.colour_picker_dialog(ui, &mut close),
        Dialog::TableFormat => self.table_format_dialog(ui, &mut close),
        Dialog::TableFormula => self.table_formula_dialog(ui, &mut close),
        Dialog::EditableObject => self.editable_object_dialog(ui, &mut close),
        Dialog::Symbols=>{
ui.label(RichText::new("Special characters").size(24.0));
ui.add_space(16.0);
egui::Grid::new("symbols").spacing(vec2(8.0,
8.0)).show(ui,
|ui|{
for(i,
symbol)in ["©",
"®",
"™",
"§",
"¶",
"°",
"±",
"×",
"÷",
"≠",
"≤",
"≥",
"∞",
"π",
"Ω",
"μ",
"α",
"β",
"δ",
"λ",
"←",
"→",
"↑",
"↓",
"✓",
"•",
"…",
"—",
"–",
"€",
"£",
"¥"].iter().enumerate(){
if ui.add(egui::Button::new(RichText::new(*symbol).size(22.0)).min_size(vec2(34.0,
36.0))).clicked(){
self.session.editor.insert(symbol);
close=true;
}

if(i+1)%8==0{
ui.end_row();
}
}
}
);
ui.add_space(20.0);

if ui.button("Cancel").clicked(){
close=true;
}
}
,

        Dialog::Print=>{},

        Dialog::Delete=>{
ui.label(RichText::new("Move to trash?").size(24.0));
ui.add_space(16.0);
ui.label("The document will leave your library. Its file remains recoverable in AlphaDocs’s local trash folder.");
ui.add_space(20.0);

if primary(ui,
"Move to trash",
p,
true).clicked(){
if let Some(id)=&self.delete_id{
let source=persistence::existing_library_path(id).unwrap_or_else(||persistence::data_dir().join("documents").join(format!("{id}.adocs")));
let destination=persistence::data_dir().join("trash").join(format!("{id}.adocs"));
let result=std::fs::create_dir_all(destination.parent().expect("trash directory")).and_then(|_|std::fs::rename(source,
destination));
match result{
Ok(())=>{
self.home=true;
self.recents=persistence::recent();
}
,
Err(e)=>self.status=e.to_string()}
}
close=true;
}

if ui.button("Cancel").clicked(){
close=true;
}
}
,

        Dialog::Help => { if self.help_contents(ui) {close=true;} },
        Dialog::About => { if self.about_contents(ui) {close=true;} },
Dialog::ExportResult => {ui.label(RichText::new("Export").size(24.0));ui.add_space(16.0);ui.label(&self.message);ui.add_space(20.0);if primary(ui,"Done",p,true).clicked(){close=true;}},
Dialog::Message => { ui.label(RichText::new("AlphaDocs").size(24.0)); ui.add_space(16.0); ui.label(&self.message); ui.add_space(20.0); if primary(ui,"Continue",p,true).clicked(){close=true;} },
Dialog::Find=>{
}

        }
}
);

        self.feature_destination(feature_search::Target::Dialog, response.response.rect);
        if response.should_close() {
            close = true;
        }

        if let Some(path) = restore {
            match persistence::load_document(&path) {
                Ok(doc) => {
                    let id = self.session.editor.doc.id.clone();

                    self.session.editor.transaction(|e| {
                        e.doc = doc.clone();

                        e.doc.id = id.clone();
                    });

                    self.textures.clear();

                    self.status = "Snapshot restored; Undo returns to previous version".into();

                    close = true;
                }

                Err(e) => self.status = e,
            }
        }

        if close {
            self.dialog = None;
            if matches!(dialog, Dialog::Emojis) {
                self.emoji_reaction_target = None;
                self.emoji_reaction_comment = None;
                self.text_reaction_range = None;
            }

            self.refocus = !self.home;

            // Restore typing focus without moving a reader back to an
            // offscreen selection after informational or preference dialogs.
            if !matches!(
                dialog,
                Dialog::Preferences
                    | Dialog::Help
                    | Dialog::About
                    | Dialog::Count
                    | Dialog::Rename
                    | Dialog::Message
                    | Dialog::ExportResult
                    | Dialog::TableFormat
                    | Dialog::TableFormula
                    | Dialog::EditableObject
                    | Dialog::Tool(_)
            ) {
                self.scroll_caret = true;
            }

            #[cfg(not(test))]
            let _ = persistence::save_preferences(&self.prefs);
        }
    }

    fn find_window(&mut self, ctx: &Context) {
        let mut open = true;

        let (mut next, mut previous, mut replace, mut all) = (false, false, false, false);
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            self.dialog = None;
            self.refocus = true;
            return;
        }

        let matches = self
            .session
            .editor
            .find(&self.find_query, self.case_sensitive);

        let p = self.palette;
        let window = egui::Window::new("Find and replace")
            .id(Id::new("find_window"))
            .anchor(Align2::RIGHT_TOP, vec2(-36.0, 140.0))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .frame(
                Frame::new()
                    .fill(p.chrome)
                    .stroke(Stroke::new(0.7, p.border))
                    .corner_radius(20)
                    .inner_margin(20)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 6],
                        blur: 24,
                        spread: 0,
                        color: Color32::from_black_alpha(35),
                    }),
            )
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(24.0, 28.0), Sense::hover());
                    icons::draw(ui, rect, Icon::FindReplace, p.accent);
                    ui.label(RichText::new("Find and replace").size(18.0).strong());
                    ui.with_layout(UiLayout::right_to_left(Align::Center), |ui| {
                        if icons::button(ui, Icon::Close, "Close find and replace", false, true)
                            .clicked()
                        {
                            open = false;
                        }
                    });
                });
                ui.add_space(16.0);
                ui.label("Find");
                let find_id = Id::new("find_query_field");
                next |= ctx.memory(|m| m.has_focus(find_id))
                    && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.find_query)
                        .id(find_id)
                        .hint_text("Search this document tab")
                        .margin(vec2(10.0, 8.0))
                        .desired_width(f32::INFINITY),
                );
                if self.find_focus {
                    response.request_focus();
                    self.find_focus = false;
                }
                if response.changed() {
                    self.find_index = 0;
                    self.find_active = false;
                }
                if (response.has_focus() || response.lost_focus())
                    && ui.input(|i| i.key_pressed(Key::Enter))
                {
                    next = true;
                    response.request_focus();
                }
                ui.add_space(10.0);
                ui.label("Replace");
                ui.add_enabled(
                    !self.session.viewing,
                    egui::TextEdit::singleline(&mut self.replace_text)
                        .hint_text("Replacement text")
                        .margin(vec2(10.0, 8.0))
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(10.0);
                if ui
                    .checkbox(&mut self.case_sensitive, "Match case")
                    .changed()
                {
                    self.find_index = 0;
                    self.find_active = false;
                }
                ui.label(
                    RichText::new(format!("{} matches in this tab", matches.len()))
                        .size(12.0)
                        .color(p.muted),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    previous = ui
                        .add_enabled(
                            !matches.is_empty(),
                            egui::Button::new("Previous")
                                .corner_radius(18)
                                .min_size(vec2(100.0, 36.0)),
                        )
                        .clicked();
                    next |= ui
                        .add_enabled(
                            !matches.is_empty(),
                            egui::Button::new("Next")
                                .corner_radius(18)
                                .min_size(vec2(100.0, 36.0)),
                        )
                        .clicked();
                });
                ui.add_space(10.0);
                ui.add_enabled_ui(!self.session.viewing && !matches.is_empty(), |ui| {
                    ui.horizontal(|ui| {
                        replace = ui
                            .add(
                                egui::Button::new("Replace")
                                    .corner_radius(20)
                                    .min_size(vec2(100.0, 40.0)),
                            )
                            .clicked();
                        all = primary(ui, "Replace all", p, true).clicked();
                    });
                });
            });
        if let Some(window) = window {
            self.feature_destination(feature_search::Target::Dialog, window.response.rect);
        }

        if !open {
            self.dialog = None;

            self.refocus = true;

            return;
        }

        if (next || previous) && !matches.is_empty() {
            self.find_index = if !self.find_active {
                if previous { matches.len() - 1 } else { 0 }
            } else if previous {
                (self.find_index + matches.len() - 1) % matches.len()
            } else {
                (self.find_index + 1) % matches.len()
            };

            let (s, e) = matches[self.find_index];

            self.session.editor.anchor = s;

            self.session.editor.caret = e;

            self.scroll_caret = true;

            self.find_active = true;
        }

        if replace && !matches.is_empty() {
            let (s, e) = matches[self.find_index % matches.len()];

            self.session.editor.anchor = s;

            self.session.editor.caret = e;

            self.session.editor.insert(&self.replace_text);

            self.scroll_caret = true;
        }

        if all {
            let count = self.session.editor.replace_all(
                &self.find_query,
                &self.replace_text,
                self.case_sensitive,
            );

            self.status = format!("Replaced {count} matches");
        }
    }
}
