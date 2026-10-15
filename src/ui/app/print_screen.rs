use super::*;
use crate::printing::{self, Area, Capabilities, Scale, Settings, Validated};
use std::sync::{Arc, mpsc};

enum Event {
    Source(Result<Vec<u8>, String>),
    Printers(Result<Vec<printing::Printer>, String>),
    Capabilities(u64, Result<Capabilities, String>),
    Preview(u64, Result<Prepared, String>),
    Submitted(Result<(), String>),
}
struct Prepared {
    bytes: Vec<u8>,
    image: Result<Vec<u8>, String>,
    validated: Option<Validated>,
    area: Area,
}
pub(super) struct PrintState {
    source: Option<Arc<Vec<u8>>>,
    pages: usize,
    title: String,
    printers: Vec<printing::Printer>,
    discovering: bool,
    discovery_error: Option<String>,
    destination: Option<String>,
    caps: Option<Capabilities>,
    caps_loading: bool,
    caps_generation: u64,
    media: usize,
    choices: Vec<Option<usize>>,
    settings: Settings,
    percent: f32,
    preview_page: usize,
    revision: u64,
    changed: Instant,
    in_flight: bool,
    prepared: Option<Prepared>,
    preview: Option<egui::TextureHandle>,
    error: Option<String>,
    preview_error: Option<String>,
    submitting: bool,
    completion: Option<String>,
    tx: mpsc::Sender<Event>,
    rx: mpsc::Receiver<Event>,
}
impl PrintState {
    fn new(document: Document, layout: Layout) -> Self {
        let mut state = Self::empty(document.title.clone(), layout.pages);
        let sender = state.tx.clone();
        std::thread::spawn(move || {
            let _ = sender.send(Event::Source(compat::pdf::encode(&document, &layout)));
        });
        state.discover();
        state
    }
    fn empty(title: String, pages: usize) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            source: None,
            pages,
            title,
            printers: vec![],
            discovering: false,
            discovery_error: None,
            destination: None,
            caps: None,
            caps_loading: false,
            caps_generation: 0,
            media: 0,
            choices: vec![],
            settings: Settings::default(),
            percent: 100.0,
            preview_page: 0,
            revision: 0,
            changed: Instant::now(),
            in_flight: false,
            prepared: None,
            preview: None,
            error: None,
            preview_error: None,
            submitting: false,
            completion: None,
            tx,
            rx,
        }
    }
    fn discover(&mut self) {
        if self.discovering {
            return;
        }
        self.discovering = true;
        self.discovery_error = None;
        let sender = self.tx.clone();
        std::thread::spawn(move || {
            let _ = sender.send(Event::Printers(printing::discover()));
        });
    }
    fn select_destination(&mut self, destination: Option<String>) {
        self.destination = destination;
        self.caps = None;
        self.caps_generation += 1;
        self.media = 0;
        self.choices.clear();
        self.settings = Settings {
            range: self.settings.range.clone(),
            scale: self.settings.scale,
            ..Default::default()
        };
        self.caps_loading = self.destination.is_some();
        if let Some(name) = self.destination.clone() {
            let sender = self.tx.clone();
            let key = self.caps_generation;
            std::thread::spawn(move || {
                let _ = sender.send(Event::Capabilities(key, printing::capabilities(&name)));
            });
        }
        self.invalidate();
    }
    fn invalidate(&mut self) {
        self.revision += 1;
        self.changed = Instant::now();
        self.prepared = None;
        self.preview = None;
        self.error = None;
        self.preview_error = None;
        self.completion = None;
    }
    fn selected_pages(&self) -> Result<Vec<usize>, String> {
        printing::page_range(&self.settings.range, self.pages)
    }
    fn settings(&self) -> Settings {
        let mut settings = self.settings.clone();
        settings.tickets.clear();
        if let Some(caps) = &self.caps {
            if let Some(media) = caps.media.get(self.media) {
                settings.paper = media.size;
                settings.tickets.push(media.option.ticket.clone());
            }
            for (feature, choice) in caps.features.iter().zip(&self.choices) {
                if let Some(option) = choice.and_then(|i| feature.options.get(i)) {
                    settings.tickets.push(option.ticket.clone());
                }
            }
        }
        settings
    }
    fn poll(&mut self, ctx: &Context) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Source(result) => match result {
                    Ok(bytes) => self.source = Some(Arc::new(bytes)),
                    Err(e) => self.error = Some(e),
                },
                Event::Printers(result) => {
                    self.discovering = false;
                    match result {
                        Ok(printers) => self.printers = printers,
                        Err(e) => self.discovery_error = Some(e),
                    }
                }
                Event::Capabilities(key, result) if key == self.caps_generation => {
                    self.caps_loading = false;
                    match result {
                        Ok(caps) => {
                            if caps.media.is_empty() {
                                self.error=Some("The driver did not report paper sizes. Choose Save as PDF or another printer.".into());
                            }
                            self.media = caps
                                .media
                                .iter()
                                .position(|m| {
                                    (m.size[0] - 595.28).abs() < 1.0
                                        && (m.size[1] - 841.89).abs() < 1.0
                                })
                                .unwrap_or(0);
                            self.choices = caps
                                .features
                                .iter()
                                .map(|f| {
                                    if f.label == "Orientation" {
                                        f.options
                                            .iter()
                                            .position(|o| o.landscape == Some(false))
                                            .or(Some(0))
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            self.caps = Some(caps);
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
                Event::Capabilities(..) => {}
                Event::Preview(key, result) => {
                    self.in_flight = false;
                    if key != self.revision {
                        continue;
                    }
                    match result {
                        Ok(prepared) => {
                            match &prepared.image {
                                Ok(bytes) => match image::load_from_memory(bytes) {
                                    Ok(image) => {
                                        let rgba = image.to_rgba8();
                                        self.preview = Some(ctx.load_texture(
                                            "print-preview",
                                            egui::ColorImage::from_rgba_unmultiplied(
                                                [rgba.width() as usize, rgba.height() as usize],
                                                rgba.as_raw(),
                                            ),
                                            egui::TextureOptions::LINEAR,
                                        ));
                                    }
                                    Err(e) => {
                                        self.preview_error =
                                            Some(format!("Could not display preview: {e}"))
                                    }
                                },
                                Err(e) => self.preview_error = Some(e.clone()),
                            }
                            self.prepared = Some(prepared);
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
                Event::Submitted(result) => {
                    self.submitting = false;
                    match result {
                        Ok(()) => {
                            self.completion = Some("Job handed to the Windows print system".into())
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
            }
        }
        if self.source.is_some()
            && self.prepared.is_none()
            && self.error.is_none()
            && !self.in_flight
            && !self.caps_loading
            && (self.destination.is_none()
                || self.caps.as_ref().is_some_and(|c| !c.media.is_empty()))
            && self.changed.elapsed() > Duration::from_millis(180)
        {
            let indices = match self.selected_pages() {
                Ok(v) => v,
                Err(e) => {
                    self.error = Some(e);
                    return;
                }
            };
            self.preview_page = self.preview_page.min(indices.len().saturating_sub(1));
            let source = self.source.clone().unwrap();
            let settings = self.settings();
            let destination = self.destination.clone();
            let sender = self.tx.clone();
            let key = self.revision;
            let page = self.preview_page;
            self.in_flight = true;
            std::thread::spawn(move || {
                let result = (|| {
                    let validated = destination
                        .as_deref()
                        .map(|name| printing::validate(name, &settings))
                        .transpose()?;
                    let area = validated
                        .as_ref()
                        .and_then(|v| v.area)
                        .unwrap_or_else(|| Area::full(settings.output_size()));
                    let bytes = printing::prepare_pdf(&source, &settings, area)?;
                    let image = printing::render_preview(&bytes, page).and_then(|png| {
                        if validated
                            .as_ref()
                            .is_some_and(|v| v.monochrome == Some(true))
                        {
                            let image = image::load_from_memory(&png)
                                .map_err(|e| e.to_string())?
                                .grayscale();
                            let mut cursor = std::io::Cursor::new(Vec::new());
                            image
                                .write_to(&mut cursor, image::ImageFormat::Png)
                                .map_err(|e| e.to_string())?;
                            Ok(cursor.into_inner())
                        } else {
                            Ok(png)
                        }
                    });
                    Ok(Prepared {
                        bytes,
                        image,
                        validated,
                        area,
                    })
                })();
                let _ = sender.send(Event::Preview(key, result));
            });
        }
        if self.discovering
            || self.caps_loading
            || self.in_flight
            || self.submitting
            || self.prepared.is_none()
        {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
    fn submit(&mut self) {
        let Some(name) = self.destination.clone() else {
            return;
        };
        let Some(prepared) = &self.prepared else {
            return;
        };
        let Some(validated) = &prepared.validated else {
            return;
        };
        let bytes = prepared.bytes.clone();
        let ticket = validated.ticket.clone();
        let area = validated.area;
        let monochrome = validated.monochrome;
        let settings = self.settings();
        let sender = self.tx.clone();
        self.submitting = true;
        self.error = None;
        std::thread::spawn(move || {
            let result = (|| {
                // Revalidate immediately before submission. Changed margins invalidate the preview.
                let fresh = printing::validate(&name, &settings)?;
                if fresh.ticket != ticket || fresh.area != area || fresh.monochrome != monochrome {
                    return Err("The printer settings changed. Reopen Print to refresh the preview before printing.".into());
                }
                let path = persistence::data_dir()
                    .join("print")
                    .join(format!("job-{}.pdf", Document::new("Print job").id));
                persistence::atomic_write(&path, &bytes)?;
                let result = printing::submit(&name, &path, ticket);
                let _ = std::fs::remove_file(&path);
                result
            })();
            let _ = sender.send(Event::Submitted(result));
        });
    }
}
impl AlphaDocs {
    pub(super) fn start_print(&mut self, ctx: &Context) {
        self.ensure_layout(ctx);
        let document = self.session.editor.doc.flattened();
        let layout = Layout::measure(ctx, &document);
        self.print_state = Some(PrintState::new(document, layout));
        self.dialog = Some(Dialog::Print);
    }
    pub(super) fn print_screen(&mut self, ctx: &Context) {
        let Some(mut state) = self.print_state.take() else {
            self.dialog = None;
            return;
        };
        state.poll(ctx);
        let p = self.palette;
        let mut close = false;
        let mut save = false;
        let mut submit = false;
        let width = (ctx.content_rect().width() - 64.0).clamp(840.0, 1080.0);
        let height = (ctx.content_rect().height() - 280.0).clamp(280.0, 660.0);
        let response=egui::Modal::new(Id::new("native_print")).frame(Frame::new().fill(p.chrome).corner_radius(24).inner_margin(24)).show(ctx,|ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Print").size(24.0));
                ui.with_layout(UiLayout::right_to_left(Align::Center),|ui|{if icons::button(ui,Icon::Close,"Close print preview",false,true).clicked(){close=true;}});
            });
            ui.add_space(4.0);ui.label(RichText::new(&state.title).color(p.muted));ui.add_space(18.0);
            let before=state.settings.clone();let before_media=state.media;let before_choices=state.choices.clone();
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(vec2(300.0,height),UiLayout::top_down(Align::Min),|ui| {
                    if state.submitting {ui.disable();}
                    // Material-style form controls are local to the print sheet.
                    ui.spacing_mut().interact_size.y=36.0;
                    ui.spacing_mut().button_padding=vec2(12.0,8.0);
                    ui.visuals_mut().button_frame=true;
                    ui.visuals_mut().widgets.inactive.bg_fill=p.chrome;
                    ui.visuals_mut().widgets.inactive.weak_bg_fill=p.chrome;
                    ui.visuals_mut().widgets.inactive.bg_stroke=Stroke::new(1.0,p.border);
                    ui.visuals_mut().widgets.inactive.corner_radius=8.into();
                    ui.visuals_mut().widgets.hovered.corner_radius=8.into();
                    ui.visuals_mut().widgets.active.corner_radius=8.into();
                    egui::ScrollArea::vertical().id_salt("print_settings").max_height(height).show(ui,|ui| {
                        ui.set_width(280.0);
                        section(ui,"Destination",p);
                        let mut destination=state.destination.clone();
                        egui::ComboBox::from_id_salt("print_destination").width(274.0).selected_text(destination.as_deref().unwrap_or("Save as PDF")).show_ui(ui,|ui| {
                            ui.selectable_value(&mut destination,None,"Save as PDF");
                            for printer in &state.printers {
                                let label=format!("{}{}",printer.name,if printer.is_default {" · default"} else {""});
                                ui.selectable_value(&mut destination,Some(printer.name.clone()),label);
                            }
                        });
                        if destination!=state.destination && !state.submitting {state.select_destination(destination);}
                        ui.horizontal(|ui| {if state.discovering {ui.spinner();ui.label("Finding printers…");} else if ui.add(egui::Button::new(RichText::new("Refresh printers").color(p.accent).size(12.0)).frame(false).min_size(vec2(0.0,28.0))).clicked(){state.discover();}});
                        if let Some(error)=&state.discovery_error {ui.label(RichText::new(error).small().color(p.muted));}
                        else if !state.discovering && state.printers.is_empty() {ui.label(RichText::new("No system printers found").small().color(p.muted));}
                        section(ui,"Pages",p);
                        ui.label(format!("{} {} · all document tabs",state.pages,if state.pages==1 {"page"}else{"pages"}));
                        ui.add_enabled(!state.submitting,egui::TextEdit::singleline(&mut state.settings.range).desired_width(274.0).hint_text("All pages, or 1-3, 5").margin(vec2(10.0,8.0)));
                        section(ui,"Paper size",p);
                        if state.caps_loading {ui.horizontal(|ui|{ui.spinner();ui.label("Reading printer settings…");});}
                        else if state.destination.is_none() {
                            egui::ComboBox::from_id_salt("pdf_paper").width(274.0).selected_text(paper_label(state.settings.paper)).show_ui(ui,|ui|{
                                for (label,size) in [("A4",[595.28,841.89]),("Letter",[612.0,792.0]),("Legal",[612.0,1008.0]),("A3",[841.89,1190.55])] {ui.selectable_value(&mut state.settings.paper,size,label);}
                            });
                            ui.horizontal(|ui|{ui.selectable_value(&mut state.settings.landscape,false,"Portrait");ui.selectable_value(&mut state.settings.landscape,true,"Landscape");});
                        } else if let Some(caps)=&state.caps {
                            egui::ComboBox::from_id_salt("driver_paper").width(274.0).selected_text(caps.media.get(state.media).map_or("Not reported",|m|m.option.label.as_str())).show_ui(ui,|ui| {
                                for (index,media) in caps.media.iter().enumerate(){ui.selectable_value(&mut state.media,index,&media.option.label);}
                            });
                            ui.add_space(12.0);
                            if let Some(max)=caps.max_copies.filter(|v|*v>1) {
                                ui.horizontal(|ui|{ui.label("Copies");ui.add(egui::DragValue::new(&mut state.settings.copies).range(1..=max));});
                            }
                            driver_features(ui,&mut state,p,false);
                        }
                        section(ui,"Scale",p);
                        let mut fit=matches!(state.settings.scale,Scale::Fit);
                        ui.horizontal(|ui|{ui.selectable_value(&mut fit,true,"Fit to printable area");ui.selectable_value(&mut fit,false,"Custom");});
                        if fit {state.settings.scale=Scale::Fit;} else {
                            image_sidebar::adjustment_control(ui,&mut state.percent,10.0..=200.0,100.0,p);
                            state.settings.scale=Scale::Percent(state.percent);
                            ui.label(RichText::new("Double-click the slider to reset to 100%").small().color(p.muted));
                        }
                        if state.caps.as_ref().is_some_and(|c|c.features.iter().any(|f|!matches!(f.label.as_str(),"Orientation"|"Colour"))) {
                            ui.add_space(16.0);
                            egui::CollapsingHeader::new("More settings").id_salt("print_more_settings").show_unindented(ui,|ui|driver_features(ui,&mut state,p,true));
                        }
                        ui.add_space(16.0);ui.label(RichText::new("Print settings do not change your document.").small().color(p.muted));
                    });
                });
                let preview_width=width-320.0;
                Frame::new().fill(p.toolbar).corner_radius(16).inner_margin(18).show(ui,|ui| {
                    ui.set_width(preview_width-36.0);ui.set_height(height-36.0);
                    ui.with_layout(UiLayout::top_down(Align::Min),|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Preview").strong());
                        let count=state.selected_pages().map_or(0,|v|v.len());
                        ui.with_layout(UiLayout::right_to_left(Align::Center),|ui|{
                            if ui.add_enabled(state.preview_page+1<count,egui::Button::new("›")).clicked(){state.preview_page+=1;state.invalidate();}
                            ui.label(format!("{} / {}",if count==0 {0} else {state.preview_page+1},count));
                            if ui.add_enabled(state.preview_page>0,egui::Button::new("‹")).clicked(){state.preview_page-=1;state.invalidate();}
                        });
                    });
                    ui.add_space(12.0);
                    let size=state.settings().output_size();
                    let available=vec2(preview_width-54.0,height-105.0);
                    let factor=(available.x/size[0]).min(available.y/size[1]);let sheet=vec2(size[0]*factor,size[1]*factor);
                    ui.allocate_ui_with_layout(available,UiLayout::top_down(Align::Center),|ui| {
                        if let Some(texture)=&state.preview {ui.add(egui::Image::new(texture).fit_to_exact_size(sheet));}
                        else {
                            let (rect,_)=ui.allocate_exact_size(sheet,Sense::hover());
                            ui.painter().rect_filled(rect,1,Color32::WHITE);
                            let label=if let Some(error)=&state.error {error.as_str()} else if let Some(error)=&state.preview_error {error.as_str()} else {"Preparing preview…"};
                            let galley=ui.painter().layout(label.into(),FontId::proportional(14.0),Color32::from_gray(80),sheet.x-40.0);
                            ui.painter().galley(rect.center()-galley.size()*0.5,galley,Color32::from_gray(80));
                        }
                    });
                    let size=state.settings().output_size();
                    ui.label(RichText::new(format!("{:.0} × {:.0} mm",size[0]*25.4/72.0,size[1]*25.4/72.0)).small().color(p.muted));
                    });
                });
            });
            if before.paper!=state.settings.paper || before.landscape!=state.settings.landscape || before.scale!=state.settings.scale || before.copies!=state.settings.copies || before.range!=state.settings.range || before_media!=state.media || before_choices!=state.choices {state.invalidate();}
            ui.add_space(18.0);
            if let Some(error)=&state.error {ui.label(RichText::new(error).color(p.accent));}
            if let Some(prepared)=&state.prepared {
                if state.destination.is_some() && prepared.validated.as_ref().is_some_and(|v|v.area.is_none()) {
                    ui.label(RichText::new("This driver did not report printable margins. The preview shows the full sheet; hardware margins may clip it.").small().color(p.muted));
                }
                let source_size=self.session.editor.doc.page.size();
                if let Ok(transform)=printing::Transform::new([source_size.0,source_size.1],state.settings().output_size(),prepared.area,state.settings.scale) {
                    let colour=prepared.validated.as_ref().and_then(|v|v.monochrome).map_or(if state.destination.is_none(){"Document colours"}else{"Driver colour mode not reported"},|mono|if mono {"Grayscale preview"}else{"Colour preview"});
                    ui.label(RichText::new(format!("Content scale: {:.1}% · {colour}",transform.scale*100.0)).small().color(p.muted));
                }
            }
            if let Some(completion)=&state.completion {ui.label(RichText::new(completion).color(p.accent));}
            ui.horizontal(|ui| {
                if state.submitting {ui.spinner();ui.label("Submitting print job…");}
                ui.with_layout(UiLayout::right_to_left(Align::Center),|ui|{
                    let ready=state.prepared.is_some() && state.error.is_none() && !state.submitting;
                    if primary(ui,if state.destination.is_some(){"Print"}else{"Save PDF"},p,ready).clicked(){if state.destination.is_some(){submit=true;}else{save=true;}}
                    if ui.button(if state.submitting {"Close"} else {"Cancel"}).clicked(){close=true;}
                });
            });
        });
        self.feature_destination(feature_search::Target::Dialog, response.response.rect);
        if save
            && let Some(prepared) = &state.prepared
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("PDF document", &["pdf"])
                .set_file_name(format!("{}.pdf", state.title))
                .save_file()
        {
            match persistence::atomic_write(&path, &prepared.bytes) {
                Ok(()) => {
                    self.status = "Print PDF saved with the selected paper and scale".into();
                    close = true;
                }
                Err(e) => state.error = Some(e),
            }
        }
        if submit {
            state.submit();
        }
        if close || response.should_close() {
            self.dialog = None;
            self.refocus = true;
        } else {
            self.print_state = Some(state);
        }
    }
}
fn section(ui: &mut Ui, label: &str, p: Palette) {
    ui.add_space(14.0);
    ui.label(RichText::new(label).strong().color(p.text));
    ui.add_space(4.0);
}
fn driver_features(ui: &mut Ui, state: &mut PrintState, p: Palette, more: bool) {
    let Some(caps) = &state.caps else {
        return;
    };
    for (index, feature) in caps
        .features
        .iter()
        .enumerate()
        .filter(|(_, f)| more != matches!(f.label.as_str(), "Orientation" | "Colour"))
    {
        section(ui, &feature.label, p);
        let choice = &mut state.choices[index];
        let default_colour = if feature.label == "Colour" {
            state
                .prepared
                .as_ref()
                .and_then(|p| p.validated.as_ref())
                .and_then(|v| v.monochrome)
                .map(|mono| {
                    if mono {
                        "Default · Grayscale"
                    } else {
                        "Default · Colour"
                    }
                })
        } else {
            None
        };
        if feature.options.len() == 1 {
            Frame::new()
                .stroke(Stroke::new(1.0, p.border))
                .corner_radius(8)
                .inner_margin(10)
                .show(ui, |ui| {
                    ui.set_width(254.0);
                    ui.label(&feature.options[0].label);
                });
        } else {
            egui::ComboBox::from_id_salt(("driver_feature", index))
                .width(274.0)
                .selected_text(
                    choice
                        .and_then(|i| feature.options.get(i))
                        .map_or(default_colour.unwrap_or("Printer default"), |o| {
                            o.label.as_str()
                        }),
                )
                .show_ui(ui, |ui| {
                    if feature.label != "Orientation" {
                        ui.selectable_value(choice, None, "Printer default");
                    }
                    for (i, option) in feature.options.iter().enumerate() {
                        ui.selectable_value(choice, Some(i), &option.label);
                    }
                });
        }
        if feature.label == "Orientation" {
            state.settings.landscape = choice
                .and_then(|i| feature.options.get(i))
                .is_some_and(|o| o.landscape == Some(true));
        }
    }
}
fn paper_label(size: [f32; 2]) -> &'static str {
    if (size[0] - 612.0).abs() < 1.0 {
        if size[1] > 900.0 { "Legal" } else { "Letter" }
    } else if size[0] > 800.0 {
        "A3"
    } else {
        "A4"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn print_modal_fits_native_viewports_in_both_themes_without_document_mutation() {
        for theme in [Theme::Light, Theme::Dark] {
            for size in [vec2(960.0, 640.0), vec2(1440.0, 960.0)] {
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
                app.dialog = Some(Dialog::Print);
                app.print_state = Some(PrintState::empty("Print QA".into(), 2));
                app.feature_pulse = Some((Instant::now(), feature_search::Target::Dialog));
                let original = app.session.editor.doc.clone();
                let revision = app.session.editor.revision;
                for _ in 0..3 {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                            ..Default::default()
                        },
                        |ui| app.update_ui(ui),
                    );
                    output.textures_delta.clear();
                }
                let rect = app.feature_rect.unwrap();
                assert!(
                    rect.width() <= size.x && rect.height() <= size.y,
                    "{theme:?} {size:?} modal {rect:?}"
                );
                assert!(
                    rect.min.x >= 0.0 && rect.min.y >= 0.0,
                    "Modal escaped viewport: {rect:?}"
                );
                assert_eq!(app.session.editor.doc, original);
                assert_eq!(app.session.editor.revision, revision);
            }
        }
    }
    #[test]
    fn stale_preview_or_driver_results_cannot_replace_current_settings() {
        let ctx = Context::default();
        let mut state = PrintState::empty("QA".into(), 3);
        state.revision = 5;
        state.caps_generation = 4;
        state
            .tx
            .send(Event::Preview(3, Err("Stale preview failure".into())))
            .unwrap();
        state
            .tx
            .send(Event::Capabilities(2, Err("Stale driver failure".into())))
            .unwrap();
        state.poll(&ctx);
        assert!(state.error.is_none());
        assert!(state.caps.is_none());
        state
            .tx
            .send(Event::Preview(5, Err("Current setting failure".into())))
            .unwrap();
        state.poll(&ctx);
        assert_eq!(state.error.as_deref(), Some("Current setting failure"));
    }
}
