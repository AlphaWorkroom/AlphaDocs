use super::*;

pub(super) struct ResizeDrag {
    block: usize,
    handle: usize,
    pointer: Pos2,
    original: egui::Vec2,
    pub preview: egui::Vec2,
}
#[derive(Default)]
pub(super) struct ImageAction {
    pub size: Option<egui::Vec2>,
    pub options: bool,
    pub delete: bool,
    pub flow: Option<ImageFlow>,
    pub comment: bool,
    pub reaction: bool,
}
pub(super) struct SelectionBounds {
    pub rect: Rect,
    pub moving: bool,
}
pub(super) fn preview_rect(
    rect: Rect,
    block: usize,
    image: &EmbeddedImage,
    drag: &Option<ResizeDrag>,
) -> Rect {
    if let Some(drag) = drag.as_ref().filter(|d| d.block == block) {
        Rect::from_min_size(
            rect.min,
            rect.size() * drag.preview / vec2(image.width, image.height),
        )
    } else {
        rect
    }
}

/// Canvas adornments stay in logical pixels while their image dimensions use
/// document points. A drag previews locally and commits one undo transaction.
pub(super) fn show(
    ui: &mut Ui,
    block: usize,
    bounds: SelectionBounds,
    image: &EmbeddedImage,
    drag: &mut Option<ResizeDrag>,
    viewing: bool,
    palette: Palette,
) -> ImageAction {
    let rect = bounds.rect;
    let blue = Color32::from_rgb(66, 133, 244);
    let mut action = ImageAction::default();
    ui.painter()
        .rect_stroke(rect, 0.0, Stroke::new(1.5, blue), egui::StrokeKind::Outside);
    let handles = [
        rect.left_top(),
        rect.center_top(),
        rect.right_top(),
        rect.right_center(),
        rect.right_bottom(),
        rect.center_bottom(),
        rect.left_bottom(),
        rect.left_center(),
    ];
    for (index, center) in handles.into_iter().enumerate() {
        let handle = Rect::from_center_size(center, vec2(8.0, 8.0));
        ui.painter()
            .rect_filled(handle.expand(1.0), 0.0, Color32::WHITE);
        ui.painter().rect_filled(handle, 0.0, blue);
        if viewing {
            continue;
        }
        let region = resize_region(rect, index);
        let response = ui.interact(
            region,
            Id::new(("image_resize", block, index)),
            Sense::drag(),
        );
        if let Some((start, end)) = pointer_gesture::completed_drag(ui, region) {
            let scale = rect.width() / image.width;
            action.size = Some(resized(
                vec2(image.width, image.height),
                index,
                (end - start) / scale,
                ui.input(|i| i.modifiers.shift),
            ));
            *drag = None;
            continue;
        }
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Resize image")
        });
        let cursor = match index {
            0 | 4 => egui::CursorIcon::ResizeNwSe,
            2 | 6 => egui::CursorIcon::ResizeNeSw,
            1 | 5 => egui::CursorIcon::ResizeVertical,
            _ => egui::CursorIcon::ResizeHorizontal,
        };
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(cursor);
        }
        let pressed = pointer_gesture::pressed_in(ui, region);
        if response.drag_started() || pressed.is_some() {
            *drag = Some(ResizeDrag {
                block,
                handle: index,
                pointer: pressed
                    .or_else(|| ui.input(|i| i.pointer.press_origin()))
                    .unwrap_or(center),
                original: vec2(image.width, image.height),
                preview: vec2(image.width, image.height),
            });
            if ui.input(|i| i.pointer.primary_down()) {
                ui.ctx().set_dragged_id(response.id);
            }
        }
        if let Some(current) = drag
            .as_mut()
            .filter(|d| d.block == block && d.handle == index)
        {
            if let Some(pointer) = response
                .interact_pointer_pos()
                .or_else(|| ui.input(|i| i.pointer.interact_pos()))
            {
                let scale = rect.width() / current.preview.x;
                let delta = (pointer - current.pointer) / scale;
                current.preview = resized(
                    current.original,
                    index,
                    delta,
                    ui.input(|i| i.modifiers.shift),
                );
            }
            if response.drag_stopped()
                || ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary))
            {
                if current.preview != current.original {
                    action.size = Some(current.preview);
                }
                *drag = None;
            }
        }
    }
    if drag.is_none() && !bounds.moving {
        let position = pos2(
            rect.left(),
            (rect.bottom() + 12.0).min(ui.clip_rect().bottom() - 48.0),
        );
        egui::Area::new(Id::new(("image_toolbar", block)))
            .order(egui::Order::Foreground)
            .fixed_pos(position)
            .show(ui.ctx(), |ui| {
                Frame::new()
                    .fill(palette.chrome)
                    .stroke(Stroke::new(0.7, palette.border))
                    .corner_radius(20)
                    .inner_margin(Margin::symmetric(8, 4))
                    .shadow(egui::Shadow {
                        offset: [0, 2],
                        blur: 8,
                        spread: 0,
                        color: Color32::from_black_alpha(35),
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let wrap = icons::button(
                                ui,
                                Icon::Wrap,
                                image.placement.flow.label(),
                                false,
                                !viewing,
                            );
                            egui::Popup::menu(&wrap).show(|ui| {
                                flow_menu(ui, image.placement.flow, &mut action, !viewing);
                            });
                            if icons::button_with_id(
                                ui,
                                Id::new(("image_options", block)),
                                Icon::Paintbrush,
                                "Image options",
                                false,
                                true,
                            )
                            .clicked()
                            {
                                action.options = true;
                            }
                            ui.separator();
                            if icons::button_with_id(
                                ui,
                                Id::new(("image_comment", block)),
                                Icon::Comment,
                                "Add comment",
                                false,
                                !viewing,
                            )
                            .clicked()
                            {
                                action.comment = true;
                            }
                            if icons::button_with_id(
                                ui,
                                Id::new(("image_reaction", block)),
                                Icon::Reaction,
                                "Add emoji reaction",
                                false,
                                !viewing,
                            )
                            .clicked()
                            {
                                action.reaction = true;
                            }
                        });
                    });
            });
    }
    action
}
fn resize_region(rect: Rect, handle: usize) -> Rect {
    let corners = [
        rect.left_top(),
        rect.center_top(),
        rect.right_top(),
        rect.right_center(),
        rect.right_bottom(),
        rect.center_bottom(),
        rect.left_bottom(),
        rect.left_center(),
    ];
    if handle.is_multiple_of(2) || rect.width() < 24.0 || rect.height() < 24.0 {
        return Rect::from_center_size(corners[handle], vec2(18.0, 18.0));
    }
    match handle {
        1 => Rect::from_min_max(
            pos2(rect.left() + 9.0, rect.top() - 6.0),
            pos2(rect.right() - 9.0, rect.top() + 6.0),
        ),
        3 => Rect::from_min_max(
            pos2(rect.right() - 6.0, rect.top() + 9.0),
            pos2(rect.right() + 6.0, rect.bottom() - 9.0),
        ),
        5 => Rect::from_min_max(
            pos2(rect.left() + 9.0, rect.bottom() - 6.0),
            pos2(rect.right() - 9.0, rect.bottom() + 6.0),
        ),
        _ => Rect::from_min_max(
            pos2(rect.left() - 6.0, rect.top() + 9.0),
            pos2(rect.left() + 6.0, rect.bottom() - 9.0),
        ),
    }
}
pub(super) fn flow_menu(ui: &mut Ui, current: ImageFlow, action: &mut ImageAction, enabled: bool) {
    ui.set_min_width(178.0);
    ui.add_enabled(false, egui::Button::new("Inline with text"))
        .on_disabled_hover_text("Inline images within text runs are not implemented yet.");
    for flow in [
        ImageFlow::Wrap,
        ImageFlow::Break,
        ImageFlow::Behind,
        ImageFlow::Front,
    ] {
        let response = ui.add_enabled(
            enabled,
            egui::Button::new(flow.label()).selected(flow == current),
        );
        if flow == current {
            let c = response.rect.right_center() - vec2(12.0, 0.0);
            ui.painter().line_segment(
                [c - vec2(4.0, 0.0), c + vec2(-1.0, 3.0)],
                Stroke::new(1.5, ui.visuals().text_color()),
            );
            ui.painter().line_segment(
                [c + vec2(-1.0, 3.0), c + vec2(5.0, -4.0)],
                Stroke::new(1.5, ui.visuals().text_color()),
            );
        }
        if response.clicked() {
            action.flow = Some(flow);
            ui.close();
        }
    }
}

fn resized(original: egui::Vec2, handle: usize, delta: egui::Vec2, stretch: bool) -> egui::Vec2 {
    let horizontal = match handle {
        0 | 6 | 7 => -delta.x,
        2..=4 => delta.x,
        _ => 0.0,
    };
    let vertical = match handle {
        0..=2 => -delta.y,
        4..=6 => delta.y,
        _ => 0.0,
    };
    let mut size = original + vec2(horizontal, vertical);
    if handle.is_multiple_of(2) && !stretch {
        let factor = if (horizontal / original.x).abs() > (vertical / original.y).abs() {
            size.x / original.x
        } else {
            size.y / original.y
        };
        size = original * factor.clamp(10.0 / original.min_elem(), 1200.0 / original.max_elem());
    }
    size.clamp(vec2(10.0, 10.0), vec2(1200.0, 1200.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corners_preserve_ratio_and_side_handles_change_one_dimension() {
        let size = vec2(300.0, 200.0);
        for corner in [0, 2, 4, 6] {
            let result = resized(size, corner, vec2(60.0, 10.0), false);
            assert!((result.x / result.y - 1.5).abs() < 0.001);
        }
        assert_eq!(
            resized(size, 3, vec2(40.0, 50.0), false),
            vec2(340.0, 200.0)
        );
        assert_eq!(
            resized(size, 5, vec2(40.0, 50.0), false),
            vec2(300.0, 250.0)
        );
        assert_eq!(resized(size, 4, vec2(40.0, 50.0), true), vec2(340.0, 250.0));
    }
}
