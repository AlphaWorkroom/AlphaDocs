use super::*;
use crate::compat::export::{Format, Report};
use std::sync::mpsc::{self, Receiver};
pub(super) struct ExportJob {
    receiver: Receiver<Result<Report, String>>,
    format: Format,
    path: PathBuf,
    pub(super) close_requested: bool,
}

impl AlphaDocs {
    pub(super) fn export_format(&mut self, format: Format, ctx: &Context) {
        if self.export_job.is_some() {
            self.status = "An export is already in progress".into();
            return;
        }
        let Some(mut path) = rfd::FileDialog::new()
            .set_file_name(format!(
                "{}.{}",
                safe_name(&self.session.editor.doc.title),
                format.extension()
            ))
            .add_filter(format.label(), &[format.extension()])
            .save_file()
        else {
            return;
        };
        if format == Format::Native || path.extension().is_none() {
            path.set_extension(format.extension());
        }
        let document = self.session.editor.doc.clone();
        let destination = path.clone();
        let (sender, receiver) = mpsc::channel();
        let repaint = ctx.clone();
        std::thread::spawn(move || {
            let _ = sender.send(compat::export::write(&document, format, &destination));
            repaint.request_repaint();
        });
        self.status = format!("Exporting {}…", format.label());
        self.export_job = Some(ExportJob {
            receiver,
            format,
            path,
            close_requested: false,
        });
    }
    pub(super) fn poll_export(&mut self, ctx: &Context) {
        let Some(job) = self.export_job.take() else {
            return;
        };
        match job.receiver.try_recv() {
            Ok(Ok(report)) => {
                self.status = format!("Exported {}", job.path.display());
                if !report.notices.is_empty() {
                    self.message = format!(
                        "{} exported\n\n{}\n\n{}",
                        job.format.label(),
                        job.path.display(),
                        report.notices.join("\n\n")
                    );
                    self.dialog = Some(Dialog::ExportResult);
                }
                if job.close_requested {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Ok(Err(error)) => {
                self.message =
                    format!("Export failed\n\n{error}\n\nExisting destination files were kept.");
                self.status = "Export failed".into();
                self.dialog = Some(Dialog::ExportResult);
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.export_job = Some(job);
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.status = "Export stopped unexpectedly; the document is unchanged".into();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completion_reports_real_success_and_errors_without_mutating_document() {
        for success in [true, false] {
            let ctx = Context::default();
            fonts::install(&ctx);
            let mut app = AlphaDocs::fresh(&ctx, Preferences::default(), vec![]);
            app.home = false;
            app.session.editor.insert("Keep this document");
            let original = app.session.editor.doc.clone();
            let (sender, receiver) = mpsc::channel();
            sender
                .send(if success {
                    Ok(Report {
                        notices: vec!["Plain text keeps text only".into()],
                    })
                } else {
                    Err("Destination unavailable".into())
                })
                .unwrap();
            app.export_job = Some(ExportJob {
                receiver,
                format: Format::Txt,
                path: PathBuf::from("example.txt"),
                close_requested: false,
            });
            for _ in 0..2 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0))),
                        ..Default::default()
                    },
                    |ui| app.update_ui(ui),
                );
                output.textures_delta.clear();
            }
            assert!(app.export_job.is_none());
            assert!(matches!(app.dialog, Some(Dialog::ExportResult)));
            assert_eq!(app.session.editor.doc, original);
            assert!(app.message.contains(if success {
                "exported"
            } else {
                "Destination unavailable"
            }));
        }
    }
}
