use super::*;

/// Client-drawn tabs and controls; all window operations use native viewport commands.
pub(super) fn show(app: &mut AlphaDocs, ui: &mut Ui) {
    let p = app.palette;
    let ctx = ui.ctx().clone();
    egui::Panel::top("desktop_header")
        .exact_size(42.0)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(p.toolbar)
                .inner_margin(Margin::symmetric(10, 0)),
        )
        .show(ui, |ui| {
            let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 42.0), Sense::hover());
            let controls = Rect::from_min_max(pos2(row.right() - 138.0, row.top()), row.max);
            let mut tabs_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(Rect::from_min_max(
                        row.min,
                        pos2(controls.left() - 184.0, row.bottom()),
                    ))
                    .layout(UiLayout::left_to_right(Align::Center)),
            );
            tabs_ui.spacing_mut().item_spacing.x = 6.0;
            let (brand, _) = tabs_ui.allocate_exact_size(vec2(24.0, 28.0), Sense::hover());
            icons::brand(&mut tabs_ui, brand.shrink2(vec2(2.0, 1.0)), p.accent);
            if tab(&mut tabs_ui, "Documents", app.home, p, None)
                .0
                .clicked()
            {
                app.queue(Action::Home);
            }
            let mut change = None;
            let mut close = None;
            let mut rename = false;
            let available = (tabs_ui.available_width() - 38.0).max(40.0);
            egui::ScrollArea::horizontal()
                .id_salt("document_tabs")
                .max_width(available)
                .show(&mut tabs_ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for (index, document) in app.tabs.tabs.iter().enumerate() {
                            let active = app.tabs.active == Some(index);
                            let dirty = if active {
                                app.session.dirty()
                            } else {
                                document
                                    .session
                                    .as_ref()
                                    .is_some_and(DocumentSession::dirty)
                            };
                            let label = if dirty {
                                format!("{} •", document.title)
                            } else {
                                document.title.clone()
                            };
                            let (response, close_response) =
                                tab(ui, &label, active && !app.home, p, Some(index));
                            if response.clicked() {
                                change = Some(index);
                            }
                            if response.double_clicked() && !app.session.viewing {
                                change = Some(index);
                                rename = true;
                            }
                            if close_response.is_some_and(|r| r.clicked()) {
                                close = Some(index);
                            }
                        }
                    });
                });
            if icons::button(
                &mut tabs_ui,
                Icon::Plus,
                "New document (Ctrl+N)",
                false,
                true,
            )
            .clicked()
            {
                app.queue(Action::New);
            }
            let drag_rect = Rect::from_min_max(
                pos2(tabs_ui.next_widget_position().x, row.top()),
                pos2(controls.left() - 184.0, row.bottom()),
            );
            if drag_rect.width() > 0.0 {
                let drag = ui.interact(drag_rect, Id::new("window_drag"), Sense::click_and_drag());
                if drag.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag.double_clicked() {
                    toggle_maximize(&ctx);
                }
            }
            let search_rect = Rect::from_min_size(
                pos2(controls.left() - 176.0, row.top() + 5.0),
                vec2(164.0, 32.0),
            );
            let response = ui.interact(
                search_rect,
                Id::new("feature_search_button"),
                Sense::click(),
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Search features")
            });
            ui.painter().rect_filled(
                search_rect,
                18,
                if response.hovered() {
                    p.selection
                } else {
                    p.bg
                },
            );
            icons::draw(
                ui,
                Rect::from_center_size(
                    search_rect.left_center() + vec2(18.0, 0.0),
                    vec2(24.0, 24.0),
                ),
                Icon::Search,
                p.muted,
            );
            ui.painter().text(
                search_rect.left_center() + vec2(36.0, 0.0),
                Align2::LEFT_CENTER,
                "Search features",
                FontId::proportional(13.0),
                p.muted,
            );
            if response
                .on_hover_text("Search menus and features (Alt+/)")
                .clicked()
            {
                app.open_feature_search();
            }
            let mut controls_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(controls)
                    .layout(UiLayout::left_to_right(Align::Center)),
            );
            controls_ui.spacing_mut().item_spacing.x = 0.0;
            if window_button(&mut controls_ui, "Minimize", 0, p).clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
            if window_button(
                &mut controls_ui,
                if maximized {
                    "Restore window"
                } else {
                    "Maximize"
                },
                if maximized { 2 } else { 1 },
                p,
            )
            .clicked()
            {
                toggle_maximize(&ctx);
            }
            if window_button(&mut controls_ui, "Close window", 3, p).clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if let Some(index) = change {
                app.switch_tab(index);
            }
            if rename {
                app.execute(Action::Rename, &ctx);
            }
            if let Some(index) = close {
                match app.tabs.close(&mut app.session, index) {
                    Ok(()) => {
                        app.home = app.tabs.active.is_none();
                        app.reset_document_view();
                        app.persist_session();
                    }
                    Err(e) => app.status = format!("Tab remains open because saving failed: {e}"),
                }
            }
        });
    resize_edges(ui);
}
fn tab(
    ui: &mut Ui,
    label: &str,
    selected: bool,
    p: Palette,
    close: Option<usize>,
) -> (egui::Response, Option<egui::Response>) {
    let width = if close.is_some() { 192.0 } else { 116.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(width, 34.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, label)
    });
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius {
                nw: 10,
                ne: 10,
                sw: 0,
                se: 0,
            },
            if selected { p.bg } else { p.hover },
        );
    }
    if selected {
        ui.painter().line_segment(
            [
                rect.left_bottom() + vec2(8.0, -1.0),
                rect.right_bottom() - vec2(8.0, 1.0),
            ],
            Stroke::new(2.0, p.accent),
        );
    }
    let text_rect = Rect::from_min_max(
        rect.min + vec2(12.0, 0.0),
        pos2(
            rect.right() - if close.is_some() { 30.0 } else { 8.0 },
            rect.bottom(),
        ),
    );
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::proportional(13.0),
        if selected { p.text } else { p.muted },
    );
    let overflow = (galley.size().x - text_rect.width()).max(0.0);
    let id = Id::new(("tab_title_scroll", close));
    let animated = overflow > 0.0 && (selected || response.hovered());
    let offset = if animated {
        let now = ui.input(|i| i.time);
        let since = ui
            .ctx()
            .data_mut(|d| *d.get_temp_mut_or_insert_with(id, || now));
        ui.ctx().request_repaint_after(Duration::from_millis(33));
        title_scroll_offset(now - since, overflow)
    } else {
        ui.ctx().data_mut(|d| d.remove::<f64>(id));
        0.0
    };
    ui.painter()
        .with_clip_rect(ui.clip_rect().intersect(text_rect))
        .galley(
            pos2(
                text_rect.left() - offset,
                text_rect.center().y - galley.size().y * 0.5,
            ),
            galley,
            p.text,
        );
    let close_response = close.map(|index| {
        let r =
            Rect::from_center_size(pos2(rect.right() - 15.0, rect.center().y), vec2(22.0, 24.0));
        let response = ui.interact(r, Id::new(("close_document", index)), Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Close {label}"))
        });
        if response.hovered() {
            ui.painter().rect_filled(r, 5.0, p.hover);
        }
        icons::draw(ui, r, Icon::Close, p.muted);
        response.on_hover_text("Close tab — saved in your local library")
    });
    (response.on_hover_text(label), close_response)
}
fn toggle_maximize(ctx: &Context) {
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
}
fn window_button(ui: &mut Ui, label: &str, kind: u8, p: Palette) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(46.0, 42.0), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let fg = if kind == 3 && response.hovered() {
        Color32::WHITE
    } else {
        p.text
    };
    let s = Stroke::new(1.0, fg);
    if response.hovered() {
        ui.painter().rect_filled(
            rect,
            0.0,
            if kind == 3 {
                Color32::from_rgb(196, 43, 28)
            } else {
                p.hover
            },
        );
    }
    let c = rect.center();
    match kind {
        0 => {
            ui.painter()
                .line_segment([c + vec2(-5.0, 0.0), c + vec2(5.0, 0.0)], s);
        }
        1 => {
            ui.painter().rect_stroke(
                Rect::from_center_size(c, vec2(10.0, 10.0)),
                0.0,
                s,
                egui::StrokeKind::Inside,
            );
        }
        2 => {
            ui.painter().rect_stroke(
                Rect::from_center_size(c + vec2(1.5, -1.5), vec2(9.0, 9.0)),
                0.0,
                s,
                egui::StrokeKind::Inside,
            );
            ui.painter().rect_filled(
                Rect::from_center_size(c + vec2(-1.5, 1.5), vec2(9.0, 9.0)),
                0.0,
                if response.hovered() {
                    p.hover
                } else {
                    p.toolbar
                },
            );
            ui.painter().rect_stroke(
                Rect::from_center_size(c + vec2(-1.5, 1.5), vec2(9.0, 9.0)),
                0.0,
                s,
                egui::StrokeKind::Inside,
            );
        }
        _ => {
            ui.painter()
                .line_segment([c + vec2(-5.0, -5.0), c + vec2(5.0, 5.0)], s);
            ui.painter()
                .line_segment([c + vec2(-5.0, 5.0), c + vec2(5.0, -5.0)], s);
        }
    }
    response.on_hover_text(label)
}
fn resize_edges(ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    if ctx.input(|i| {
        i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)
    }) {
        return;
    }
    let rect = ctx.content_rect();
    let t = 5.0;
    let c = 12.0;
    use egui::ResizeDirection::*;
    let regions = [
        (NorthWest, Rect::from_min_size(rect.min, vec2(c, c))),
        (
            NorthEast,
            Rect::from_min_size(rect.right_top() - vec2(c, 0.0), vec2(c, c)),
        ),
        (
            SouthWest,
            Rect::from_min_size(rect.left_bottom() - vec2(0.0, c), vec2(c, c)),
        ),
        (
            SouthEast,
            Rect::from_min_size(rect.max - vec2(c, c), vec2(c, c)),
        ),
        (
            West,
            Rect::from_min_max(rect.min + vec2(0.0, c), rect.left_bottom() + vec2(t, -c)),
        ),
        (
            East,
            Rect::from_min_max(rect.right_top() + vec2(-t, c), rect.max - vec2(0.0, c)),
        ),
        (
            North,
            Rect::from_min_max(rect.min + vec2(c, 0.0), rect.right_top() + vec2(-c, t)),
        ),
        (
            South,
            Rect::from_min_max(rect.left_bottom() + vec2(c, -t), rect.max - vec2(c, 0.0)),
        ),
    ];
    for (index, (direction, rect)) in regions.into_iter().enumerate() {
        let response = ui.interact(rect, Id::new(("window_resize", index)), Sense::drag());
        if response.hovered() {
            ctx.set_cursor_icon(match direction {
                West | East => egui::CursorIcon::ResizeHorizontal,
                North | South => egui::CursorIcon::ResizeVertical,
                NorthWest | SouthEast => egui::CursorIcon::ResizeNwSe,
                _ => egui::CursorIcon::ResizeNeSw,
            });
        }
        if response.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
        }
    }
}

// Pause at each end, then pan back: every part remains readable without a jump.
fn title_scroll_offset(elapsed: f64, overflow: f32) -> f32 {
    let travel = f64::from(overflow) / 28.0;
    let phase = elapsed.rem_euclid(2.0 * travel + 2.4);
    if phase < 1.2 {
        0.0
    } else if phase < travel + 1.2 {
        ((phase - 1.2) * 28.0) as f32
    } else if phase < travel + 2.4 {
        overflow
    } else {
        (overflow - ((phase - travel - 2.4) * 28.0) as f32).max(0.0)
    }
}
#[cfg(test)]
mod title_tests {
    use super::*;
    #[test]
    fn long_titles_pause_reach_the_end_and_return_within_bounds() {
        assert_eq!(title_scroll_offset(0.0, 140.0), 0.0);
        assert_eq!(title_scroll_offset(6.5, 140.0), 140.0);
        assert_eq!(title_scroll_offset(12.4, 140.0), 0.0);
        for n in 0..250 {
            assert!((0.0..=140.0).contains(&title_scroll_offset(n as f64 * 0.1, 140.0)));
        }
    }
}
