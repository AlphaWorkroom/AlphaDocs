use super::*;
pub(super) fn clicked_in(ui: &Ui, rect: Rect) -> bool {
    let point = ui.input(|i| {
        i.pointer
            .primary_clicked()
            .then(|| i.pointer.interact_pos())
            .flatten()
    });
    point.is_some_and(|pos| {
        rect.contains(pos)
            && ui.clip_rect().contains(pos)
            && ui
                .ctx()
                .layer_id_at(pos)
                .is_none_or(|layer| layer == ui.layer_id())
    })
}

pub(super) fn secondary_clicked_in(ui: &Ui, rect: Rect) -> bool {
    let point = ui.input(|i| {
        i.pointer
            .secondary_clicked()
            .then(|| i.pointer.interact_pos())
            .flatten()
    });
    point.is_some_and(|pos| {
        rect.contains(pos)
            && ui.clip_rect().contains(pos)
            && ui
                .ctx()
                .layer_id_at(pos)
                .is_none_or(|layer| layer == ui.layer_id())
    })
}

pub(super) fn pressed_in(ui: &Ui, rect: Rect) -> Option<Pos2> {
    ui.input(|i| {
        i.events.iter().find_map(|event| match event {
            Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                ..
            } if rect.contains(*pos) && ui.clip_rect().contains(*pos) => Some(*pos),
            _ => None,
        })
    })
    .filter(|pos| {
        ui.ctx()
            .layer_id_at(*pos)
            .is_none_or(|layer| layer == ui.layer_id())
    })
}

/// A fast gesture may arrive entirely in one pass. Toolkit drag responses are
/// based on the final pointer state, so preserve the press hit and release point.
pub(super) fn completed_drag(ui: &Ui, rect: Rect) -> Option<(Pos2, Pos2)> {
    let events = ui.input(|i| i.events.clone());
    let mut start = None;
    for event in events {
        match event {
            Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                ..
            } if rect.contains(pos)
                && ui.clip_rect().contains(pos)
                && ui
                    .ctx()
                    .layer_id_at(pos)
                    .is_none_or(|layer| layer == ui.layer_id()) =>
            {
                start = Some(pos);
            }
            Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                ..
            } => {
                if let Some(start) = start
                    && start.distance(pos) >= 6.0
                {
                    return Some((start, pos));
                }
                start = None;
            }
            Event::PointerGone => start = None,
            _ => {}
        }
    }
    None
}
