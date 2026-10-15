use super::*;
use std::collections::BTreeMap;

pub(super) struct TableDrag {
    block: usize,
    start: Pos2,
    before: Option<usize>,
    revision: u64,
}

pub(super) fn show(
    ui: &mut Ui,
    layout: &Layout,
    origin: &impl Fn(usize) -> Pos2,
    scale: f32,
    editor: (u64, usize),
    drag: &mut Option<TableDrag>,
    viewing: bool,
) -> Option<(usize, usize)> {
    let (revision, caret_block) = editor;
    if std::env::var_os("ALPHADOCS_TRACE_TABLE_DRAG").is_some() {
        let events: Vec<_> = ui.input(|i| {
            i.events
                .iter()
                .filter(|e| {
                    matches!(
                        e,
                        Event::PointerMoved(_) | Event::PointerButton { .. } | Event::PointerGone
                    )
                })
                .map(|e| format!("{e:?}"))
                .collect()
        });
        if !events.is_empty() {
            eprintln!(
                "Pointer events: {events:?}; drag={:?}; canvas={:?}",
                ui.ctx().dragged_id(),
                Id::new("document_canvas")
            );
        }
    }
    if viewing {
        *drag = None;
        return None;
    }
    let mut regions: BTreeMap<(usize, usize), Rect> = BTreeMap::new();
    for object in &layout.objects {
        if let Object::Border {
            page,
            rect,
            address,
        } = object
        {
            let rect = Rect::from_min_size(
                origin(*page) + rect.min.to_vec2() * scale,
                rect.size() * scale,
            );
            regions
                .entry((address.block, *page))
                .and_modify(|r| *r = r.union(rect))
                .or_insert(rect);
        }
    }
    let mut result = None;
    for ((block, page), rect) in regions {
        let table_width = rect.width();
        if !ui.clip_rect().intersects(rect.expand(24.0)) {
            continue;
        }
        let handle = Rect::from_min_size(rect.left_top() - vec2(20.0, 20.0), vec2(18.0, 18.0));
        let pressed = pointer_gesture::pressed_in(ui, handle.expand(2.0));
        let visible = pressed.is_some()
            || caret_block == block
            || drag.as_ref().is_some_and(|d| d.block == block)
            || ui
                .input(|i| i.pointer.hover_pos())
                .is_some_and(|p| rect.expand(24.0).contains(p));
        if !visible {
            continue;
        }
        let response = ui.interact(
            handle.expand(2.0),
            Id::new(("table_move", block, page)),
            Sense::drag(),
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Drag to move table")
        });
        if std::env::var_os("ALPHADOCS_TRACE_TABLE_DRAG").is_some()
            && (response.hovered()
                || response.drag_started()
                || response.dragged()
                || response.drag_stopped())
        {
            eprintln!(
                "Table drag: block={block} handle={handle:?} hovered={} start={} dragging={} stopped={} pointer={:?} active={}",
                response.hovered(),
                response.drag_started(),
                response.dragged(),
                response.drag_stopped(),
                response.interact_pointer_pos(),
                drag.is_some()
            );
        }
        ui.painter()
            .rect_filled(handle, 3.0, ui.visuals().panel_fill);
        let c = handle.center();
        let stroke = Stroke::new(1.2, ui.visuals().text_color());
        for direction in [
            vec2(1.0, 0.0),
            vec2(-1.0, 0.0),
            vec2(0.0, 1.0),
            vec2(0.0, -1.0),
        ] {
            let tip = c + direction * 6.0;
            let side = vec2(-direction.y, direction.x) * 2.0;
            ui.painter().line_segment([c, tip], stroke);
            ui.painter()
                .line_segment([tip, tip - direction * 3.0 + side], stroke);
            ui.painter()
                .line_segment([tip, tip - direction * 3.0 - side], stroke);
        }
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
        }
        if response.drag_started() || pressed.is_some() {
            *drag = Some(TableDrag {
                block,
                start: pressed
                    .or_else(|| ui.input(|i| i.pointer.press_origin()))
                    .unwrap_or(handle.center()),
                before: None,
                revision,
            });
            if ui.input(|i| i.pointer.primary_down()) {
                ui.ctx().set_dragged_id(response.id);
            }
        }
        let completed = pointer_gesture::completed_drag(ui, handle.expand(2.0));
        if let Some((start, _)) = completed
            && drag.is_none()
        {
            *drag = Some(TableDrag {
                block,
                start,
                before: None,
                revision,
            });
        }
        if let Some(active) = drag.as_mut().filter(|d| d.block == block) {
            if active.revision != revision {
                *drag = None;
                continue;
            }
            ui.painter().rect_stroke(
                rect,
                0.0,
                Stroke::new(1.5, Color32::from_rgb(66, 133, 244)),
                egui::StrokeKind::Outside,
            );
            if let Some(pointer) = completed
                .map(|(_, end)| end)
                .or_else(|| response.interact_pointer_pos())
                .or_else(|| ui.input(|i| i.pointer.interact_pos()))
            {
                let mut targets: BTreeMap<(usize, usize), Rect> = BTreeMap::new();
                for line in &layout.lines {
                    if line.address.block == block {
                        continue;
                    }
                    let rect = Rect::from_min_size(
                        origin(line.page) + line.rect.min.to_vec2() * scale,
                        line.rect.size() * scale,
                    );
                    targets
                        .entry((line.address.block, line.page))
                        .and_modify(|r| *r = r.union(rect))
                        .or_insert(rect);
                }
                for object in &layout.objects {
                    if let Object::Image {
                        block: b,
                        page,
                        rect,
                        ..
                    } = object
                    {
                        targets.insert(
                            (*b, *page),
                            Rect::from_min_size(
                                origin(*page) + rect.min.to_vec2() * scale,
                                rect.size() * scale,
                            ),
                        );
                    }
                }
                if let Some(((target, _), rect)) = targets.into_iter().min_by(|(_, a), (_, b)| {
                    a.distance_to_pos(pointer)
                        .total_cmp(&b.distance_to_pos(pointer))
                }) {
                    let after = pointer.y > rect.center().y;
                    active.before = Some(target + usize::from(after));
                    let y = if after {
                        rect.bottom() + 4.0
                    } else {
                        rect.top() - 4.0
                    };
                    ui.painter().line_segment(
                        [pos2(rect.left(), y), pos2(rect.left() + table_width, y)],
                        Stroke::new(2.0, Color32::from_rgb(66, 133, 244)),
                    );
                }
            }
            if response.drag_stopped()
                || completed.is_some()
                || ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary))
            {
                if ui
                    .input(|i| i.pointer.interact_pos())
                    .is_some_and(|end| active.start.distance(end) >= 6.0)
                {
                    result = active.before.map(|before| (block, before));
                }
                *drag = None;
            }
        }
    }
    result
}
