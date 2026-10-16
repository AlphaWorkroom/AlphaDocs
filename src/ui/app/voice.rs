use super::*;
use crate::speech::{
    self, Engine, InstallError, InstallPhase, InstallProgress, InstalledRuntime, SpeechAnchor,
    SpeechTask, WorkerEvent, WorkerEventKind,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
};

enum InstallEvent {
    Progress(InstallProgress),
    Finished(Result<InstalledRuntime, InstallError>),
}
struct Installation {
    receiver: Receiver<InstallEvent>,
    cancelled: Arc<AtomicBool>,
}
impl Drop for Installation {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}
#[derive(Default, PartialEq)]
enum Phase {
    #[default]
    Idle,
    Starting,
    Listening,
    Stopping,
}

trait VoiceTask {
    fn start(&mut self) -> Result<(), String>;
    fn stop(&mut self) -> Result<(), String>;
    fn cancel(&mut self) -> Result<(), String>;
    fn poll_event(&self) -> Result<Option<WorkerEvent>, String>;
}

struct SpeechTaskAdapter(SpeechTask);

impl VoiceTask for SpeechTaskAdapter {
    fn start(&mut self) -> Result<(), String> {
        self.0.start()
    }
    fn stop(&mut self) -> Result<(), String> {
        self.0.stop()
    }
    fn cancel(&mut self) -> Result<(), String> {
        self.0.cancel()
    }
    fn poll_event(&self) -> Result<Option<WorkerEvent>, String> {
        self.0.poll_event()
    }
}

pub(super) struct VoiceUi {
    pub open: bool,
    engine: Engine,
    runtime: Option<InstalledRuntime>,
    checked_engine: Option<Engine>,
    installation: Option<Installation>,
    progress: Option<InstallProgress>,
    task: Option<Box<dyn VoiceTask>>,
    anchor: Option<SpeechAnchor>,
    phase: Phase,
    transcript: String,
    level: f32,
    message: String,
    error: String,
    next_session: u64,
    startup_started: Option<Instant>,
}
impl Default for VoiceUi {
    fn default() -> Self {
        Self {
            open: false,
            engine: Engine::VoskSmallEnUs,
            runtime: None,
            checked_engine: None,
            installation: None,
            progress: None,
            task: None,
            anchor: None,
            phase: Phase::Idle,
            transcript: String::new(),
            level: 0.0,
            message: String::new(),
            error: String::new(),
            next_session: 0,
            startup_started: None,
        }
    }
}
impl VoiceUi {
    pub(super) fn cancel(&mut self) {
        if let Some(task) = &mut self.task {
            let _ = task.cancel();
        }
        self.task = None;
        self.anchor = None;
        self.phase = Phase::Idle;
        self.startup_started = None;
        self.level = 0.0;
    }
    fn install(&mut self, ctx: &Context) {
        let (tx, receiver) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancel = cancelled.clone();
        let root = persistence::data_dir().join("speech");
        let engine = self.engine;
        let ctx = ctx.clone();
        self.error.clear();
        self.progress = None;
        self.installation = Some(Installation {
            receiver,
            cancelled,
        });
        std::thread::spawn(move || {
            let result = speech::install_runtime(
                &root,
                engine,
                |progress| {
                    let _ = tx.send(InstallEvent::Progress(progress));
                    ctx.request_repaint();
                },
                || cancel.load(Ordering::Relaxed),
            );
            let _ = tx.send(InstallEvent::Finished(result));
            ctx.request_repaint();
        });
    }
}
impl AlphaDocs {
    pub(super) fn poll_voice(&mut self, ctx: &Context) {
        if (!self.voice.open || self.home || self.dialog.is_some() || self.signing.open)
            && self.voice.task.is_some()
        {
            self.voice.cancel();
            self.voice.message = "Voice typing stopped. The preview was discarded.".into();
        }
        if self
            .voice
            .anchor
            .as_ref()
            .is_some_and(|a| !a.still_matches(&self.session.editor, self.session.viewing))
        {
            self.voice.cancel();
            self.voice.message =
                "The document or insertion point changed. Start again at the new caret.".into();
            self.voice.transcript.clear();
        }
        if self.voice.phase == Phase::Starting
            && self.voice.startup_started.is_some_and(|started| {
                Instant::now().saturating_duration_since(started) > Duration::from_secs(30)
            })
        {
            self.voice.error = "The offline speech worker did not become ready in time.".into();
            self.voice.cancel();
        }
        loop {
            let event = match self
                .voice
                .installation
                .as_ref()
                .map(|i| i.receiver.try_recv())
            {
                Some(Ok(event)) => Some(event),
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    self.voice.installation = None;
                    self.voice.progress = None;
                    self.voice.error = "The offline voice installer stopped unexpectedly.".into();
                    break;
                }
                Some(Err(mpsc::TryRecvError::Empty)) | None => None,
            };
            let Some(event) = event else {
                break;
            };
            match event {
                InstallEvent::Progress(progress) => self.voice.progress = Some(progress),
                InstallEvent::Finished(result) => {
                    self.voice.installation = None;
                    self.voice.progress = None;
                    match result {
                        Ok(runtime) => {
                            self.voice.runtime = Some(runtime);
                            self.voice.message =
                                "Ready. Click Start when you want to use the microphone.".into();
                        }
                        Err(InstallError::Cancelled) => {
                            self.voice.message = "Download cancelled.".into()
                        }
                        Err(InstallError::Failed(e)) => self.voice.error = e,
                    }
                }
            }
        }
        for _ in 0..64 {
            let Some(task) = &self.voice.task else {
                break;
            };
            match task.poll_event() {
                Ok(Some(event)) => match event.event {
                    WorkerEventKind::Ready => {
                        if self.voice.phase == Phase::Starting {
                            if let Some(task) = &mut self.voice.task
                                && let Err(e) = task.start()
                            {
                                self.voice.error = e;
                                self.voice.cancel();
                            }
                        } else {
                            self.voice.cancel();
                        }
                    }
                    WorkerEventKind::Listening => self.voice.phase = Phase::Listening,
                    WorkerEventKind::Partial { transcript, level } => {
                        self.voice.transcript = transcript;
                        self.voice.level = if level.is_finite() {
                            level.clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                    }
                    WorkerEventKind::Stopped { transcript } => {
                        if let Some(anchor) = self.voice.anchor.take() {
                            if transcript.trim().is_empty() {
                                self.voice.message="No speech was recognised. Try again closer to your microphone.".into();
                            } else {
                                match anchor.commit(&mut self.session.editor,self.session.viewing,&transcript){Ok(())=>{self.voice.message="Transcript inserted. Undo restores the previous text.".into();self.refocus=true;self.scroll_caret=true;},Err(_)=>self.voice.error="The insertion point changed. The transcript was not inserted.".into()}
                            }
                        }
                        self.voice.transcript = transcript;
                        self.voice.cancel();
                    }
                    WorkerEventKind::Cancelled => self.voice.cancel(),
                    WorkerEventKind::Error { message } => {
                        self.voice.error = message;
                        self.voice.cancel();
                    }
                },
                Ok(None) => break,
                Err(error) => {
                    self.voice.error = error;
                    self.voice.cancel();
                    break;
                }
            }
        }
        if self.voice.task.is_some() || self.voice.installation.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
    pub(super) fn voice_window(&mut self, ctx: &Context) {
        if !self.voice.open {
            return;
        }
        let mut state = std::mem::take(&mut self.voice);
        let p = self.palette;
        let mut open = true;
        egui::Window::new("Voice typing").id(Id::new("native_voice_typing")).open(&mut open).collapsible(false).resizable(false).default_width(336.0)
            .frame(Frame::new().fill(p.chrome).corner_radius(24).inner_margin(24).shadow(egui::epaint::Shadow{offset:[0,6],blur:24,spread:0,color:Color32::from_black_alpha(45)}))
            .show(ctx,|ui| {
                ui.set_width(336.0);
                let busy=state.phase!=Phase::Idle;
                ui.label(RichText::new("Offline dictation").strong());
                if state.checked_engine!=Some(state.engine) {
                    state.runtime=None;state.error.clear();state.message.clear();state.checked_engine=Some(state.engine);
                    match speech::detect_installed_runtime(&persistence::data_dir().join("speech"),state.engine){Ok(runtime)=>state.runtime=runtime,Err(e)=>state.error=e}
                }
                ui.add_space(8.0);
                ui.label(RichText::new("English (United States)").size(12.0).color(p.muted));
                ui.add_space(18.0);
                if state.runtime.is_some() {
                    ui.vertical_centered(|ui| {
                        let (rect,response)=ui.allocate_exact_size(vec2(80.0,80.0),Sense::click());
                        tools::bounds(ui,"voice_microphone",&response);
                        let enabled=busy || (!self.home && !self.session.viewing && self.session.editor.table_selection().is_none());
                        ui.painter().circle_filled(rect.center(),40.0,if busy{p.selection}else{p.toolbar});
                        if busy {ui.painter().rect_filled(Rect::from_center_size(rect.center(),vec2(22.0,22.0)),4,p.accent);}
                        else {icons::draw(ui,rect.shrink(24.0),Icon::Microphone,if enabled{p.accent}else{p.muted});}
                        response.widget_info(||egui::WidgetInfo::labeled(egui::WidgetType::Button,enabled,if busy{"Stop and insert"}else{"Start microphone"}));
                        let clicked=response.clicked() && enabled;
                        ui.add_space(8.0);
                        ui.label(RichText::new(match state.phase{Phase::Idle=>"Ready to listen",Phase::Starting=>"Loading speech engine…",Phase::Listening=>"Listening…",Phase::Stopping=>"Finishing transcript…"}).strong().color(p.accent));
                        ui.add_space(8.0);
                        ui.add(egui::ProgressBar::new(state.level).desired_width(150.0).desired_height(4.0).fill(p.accent));
                        ui.add_space(12.0);
                        let can_activate=enabled && matches!(state.phase,Phase::Idle|Phase::Listening);
                        let r=primary(ui,if busy{"Stop and insert"}else{"Start"},p,can_activate);
                        tools::bounds(ui,"voice_start_stop",&r);
                        if (clicked && can_activate) || r.clicked() {
                            if busy {if state.phase!=Phase::Stopping&& let Some(task)=&mut state.task{match task.stop(){Ok(())=>state.phase=Phase::Stopping,Err(e)=>{state.error=e;state.cancel();}}}}
                            else {
                                state.next_session=state.next_session.wrapping_add(1);
                                match SpeechAnchor::capture(state.next_session,&self.session.editor,self.session.viewing) {
                                    Ok(anchor)=>match SpeechTask::spawn(state.runtime.as_ref().expect("runtime checked"),state.next_session){Ok(task)=>{state.anchor=Some(anchor);state.task=Some(Box::new(SpeechTaskAdapter(task)));state.phase=Phase::Starting;state.startup_started=Some(Instant::now());state.transcript.clear();state.error.clear();state.message.clear();},Err(e)=>state.error=e},
                                    Err(_)=>state.error="Choose an editable insertion point in the document first.".into(),
                                }
                            }
                        }
                        if busy && ui.button("Cancel and discard").clicked(){state.cancel();state.transcript.clear();state.message="Preview discarded.".into();}
                    });
                    ui.add_space(16.0);
                    if !state.transcript.is_empty(){Frame::new().fill(p.toolbar).corner_radius(14).inner_margin(14).show(ui,|ui|{ui.label(RichText::new(if busy{"Transcript preview"}else{"Last transcript"}).size(12.0).color(p.muted));egui::ScrollArea::vertical().max_height(130.0).show(ui,|ui|{ui.label(&state.transcript);});});}
                    ui.label(RichText::new("Speech stays on this device. Stop and insert adds the transcript at your original caret in one undoable edit. Moving the caret or editing discards a running preview.").size(12.0).color(p.muted));
                } else if let Some(installation)=&state.installation {
                    ui.label(RichText::new("Setting up offline voice typing").strong());
                    if let Some(progress)=state.progress {
                        ui.label(match progress.phase {InstallPhase::DownloadingRuntime=>"Downloading offline speech runtime…",InstallPhase::ExtractingModel=>"Preparing the offline language model…",InstallPhase::DownloadingModel=>"Downloading language model…",InstallPhase::Ready=>"Ready"});
                        if let Some(total)=progress.total_bytes.filter(|n|*n>0){ui.add(egui::ProgressBar::new(progress.completed_bytes as f32/total as f32).show_percentage());ui.label(RichText::new(format!("{:.1} / {:.1} MB",progress.completed_bytes as f64/1e6,total as f64/1e6)).size(12.0).color(p.muted));}else{ui.spinner();}
                    }else{ui.spinner();}
                    if ui.button("Cancel download").clicked(){installation.cancelled.store(true,Ordering::Relaxed);}
                } else {
                    ui.label(RichText::new("Download once. Dictate offline.").size(18.0));ui.add_space(8.0);
                    ui.label(format!("First-time setup downloads {:.1} MB: the compact English model and its recognition runtime. After setup, dictation works offline.",(speech::DEFAULT_MODEL_ARCHIVE_BYTES+speech::runtime_download_bytes()) as f64/1e6));
                    ui.add_space(8.0);
                    ui.label(RichText::new("This download contacts the model/runtime providers. Your document and microphone audio are not uploaded. Nothing is downloaded until you choose Install.").size(12.0).color(p.muted));
                    ui.add_space(16.0);
                    let r=primary(ui,"Install offline voice typing",p,true);tools::bounds(ui,"voice_install",&r);
                    if r.clicked(){state.install(ctx);}
                }
                if !state.error.is_empty(){ui.add_space(12.0);ui.colored_label(if p.dark{Color32::from_rgb(255,180,171)}else{Color32::from_rgb(179,38,30)},&state.error);}
                if !state.message.is_empty(){ui.add_space(12.0);ui.label(RichText::new(&state.message).color(p.accent));}
            });
        if !open {
            state.open = false;
            state.cancel();
            state.installation = None;
        }
        self.voice = state;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::SPEECH_PROTOCOL_VERSION;
    use std::{collections::VecDeque, sync::atomic::AtomicUsize};

    struct QueueTask {
        events: std::sync::Mutex<VecDeque<Result<WorkerEvent, String>>>,
        starts: Arc<AtomicUsize>,
        stops: Arc<AtomicUsize>,
        cancels: Arc<AtomicUsize>,
    }

    impl VoiceTask for QueueTask {
        fn start(&mut self) -> Result<(), String> {
            self.starts.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn stop(&mut self) -> Result<(), String> {
            self.stops.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn cancel(&mut self) -> Result<(), String> {
            self.cancels.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn poll_event(&self) -> Result<Option<WorkerEvent>, String> {
            self.events.lock().unwrap().pop_front().transpose()
        }
    }

    fn fake_task(
        events: impl IntoIterator<Item = (Option<u64>, WorkerEventKind)>,
    ) -> (
        Box<dyn VoiceTask>,
        Arc<AtomicUsize>,
        Arc<AtomicUsize>,
        Arc<AtomicUsize>,
    ) {
        let starts = Arc::new(AtomicUsize::new(0));
        let stops = Arc::new(AtomicUsize::new(0));
        let cancels = Arc::new(AtomicUsize::new(0));
        let events = events
            .into_iter()
            .map(|(session_id, event)| {
                Ok(WorkerEvent {
                    protocol: SPEECH_PROTOCOL_VERSION,
                    session_id,
                    event,
                })
            })
            .collect();
        (
            Box::new(QueueTask {
                events: std::sync::Mutex::new(events),
                starts: starts.clone(),
                stops: stops.clone(),
                cancels: cancels.clone(),
            }),
            starts,
            stops,
            cancels,
        )
    }

    fn app() -> (Context, AlphaDocs) {
        let ctx = Context::default();
        fonts::install(&ctx);
        let mut app = AlphaDocs::fresh(&ctx, Preferences::default(), vec![]);
        app.home = false;
        let mut doc = app.session.editor.doc.clone();
        doc.blocks[0] = Block::Paragraph(Paragraph::from_text("original"));
        app.session.editor = crate::engine::Editor::new(doc);
        app.session.editor.place(
            Position {
                address: app.session.editor.caret.address,
                offset: 8,
            },
            false,
        );
        (ctx, app)
    }

    fn frame(ctx: &Context, app: &mut AlphaDocs, events: Vec<Event>) {
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(960.0, 640.0))),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(raw, |ui| {
            app.poll_voice(ui.ctx());
            app.voice_window(ui.ctx());
        });
        output.textures_delta.clear();
    }

    fn click(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        }
    }

    fn dummy_runtime(engine: Engine) -> InstalledRuntime {
        InstalledRuntime {
            worker_path: "unused-test-worker".into(),
            args: vec![],
            engine,
        }
    }

    #[test]
    fn partial_transcript_is_preview_only_and_stop_commits_once_for_undo() {
        let (ctx, mut app) = app();
        let original = app.session.editor.doc.clone();
        let anchor = SpeechAnchor::capture(7, &app.session.editor, false).unwrap();
        let (task, _, _, _) = fake_task([(
            Some(7),
            WorkerEventKind::Partial {
                transcript: " preview".into(),
                level: 0.4,
            },
        )]);
        app.voice.anchor = Some(anchor);
        app.voice.task = Some(task);
        app.voice.phase = Phase::Listening;
        app.voice.open = true;
        app.voice.runtime = Some(dummy_runtime(app.voice.engine));
        app.voice.checked_engine = Some(app.voice.engine);
        frame(&ctx, &mut app, vec![]);
        assert_eq!(app.session.editor.doc, original);
        assert_eq!(app.voice.transcript, " preview");

        let (task, _, _, _) = fake_task([(
            Some(7),
            WorkerEventKind::Stopped {
                transcript: " final".into(),
            },
        )]);
        app.voice.anchor = Some(SpeechAnchor::capture(7, &app.session.editor, false).unwrap());
        app.voice.task = Some(task);
        app.voice.phase = Phase::Stopping;
        frame(&ctx, &mut app, vec![]);
        assert_eq!(app.session.editor.doc.text(), "original final");
        assert!(app.session.editor.can_undo());
        app.session.editor.undo();
        assert_eq!(app.session.editor.doc, original);
    }

    #[test]
    fn ready_after_stop_never_starts_and_stale_anchor_cancels_preview() {
        let (ctx, mut app) = app();
        let (task, starts, _, _) = fake_task([(None, WorkerEventKind::Ready)]);
        app.voice.anchor = Some(SpeechAnchor::capture(9, &app.session.editor, false).unwrap());
        app.voice.task = Some(task);
        app.voice.phase = Phase::Stopping;
        app.voice.open = true;
        app.voice.runtime = Some(dummy_runtime(app.voice.engine));
        app.voice.checked_engine = Some(app.voice.engine);
        frame(&ctx, &mut app, vec![]);
        assert_eq!(starts.load(Ordering::SeqCst), 0);
        assert!(app.voice.task.is_none());

        let (task, _, _, cancels) = fake_task([]);
        app.voice.task = Some(task);
        app.voice.anchor = Some(SpeechAnchor::capture(10, &app.session.editor, false).unwrap());
        app.voice.phase = Phase::Listening;
        app.voice.transcript = "discard me".into();
        app.session.editor.insert(" changed");
        app.voice.open = true;
        app.voice.runtime = Some(dummy_runtime(app.voice.engine));
        app.voice.checked_engine = Some(app.voice.engine);
        frame(&ctx, &mut app, vec![]);
        assert!(app.voice.task.is_none());
        assert_eq!(app.voice.transcript, "");
        assert!(cancels.load(Ordering::SeqCst) > 0);
    }

    #[test]
    fn startup_timeout_cancels_a_late_ready_and_disconnected_installer_fails_cleanly() {
        let (ctx, mut app) = app();
        let (task, starts, _, _) = fake_task([(None, WorkerEventKind::Ready)]);
        app.voice.anchor = Some(SpeechAnchor::capture(19, &app.session.editor, false).unwrap());
        app.voice.task = Some(task);
        app.voice.phase = Phase::Starting;
        app.voice.startup_started = Some(Instant::now() - Duration::from_secs(31));
        app.voice.open = true;
        app.voice.runtime = Some(dummy_runtime(app.voice.engine));
        app.voice.checked_engine = Some(app.voice.engine);
        frame(&ctx, &mut app, vec![]);
        assert!(app.voice.task.is_none());
        assert_eq!(starts.load(Ordering::SeqCst), 0);
        assert!(app.voice.error.contains("did not become ready"));

        let (sender, receiver) = mpsc::channel();
        drop(sender);
        app.voice.installation = Some(Installation {
            receiver,
            cancelled: Arc::new(AtomicBool::new(false)),
        });
        app.voice.runtime = None;
        frame(&ctx, &mut app, vec![]);
        assert!(app.voice.installation.is_none());
        assert!(app.voice.error.contains("stopped unexpectedly"));
    }

    #[test]
    fn selection_tab_and_read_only_changes_cancel_the_captured_session() {
        for change in 0..3 {
            let (ctx, mut app) = app();
            let (task, _, _, cancels) = fake_task([]);
            app.voice.anchor =
                Some(SpeechAnchor::capture(20 + change, &app.session.editor, false).unwrap());
            app.voice.task = Some(task);
            app.voice.phase = Phase::Listening;
            app.voice.open = true;
            app.voice.runtime = Some(dummy_runtime(app.voice.engine));
            app.voice.checked_engine = Some(app.voice.engine);
            match change {
                0 => app.session.editor.place(
                    Position {
                        address: app.session.editor.caret.address,
                        offset: 0,
                    },
                    false,
                ),
                1 => app.session.editor.add_tab(),
                _ => app.session.viewing = true,
            }
            frame(&ctx, &mut app, vec![]);
            assert!(app.voice.task.is_none(), "change kind {change}");
            assert_eq!(cancels.load(Ordering::SeqCst), 1, "change kind {change}");
            assert!(app.voice.transcript.is_empty());
        }
    }

    #[test]
    fn closing_voice_window_cancels_and_opening_never_installs_automatically() {
        let (ctx, mut app) = app();
        let (task, starts, _, cancels) = fake_task([(None, WorkerEventKind::Ready)]);
        app.voice.task = Some(task);
        app.voice.anchor = Some(SpeechAnchor::capture(11, &app.session.editor, false).unwrap());
        app.voice.phase = Phase::Listening;
        app.voice.open = false;
        frame(&ctx, &mut app, vec![]);
        assert!(app.voice.task.is_none());
        assert_eq!(cancels.load(Ordering::SeqCst), 1);
        assert_eq!(starts.load(Ordering::SeqCst), 0);

        app.voice.open = true;
        app.voice.checked_engine = Some(app.voice.engine);
        app.voice.runtime = None;
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        assert!(app.voice.installation.is_none());
        assert!(
            ctx.data(|data| data.get_temp::<Rect>(Id::new("voice_install")))
                .is_some()
        );
    }

    #[test]
    fn stop_button_sends_stop_without_committing_until_final_event() {
        let (ctx, mut app) = app();
        let (task, _, stops, _) = fake_task([]);
        app.voice.task = Some(task);
        app.voice.anchor = Some(SpeechAnchor::capture(12, &app.session.editor, false).unwrap());
        app.voice.phase = Phase::Listening;
        app.voice.runtime = Some(dummy_runtime(app.voice.engine));
        app.voice.checked_engine = Some(app.voice.engine);
        app.voice.open = true;
        let original = app.session.editor.doc.clone();
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        let rect = ctx
            .data(|data| data.get_temp::<Rect>(Id::new("voice_start_stop")))
            .unwrap();
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(rect.center()),
                click(rect.center(), true),
            ],
        );
        frame(&ctx, &mut app, vec![click(rect.center(), false)]);
        assert_eq!(stops.load(Ordering::SeqCst), 1);
        assert!(app.voice.phase == Phase::Stopping);
        assert_eq!(app.session.editor.doc, original);
    }
}
