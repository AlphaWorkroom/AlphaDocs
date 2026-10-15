use super::*;

pub(super) struct ImageDrag {
    block: usize,
    revision: u64,
    start: Pos2,
    pub delta: egui::Vec2,
}
pub(super) enum Move {
    Block(usize, usize, ImagePlacement),
    Text(usize, Position, ImagePlacement),
    Position(usize, ImagePlacement),
}
pub(super) fn preview_rect(rect: Rect, block: usize, drag: &Option<ImageDrag>) -> Rect {
    drag.as_ref()
        .filter(|d| d.block == block && d.delta.length() >= 6.0)
        .map_or(rect, |d| rect.translate(d.delta))
}
pub(super) fn hit_image(
    layout: &Layout,
    doc: &Document,
    origin: &impl Fn(usize) -> Pos2,
    scale: f32,
    pointer: Pos2,
    alt: bool,
) -> Option<usize> {
    let mut hits = vec![];
    for object in &layout.objects {
        let Object::Image {
            block, page, rect, ..
        } = object
        else {
            continue;
        };
        let Block::Image(image) = &doc.blocks[*block] else {
            continue;
        };
        if !Rect::from_min_size(
            origin(*page) + rect.min.to_vec2() * scale,
            rect.size() * scale,
        )
        .contains(pointer)
        {
            continue;
        }
        if image.placement.flow == ImageFlow::Behind
            && !alt
            && layout.lines.iter().any(|line| {
                line.page == *page
                    && doc
                        .paragraph(line.address)
                        .is_some_and(|p| !p.chars.is_empty())
                    && Rect::from_min_size(
                        origin(line.page) + line.rect.min.to_vec2() * scale,
                        line.rect.size() * scale,
                    )
                    .contains(pointer)
            })
        {
            continue;
        }
        hits.push((image.placement.flow == ImageFlow::Front, *block));
    }
    hits.into_iter().max().map(|(_, block)| block)
}

pub(super) fn show(
    ui: &mut Ui,
    layout: &Layout,
    origin: &impl Fn(usize) -> Pos2,
    scale: f32,
    editor: &crate::engine::Editor,
    drag: &mut Option<ImageDrag>,
    viewing: bool,
) -> Option<Move> {
    if viewing {
        *drag = None;
        return None;
    }
    for object in &layout.objects {
        let Object::Image {
            block,
            page,
            rect,
            anchor,
        } = object
        else {
            continue;
        };
        let Some(Block::Image(image)) = editor.doc.blocks.get(*block) else {
            continue;
        };
        let screen = Rect::from_min_size(
            origin(*page) + rect.min.to_vec2() * scale,
            rect.size() * scale,
        );
        let body = screen.shrink(12.0);
        if image.placement.flow == ImageFlow::Behind
            && drag.is_none()
            && !ui.input(|i| i.modifiers.alt)
            && ui
                .input(|i| i.pointer.interact_pos())
                .is_some_and(|pointer| {
                    layout.lines.iter().any(|line| {
                        line.page == *page
                            && !editor
                                .doc
                                .paragraph(line.address)
                                .is_some_and(|p| p.chars.is_empty())
                            && Rect::from_min_size(
                                origin(line.page) + line.rect.min.to_vec2() * scale,
                                line.rect.size() * scale,
                            )
                            .contains(pointer)
                    })
                })
        {
            continue;
        }
        let completed = pointer_gesture::completed_drag(ui, body);
        let pressed = pointer_gesture::pressed_in(ui, body);
        if let Some(start) = completed.map(|(start, _)| start).or(pressed)
            && hit_image(
                layout,
                &editor.doc,
                origin,
                scale,
                start,
                ui.input(|i| i.modifiers.alt),
            ) != Some(*block)
        {
            continue;
        }
        let id = Id::new(("image_move", *block));
        let response = ui.interact(body, id, Sense::drag());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Drag to move image")
        });
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
        }
        if let Some(start) = completed.map(|(start, _)| start).or(pressed) {
            *drag = Some(ImageDrag {
                block: *block,
                revision: editor.revision,
                start,
                delta: egui::Vec2::ZERO,
            });
            if ui.input(|i| i.pointer.primary_down()) {
                ui.ctx().set_dragged_id(id);
            }
        }
        let Some(active) = drag.as_mut().filter(|d| d.block == *block) else {
            continue;
        };
        if active.revision != editor.revision
            || ui.input(|i| i.events.contains(&Event::PointerGone) || i.key_pressed(Key::Escape))
        {
            *drag = None;
            continue;
        }
        let Some(pointer) = completed
            .map(|(_, end)| end)
            .or_else(|| ui.input(|i| i.pointer.interact_pos()))
        else {
            continue;
        };
        active.delta = pointer - active.start;
        let mut before = None;
        let mut text_target = None;
        if image.placement.flow == ImageFlow::Break {
            let mut targets = vec![];
            for line in &layout.lines {
                if !matches!(
                    editor.doc.blocks.get(line.address.block),
                    Some(Block::Paragraph(_))
                ) {
                    continue;
                }
                let r = Rect::from_min_size(
                    origin(line.page) + line.rect.min.to_vec2() * scale,
                    line.rect.size() * scale,
                );
                targets.push((line.address.block, Some(line), r));
            }
            for object in &layout.objects {
                if let Object::Image {
                    block: other,
                    page,
                    rect,
                    ..
                } = object
                    && other != block
                {
                    targets.push((
                        *other,
                        None,
                        Rect::from_min_size(
                            origin(*page) + rect.min.to_vec2() * scale,
                            rect.size() * scale,
                        ),
                    ));
                }
            }
            if let Some((target, line, rect)) =
                targets.into_iter().min_by(|(_, _, a), (_, _, b)| {
                    a.distance_to_pos(pointer)
                        .total_cmp(&b.distance_to_pos(pointer))
                })
            {
                let after = pointer.y > rect.center().y;
                if let Some(line) = line.filter(|line| {
                    editor
                        .doc
                        .paragraph(line.address)
                        .is_some_and(|p| !p.chars.is_empty())
                }) {
                    text_target = Some(Position {
                        address: line.address,
                        offset: if after { line.end } else { line.start },
                    });
                } else {
                    before = Some(target + usize::from(after));
                }
                let y = if after {
                    rect.bottom() + 4.0
                } else {
                    rect.top() - 4.0
                };
                ui.painter().line_segment(
                    [pos2(rect.left(), y), pos2(rect.left() + screen.width(), y)],
                    Stroke::new(2.0, Color32::from_rgb(66, 133, 244)),
                );
            }
        } else {
            if active.delta.length() >= 6.0 {
                let setup = &editor.doc.page;
                let count = setup.columns.max(1) as f32;
                let width = (layout.size.x
                    - setup.margins[2]
                    - setup.margins[3]
                    - setup.column_gap * (count - 1.0))
                    / count;
                let column = ((rect.left() - setup.margins[2]) / (width + setup.column_gap))
                    .floor()
                    .clamp(0.0, count - 1.0);
                let left = setup.margins[2] + column * (width + setup.column_gap);
                let bounds = Rect::from_min_max(
                    origin(*page) + vec2(left, setup.margins[0]) * scale,
                    origin(*page) + vec2(left + width, layout.size.y - setup.margins[1]) * scale,
                );
                let paper = Rect::from_min_size(origin(*page), layout.size * scale);
                let targets = image_guides::Targets::collect(
                    layout,
                    *block,
                    *page,
                    bounds,
                    paper,
                    origin(*page),
                    scale,
                );
                let result =
                    targets.resolve(screen.translate(active.delta), bounds, image.placement);
                active.delta = result.rect.min - screen.min;
                result.paint(ui, paper);
            }
            ui.painter().rect_stroke(
                screen.translate(active.delta),
                0.0,
                Stroke::new(1.5, Color32::from_rgb(66, 133, 244)),
                egui::StrokeKind::Outside,
            );
        }
        if completed.is_some()
            || ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary))
        {
            let mut placement = image.placement;
            let columns = editor.doc.page.columns.max(1) as f32;
            let content = (layout.size.x
                - editor.doc.page.margins[2]
                - editor.doc.page.margins[3]
                - editor.doc.page.column_gap * (columns - 1.0))
                / columns;
            placement.offset[0] = (rect.left() - anchor.x + active.delta.x / scale)
                .clamp(0.0, (content - rect.width()).max(0.0));
            let result = if active.delta.length() < 6.0 {
                None
            } else if image.placement.flow == ImageFlow::Break {
                text_target
                    .map(|target| Move::Text(*block, target, placement))
                    .or_else(|| before.map(|before| Move::Block(*block, before, placement)))
            } else {
                placement.offset[1] =
                    (rect.top() - anchor.y + active.delta.y / scale).clamp(-2400.0, 2400.0);
                Some(Move::Position(*block, placement))
            };
            *drag = None;
            return result;
        }
    }
    None
}
