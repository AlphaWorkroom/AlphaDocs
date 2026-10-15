use super::*;
use crate::collaboration::live::{self, LiveSession};
use std::sync::mpsc::{Receiver, TryRecvError};
pub(super) struct ShareState {
    name: String,
    ip: String,
    invitation: String,
    joining: bool,
    viewer: bool,
    pending: Option<Receiver<Result<LiveSession, String>>>,
    target: Option<(String, u64)>,
    error: String,
}
impl Default for ShareState {
    fn default() -> Self {
        Self {
            name: "You".into(),
            ip: live::suggest_ip(),
            invitation: String::new(),
            joining: false,
            viewer: false,
            pending: None,
            target: None,
            error: String::new(),
        }
    }
}
impl AlphaDocs {
    pub(super) fn open_sharing(&mut self) {
        if self.home {
            self.sharing.joining = true;
        }
        self.dialog = Some(Dialog::Share);
    }
    pub(super) fn poll_sharing(&mut self, ctx: &Context) {
        let result = self.sharing.pending.as_ref().map(|rx| rx.try_recv());
        if let Some(result) = result {
            match result {
                Ok(Ok(session)) => {
                    self.sharing.pending = None;
                    let current = self.sharing.target.as_ref().is_some_and(|(id, rev)| {
                        id == &self.session.editor.doc.id && *rev == self.session.editor.revision
                    });
                    if session.host && !current {
                        self.sharing.error =
                            "Document changed while starting; start sharing again".into();
                    } else {
                        let attached = (|| {
                            if !session.host {
                                let mut replica = session.editor_replica()?;
                                let doc = replica.project(&blank_document("Shared document"))?;
                                if !self.flush() {
                                    return Err("Save the current document before joining".into());
                                }
                                self.load(doc, None);
                            }
                            self.session
                                .editor
                                .attach_collaboration(session.room.clone())?;
                            self.session.viewing = !self.session.editor.collaboration_writable();
                            self.session.collaboration = Some(session);
                            self.status =
                                "Shared session started · changes also save locally".into();
                            Ok::<(), String>(())
                        })();
                        if let Err(e) = attached {
                            self.sharing.error = e;
                        }
                    }
                    self.sharing.invitation.clear();
                    self.sharing.target = None;
                }
                Ok(Err(error)) => {
                    self.sharing.pending = None;
                    self.sharing.error = error;
                    self.sharing.target = None;
                }
                Err(TryRecvError::Disconnected) => {
                    self.sharing.pending = None;
                    self.sharing.error = "Sharing worker stopped".into();
                    self.sharing.target = None;
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        fn refresh(session: &mut DocumentSession) {
            session.editor.refresh_collaboration(false);
            if session.collaboration.is_some() && !session.editor.collaboration_writable() {
                session.viewing = true;
            }
        }
        refresh(&mut self.session);
        for tab in &mut self.tabs.tabs {
            if let Some(session) = &mut tab.session {
                refresh(session);
            }
        }
        if self.sharing.pending.is_some()
            || self.session.collaboration.is_some()
            || self.tabs.tabs.iter().any(|t| {
                t.session
                    .as_ref()
                    .is_some_and(|s| s.collaboration.is_some())
            })
        {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
    pub(super) fn sharing_dialog(&mut self, ctx: &Context) {
        let p = self.palette;
        let mut close = false;
        let modal=egui::Modal::new(Id::new("sharing_dialog")).frame(Frame::new().fill(p.chrome).corner_radius(24).inner_margin(Margin::same(24))).show(ctx,|ui|{
            ui.set_width(456.0);
            ui.label(RichText::new("Share document").size(24.0));
            ui.add_space(12.0);
            if let Some(session)=&self.session.collaboration {
                let host=session.host;
                let(status,peers,local_name)=session.room.lock().map(|r|(r.status.clone(),r.peers.values().filter(|p|p.seen.elapsed()<Duration::from_secs(10)).map(|p|p.presence.name.clone()).collect::<Vec<_>>(),r.local.name.clone())).unwrap_or_else(|_|("Session unavailable".into(),vec![],String::new()));
                ui.label(RichText::new(status).color(p.accent));
                ui.add_space(8.0);
                ui.label(format!("{local_name} (you){}",if host{" · Host"}else{""}));
                for name in peers {ui.label(format!("{name} · Connected"));}
                if host {
                    ui.separator();
                    ui.label("Anyone holding an invitation can use its access until it expires or sharing ends. Send it privately.");
                    ui.horizontal(|ui|{ui.selectable_value(&mut self.sharing.viewer,false,"Can edit");ui.selectable_value(&mut self.sharing.viewer,true,"Can view");});
                    let invite=if self.sharing.viewer{&session.view_invite}else{&session.edit_invite};
                    if let Some(invite)=invite {
                        ui.label(RichText::new(invite.destination()).size(11.0).color(p.muted));
                        if tonal(ui,"Copy invitation",p,true).clicked(){
                            match invite.encode(){Ok(token)=>{ctx.copy_text(token);self.sharing.error="Invitation copied · expires after one hour".into();},Err(e)=>self.sharing.error=e}
                        }
                    }
                    ui.add_space(8.0);
                    ui.label(RichText::new("Direct LAN/VPN only. The host must remain open. No relay, accounts or email delivery.").size(12.0).color(p.muted));
                }else{ui.label("This is a local document copy. Changes synchronize while the host is reachable.");}
                ui.add_space(18.0);
                if ui.button(if host{"End sharing"}else{"Leave session"}).clicked(){
                    self.session.editor.disconnect_collaboration();
                    self.session.collaboration=None;
                    self.session.viewing=false;
                    self.status="Session ended · document retained locally".into();
                    self.sharing.error.clear();
                }
            }else{
                ui.horizontal(|ui|{ui.selectable_value(&mut self.sharing.joining,false,"Share this document");ui.selectable_value(&mut self.sharing.joining,true,"Join a session");});
                ui.add_space(10.0);
                ui.label("Display name");
                ui.add(egui::TextEdit::singleline(&mut self.sharing.name).char_limit(40).desired_width(f32::INFINITY).margin(vec2(10.0,8.0)));
                ui.label(RichText::new("Names are chosen by participants; they are not verified accounts.").size(12.0).color(p.muted));
                ui.add_space(12.0);
                let busy=self.sharing.pending.is_some();
                if self.sharing.joining{
                    ui.label("Invitation");
                    ui.add(egui::TextEdit::multiline(&mut self.sharing.invitation).password(false).desired_rows(3).desired_width(f32::INFINITY).char_limit(2048).hint_text("alphadocs://join/…").margin(vec2(10.0,8.0)));
                    if let Ok(invite)=live::Invite::parse(&self.sharing.invitation){ui.label(RichText::new(invite.destination()).size(11.0).color(p.muted));}
                    ui.add_space(12.0);
                    let join=primary(ui,"Join",p,!busy&&!self.sharing.name.trim().is_empty()&&!self.sharing.invitation.trim().is_empty());
                    #[cfg(test)]ctx.data_mut(|d|d.insert_temp(Id::new("share_join_bounds"),join.rect));
                    if join.clicked(){
                        match live::Invite::parse(&self.sharing.invitation){Ok(invite)=>{self.sharing.pending=Some(live::join(invite,self.sharing.name.trim().into()));self.sharing.error.clear();},Err(e)=>self.sharing.error=e}
                    }
                }else{
                    ui.label("Your LAN or VPN address");
                    ui.add(egui::TextEdit::singleline(&mut self.sharing.ip).desired_width(f32::INFINITY).margin(vec2(10.0,8.0)));
                    ui.add_space(8.0);
                    ui.label("Encrypted sharing connects directly to this computer. Guests need a reachable LAN/VPN route and your private invitation. Sharing starts a new undo history; existing content is retained.");
                    if self.sharing.ip.trim()=="127.0.0.1"{ui.label("127.0.0.1 allows testing on this computer only.");}
                    ui.add_space(12.0);
                    let start=primary(ui,"Start sharing",p,!busy&&!self.home&&self.session.editor.collaboration_writable()&&!self.sharing.name.trim().is_empty());
                    #[cfg(test)]ctx.data_mut(|d|d.insert_temp(Id::new("share_start_bounds"),start.rect));
                    if start.clicked(){
                        match self.sharing.ip.trim().parse(){
                            Ok(ip)=>{self.sharing.pending=Some(live::host(self.session.editor.doc.clone(),ip,self.sharing.name.trim().into()));self.sharing.target=Some((self.session.editor.doc.id.clone(),self.session.editor.revision));self.sharing.error.clear();},
                            Err(_)=>self.sharing.error="Enter a local IPv4 or IPv6 address".into(),
                        }
                    }
                }
                if busy{ui.horizontal(|ui|{ui.spinner();ui.label("Connecting…");});}
            }
            if !self.sharing.error.is_empty(){ui.add_space(10.0);ui.label(&self.sharing.error);}
            ui.add_space(16.0);
            ui.with_layout(UiLayout::right_to_left(Align::Center),|ui|{if tonal(ui,"Done",p,true).clicked(){close=true;}});
        });
        if close || modal.should_close() {
            self.dialog = None;
            self.refocus = true;
        }
    }
}
pub(super) fn paint_presence(
    ui: &Ui,
    peers: &[(String, String, Position, Position)],
    line: &layout::Line,
    display: &egui::Galley,
    pos: Pos2,
    height: f32,
) {
    let colors = [
        Color32::from_rgb(20, 130, 100),
        Color32::from_rgb(155, 70, 190),
        Color32::from_rgb(210, 105, 20),
        Color32::from_rgb(20, 120, 185),
    ];
    for (id, name, caret, anchor) in peers {
        let color = colors[id
            .bytes()
            .fold(0usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize))
            % colors.len()];
        let (s, e) = if caret < anchor {
            (*caret, *anchor)
        } else {
            (*anchor, *caret)
        };
        if line.address >= s.address && line.address <= e.address {
            let begin = if line.address == s.address {
                s.offset.max(line.start)
            } else {
                line.start
            };
            let end = if line.address == e.address {
                e.offset.min(line.end)
            } else {
                line.end
            };
            if end > begin {
                ui.painter().rect_filled(
                    Rect::from_min_max(
                        pos + vec2(layout::display_x(display, line.start, begin), 0.0),
                        pos + vec2(layout::display_x(display, line.start, end), height),
                    ),
                    0,
                    color.gamma_multiply(0.18),
                );
            }
        }
        if caret.address == line.address && caret.offset >= line.start && caret.offset <= line.end {
            let x = pos.x + layout::display_x(display, line.start, caret.offset);
            ui.painter().line_segment(
                [pos2(x, pos.y), pos2(x, pos.y + height)],
                Stroke::new(2.0, color),
            );
            let galley = ui.painter().layout_no_wrap(
                name.clone(),
                FontId::proportional(11.0),
                Color32::WHITE,
            );
            let r = Rect::from_min_size(pos2(x, pos.y - 17.0), galley.size() + vec2(8.0, 4.0));
            ui.painter().rect_filled(r, 3, color);
            ui.painter()
                .galley(r.min + vec2(4.0, 2.0), galley, Color32::WHITE);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn app(theme: Theme) -> (Context, AlphaDocs) {
        let ctx = Context::default();
        fonts::install(&ctx);
        let mut app = AlphaDocs::fresh(
            &ctx,
            Preferences {
                theme,
                autosave: false,
                ..Default::default()
            },
            vec![],
        );
        app.home = false;
        (ctx, app)
    }
    fn frame(ctx: &Context, app: &mut AlphaDocs, events: Vec<Event>) {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 900.0))),
                events,
                ..Default::default()
            },
            |ui| app.update_ui(ui),
        );
        output.textures_delta.clear();
    }
    fn click(ctx: &Context, app: &mut AlphaDocs, id: &str) {
        frame(ctx, app, vec![]);
        frame(ctx, app, vec![]);
        let r = ctx.data_mut(|d| d.get_temp::<Rect>(Id::new(id))).unwrap();
        assert!(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 900.0)).contains_rect(r));
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(r.center()),
                Event::PointerButton {
                    pos: r.center(),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                Event::PointerButton {
                    pos: r.center(),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    #[test]
    fn native_ui_frames_start_join_edit_and_view_without_overwriting_local_document() {
        for theme in [Theme::Light, Theme::Dark] {
            let (ctx, mut host) = app(theme);
            host.session.editor.insert("Shared content");
            host.sharing.ip = "127.0.0.1".into();
            host.sharing.name = "Host QA".into();
            host.execute(Action::Share, &ctx);
            click(&ctx, &mut host, "share_start_bounds");
            for _ in 0..100 {
                frame(&ctx, &mut host, vec![]);
                if host.session.collaboration.is_some() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            assert!(
                host.session.collaboration.is_some(),
                "{}",
                host.sharing.error
            );
            let token = host
                .session
                .collaboration
                .as_ref()
                .unwrap()
                .view_invite
                .as_ref()
                .unwrap()
                .encode()
                .unwrap();
            let (gctx, mut guest) = app(theme);
            let previous_id = guest.session.editor.doc.id.clone();
            guest.sharing.joining = true;
            guest.sharing.name = "Viewer QA".into();
            guest.sharing.invitation = token;
            guest.execute(Action::Share, &gctx);
            click(&gctx, &mut guest, "share_join_bounds");
            for _ in 0..100 {
                frame(&gctx, &mut guest, vec![]);
                if guest.session.collaboration.is_some() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            assert!(
                guest.session.collaboration.is_some(),
                "{}",
                guest.sharing.error
            );
            assert!(guest.session.viewing);
            assert_ne!(guest.session.editor.doc.id, previous_id);
            assert_eq!(guest.session.editor.doc.text(), "Shared content");
            guest.dialog = None;
            gctx.memory_mut(|m| m.request_focus(Id::new("document_canvas")));
            frame(&gctx, &mut guest, vec![Event::Text("Denied".into())]);
            assert_eq!(guest.session.editor.doc.text(), "Shared content");
            host.dialog = None;
            ctx.memory_mut(|m| m.request_focus(Id::new("document_canvas")));
            frame(&ctx, &mut host, vec![Event::Text(" edit".into())]);
            for _ in 0..100 {
                frame(&ctx, &mut host, vec![]);
                frame(&gctx, &mut guest, vec![]);
                if guest.session.editor.doc.text() == host.session.editor.doc.text() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            assert_eq!(
                guest.session.editor.doc.text(),
                host.session.editor.doc.text()
            );
            assert!(guest.session.editor.doc.text().contains(" edit"));
        }
    }
}
