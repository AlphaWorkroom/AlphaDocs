use super::*;
use crate::signing::{
    FieldKind, FieldValue, Point, Signer, SignerResponse, SigningField, SigningRequest,
    SigningStatus, Stroke as SignatureStroke,
};
use std::{
    collections::BTreeMap,
    sync::mpsc::{self, Receiver},
};

#[derive(Default)]
pub(super) struct SigningUi {
    pub open: bool,
    requester: String,
    drafts: Vec<DraftSigner>,
    request: Option<SigningRequest>,
    path: Option<PathBuf>,
    preview: Option<Box<AlphaDocs>>,
    selected: usize,
    answers: HashMap<String, Answer>,
    consent: bool,
    review: Option<SignerResponse>,
    error: String,
    notice: String,
    job: Option<Receiver<Result<String, String>>>,
    confirm_close_request: bool,
}
struct DraftSigner {
    name: String,
    initials: bool,
    name_field: bool,
    note: String,
}
impl Default for DraftSigner {
    fn default() -> Self {
        Self {
            name: String::new(),
            initials: false,
            name_field: true,
            note: String::new(),
        }
    }
}
#[derive(Default)]
struct Answer {
    text: String,
    drawn: bool,
    strokes: Vec<SignatureStroke>,
}

impl AlphaDocs {
    pub(super) fn signing_screen(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let p = theme::apply(&ctx, self.prefs.theme);
        self.palette = p;
        ui.painter().rect_filled(ui.max_rect(), 0, p.bg);
        ui.set_style(ctx.style_of(ctx.theme()));
        let mut state = std::mem::take(&mut self.signing);
        if let Some(rx) = &state.job {
            match rx.try_recv() {
                Ok(Ok(message)) => {
                    state.notice = message;
                    state.job = None;
                }
                Ok(Err(error)) => {
                    state.error = error;
                    state.job = None;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(100))
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    state.error = "PDF export ended unexpectedly".into();
                    state.job = None;
                }
            }
        }
        egui::Panel::top("signing_header")
            .exact_size(48.0)
            .show_separator_line(false)
            .frame(
                Frame::new()
                    .fill(p.toolbar)
                    .inner_margin(Margin::symmetric(16, 4)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if cfg!(target_os = "macos") {
                        ui.add_space(70.0);
                    }
                    let back = ui.add_enabled(
                        state.job.is_none(),
                        egui::Button::new("← Back to document").corner_radius(18),
                    );
                    tools::bounds(ui, "signing_back", &back);
                    if back.clicked() {
                        state.open = false;
                    }
                    ui.add_space(12.0);
                    ui.label(RichText::new("eSignature").size(18.0));
                    ui.with_layout(UiLayout::right_to_left(Align::Center), |ui| {
                        if !cfg!(target_os = "macos") {
                            if chrome::window_button(ui, "Close window", 3, p).clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                            if chrome::window_button(ui, "Maximize", 1, p).clicked() {
                                let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                            }
                            if chrome::window_button(ui, "Minimize", 0, p).clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                            }
                        }
                    });
                });
            });
        if state.request.is_none() {
            egui::CentralPanel::default().frame(Frame::new().fill(p.bg).inner_margin(24))
                .show(ui, |ui| { egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.set_max_width(620.0);
                        Frame::new().fill(p.chrome).corner_radius(24).inner_margin(28).show(ui, |ui| {ui.with_layout(UiLayout::top_down(Align::Min),|ui| {
                            ui.label(RichText::new("Request a signature").size(28.0));
                            ui.add_space(8.0);
                            ui.label("Create a signing file, share it with your signers, then import their returned responses. No account is needed.");
                            ui.add_space(16.0);
                            ui.label(RichText::new(&self.session.editor.doc.title).strong());
                            ui.label(RichText::new(format!("A frozen copy of all {} document tabs. Your original stays editable.",self.session.editor.doc.tabs.len())).size(12.0).color(p.muted));
                            ui.add_space(16.0);
                            ui.label("Your name");
                            let r=text_field(ui,&mut state.requester,"Requester name",p);
                            tools::bounds(ui,"signing_requester",&r);
                            if state.drafts.is_empty() { state.drafts.push(DraftSigner::default()); }
                            let mut remove=None;
                            for (index, draft) in state.drafts.iter_mut().enumerate() {
                                ui.add_space(16.0);
                                ui.horizontal(|ui| { ui.label(RichText::new(format!("Signer {}",index+1)).strong()); if index>0 && ui.small_button("Remove").clicked() {remove=Some(index);} });
                                let r=text_field(ui,&mut draft.name,"Full name",p);
                                tools::bounds(ui,&format!("signing_name_{index}"),&r);
                                ui.label(RichText::new("Required fields").size(12.0).color(p.muted));
                                ui.horizontal_wrapped(|ui| {
                                    ui.label("Signature · Date signed");
                                    ui.checkbox(&mut draft.name_field,"Name");
                                    ui.checkbox(&mut draft.initials,"Initials");
                                });
                                text_field(ui,&mut draft.note,"Optional additional text field, e.g. Job title",p);
                            }
                            if let Some(index)=remove {state.drafts.remove(index);}
                            ui.add_space(12.0);
                            if ui.add_enabled(state.drafts.len()<10,egui::Button::new("+ Add signer")).clicked() {state.drafts.push(DraftSigner::default());}
                            ui.add_space(18.0);
                            ui.label(RichText::new("Signatures appear in a separate signing section of the completed PDF. Names are self-declared; dates come from each device’s clock.").size(12.0).color(p.muted));
                            feedback(ui,&state,p);
                            ui.add_space(20.0);
                            ui.horizontal(|ui| {
                                if ui.button("Open signing file…").clicked() { state.open_request(&ctx,&self.prefs); }
                                ui.with_layout(UiLayout::right_to_left(Align::Center),|ui| {
                                    let valid=!state.requester.trim().is_empty() && state.drafts.iter().all(|d|!d.name.trim().is_empty());
                                    let r=primary(ui,"Create signing file",p,valid);
                                    tools::bounds(ui,"signing_create",&r);
                                    if r.clicked() {
                                        match state.build_request(&self.session.editor.doc) {
                                            Ok(request) => if let Some(path)=rfd::FileDialog::new().add_filter("AlphaDocs signing request",&["asign"]).set_file_name(format!("{}.asign",safe_name(&request.document.title))).save_file() {
                                                match request.save(&path) {Ok(())=>{state.set_request(request,path,&ctx,&self.prefs);state.notice="Signing file saved. Share it with each signer; use Import response when they return their response files.".into();},Err(e)=>state.error=e}
                                            },
                                            Err(e)=>state.error=e,
                                        }
                                    }
                                });
                            });
                        });});
                    });
                }); });
        } else {
            egui::Panel::right("signing_fields")
                .exact_size(336.0)
                .resizable(false)
                .show_separator_line(false)
                .frame(Frame::new().fill(p.chrome).inner_margin(20))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("signing_fields_scroll")
                        .show(ui, |ui| {
                            state.request_panel(ui, p);
                        });
                });
            egui::CentralPanel::default()
                .frame(Frame::new().fill(p.bg).inner_margin(16))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Document for review").size(18.0));
                        ui.label(
                            RichText::new("Frozen copy · all tabs")
                                .size(12.0)
                                .color(p.muted),
                        );
                    });
                    ui.add_space(12.0);
                    if let Some(preview) = &mut state.preview {
                        preview.palette = p;
                        if preview.prepare_fonts(&ctx) {
                            ctx.request_discard("Register signing snapshot fonts");
                        }
                        preview.ensure_layout(&ctx);
                        preview.canvas(ui);
                    }
                });
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            #[cfg(not(test))]
            if let Err(error) = self.tabs.save_all(&mut self.session) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                state.error = format!("Your open documents could not be saved: {error}");
            }
        }
        if state.confirm_close_request {
            let mut close_request = false;
            let mut keep_open = false;
            egui::Modal::new(Id::new("signing_close_request"))
                .frame(
                    Frame::new()
                        .fill(p.chrome)
                        .corner_radius(24)
                        .inner_margin(Margin::same(24)),
                )
                .show(&ctx, |ui| {
                    ui.set_width(360.0);
                    ui.label(RichText::new("Close signing request?").size(24.0));
                    ui.add_space(16.0);
                    ui.label("Save the request first to keep imported responses and its current status. Your document is unaffected.");
                    ui.add_space(24.0);
                    ui.with_layout(UiLayout::right_to_left(Align::Center), |ui| {
                        let close = primary(ui, "Close request", p, state.job.is_none());
                        tools::bounds(ui, "signing_confirm_close", &close);
                        if close.clicked() && state.job.is_none() {
                            close_request = true;
                        }
                        let keep = ui.button("Keep open");
                        tools::bounds(ui, "signing_keep_open", &keep);
                        keep_open = keep.clicked();
                    });
                });
            if close_request && state.job.is_none() {
                state.clear_current_request();
            } else if keep_open {
                state.confirm_close_request = false;
            }
        }
        self.signing = state;
    }
}
impl SigningUi {
    fn build_request(&self, document: &Document) -> Result<SigningRequest, String> {
        let mut signers = Vec::new();
        let mut fields = Vec::new();
        for (i, draft) in self.drafts.iter().enumerate() {
            let signer = Signer::new(draft.name.trim())?;
            let mut kinds = vec![
                (FieldKind::Signature, "Signature".to_owned()),
                (FieldKind::Date, "Date signed".to_owned()),
            ];
            if draft.name_field {
                kinds.push((FieldKind::Name, "Full name".into()));
            }
            if draft.initials {
                kinds.push((FieldKind::Initials, "Initials".into()));
            }
            if !draft.note.trim().is_empty() {
                kinds.push((FieldKind::Text, draft.note.trim().to_owned()));
            }
            for (j, (kind, label)) in kinds.into_iter().enumerate() {
                fields.push(SigningField {
                    id: format!("field-{i}-{j}"),
                    signer_id: signer.id.clone(),
                    kind,
                    required: true,
                    label,
                });
            }
            signers.push(signer);
        }
        SigningRequest::new(document, self.requester.trim(), signers, fields)
    }
    fn set_request(
        &mut self,
        request: SigningRequest,
        path: PathBuf,
        ctx: &Context,
        prefs: &Preferences,
    ) {
        let mut settings = prefs.clone();
        settings.autosave = false;
        settings.check_updates = false;
        let mut preview = AlphaDocs::fresh(ctx, settings, Vec::new());
        let mut doc = request.document.flattened();
        doc.id = format!("signing-preview-{}", request.request_id);
        preview.session = DocumentSession::new(doc, None);
        preview.session.viewing = true;
        preview.home = false;
        self.preview = Some(Box::new(preview));
        self.request = Some(request);
        self.path = Some(path);
        self.selected = 0;
        self.answers.clear();
        self.review = None;
        self.consent = false;
        self.error.clear();
        self.notice.clear();
        self.confirm_close_request = false;
    }
    fn clear_current_request(&mut self) {
        if self.job.is_some() {
            return;
        }
        self.request = None;
        self.path = None;
        self.preview = None;
        self.selected = 0;
        self.answers.clear();
        self.consent = false;
        self.review = None;
        self.error.clear();
        self.notice.clear();
        self.confirm_close_request = false;
    }
    fn open_request(&mut self, ctx: &Context, prefs: &Preferences) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("AlphaDocs signing request", &["asign"])
            .pick_file()
        {
            match SigningRequest::load(&path) {
                Ok(request) => self.set_request(request, path, ctx, prefs),
                Err(e) => self.error = e,
            }
        }
    }
    fn request_panel(&mut self, ui: &mut Ui, p: Palette) {
        let mut returned_response = None;
        let Some(request) = self.request.as_ref() else {
            return;
        };
        ui.label(RichText::new("Signing request").size(22.0));
        ui.add_space(4.0);
        ui.label(&request.document.title);
        ui.label(
            RichText::new(format!("From {}", request.requester))
                .color(p.muted)
                .size(12.0),
        );
        ui.add_space(12.0);
        let completed = request
            .responses
            .iter()
            .filter(|r| r.status == crate::signing::SignerStatus::Completed)
            .count();
        ui.label(
            RichText::new(format!(
                "{} · {completed}/{} signed",
                status_label(request.status),
                request.signers.len()
            ))
            .color(p.accent),
        );
        ui.add_space(8.0);
        let current = self.selected.min(request.signers.len() - 1);
        let old = self.selected;
        self.selected = current;
        egui::ComboBox::from_id_salt("signing_person")
            .width(ui.available_width())
            .selected_text(&request.signers[current].name)
            .show_ui(ui, |ui| {
                for (i, signer) in request.signers.iter().enumerate() {
                    ui.selectable_value(&mut self.selected, i, &signer.name);
                }
            });
        if old != self.selected {
            self.answers.clear();
            self.review = None;
            self.consent = false;
            self.error.clear();
        }
        let signer = &request.signers[self.selected];
        let prior = request.responses.iter().find(|r| r.signer_id == signer.id);
        if let Some(response) = prior {
            ui.add_space(12.0);
            ui.label(
                if response.status == crate::signing::SignerStatus::Completed {
                    "This signer has completed their fields."
                } else {
                    "This signer declined the request."
                },
            );
        } else if request.status == SigningStatus::Pending {
            ui.add_space(12.0);
            ui.label("Fill in your assigned fields, then review your response.");
            for field in request.fields.iter().filter(|f| f.signer_id == signer.id) {
                ui.add_space(14.0);
                ui.label(RichText::new(&field.label).strong());
                let answer = self.answers.entry(field.id.clone()).or_default();
                ui.add_enabled_ui(self.review.is_none(), |ui| {
                    ui.push_id(&field.id, |ui| match field.kind {
                        FieldKind::Date => {
                            ui.label(
                                RichText::new("Set automatically when you sign")
                                    .size(12.0)
                                    .color(p.muted),
                            );
                        }
                        FieldKind::Signature | FieldKind::Initials => {
                            ui.horizontal(|ui| {
                                ui.selectable_value(&mut answer.drawn, false, "Type");
                                ui.selectable_value(&mut answer.drawn, true, "Draw");
                            });
                            if answer.drawn {
                                signature_pad(ui, answer, p);
                            } else {
                                let r = text_field(
                                    ui,
                                    &mut answer.text,
                                    if field.kind == FieldKind::Initials {
                                        "Your initials"
                                    } else {
                                        "Your signature"
                                    },
                                    p,
                                );
                                tools::bounds(ui, "signing_signature", &r);
                                if !answer.text.trim().is_empty() {
                                    Frame::new()
                                        .fill(p.toolbar)
                                        .corner_radius(12)
                                        .inner_margin(12)
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new(&answer.text).italics().size(26.0),
                                            );
                                        });
                                }
                            }
                        }
                        FieldKind::Name | FieldKind::Text => {
                            let r = text_field(ui, &mut answer.text, "", p);
                            tools::bounds(
                                ui,
                                if field.kind == FieldKind::Name {
                                    "signing_full_name"
                                } else {
                                    "signing_text"
                                },
                                &r,
                            );
                        }
                    })
                });
            }
            ui.add_space(16.0);
            let r = ui.add_enabled(
                self.review.is_none(),
                egui::Checkbox::new(
                    &mut self.consent,
                    format!("I am {} and intend to sign this document.", signer.name),
                ),
            );
            tools::bounds(ui, "signing_consent", &r);
            ui.add_space(12.0);
            if let Some(response) = self.review.clone() {
                Frame::new().fill(p.toolbar).corner_radius(14).inner_margin(14).show(ui,|ui|{
                    ui.label(RichText::new(if response.status==crate::signing::SignerStatus::Completed{"Ready to sign"}else{"Ready to decline"}).strong());
                    for field in request.fields.iter().filter(|f|f.signer_id==signer.id) {
                        if let Some(value)=response.values.get(&field.id) {
                            ui.label(RichText::new(&field.label).size(12.0).color(p.muted));
                            match value {
                                FieldValue::Text(text)|FieldValue::Date(text)=>{ui.label(text);},
                                FieldValue::TypedSignature(text)=>{ui.label(RichText::new(text).italics().size(24.0));},
                                FieldValue::DrawnSignature(strokes)=>{draw_signature(ui,strokes,p);},
                            }
                        }
                    }
                    ui.add_space(8.0);
                    ui.label("Save the response file and return it to the requester. This does not send anything automatically.");
                    ui.add_space(12.0);
                    if primary(ui,"Save response…",p,true).clicked()
                        && let Some(path)=rfd::FileDialog::new().add_filter("AlphaDocs signing response",&["asign-response"]).set_file_name(format!("{}-{}.asign-response",safe_name(&request.document.title),safe_name(&signer.name))).save_file() {
                            match response.save(request,&path) {
                                Ok(())=>{returned_response=Some((response,path));}
                                Err(e)=>self.error=e,
                            }
                        }
                });
                if ui.button("Edit response").clicked() {
                    self.review = None;
                }
            } else {
                let r = primary(ui, "Review signature", p, self.consent);
                tools::bounds(ui, "signing_review", &r);
                if r.clicked() {
                    let values = self.values(request, &signer.id);
                    match SignerResponse::complete(request, &signer.id, values) {
                        Ok(response) => {
                            self.review = Some(response);
                            self.error.clear();
                        }
                        Err(e) => self.error = e,
                    }
                }
                if ui.button("Decline to sign…").clicked() {
                    match SignerResponse::decline(request, &signer.id) {
                        Ok(response) => self.review = Some(response),
                        Err(e) => self.error = e,
                    }
                }
            }
        }
        if let Some((response, path)) = returned_response {
            match self
                .request
                .as_mut()
                .expect("request exists")
                .merge_response(response)
            {
                Ok(()) => {
                    self.review = None;
                    self.notice = format!(
                        "Response saved to {}. Return this file to the requester.",
                        path.display()
                    );
                }
                Err(e) => self.error = e,
            }
            return;
        }
        ui.add_space(20.0);
        ui.separator();
        ui.add_space(12.0);
        // Use a fresh borrow after the response controls have potentially updated state.
        let request = self.request.as_ref().expect("request exists");
        if ui.button("Import returned response…").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("AlphaDocs signing response", &["asign-response"])
                .pick_file()
        {
            let result = SignerResponse::load(&path, request).and_then(|r| {
                self.request
                    .as_mut()
                    .expect("request exists")
                    .merge_response(r)
            });
            match result {
                Ok(()) => {
                    self.notice =
                        "Response imported. Save the request to retain its updated status.".into();
                    self.error.clear();
                }
                Err(e) => self.error = e,
            }
        }
        if ui.button("Save request and status…").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("AlphaDocs signing request", &["asign"])
                .set_file_name(format!(
                    "{}.asign",
                    safe_name(
                        &self
                            .request
                            .as_ref()
                            .expect("request exists")
                            .document
                            .title
                    )
                ))
                .save_file()
        {
            match self.request.as_ref().expect("request exists").save(&path) {
                Ok(()) => {
                    self.path = Some(path);
                    self.notice = "Request and responses saved.".into();
                    self.error.clear();
                }
                Err(e) => self.error = e,
            }
        }
        let close = ui.add_enabled(self.job.is_none(), egui::Button::new("Close request…"));
        tools::bounds(ui, "signing_close_request", &close);
        if close.clicked() && self.job.is_none() {
            self.confirm_close_request = true;
        }
        let request = self.request.as_ref().expect("request exists");
        if request.is_completed() {
            ui.add_space(12.0);
            if primary(
                ui,
                if self.job.is_some() {
                    "Preparing PDF…"
                } else {
                    "Export completed PDF…"
                },
                p,
                self.job.is_none(),
            )
            .clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("PDF document", &["pdf"])
                    .set_file_name(format!("{}-signed.pdf", safe_name(&request.document.title)))
                    .save_file()
            {
                let snapshot = request.clone();
                let (tx, rx) = mpsc::channel();
                let ctx = ui.ctx().clone();
                self.job = Some(rx);
                std::thread::spawn(move || {
                    let result = snapshot
                        .completed_pdf()
                        .and_then(|bytes| persistence::atomic_write(&path, &bytes))
                        .map(|()| format!("Completed PDF saved to {}", path.display()));
                    let _ = tx.send(result);
                    ctx.request_repaint();
                });
            }
        }
        ui.add_space(12.0);
        ui.collapsing("Signing record", |ui| {
            for event in &self.request.as_ref().expect("request exists").audit {
                ui.label(
                    RichText::new(format!(
                        "{} · {}\n{}",
                        event.local_time, event.actor, event.detail
                    ))
                    .size(12.0),
                );
                ui.add_space(6.0);
            }
        });
        ui.label(RichText::new("Local signing files use self-declared names and device timestamps. They do not verify identity or provide certificate-backed digital signatures.").size(11.0).color(p.muted));
        feedback(ui, self, p);
    }
    fn values(&self, request: &SigningRequest, signer_id: &str) -> BTreeMap<String, FieldValue> {
        request
            .fields
            .iter()
            .filter(|f| f.signer_id == signer_id && f.kind != FieldKind::Date)
            .filter_map(|field| {
                let answer = self.answers.get(&field.id)?;
                let value = if matches!(field.kind, FieldKind::Signature | FieldKind::Initials) {
                    if answer.drawn {
                        FieldValue::DrawnSignature(answer.strokes.clone())
                    } else {
                        FieldValue::TypedSignature(answer.text.trim().into())
                    }
                } else {
                    FieldValue::Text(answer.text.trim().into())
                };
                Some((field.id.clone(), value))
            })
            .collect()
    }
}
fn feedback(ui: &mut Ui, state: &SigningUi, p: Palette) {
    if !state.error.is_empty() {
        ui.add_space(12.0);
        ui.colored_label(
            if p.dark {
                Color32::from_rgb(255, 180, 171)
            } else {
                Color32::from_rgb(179, 38, 30)
            },
            &state.error,
        );
    }
    if !state.notice.is_empty() {
        ui.add_space(12.0);
        ui.label(RichText::new(&state.notice).color(p.accent));
    }
}
fn status_label(status: SigningStatus) -> &'static str {
    match status {
        SigningStatus::Pending => "Awaiting signatures",
        SigningStatus::Completed => "Completed",
        SigningStatus::Declined => "Declined",
        SigningStatus::Cancelled => "Cancelled",
    }
}
fn signature_pad(ui: &mut Ui, answer: &mut Answer, p: Palette) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 110.0), Sense::drag());
    tools::bounds(ui, "signing_draw_pad", &response);
    ui.painter().rect(
        rect,
        12,
        p.toolbar,
        Stroke::new(1.0, p.border),
        egui::StrokeKind::Inside,
    );
    ui.painter().line_segment(
        [
            rect.left_bottom() + vec2(12.0, -22.0),
            rect.right_bottom() + vec2(-12.0, -22.0),
        ],
        Stroke::new(1.0, p.border),
    );
    if (response.drag_started() || response.is_pointer_button_down_on())
        && let Some(point) = response.interact_pointer_pos()
    {
        if (response.drag_started() || answer.strokes.is_empty()) && answer.strokes.len() < 64 {
            answer.strokes.push(SignatureStroke { points: Vec::new() });
        }
        let total = answer.strokes.iter().map(|s| s.points.len()).sum::<usize>();
        if let Some(stroke) = answer.strokes.last_mut() {
            let point = Point {
                x: ((point.x - rect.left()) / rect.width()).clamp(0.0, 1.0),
                y: ((point.y - rect.top()) / rect.height()).clamp(0.0, 1.0),
            };
            if stroke.points.len() < 512
                && total < 5000
                && stroke.points.last().is_none_or(|last| {
                    ((last.x - point.x).abs() + (last.y - point.y).abs()) > 0.002
                })
            {
                stroke.points.push(point);
            }
        }
        ui.ctx().request_repaint();
    }
    for stroke in &answer.strokes {
        for pair in stroke.points.windows(2) {
            ui.painter().line_segment(
                [pair[0], pair[1]]
                    .map(|v| rect.min + vec2(v.x * rect.width(), v.y * rect.height())),
                Stroke::new(2.0, p.text),
            );
        }
        if stroke.points.len() == 1 {
            let v = stroke.points[0];
            ui.painter().circle_filled(
                rect.min + vec2(v.x * rect.width(), v.y * rect.height()),
                1.0,
                p.text,
            );
        }
    }
    if ui.small_button("Clear drawing").clicked() {
        answer.strokes.clear();
    }
}
fn draw_signature(ui: &mut Ui, strokes: &[SignatureStroke], p: Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 76.0), Sense::hover());
    for stroke in strokes {
        for pair in stroke.points.windows(2) {
            ui.painter().line_segment(
                [pair[0], pair[1]]
                    .map(|v| rect.min + vec2(v.x * rect.width(), v.y * rect.height())),
                Stroke::new(1.8, p.text),
            );
        }
    }
}

fn text_field(ui: &mut Ui, value: &mut String, hint: &str, p: Palette) -> egui::Response {
    let frame = Frame::new()
        .fill(p.chrome)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(6)
        .inner_margin(Margin::symmetric(10, 8));
    frame
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(value)
                    .frame(Frame::NONE)
                    .desired_width(f32::INFINITY)
                    .hint_text(hint),
            )
        })
        .inner
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Block, ContentTab, Paragraph};

    fn fixture_document() -> Document {
        let mut document = Document::new("Signing fixture");
        document.blocks = vec![Block::Paragraph(Paragraph::from_text("First tab text"))];
        let mut other = Document::new("Other tab");
        other.blocks = vec![Block::Paragraph(Paragraph::from_text("Second tab text"))];
        document.tabs.push(ContentTab {
            id: format!("tab-{}", other.id),
            title: "Second tab".into(),
            blocks: other.blocks.into(),
        });
        document
    }

    #[test]
    fn signing_screen_renders_a_readonly_frozen_copy_of_all_tabs() {
        let ctx = Context::default();
        crate::fonts::install(&ctx);
        let source = fixture_document();
        let source_snapshot = source.clone();
        let signer = Signer::new("Alex Example").unwrap();
        let fields = vec![SigningField {
            id: "signature-1".into(),
            signer_id: signer.id.clone(),
            kind: FieldKind::Signature,
            required: true,
            label: "Signature".into(),
        }];
        let request = SigningRequest::new(&source, "Requester", vec![signer], fields).unwrap();

        let mut app = AlphaDocs::fresh(&ctx, Preferences::default(), vec![]);
        app.home = false;
        app.session = DocumentSession::new(source, None);
        app.signing.open = true;
        app.signing.set_request(
            request.clone(),
            PathBuf::from("fixture.asign"),
            &ctx,
            &app.prefs,
        );
        let preview = app.signing.preview.as_ref().unwrap();
        assert!(preview.session.viewing);
        assert!(preview.session.editor.doc.blocks.iter().any(|block| {
            matches!(block, Block::Paragraph(paragraph) if paragraph.text().contains("First tab text"))
        }));
        assert!(preview.session.editor.doc.blocks.iter().any(|block| {
            matches!(block, Block::Paragraph(paragraph) if paragraph.text().contains("Second tab text"))
        }));

        let mut output = ctx.run_ui(Default::default(), |ui| app.update_ui(ui));
        output.textures_delta.clear();
        assert_eq!(app.session.editor.doc, source_snapshot);
        assert_eq!(
            app.signing.request.as_ref().unwrap().document,
            source_snapshot
        );
        assert!(app.signing.preview.as_ref().unwrap().session.viewing);
    }

    fn app_with_request(ctx: &Context) -> AlphaDocs {
        crate::fonts::install(ctx);
        let source = fixture_document();
        let signer = Signer::new("Alex Example").unwrap();
        let request = SigningRequest::new(
            &source,
            "Requester",
            vec![signer.clone()],
            vec![SigningField {
                id: "signature-1".into(),
                signer_id: signer.id,
                kind: FieldKind::Signature,
                required: true,
                label: "Signature".into(),
            }],
        )
        .unwrap();
        let mut app = AlphaDocs::fresh(ctx, Preferences::default(), vec![]);
        app.home = false;
        app.session = DocumentSession::new(source, None);
        app.signing.open = true;
        app.signing.requester = "Retained requester".into();
        app.signing.drafts = vec![DraftSigner {
            name: "Retained signer".into(),
            ..Default::default()
        }];
        app.signing
            .set_request(request, PathBuf::from("fixture.asign"), ctx, &app.prefs);
        app
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

    fn click(ctx: &Context, app: &mut AlphaDocs, id: &str) {
        let rect = ctx
            .data(|data| data.get_temp::<Rect>(Id::new(id)))
            .expect("button bounds from the last frame");
        let point = rect.center();
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        frame(
            ctx,
            app,
            vec![Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }

    #[test]
    fn close_request_confirmation_can_be_cancelled_or_confirmed_without_losing_drafts() {
        let ctx = Context::default();
        let mut app = app_with_request(&ctx);
        app.signing.confirm_close_request = true;
        for _ in 0..2 {
            frame(&ctx, &mut app, vec![]);
        }
        click(&ctx, &mut app, "signing_keep_open");
        assert!(app.signing.request.is_some());
        assert!(!app.signing.confirm_close_request);
        assert_eq!(app.signing.requester, "Retained requester");
        assert_eq!(app.signing.drafts[0].name, "Retained signer");

        app.signing.confirm_close_request = true;
        for _ in 0..2 {
            frame(&ctx, &mut app, vec![]);
        }
        click(&ctx, &mut app, "signing_confirm_close");
        assert!(app.signing.request.is_none());
        assert!(app.signing.path.is_none());
        assert!(app.signing.preview.is_none());
        assert!(app.signing.answers.is_empty());
        assert!(app.signing.review.is_none());
        assert!(app.signing.error.is_empty());
        assert!(app.signing.notice.is_empty());
        assert!(!app.signing.confirm_close_request);
        assert_eq!(app.signing.requester, "Retained requester");
        assert_eq!(app.signing.drafts[0].name, "Retained signer");
    }

    #[test]
    fn fields_cannot_change_after_reviewed_response_is_created() {
        let ctx = Context::default();
        let mut app = app_with_request(&ctx);
        let request = app.signing.request.as_ref().unwrap();
        let signer_id = request.signers[0].id.clone();
        let response = SignerResponse::complete(
            request,
            &signer_id,
            BTreeMap::from([(
                "signature-1".into(),
                FieldValue::TypedSignature("Alex Example".into()),
            )]),
        )
        .unwrap();
        app.signing.answers.insert(
            "signature-1".into(),
            Answer {
                text: "Alex Example".into(),
                ..Default::default()
            },
        );
        app.signing.review = Some(response.clone());
        app.signing.consent = true;
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        let rect = ctx
            .data(|data| data.get_temp::<Rect>(Id::new("signing_signature")))
            .expect("signature field bounds");
        let point = rect.center();
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                Event::Text("Changed after review".into()),
            ],
        );
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
        assert_eq!(app.signing.answers["signature-1"].text, "Alex Example");
        assert_eq!(app.signing.review.as_ref().unwrap(), &response);
    }
}
