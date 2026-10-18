use super::*;
use std::{
    io::Read,
    sync::mpsc::{self, Receiver},
};
#[derive(Default, PartialEq)]
enum Phase {
    #[default]
    Idle,
    Downloading,
    Handoff,
}
#[derive(Default)]
pub(super) struct UpdateState {
    receiver: Option<Receiver<Result<Option<crate::updates::Available>, String>>>,
    pub available: Option<crate::updates::Available>,
    detail: String,
    open: bool,
    task: Option<crate::updater::UpdateTask>,
    phase: Phase,
    received: u64,
    total: u64,
    error: bool,
}
impl UpdateState {
    pub(super) fn busy(&self) -> bool {
        self.phase != Phase::Idle
    }
}

impl AlphaDocs {
    pub(super) fn check_updates(&mut self, ctx: &Context, manual: bool) {
        if manual {
            self.update_state.open = true;
        }
        if self.update_state.receiver.is_some() || (!manual && !self.prefs.check_updates) {
            return;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let path = persistence::data_dir().join("update-check.json");
        let mut bytes = Vec::new();
        let previous = std::fs::File::open(&path)
            .ok()
            .and_then(|f| f.take(64).read_to_end(&mut bytes).ok())
            .and_then(|_| serde_json::from_slice::<u64>(&bytes).ok())
            .unwrap_or(0);
        if !manual && !crate::updates::due(now, previous) {
            return;
        }
        #[cfg(not(test))]
        {
            let _ = persistence::atomic_write(&path, now.to_string().as_bytes());
            let (sender, receiver) = mpsc::channel();
            let repaint = ctx.clone();
            std::thread::spawn(move || {
                let _ = sender.send(crate::updates::check());
                repaint.request_repaint();
            });
            self.update_state.receiver = Some(receiver);
            self.update_state.detail = "Checking GitHub…".into();
        }
        #[cfg(test)]
        {
            let _ = (ctx, mpsc::channel::<()>());
            self.update_state.detail = "Network checks are disabled in UI tests".into();
        }
    }
    pub(super) fn poll_updates(&mut self, ctx: &Context) {
        if let Some(receiver) = &self.update_state.receiver {
            match receiver.try_recv() {
                Ok(Ok(Some(available))) => {
                    self.update_state.detail =
                        format!("AlphaDocs {} is available", available.version);
                    self.update_state.available = Some(available);
                    self.update_state.receiver = None;
                }
                Ok(Ok(None)) => {
                    self.update_state.detail = "No newer stable release is available".into();
                    self.update_state.available = None;
                    self.update_state.receiver = None;
                }
                Ok(Err(error)) => {
                    self.update_state.detail = format!("{error}. You can keep working offline.");
                    self.update_state.receiver = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.update_state.detail = "Update check ended unexpectedly".into();
                    self.update_state.receiver = None;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(100))
                }
            }
        }
        while let Some(task) = &self.update_state.task {
            match task.receiver.try_recv() {
                Ok(crate::updater::Event::Progress { received, total }) => {
                    self.update_state.received = received;
                    self.update_state.total = total;
                    self.update_state.detail = if received >= total {
                        "Verifying the download…".into()
                    } else {
                        "Downloading from GitHub…".into()
                    };
                }
                Ok(crate::updater::Event::Ready(prepared)) => {
                    self.update_state.task = None;
                    #[cfg(not(test))]
                    let saved = self.tabs.save_all(&mut self.session);
                    #[cfg(test)]
                    let saved: Result<(), String> = Ok(());
                    if let Err(error) = saved {
                        self.update_error(format!(
                            "Your documents could not be saved. Update stopped. {error}"
                        ));
                        break;
                    }
                    self.persist_session();
                    #[cfg(not(test))]
                    let launched = prepared.launch(std::process::id(), self.palette.dark);
                    #[cfg(test)]
                    let launched: Result<
                        crate::updater::InstallHandle,
                        String,
                    > = {
                        let _ = prepared;
                        Err("Installer execution is disabled in UI tests".into())
                    };
                    match launched {
                        Ok(_handle) => {
                            self.update_state.phase = Phase::Handoff;
                            self.update_state.detail =
                                "Installing and restarting AlphaDocs…".into();
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        Err(error) => self.update_error(error),
                    }
                    break;
                }
                Ok(crate::updater::Event::Error(error)) => {
                    self.update_error(error);
                    break;
                }
                Ok(crate::updater::Event::Cancelled) => {
                    self.update_state.task = None;
                    self.update_state.phase = Phase::Idle;
                    self.update_state.detail = "Update cancelled. You can keep working.".into();
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(50));
                    break;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.update_error(
                        "The updater stopped unexpectedly. Your installed app is unchanged.".into(),
                    );
                    break;
                }
            }
        }
    }
    fn update_error(&mut self, error: String) {
        self.update_state.phase = Phase::Idle;
        self.update_state.task = None;
        self.update_state.error = true;
        self.update_state.detail = error;
    }
    fn download_update(&mut self) {
        if self.export_job.is_some() {
            self.update_error(
                "Wait for the current export to finish, then start the update again.".into(),
            );
            return;
        }
        let Some(available) = self.update_state.available.clone() else {
            return;
        };
        if available.asset.is_none() {
            self.update_error(available.install_error.unwrap_or(
                "No compatible verified installer was published for this release.".into(),
            ));
            return;
        }
        #[cfg(not(test))]
        if let Err(error) = self.tabs.save_all(&mut self.session) {
            self.update_error(format!("Save failed. Update was not started: {error}"));
            return;
        }
        self.persist_session();
        self.feature_search_open = false;
        self.dialog = None;
        self.proof_state = None;
        let executable = match std::env::current_exe() {
            Ok(path) => path,
            Err(e) => {
                self.update_error(e.to_string());
                return;
            }
        };
        self.update_state.total = available.asset.as_ref().map_or(0, |a| a.size);
        self.update_state.received = 0;
        self.update_state.error = false;
        self.update_state.detail = "Preparing download…".into();
        self.update_state.phase = Phase::Downloading;
        self.update_state.task = Some(crate::updater::start(available, executable));
    }
    pub(super) fn updating_screen(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let p = self.palette;
        if self.update_state.phase != Phase::Handoff
            && ctx.input(|i| i.viewport().close_requested())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        egui::CentralPanel::default()
            .frame(Frame::new().fill(p.bg))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(32.0);
                    ui.label(RichText::new("AlphaDocs").size(20.0).color(p.muted));
                });
            });
        self.updates_ui(&ctx);
        ctx.request_repaint_after(Duration::from_millis(50));
    }
    pub(super) fn updates_ui(&mut self, ctx: &Context) {
        if !self.update_state.open {
            return;
        }
        let (mut close, mut check, mut download, mut cancel) = (false, false, false, false);
        let p = self.palette;
        let busy = self.update_state.busy();
        let response=egui::Modal::new(Id::new("updates_window")).frame(Frame::new().fill(p.chrome).corner_radius(24).inner_margin(24).shadow(egui::epaint::Shadow{offset:[0,6],blur:24,spread:0,color:Color32::from_black_alpha(45)})).show(ctx,|ui|{
            ui.set_width(408.0);
            ui.horizontal(|ui|{
                let (r,_)=ui.allocate_exact_size(vec2(30.0,34.0),Sense::hover());icons::brand(ui,r,p.accent);
                ui.add_space(8.0);ui.label(RichText::new(if busy {"Updating AlphaDocs"}else{"AlphaDocs updates"}).size(24.0));
            });
            ui.add_space(18.0);
            ui.label(RichText::new(format!("Installed version {}",env!("CARGO_PKG_VERSION"))).size(12.0).color(p.muted));
            if let Some(available)=&self.update_state.available {ui.add_space(4.0);ui.label(RichText::new(format!("Version {}",available.version)).strong().size(18.0));}
            ui.add_space(14.0);
            ui.label(RichText::new(&self.update_state.detail).color(if self.update_state.error {if p.dark {Color32::from_rgb(255,180,171)}else{Color32::from_rgb(179,38,30)}}else{p.text}));
            if busy {
                ui.add_space(18.0);
                let progress=if self.update_state.total==0 {0.0}else{self.update_state.received as f32/self.update_state.total as f32};
                ui.add(egui::ProgressBar::new(progress.clamp(0.0,1.0)).desired_width(408.0).desired_height(6.0).fill(p.accent).corner_radius(3));
                ui.add_space(8.0);
                ui.label(RichText::new(format!("{:.1} MB of {:.1} MB",self.update_state.received as f64/1_000_000.0,self.update_state.total as f64/1_000_000.0)).size(12.0).color(p.muted));
                ui.add_space(18.0);ui.label("Your open documents are saved. Editing will resume when the updated app relaunches.");
            }else{
                if self.update_state.receiver.is_some(){ui.horizontal(|ui|{ui.spinner();ui.label("Checking GitHub…");});}
                if let Some(available)=&self.update_state.available {
                    if let Some(error)=&available.install_error {ui.add_space(8.0);ui.label(error);}
                    ui.add_space(8.0);ui.label("The update is downloaded from Alpha Workroom's GitHub release, verified, installed and restarted. Your document library stays in place. Your operating system may request installation permission.");
                    #[cfg(windows)]
                    {ui.add_space(8.0);ui.label("On Windows, updates install in your user account. If you opened a portable copy, its folder is kept and the installed app opens instead. Use AlphaDocs in the Start menu afterwards.");}
                    if ui.link("View release notes on GitHub").clicked() && let Err(e)=open::that(&available.url){self.status=format!("Could not open your browser: {e}");}
                }
                ui.add_space(12.0);ui.label(RichText::new("Update requests contain release metadata only. No document text is uploaded. Automatic daily checks can be turned off in Preferences.").size(12.0).color(p.muted));
            }
            ui.add_space(24.0);
            ui.with_layout(UiLayout::right_to_left(Align::Center),|ui|{
                if busy {
                    let r=tonal(ui,"Cancel download",p,self.update_state.phase==Phase::Downloading);tools::bounds(ui,"update_cancel",&r);cancel=r.clicked();
                }else{
                    if self.update_state.available.is_some(){let enabled=self.update_state.available.as_ref().is_some_and(|a|a.asset.is_some())&&self.update_state.receiver.is_none();let r=primary(ui,"Download and install",p,enabled);tools::bounds(ui,"update_download",&r);download=r.clicked();}
                    if self.update_state.available.is_none(){check=tonal(ui,"Check again",p,self.update_state.receiver.is_none()).clicked();}
                    close=ui.add(egui::Button::new("Close").frame(false)).clicked();
                }
            });
        });
        #[cfg(test)]
        ctx.data_mut(|d| d.insert_temp(Id::new("update_modal"), response.response.rect));
        if (response.should_close() || close) && !busy {
            self.update_state.open = false;
        }
        if cancel {
            if let Some(task) = &self.update_state.task {
                task.cancel();
            }
            self.update_state.task = None;
            self.update_state.phase = Phase::Idle;
            self.update_state.detail = "Update cancelled. You can keep working.".into();
        }
        if download {
            self.download_update();
        }
        if check {
            self.check_updates(ctx, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn busy_app(ctx: &Context, theme: Theme) -> AlphaDocs {
        let mut app = AlphaDocs::fresh(
            ctx,
            Preferences {
                theme,
                autosave: false,
                ..Default::default()
            },
            vec![],
        );
        app.home = false;
        app.session.editor.insert("Keep this document intact");
        app.update_state.phase = Phase::Downloading;
        app.update_state.open = true;
        app.update_state.detail = "Downloading from GitHub…".into();
        app.update_state.total = 100;
        app.update_state.received = 40;
        app
    }

    fn update_frame(ctx: &Context, app: &mut AlphaDocs, events: Vec<Event>, size: egui::Vec2) {
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

    fn key(key: Key, modifiers: egui::Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn pointer(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn click_test_bounds(ctx: &Context, app: &mut AlphaDocs, id: &str) {
        let rect = ctx
            .data_mut(|d| d.get_temp::<Rect>(Id::new(id)))
            .unwrap_or_else(|| panic!("missing updater control bounds {id}"));
        update_frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(rect.center()),
                pointer(rect.center(), true),
            ],
            vec2(960.0, 640.0),
        );
        update_frame(
            ctx,
            app,
            vec![pointer(rect.center(), false)],
            vec2(960.0, 640.0),
        );
        update_frame(ctx, app, vec![], vec2(960.0, 640.0));
    }

    #[test]
    fn downloading_modal_blocks_text_new_undo_escape_and_outside_dismissal() {
        let ctx = Context::default();
        fonts::install(&ctx);
        let mut app = busy_app(&ctx, Theme::Light);
        let doc = app.session.editor.doc.clone();
        let selection = (app.session.editor.anchor, app.session.editor.caret);
        update_frame(&ctx, &mut app, vec![], vec2(960.0, 640.0));
        update_frame(
            &ctx,
            &mut app,
            vec![Event::Text("must not enter".into())],
            vec2(960.0, 640.0),
        );
        update_frame(
            &ctx,
            &mut app,
            vec![key(Key::N, egui::Modifiers::COMMAND)],
            vec2(960.0, 640.0),
        );
        update_frame(
            &ctx,
            &mut app,
            vec![key(Key::Z, egui::Modifiers::COMMAND)],
            vec2(960.0, 640.0),
        );
        update_frame(
            &ctx,
            &mut app,
            vec![key(Key::Escape, egui::Modifiers::NONE)],
            vec2(960.0, 640.0),
        );
        update_frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos2(8.0, 8.0)),
                pointer(pos2(8.0, 8.0), true),
                pointer(pos2(8.0, 8.0), false),
            ],
            vec2(960.0, 640.0),
        );
        assert!(app.update_state.busy());
        assert!(app.update_state.open);
        assert_eq!(app.session.editor.doc, doc);
        assert_eq!(
            (app.session.editor.anchor, app.session.editor.caret),
            selection
        );
    }

    #[test]
    fn cancelling_download_restores_editing_without_starting_a_worker() {
        let ctx = Context::default();
        fonts::install(&ctx);
        let mut app = busy_app(&ctx, Theme::Light);
        update_frame(&ctx, &mut app, vec![], vec2(960.0, 640.0));
        update_frame(&ctx, &mut app, vec![], vec2(960.0, 640.0));
        assert!(app.update_state.task.is_none());
        click_test_bounds(&ctx, &mut app, "update_cancel");
        assert!(!app.update_state.busy());
        assert!(app.update_state.task.is_none());
        ctx.memory_mut(|m| m.request_focus(Id::new("document_canvas")));
        update_frame(
            &ctx,
            &mut app,
            vec![Event::Text(" after cancel".into())],
            vec2(960.0, 640.0),
        );
        assert!(app.session.editor.doc.text().contains("after cancel"));
    }

    #[test]
    fn download_modal_stays_within_viewport_in_both_themes() {
        for theme in [Theme::Light, Theme::Dark] {
            for busy in [true, false] {
                let ctx = Context::default();
                fonts::install(&ctx);
                let mut app = busy_app(&ctx, theme);
                if !busy {
                    app.update_state.phase = Phase::Idle;
                    app.update_state.available = Some(crate::updates::Available {
                    version: "99.0.0".into(),
                    url: crate::updates::RELEASES.into(),
                    asset: Some(crate::updates::ReleaseAsset {
                        name: "AlphaDocs-Windows-x86_64-Setup.exe".into(),
                        url: "https://github.com/AlphaWorkroom/AlphaDocs/releases/download/v99.0.0/AlphaDocs-Windows-x86_64-Setup.exe".into(),
                        size: 100,
                        sha256: "0".repeat(64),
                    }),
                    install_error: None,
                });
                    app.update_state.detail = "A new version is available.".into();
                }
                update_frame(&ctx, &mut app, vec![], vec2(960.0, 640.0));
                update_frame(&ctx, &mut app, vec![], vec2(960.0, 640.0));
                let rect = ctx
                    .data_mut(|d| d.get_temp::<Rect>(Id::new("update_modal")))
                    .expect("updater modal bounds");
                assert!(rect.left() >= 0.0 && rect.right() <= 960.0, "{rect:?}");
                assert!(rect.top() >= 0.0 && rect.bottom() <= 640.0, "{rect:?}");
            }
        }
    }

    #[test]
    fn background_notice_and_manual_window_preserve_document_selection_and_history() {
        let ctx = Context::default();
        fonts::install(&ctx);
        let mut app = AlphaDocs::fresh(
            &ctx,
            Preferences {
                autosave: false,
                ..Default::default()
            },
            vec![],
        );
        app.home = false;
        app.session
            .editor
            .insert("Keep editing while updates are checked");
        let doc = app.session.editor.doc.clone();
        let selection = (app.session.editor.anchor, app.session.editor.caret);
        let (tx, rx) = mpsc::channel();
        tx.send(Ok(Some(crate::updates::Available {
            version: "99.0.0".into(),
            url: crate::updates::RELEASES.into(),
            asset: None,
            install_error: Some("Test release has no installer".into()),
        })))
        .unwrap();
        app.update_state.receiver = Some(rx);
        app.poll_updates(&ctx);
        assert!(app.update_state.available.is_some());
        assert!(!app.update_state.open);
        app.execute(Action::CheckUpdates, &ctx);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0))),
                ..Default::default()
            },
            |ui| app.update_ui(ui),
        );
        output.textures_delta.clear();
        assert!(app.update_state.open);
        assert_eq!(app.session.editor.doc, doc);
        assert_eq!(
            (app.session.editor.anchor, app.session.editor.caret),
            selection
        );
        app.session.editor.undo();
        assert_eq!(app.session.editor.doc.text(), "");
        assert!(
            serde_json::from_str::<Preferences>("{}")
                .unwrap()
                .check_updates
        );
    }
}
