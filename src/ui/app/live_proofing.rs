//! Debounced offline proofing. A single worker is in flight; stale results are
//! discarded and never used to correct a newer document revision.
use super::*;
use crate::{engine::Editor, proofing::Issue};
use std::{
    collections::HashSet,
    sync::{
        Arc,
        mpsc::{self, Receiver},
    },
};

#[derive(Clone, PartialEq, Eq)]
struct Source(String, String, u64);
fn source(editor: &Editor) -> Source {
    Source(
        editor.doc.id.clone(),
        editor.doc.tabs[editor.doc.active_tab].id.clone(),
        editor.revision,
    )
}
type Pending = (Source, Receiver<Result<Vec<Issue>, String>>);
#[derive(Default)]
pub(super) struct LiveProof {
    source: Option<Source>,
    pending: Option<Pending>,
    due: Option<Instant>,
    issues: Arc<Vec<Issue>>,
    dictionary: Vec<String>,
    ignored: HashSet<(String, String)>,
    pub context: Option<Issue>,
}
impl LiveProof {
    pub fn current(&self, editor: &Editor) -> Arc<Vec<Issue>> {
        if self.source.as_ref() == Some(&source(editor)) {
            self.issues.clone()
        } else {
            Arc::default()
        }
    }
}
impl AlphaDocs {
    pub(super) fn poll_live_proof(&mut self, ctx: &Context) {
        if self.home {
            return;
        }
        let current = source(&self.session.editor);
        let state = &mut self.live_proof;
        if state.source.as_ref() != Some(&current)
            || state.dictionary != self.prefs.personal_dictionary
        {
            state.source = Some(current.clone());
            state.dictionary.clone_from(&self.prefs.personal_dictionary);
            state.issues = Arc::default();
            state.context = None;
            state.due = Some(Instant::now() + Duration::from_millis(350));
        }
        if let Some((from, receiver)) = &state.pending {
            match receiver.try_recv() {
                Ok(result) => {
                    if from == &current && state.due.is_none() {
                        match result {
                            Ok(mut issues) => {
                                issues.retain(|i| {
                                    !state
                                        .ignored
                                        .contains(&(current.0.clone(), i.original.to_lowercase()))
                                });
                                state.issues = Arc::new(issues);
                            }
                            Err(error) => {
                                self.status = format!("Automatic proofing unavailable: {error}")
                            }
                        }
                    }
                    state.pending = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    state.pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if state.pending.is_none() && state.due.is_some_and(|due| Instant::now() >= due) {
            let document = self.session.editor.doc.clone();
            let dictionary = state.dictionary.clone();
            let (tx, rx) = mpsc::channel();
            let repaint = ctx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(crate::proofing::markers(&document, &dictionary));
                repaint.request_repaint();
            });
            state.pending = Some((current, rx));
            state.due = None;
        }
        if state.due.is_some() || state.pending.is_some() {
            ctx.request_repaint_after(Duration::from_millis(60));
        }
    }
}
pub(super) fn context_menu(
    ui: &mut Ui,
    state: &mut LiveProof,
    editor: &mut Editor,
    prefs: &mut Preferences,
    status: &mut String,
    viewing: bool,
) -> (bool, bool) {
    let mut corrected = false;
    let Some(issue) = state.context.clone() else {
        return (false, false);
    };
    let enabled = !viewing;
    ui.label(
        RichText::new(if issue.grammar {
            issue.message
        } else {
            "Did you mean:"
        })
        .italics(),
    );
    if issue.suggestions.is_empty() {
        ui.label(RichText::new("No dictionary suggestion").color(ui.visuals().weak_text_color()));
    }
    for suggestion in &issue.suggestions {
        if menu(ui, suggestion, "", enabled) {
            if crate::proofing::replace(editor, &issue, suggestion) {
                corrected = true;
            }
            state.context = None;
        }
    }
    ui.separator();
    if menu(ui, "Ignore all", "", true) {
        let word = issue.original.to_lowercase();
        state.ignored.insert((editor.doc.id.clone(), word.clone()));
        Arc::make_mut(&mut state.issues).retain(|i| i.original.to_lowercase() != word);
        state.context = None;
    }
    if !issue.grammar && menu(ui, "Add to personal dictionary", "", enabled) {
        prefs.personal_dictionary.push(issue.original.clone());
        if let Err(error) = persistence::save_preferences(prefs) {
            *status = error;
        }
        state.context = None;
    }
    let open = menu(ui, "Spelling and grammar check", "Ctrl+Alt+X", true);
    ui.separator();
    (corrected, open)
}

pub(super) fn paint(
    ui: &Ui,
    issues: &[Issue],
    line: &layout::Line,
    display: &Arc<egui::Galley>,
    pos: Pos2,
    height: f32,
) {
    for issue in issues
        .iter()
        .filter(|i| i.address == line.address && i.end > line.start && i.start < line.end)
    {
        let start = pos.x + layout::display_x(display, line.start, issue.start.max(line.start));
        let end = pos.x + layout::display_x(display, line.start, issue.end.min(line.end));
        let y = pos.y + height - 1.0;
        let points: Vec<_> = (0..=((end - start).max(0.0) / 2.0).ceil() as usize)
            .map(|n| {
                pos2(
                    (start + n as f32 * 2.0).min(end),
                    y + if n % 2 == 0 { 0.0 } else { 1.5 },
                )
            })
            .collect();
        ui.painter().add(egui::Shape::line(
            points,
            Stroke::new(
                1.0,
                if issue.grammar {
                    Color32::from_rgb(66, 133, 244)
                } else {
                    Color32::from_rgb(225, 67, 67)
                },
            ),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(ctx: &Context, app: &mut AlphaDocs, events: Vec<Event>) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0))),
                events,
                ..Default::default()
            },
            |ui| app.update_ui(ui),
        );
        output.textures_delta.clear();
        output
    }
    fn pointer(pos: Pos2, down: bool, button: egui::PointerButton) -> Event {
        Event::PointerButton {
            pos,
            pressed: down,
            button,
            modifiers: Default::default(),
        }
    }
    #[test]
    fn automatic_markers_context_correction_and_undo_use_real_frames() {
        let ctx = Context::default();
        fonts::install(&ctx);
        let prefs = Preferences {
            outline: false,
            ruler: false,
            autosave: false,
            ..Default::default()
        };
        let mut app = AlphaDocs::fresh(&ctx, prefs, vec![]);
        app.home = false;
        app.session.editor.doc.blocks = vec![Block::Paragraph(Paragraph::from_text(
            "Hello wurld. It are a apple.",
        ))];
        let original = app.session.editor.doc.clone();
        frame(&ctx, &mut app, vec![]);
        app.live_proof.due = Some(Instant::now());
        let deadline = Instant::now() + Duration::from_secs(10);
        while app.live_proof.issues.is_empty() {
            frame(&ctx, &mut app, vec![]);
            assert!(
                Instant::now() < deadline,
                "Background proofing must complete"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let output = frame(&ctx, &mut app, vec![]);
        assert!(output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Path(p) if p.stroke.color == egui::epaint::ColorMode::Solid(Color32::from_rgb(225,67,67)))));
        let layout = app.layout.as_ref().unwrap();
        let line = &layout.lines[0];
        let area = ctx.read_response(Id::new("document_canvas")).unwrap().rect;
        let scale = 4.0 / 3.0;
        let origin = pos2(
            area.left() + (area.width() - layout.size.x * scale) * 0.5,
            area.top() + 12.0,
        );
        let point = origin
            + line.rect.min.to_vec2() * scale
            + vec2(line.x(8) * scale, line.rect.height() * scale * 0.5);
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(point),
                pointer(point, true, egui::PointerButton::Secondary),
                pointer(point, false, egui::PointerButton::Secondary),
            ],
        );
        frame(&ctx, &mut app, vec![]);
        assert_eq!(app.live_proof.context.as_ref().unwrap().original, "wurld");
        let suggestion = ctx
            .data_mut(|d| d.get_temp::<Rect>(Id::new(("menu_bounds", "world"))))
            .expect("Context menu must offer world");
        let point = suggestion.center();
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(point),
                pointer(point, true, egui::PointerButton::Primary),
                pointer(point, false, egui::PointerButton::Primary),
            ],
        );
        assert_eq!(
            app.session
                .editor
                .doc
                .paragraph(Address::default())
                .unwrap()
                .text(),
            "Hello world. It are a apple."
        );
        app.session.editor.undo();
        assert_eq!(app.session.editor.doc, original);
        assert!(!app.session.editor.can_undo());
    }
    #[test]
    fn stale_background_result_is_discarded_after_edit_or_dictionary_change() {
        let ctx = Context::default();
        fonts::install(&ctx);
        let mut app = AlphaDocs::fresh(&ctx, Preferences::default(), vec![]);
        app.home = false;
        let old = source(&app.session.editor);
        let (tx, rx) = mpsc::channel();
        tx.send(Ok(vec![Issue {
            address: Address::default(),
            start: 0,
            end: 3,
            original: "old".into(),
            message: "test",
            suggestions: vec![],
            grammar: false,
        }]))
        .unwrap();
        app.live_proof.pending = Some((old, rx));
        app.session.editor.insert("New revision");
        app.poll_live_proof(&ctx);
        assert!(app.live_proof.current(&app.session.editor).is_empty());
        app.prefs.personal_dictionary.push("New".into());
        app.poll_live_proof(&ctx);
        assert!(app.live_proof.current(&app.session.editor).is_empty());
        assert!(app.live_proof.due.is_some());
    }
}
