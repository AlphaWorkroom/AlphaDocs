use super::*;
use std::{
    io::Read,
    sync::mpsc::{self, Receiver},
};
#[derive(Default)]
pub(super) struct UpdateState {
    receiver: Option<Receiver<Result<Option<crate::updates::Available>, String>>>,
    pub available: Option<crate::updates::Available>,
    detail: String,
    open: bool,
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
        let Some(receiver) = &self.update_state.receiver else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(Some(available))) => {
                self.update_state.detail = format!("AlphaDocs {} is available", available.version);
                self.update_state.available = Some(available);
                self.update_state.receiver = None;
            }
            Ok(Ok(None)) => {
                self.update_state.detail = "No newer stable release is available".into();
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
            Err(mpsc::TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(100)),
        }
    }
    pub(super) fn updates_ui(&mut self, ctx: &Context) {
        if !self.update_state.open {
            return;
        }
        let mut open = true;
        let mut check = false;
        egui::Window::new("AlphaDocs updates").id(Id::new("updates_window")).open(&mut open).collapsible(false).resizable(false).default_width(410.0).show(ctx, |ui| {
            ui.label(format!("Installed version {}", env!("CARGO_PKG_VERSION")));
            ui.add_space(8.0);
            ui.label(&self.update_state.detail);
            if let Some(available) = &self.update_state.available {
                ui.add_space(12.0);
                if tonal(ui, "Download update from GitHub", self.palette, true).clicked() {
                    ctx.open_url(egui::OpenUrl::new_tab(available.url.clone()));
                }
                ui.label("Close AlphaDocs before running the downloaded installer. Your documents stay in their existing data folder.");
            }
            ui.add_space(12.0);
            check = ui.add_enabled(self.update_state.receiver.is_none(), egui::Button::new("Check again")).clicked();
            ui.label(RichText::new("Checks request public release metadata only. No document content is sent. Turn automatic daily checks off in Preferences.").size(12.0).color(self.palette.muted));
        });
        self.update_state.open = open;
        if check {
            self.check_updates(ctx, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
