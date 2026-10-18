use super::*;
use std::collections::BTreeMap;

pub(super) struct FormatDraft {
    document: String,
    tab: usize,
    revision: u64,
    caret: Position,
    anchor: Position,
    selection: Option<crate::engine::TableSelection>,
    style: BoxStyle,
    fill_enabled: bool,
    fill: [u8; 3],
    border: [u8; 3],
    width: f32,
    change_borders: bool,
    formula: String,
    decimals: Option<u8>,
    error: String,
}
impl FormatDraft {
    pub fn new(editor: &crate::engine::Editor) -> Self {
        let style = editor
            .doc
            .paragraph(editor.caret.address)
            .and_then(|p| p.box_style)
            .unwrap_or_default();
        Self {
            document: editor.doc.id.clone(),
            tab: editor.doc.active_tab,
            revision: editor.revision,
            caret: editor.caret,
            anchor: editor.anchor,
            selection: editor.table_selection(),
            style,
            fill_enabled: style.fill.is_some(),
            fill: style.fill.unwrap_or([211, 227, 253]),
            border: style.borders[0].colour,
            width: style.borders[0].width,
            change_borders: false,
            formula: editor
                .doc
                .paragraph(editor.caret.address)
                .and_then(|p| p.formula.as_ref())
                .map(|f| f.expression.clone())
                .unwrap_or_else(|| match editor.doc.blocks.get(editor.caret.address.block) {
                    Some(Block::Table(rows)) => crate::table_formula::suggested_formula(
                        rows,
                        editor.caret.address.row,
                        editor.caret.address.cell,
                    )
                    .into(),
                    _ => "=SUM(LEFT)".into(),
                }),
            decimals: editor
                .doc
                .paragraph(editor.caret.address)
                .and_then(|p| p.formula.as_ref())
                .and_then(|f| f.decimals),
            error: String::new(),
        }
    }
    fn current(&self, editor: &crate::engine::Editor) -> bool {
        self.document == editor.doc.id
            && self.tab == editor.doc.active_tab
            && self.revision == editor.revision
            && self.caret == editor.caret
            && self.anchor == editor.anchor
            && self.selection == editor.table_selection()
            && matches!(
                editor.doc.blocks.get(editor.caret.address.block),
                Some(Block::Table(_))
            )
    }
}
impl AlphaDocs {
    pub(super) fn table_formula_dialog(&mut self, ui: &mut Ui, close: &mut bool) {
        let Some(mut draft) = self.table_format_draft.take() else {
            *close = true;
            return;
        };
        ui.label(RichText::new("Table formula").size(24.0));
        ui.add_space(12.0);
        ui.label(format!(
            "Calculate in cell {}. This replaces its text; Undo restores it.",
            crate::table_formula::cell_name(draft.caret.address.row, draft.caret.address.cell)
        ));
        ui.add_space(16.0);
        ui.label("Formula");
        editable_object_ui::outlined(ui, self.palette, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut draft.formula)
                    .frame(Frame::NONE)
                    .desired_width(f32::INFINITY),
            );
        });
        ui.add_space(8.0);
        if draft.current(&self.session.editor)
            && let Some(Block::Table(rows)) = self
                .session
                .editor
                .doc
                .blocks
                .get(draft.caret.address.block)
        {
            match crate::table_formula::evaluate(
                rows,
                draft.caret.address.row,
                draft.caret.address.cell,
                &draft.formula,
            ) {
                Ok(value) => {
                    ui.label(
                        RichText::new(format!(
                            "Result: {}",
                            crate::table_formula::formatted(value, draft.decimals)
                        ))
                        .strong()
                        .color(self.palette.accent),
                    );
                }
                Err(error) => {
                    ui.label(RichText::new(error).color(self.palette.muted));
                }
            }
        }
        ui.label(
            RichText::new(
                "SUM(LEFT) adds numbers in this row. SUM(ABOVE) adds numbers in this column.",
            )
            .color(self.palette.muted)
            .size(12.0),
        );
        ui.label(RichText::new("Examples: =SUM(ABOVE), =A2*B2, =AVERAGE(A1:A4)\nSUM, PRODUCT, AVERAGE, COUNT, MIN, MAX, ABS, INT, ROUND. Use + − * / ^ and parentheses.").color(self.palette.muted).size(12.0));
        ui.add_space(12.0);
        egui::ComboBox::from_label("Number format")
            .selected_text(
                draft
                    .decimals
                    .map(|n| format!("{n} decimal places"))
                    .unwrap_or_else(|| "Automatic".into()),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut draft.decimals, None, "Automatic");
                for n in [0, 1, 2, 3, 4, 6, 8] {
                    ui.selectable_value(
                        &mut draft.decimals,
                        Some(n),
                        format!("{n} decimal places"),
                    );
                }
            });
        ui.add_space(12.0);
        ui.label(RichText::new(if self.prefs.auto_recalculate_tables {"Automatic recalculation is on. Typing into a calculated cell replaces its formula."}else{"Automatic recalculation is off. Choose Recalculate table after changing numbers, or enable it in Preferences. Typing into a calculated cell replaces its formula."}).color(self.palette.muted).size(12.0));
        if !draft.error.is_empty() {
            ui.add_space(12.0);
            ui.colored_label(self.palette.text, &draft.error);
        }
        ui.add_space(20.0);
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), 40.0),
            UiLayout::right_to_left(Align::Center),
            |ui| {
                let calculate = primary(
                    ui,
                    "Calculate",
                    self.palette,
                    draft.current(&self.session.editor)
                        && !self.session.viewing
                        && !draft.formula.trim().is_empty(),
                );
                #[cfg(test)]
                ui.ctx()
                    .data_mut(|d| d.insert_temp(Id::new("table_calculate_bounds"), calculate.rect));
                if calculate.clicked() {
                    match self.session.editor.set_table_formula(TableFormula {
                        expression: draft.formula.trim().into(),
                        decimals: draft.decimals,
                        word_number_format: None,
                    }) {
                        Ok(()) => *close = true,
                        Err(error) => draft.error = error,
                    }
                }
                if ui.button("Cancel").clicked() {
                    *close = true;
                }
            },
        );
        if !*close {
            self.table_format_draft = Some(draft);
        }
    }
    pub(super) fn table_format_dialog(&mut self, ui: &mut Ui, close: &mut bool) {
        let Some(mut draft) = self.table_format_draft.take() else {
            *close = true;
            return;
        };
        ui.label(RichText::new("Cell background and borders").size(24.0));
        ui.add_space(12.0);
        let count = self
            .session
            .editor
            .selected_table_cell_addresses()
            .len()
            .max(1);
        ui.label(if count == 1 {
            "Applies to the current cell.".into()
        } else {
            format!("Applies to {count} selected cells.")
        });
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            preferences::checkbox(
                ui,
                &mut draft.fill_enabled,
                "Background colour",
                self.palette,
            );
            ui.add_enabled_ui(draft.fill_enabled, |ui| {
                ui.color_edit_button_srgb(&mut draft.fill);
            });
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label("Border colour");
            draft.change_borders |= ui.color_edit_button_srgb(&mut draft.border).changed();
            ui.label("Width");
            draft.change_borders |= ui
                .add(
                    egui::DragValue::new(&mut draft.width)
                        .range(0.0..=12.0)
                        .speed(0.25)
                        .suffix(" pt"),
                )
                .changed();
        });
        ui.label(
            RichText::new("Use 0 pt to hide borders. Border changes apply to all four edges.")
                .color(self.palette.muted)
                .size(12.0),
        );
        ui.add_space(16.0);
        let mut style = draft.style;
        style.fill = draft.fill_enabled.then_some(draft.fill);
        if draft.change_borders {
            style.borders = [CellBorder {
                colour: draft.border,
                width: draft.width,
            }; 4];
        }
        let (preview, _) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0), Sense::hover());
        ui.painter().rect_filled(preview, 4.0, Color32::WHITE);
        paint_box(ui, preview.shrink(4.0), style, 1.0);
        ui.painter().text(
            preview.center(),
            Align2::CENTER_CENTER,
            "Cell preview",
            FontId::proportional(13.0),
            Color32::from_gray(31),
        );
        ui.add_space(20.0);
        let current = draft.current(&self.session.editor);
        if !current {
            ui.colored_label(
                self.palette.muted,
                "The document changed. Reopen cell formatting to apply changes.",
            );
        }
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), 40.0),
            UiLayout::right_to_left(Align::Center),
            |ui| {
                let apply = primary(
                    ui,
                    "Apply",
                    self.palette,
                    current && style.valid() && !self.session.viewing,
                );
                tools::bounds(ui, "cell_format_apply", &apply);
                if apply.clicked() {
                    self.session.editor.paragraph_format(|p| {
                        let mut target = p.box_style.unwrap_or_default();
                        target.fill = style.fill;
                        if draft.change_borders {
                            target.borders = style.borders;
                        }
                        p.box_style = Some(target);
                    });
                    *close = true;
                }
                if ui.button("Cancel").clicked() {
                    *close = true;
                }
            },
        );
        if !*close {
            self.table_format_draft = Some(draft);
        }
    }
}

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

#[cfg(test)]
mod format_tests {
    use super::*;
    fn frame(ctx: &Context, app: &mut AlphaDocs, events: Vec<Event>) {
        app.last_autosave = Instant::now();
        let mut frame = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1100.0, 760.0))),
                events,
                ..Default::default()
            },
            |ui| app.update_ui(ui),
        );
        frame.textures_delta.clear();
    }
    #[test]
    fn row_background_apply_preserves_each_cells_edges_and_is_one_undo_in_both_themes() {
        for theme in [Theme::Light, Theme::Dark] {
            let ctx = Context::default();
            fonts::install(&ctx);
            let mut app = AlphaDocs::fresh(
                &ctx,
                Preferences {
                    theme,
                    ..Default::default()
                },
                vec![],
            );
            app.home = false;
            app.session.editor.insert_table(2, 2);
            let block = app.session.editor.caret.address.block;
            let Block::Table(rows) = &mut app.session.editor.doc.blocks[block] else {
                panic!()
            };
            rows[0][0].box_style = Some(BoxStyle {
                borders: [CellBorder {
                    colour: [200, 10, 10],
                    width: 2.0,
                }; 4],
                ..Default::default()
            });
            rows[0][1].box_style = Some(BoxStyle {
                borders: [CellBorder {
                    colour: [10, 10, 200],
                    width: 0.25,
                }; 4],
                ..Default::default()
            });
            app.session.editor.select_table_row(block, 0);
            let before = app.session.editor.doc.clone();
            app.table_format_draft = Some(FormatDraft::new(&app.session.editor));
            let draft = app.table_format_draft.as_mut().unwrap();
            draft.fill_enabled = true;
            draft.fill = [211, 227, 253];
            app.dialog = Some(Dialog::TableFormat);
            frame(&ctx, &mut app, vec![]);
            frame(&ctx, &mut app, vec![]);
            let pos = ctx
                .data(|d| d.get_temp::<Rect>(Id::new("cell_format_apply")))
                .unwrap()
                .center();
            frame(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            let Block::Table(rows) = &app.session.editor.doc.blocks[block] else {
                panic!()
            };
            assert_eq!(rows[0][0].box_style.unwrap().fill, Some([211, 227, 253]));
            assert_eq!(rows[0][1].box_style.unwrap().fill, Some([211, 227, 253]));
            assert_eq!(rows[0][0].box_style.unwrap().borders[0].width, 2.0);
            assert_eq!(rows[0][1].box_style.unwrap().borders[0].width, 0.25);
            assert!(rows[1][0].box_style.is_none());
            app.session.editor.undo();
            assert_eq!(app.session.editor.doc, before);
        }
    }
}
