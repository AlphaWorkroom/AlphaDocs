use super::*;
#[derive(Clone)]
struct Thread {
    id: String,
    comments: SharedVec<ImageComment>,
    reactions: SharedVec<String>,
}
fn threads(doc: &Document) -> Vec<Thread> {
    let mut result = vec![];
    let mut seen = std::collections::HashSet::new();
    for b in &doc.blocks {
        if let Block::Image(i) = b {
            result.push(Thread {
                id: i.id.clone(),
                comments: i.comments.clone(),
                reactions: i.reactions.clone(),
            });
        }
    }
    for a in doc
        .addresses()
        .into_iter()
        .filter(|a| a.block < doc.blocks.len())
    {
        if let Some(p) = doc.paragraph(a) {
            for n in &p.annotations {
                if seen.insert(n.id.clone()) {
                    result.push(Thread {
                        id: n.id.clone(),
                        comments: n.comments.clone(),
                        reactions: n.reactions.clone(),
                    });
                }
            }
        }
    }
    result
}
pub(super) fn has_annotations(doc: &Document) -> bool {
    threads(doc)
        .iter()
        .any(|t| !t.reactions.is_empty() || t.comments.iter().any(|c| !c.resolved))
}
pub(super) fn highlight(p: Palette, active: bool) -> Color32 {
    if p.dark {
        Color32::from_rgb(if active { 110 } else { 83 }, 72, 31)
    } else if active {
        Color32::from_rgb(255, 220, 151)
    } else {
        Color32::from_rgb(255, 242, 201)
    }
}
impl AlphaDocs {
    pub(super) fn begin_comment(&mut self) {
        if self.session.viewing {
            return;
        }
        if self.image_block.is_none() {
            if !self.session.editor.selected() {
                return;
            }
            self.text_comment_range =
                Some((self.session.editor.range(), self.session.editor.revision));
        } else {
            self.text_comment_range = None;
        }
        self.comments_open = true;
        self.image_options_open = false;
        self.comment_focus = true;
        self.refocus = false;
    }
    pub(super) fn comments_sidebar(&mut self, _ui: &mut Ui) {
        if !self.comments_open {
            return;
        }
        self.refocus = false;
        let id = if self.text_comment_range.is_some() {
            Some("text-draft".to_owned())
        } else {
            self.image_block
                .and_then(|b| match self.session.editor.doc.blocks.get(b) {
                    Some(Block::Image(i)) => Some(i.id.clone()),
                    _ => None,
                })
        };
        if let Some(id) = id
            && self.comment_draft_image.as_ref() != Some(&id)
        {
            if let Some(previous) = self.comment_draft_image.take() {
                self.comment_drafts
                    .insert(previous, std::mem::take(&mut self.comment_draft));
            }
            self.comment_draft = self.comment_drafts.remove(&id).unwrap_or_default();
            self.comment_draft_image = Some(id);
        }
    }
    pub(super) fn comment_cards(&mut self, ui: &mut Ui) {
        if self.image_options_open {
            return;
        }
        let p = self.palette;
        let ctx = ui.ctx().clone();
        let clip = ui.clip_rect();
        let width = if clip.width() > 1300.0 { 416.0 } else { 320.0 };
        let x = self.annotation_rail_x;
        // Keep the cards in the document's scrollable rail, never on top of paper.
        if x >= clip.right() || x + width <= clip.left() {
            return;
        }
        let mut visible: Vec<_> = threads(&self.session.editor.doc)
            .into_iter()
            .filter_map(|t| self.annotation_positions.get(&t.id).map(|r| (r.top(), t)))
            .collect();
        visible.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut bottom = clip.top() + 4.0;
        if self.comments_open
            && let Some(id) = self.comment_draft_image.clone()
            && let Some(anchor) = self.annotation_positions.get(&id).copied()
        {
            let y = anchor.top().clamp(clip.top() + 8.0, clip.bottom() - 180.0);
            let out=egui::Area::new(Id::new(("comment_composer",&self.session.editor.doc.id))).fixed_pos(pos2(x,y)).constrain(false)
            .order(egui::Order::Middle).show(&ctx,|ui| {
                ui.set_width(width);ui.set_clip_rect(clip);
                Frame::new().fill(p.chrome).stroke(Stroke::new(0.8,p.border)).corner_radius(14).inner_margin(Margin::same(16)).show(ui,|ui| {
                    ui.set_width(width-32.0);ui.horizontal(|ui|author(ui,p,0,&self.session.editor.collaboration_name()));
                    let submit = ui.ctx().memory(|m|m.has_focus(Id::new("new_image_comment"))) && ui.input_mut(|i|i.consume_key(egui::Modifiers::COMMAND,Key::Enter));
                    let draft=ui.add(egui::TextEdit::multiline(&mut self.comment_draft).id(Id::new("new_image_comment")).hint_text("Add a comment…").desired_width(f32::INFINITY).desired_rows(3));
                    if self.comment_focus {
                        if ui.is_visible() { draft.request_focus(); self.comment_focus=false; }
                        else {ui.ctx().request_discard("Measure comment composer before focusing it");}
                    }
                    ui.horizontal(|ui| {
                        if (primary(ui,"Comment",p,!self.comment_draft.trim().is_empty() && !self.session.viewing).on_hover_text(super::super::platform::shortcut_label(ui.ctx(), "Ctrl+Enter to post")).clicked() || submit) && !self.session.viewing && !self.comment_draft.trim().is_empty() {
                            let added=if let Some((range,rev))=self.text_comment_range {
                                if rev==self.session.editor.revision {self.session.editor.annotate_text(range,Some(&self.comment_draft),None)}else{self.status="Text changed while drafting. Select its range again.".into();None}
                            }else{self.session.editor.add_annotation_comment(&id,&self.comment_draft).map(|_|id.clone())};
                            if let Some(target)=added {
                                let cid=threads(&self.session.editor.doc).into_iter().find(|t|t.id==target).and_then(|t|t.comments.last().map(|c|c.id.clone())).unwrap_or_default();
                                self.active_comment=Some((target,cid));self.comment_draft.clear();self.comments_open=false;self.text_comment_range=None;
                                self.session.editor.place(self.session.editor.caret,false);self.refocus=true;
                                ui.ctx().memory_mut(|m|m.request_focus(Id::new("document_canvas")));
                            }
                        }
                        if ui.button("Cancel").clicked(){self.comments_open=false;self.text_comment_range=None;self.refocus=true;}
                    });
                });
            });
            bottom = out.response.rect.bottom() + 16.0;
        }
        for (anchor_y, t) in visible {
            let cs: Vec<_> = t
                .comments
                .iter()
                .filter(|c| !c.resolved || self.comments_resolved)
                .collect();
            if cs.is_empty() && t.reactions.is_empty() {
                continue;
            }
            let y = anchor_y.max(bottom);
            if y > clip.bottom() || anchor_y < clip.top() - 250.0 {
                continue;
            }
            let out = egui::Area::new(Id::new((
                "anchored_thread",
                &self.session.editor.doc.id,
                &t.id,
            )))
            .fixed_pos(pos2(x, y))
            .constrain(false)
            .order(egui::Order::Middle)
            .show(&ctx, |ui| {
                ui.set_width(width);
                ui.set_clip_rect(clip);
                if !t.reactions.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        for emoji in &t.reactions {
                            if reaction_chip(ui, emoji, p, !self.session.viewing).clicked() {
                                self.session
                                    .editor
                                    .toggle_annotation_reaction(&t.id, None, emoji);
                            }
                        }
                    });
                    ui.add_space(8.0);
                }
                for c in &cs {
                    let active = self
                        .active_comment
                        .as_ref()
                        .is_some_and(|(id, cid)| id == &t.id && cid == &c.id);
                    Frame::new()
                        .fill(if active { p.chrome } else { p.toolbar })
                        .corner_radius(12)
                        .inner_margin(Margin::same(16))
                        .stroke(if active {
                            Stroke::new(0.7, p.border)
                        } else {
                            Stroke::NONE
                        })
                        .show(ui, |ui| {
                            ui.set_width(width - 32.0 - if active { 1.4 } else { 0.0 });
                            ui.horizontal(|ui| {
                                author(ui, p, c.created, &c.author);
                                ui.allocate_ui_with_layout(
                                    vec2(ui.available_width(), 32.0),
                                    UiLayout::right_to_left(Align::Center),
                                    |ui| {
                                        if active {
                                            let more = icons::button(
                                                ui,
                                                Icon::More,
                                                "Comment options",
                                                false,
                                                !self.session.viewing,
                                            );
                                            egui::Popup::menu(&more).show(|ui| {
                                                if menu(
                                                    ui,
                                                    "Edit comment",
                                                    "",
                                                    !self.session.viewing,
                                                ) {
                                                    self.comment_edit = Some((
                                                        t.id.clone(),
                                                        c.id.clone(),
                                                        c.text.clone(),
                                                    ));
                                                }
                                                if menu(
                                                    ui,
                                                    "Delete comment",
                                                    "",
                                                    !self.session.viewing,
                                                ) {
                                                    self.session
                                                        .editor
                                                        .delete_annotation_comment(&t.id, &c.id);
                                                }
                                            });
                                            if icons::button(
                                                ui,
                                                Icon::Resolve,
                                                if c.resolved {
                                                    "Reopen comment"
                                                } else {
                                                    "Resolve comment"
                                                },
                                                false,
                                                !self.session.viewing,
                                            )
                                            .clicked()
                                            {
                                                self.session.editor.resolve_annotation_comment(
                                                    &t.id,
                                                    &c.id,
                                                    !c.resolved,
                                                );
                                                self.active_comment = None;
                                            }
                                        }
                                    },
                                );
                            });
                            ui.add_space(8.0);
                            if let Some((object, cid, draft)) = &mut self.comment_edit
                                && object == &t.id
                                && cid == &c.id
                            {
                                ui.add(
                                    egui::TextEdit::multiline(draft)
                                        .desired_width(f32::INFINITY)
                                        .desired_rows(3),
                                );
                                let mut close = false;
                                ui.horizontal(|ui| {
                                    if ui
                                        .add_enabled(
                                            !draft.trim().is_empty(),
                                            egui::Button::new("Save"),
                                        )
                                        .clicked()
                                    {
                                        self.session
                                            .editor
                                            .edit_annotation_comment(&t.id, &c.id, draft);
                                        close = true;
                                    }
                                    if ui.button("Cancel").clicked() {
                                        close = true;
                                    }
                                });
                                if close {
                                    self.comment_edit = None;
                                }
                            } else {
                                let body =
                                    ui.add(egui::Label::new(&c.text).wrap().sense(Sense::click()));
                                if body.clicked() {
                                    self.active_comment = Some((t.id.clone(), c.id.clone()));
                                }
                                if active || body.hovered() {
                                    ui.allocate_ui_with_layout(
                                        vec2(ui.available_width(), 32.0),
                                        UiLayout::right_to_left(Align::Center),
                                        |ui| {
                                            if icons::button(
                                                ui,
                                                Icon::Reaction,
                                                "React to comment",
                                                false,
                                                !self.session.viewing,
                                            )
                                            .clicked()
                                            {
                                                self.emoji_reaction_target = Some(t.id.clone());
                                                self.emoji_reaction_comment = Some(c.id.clone());
                                                self.dialog = Some(Dialog::Emojis);
                                            }
                                        },
                                    );
                                }
                            }
                            for reply in &c.replies {
                                ui.add_space(12.0);
                                ui.horizontal(|ui| author(ui, p, 0, "Reply"));
                                ui.label(reply);
                            }
                            if !c.reactions.is_empty() {
                                ui.horizontal_wrapped(|ui| {
                                    for emoji in &c.reactions {
                                        if reaction_chip(ui, emoji, p, !self.session.viewing)
                                            .clicked()
                                        {
                                            self.session.editor.toggle_annotation_reaction(
                                                &t.id,
                                                Some(&c.id),
                                                emoji,
                                            );
                                        }
                                    }
                                });
                            }
                            if active && !c.resolved {
                                ui.add_space(12.0);
                                let reply = self
                                    .comment_replies
                                    .entry((t.id.clone(), c.id.clone()))
                                    .or_default();
                                let response = Frame::new()
                                    .fill(p.chrome)
                                    .stroke(Stroke::new(0.7, p.border))
                                    .corner_radius(20)
                                    .inner_margin(Margin::same(8))
                                    .show(ui, |ui| {
                                        ui.add_enabled(
                                            !self.session.viewing,
                                            egui::TextEdit::multiline(reply)
                                                .frame(Frame::NONE)
                                                .hint_text("Reply…")
                                                .desired_width(f32::INFINITY)
                                                .desired_rows(1),
                                        )
                                    })
                                    .inner;
                                if (response.has_focus() || !reply.is_empty())
                                    && primary(
                                        ui,
                                        "Reply",
                                        p,
                                        !self.session.viewing && !reply.trim().is_empty(),
                                    )
                                    .clicked()
                                {
                                    self.session
                                        .editor
                                        .reply_annotation_comment(&t.id, &c.id, reply);
                                    reply.clear();
                                }
                            }
                        });
                    ui.add_space(12.0);
                }
            });
            bottom = out.response.rect.bottom() + 16.0;
            #[cfg(test)]
            ctx.data_mut(|d| {
                d.insert_temp(
                    Id::new(("annotation_thread_bounds", &t.id)),
                    out.response.rect,
                )
            });
        }
        if self.image_block.is_none()
            && self.session.editor.selected()
            && !self.comments_open
            && !self.session.viewing
            && let Some(rect) = self.selection_annotation_rect
        {
            egui::Area::new(Id::new("selection_comment_tools"))
                .fixed_pos(pos2(
                    (rect.right() - 68.0).max(clip.left()),
                    rect.bottom() + 4.0,
                ))
                .constrain(false)
                .order(egui::Order::Middle)
                .show(&ctx, |ui| {
                    Frame::new()
                        .fill(p.chrome)
                        .stroke(Stroke::new(0.7, p.border))
                        .corner_radius(18)
                        .inner_margin(Margin::same(4))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if icons::button(ui, Icon::Comment, "Add comment", false, true)
                                    .clicked()
                                {
                                    self.begin_comment();
                                }
                                if icons::button(
                                    ui,
                                    Icon::Reaction,
                                    "Add emoji reaction",
                                    false,
                                    true,
                                )
                                .clicked()
                                {
                                    self.queue(Action::Reaction);
                                }
                            });
                        });
                });
        }
    }
}
fn author(ui: &mut Ui, p: Palette, created: u64, name: &str) {
    let name = if name.is_empty() { "You" } else { name };
    let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
    ui.painter()
        .circle_filled(r.center(), 16.0, Color32::from_rgb(140, 110, 98));
    ui.painter().text(
        r.center(),
        Align2::CENTER_CENTER,
        name.chars().next().unwrap_or('Y').to_string(),
        FontId::proportional(18.0),
        Color32::WHITE,
    );
    ui.vertical(|ui| {
        ui.label(RichText::new(name).size(14.0));
        if created != 0 {
            ui.label(
                RichText::new(comment_time(created))
                    .size(11.0)
                    .color(p.muted),
            );
        }
    });
}
fn comment_time(created: u64) -> String {
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let now = time::OffsetDateTime::now_utc().to_offset(offset);
    let Some(date) = time::OffsetDateTime::from_unix_timestamp(created as i64)
        .ok()
        .map(|d| d.to_offset(offset))
    else {
        return String::new();
    };
    let day = if date.date() == now.date() {
        "Today".to_owned()
    } else if date.date() == (now - time::Duration::days(1)).date() {
        "Yesterday".to_owned()
    } else {
        date.date().to_string()
    };
    format!(
        "{}:{:02} {} {day}",
        match date.hour() % 12 {
            0 => 12,
            h => h,
        },
        date.minute(),
        if date.hour() < 12 { "AM" } else { "PM" }
    )
}
fn reaction_chip(ui: &mut Ui, emoji: &str, p: Palette, enabled: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        vec2(56.0, 32.0),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    ui.painter().rect_filled(rect, 16.0, p.toolbar);
    color_emoji(
        ui,
        Rect::from_center_size(rect.left_center() + vec2(17.0, 0.0), vec2(22.0, 22.0)),
        emoji,
        p,
    );
    ui.painter().text(
        rect.right_center() - vec2(12.0, 0.0),
        Align2::CENTER_CENTER,
        "1",
        FontId::proportional(14.0),
        p.accent,
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Button,
            enabled,
            true,
            format!("{emoji} · Your reaction"),
        )
    });
    response.on_hover_text("Remove your reaction")
}
fn color_emoji(ui: &mut Ui, rect: Rect, emoji: &str, p: Palette) {
    let bytes: Option<&[u8]> = match emoji.trim_end_matches('\u{fe0f}') {
        "😀" => Some(include_bytes!("../../../assets/emoji/1f600.png")),
        "😁" => Some(include_bytes!("../../../assets/emoji/1f601.png")),
        "😂" => Some(include_bytes!("../../../assets/emoji/1f602.png")),
        "😃" => Some(include_bytes!("../../../assets/emoji/1f603.png")),
        "😄" => Some(include_bytes!("../../../assets/emoji/1f604.png")),
        "😅" => Some(include_bytes!("../../../assets/emoji/1f605.png")),
        "😆" => Some(include_bytes!("../../../assets/emoji/1f606.png")),
        "😉" => Some(include_bytes!("../../../assets/emoji/1f609.png")),
        "😊" => Some(include_bytes!("../../../assets/emoji/1f60a.png")),
        "😍" => Some(include_bytes!("../../../assets/emoji/1f60d.png")),
        "😢" => Some(include_bytes!("../../../assets/emoji/1f622.png")),
        "😭" => Some(include_bytes!("../../../assets/emoji/1f62d.png")),
        "🤔" => Some(include_bytes!("../../../assets/emoji/1f914.png")),
        "👍" => Some(include_bytes!("../../../assets/emoji/1f44d.png")),
        "👎" => Some(include_bytes!("../../../assets/emoji/1f44e.png")),
        "❤" => Some(include_bytes!("../../../assets/emoji/2764.png")),
        "🎉" => Some(include_bytes!("../../../assets/emoji/1f389.png")),
        "🔥" => Some(include_bytes!("../../../assets/emoji/1f525.png")),
        _ => None,
    };
    if let Some(bytes) = bytes {
        let id = Id::new(("color_reaction", emoji));
        let cached = ui.ctx().data_mut(|d| d.get_temp::<egui::TextureHandle>(id));
        let texture = cached.or_else(|| {
            let im = image::load_from_memory(bytes).ok()?.to_rgba8();
            let handle = ui.ctx().load_texture(
                emoji,
                egui::ColorImage::from_rgba_unmultiplied(
                    [im.width() as usize, im.height() as usize],
                    im.as_raw(),
                ),
                egui::TextureOptions::LINEAR,
            );
            ui.ctx().data_mut(|d| d.insert_temp(id, handle.clone()));
            Some(handle)
        });
        if let Some(t) = texture {
            ui.painter().image(
                t.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                Color32::WHITE,
            );
            return;
        }
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        emoji,
        FontId::new(18.0, egui::FontFamily::Name("Emoji".into())),
        p.text,
    );
}
