use super::*;

fn hit_rect(pos: Pos2, height: f32, scale: f32) -> Rect {
    Rect::from_center_size(
        pos + vec2(-11.0 * scale, height * 0.5),
        vec2(18.0 * scale, height),
    )
}

pub(super) fn hit(
    layout: &Layout,
    origin: &impl Fn(usize) -> Pos2,
    scale: f32,
    pointer: Pos2,
) -> bool {
    layout.lines.iter().chain(&layout.running).any(|line| {
        line.marker.as_deref().is_some_and(|m| m == "☐" || m == "☑")
            && hit_rect(
                origin(line.page) + line.rect.min.to_vec2() * scale,
                line.rect.height() * scale,
                scale,
            )
            .contains(pointer)
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn checkbox(
    ui: &mut Ui,
    address: Address,
    pos: Pos2,
    height: f32,
    scale: f32,
    checked: bool,
    enabled: bool,
    p: Palette,
) -> egui::Response {
    let hit = hit_rect(pos, height, scale);
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            ui.interact(hit, Id::new(("check", address)), Sense::click())
        })
        .inner;
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            enabled,
            checked,
            if checked {
                "Completed task"
            } else {
                "Incomplete task"
            },
        )
    });
    let ratio = layout::checklist_mark_size(height / scale) / 15.0;
    let mark_scale = ratio * scale;
    let square = Rect::from_center_size(hit.center(), vec2(15.0, 15.0) * mark_scale);
    ui.painter().rect(
        square,
        3.0 * mark_scale,
        if checked {
            p.accent
        } else {
            Color32::TRANSPARENT
        },
        Stroke::new(
            1.4 * mark_scale,
            if checked || response.hovered() {
                p.accent
            } else {
                p.muted
            },
        ),
        egui::StrokeKind::Inside,
    );
    if checked {
        let ink = if p.dark { p.chrome } else { Color32::WHITE };
        let c = square.center();
        ui.painter().line_segment(
            [
                c + vec2(-4.0, 0.0) * mark_scale,
                c + vec2(-1.0, 3.0) * mark_scale,
            ],
            Stroke::new(1.7 * mark_scale, ink),
        );
        ui.painter().line_segment(
            [
                c + vec2(-1.0, 3.0) * mark_scale,
                c + vec2(4.5, -3.5) * mark_scale,
            ],
            Stroke::new(1.7 * mark_scale, ink),
        );
    }
    #[cfg(test)]
    ui.ctx()
        .data_mut(|d| d.insert_temp(Id::new(("checklist_bounds", address)), hit));
    response.on_hover_text(if checked {
        "Mark incomplete"
    } else {
        "Mark complete"
    })
}
