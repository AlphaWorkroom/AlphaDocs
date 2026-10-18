use super::*;

fn app_with_blocks(blocks: Vec<Block>) -> (Context, AlphaDocs) {
    let context = Context::default();
    context.enable_accesskit();
    fonts::install(&context);
    let mut app = AlphaDocs::fresh(&context, Preferences::default(), vec![]);
    app.home = false;
    app.prefs.outline = false;
    app.prefs.ruler = false;
    app.prefs.counts = false;
    let mut document = Document::new("Checklist and table UI tests");
    document.blocks = blocks;
    app.session.editor = crate::engine::Editor::new(document);
    (context, app)
}

fn frame(context: &Context, app: &mut AlphaDocs, events: Vec<Event>) {
    app.last_autosave = Instant::now();
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 1000.0))),
            events,
            ..Default::default()
        },
        |ui| app.update_ui(ui),
    );
    output.textures_delta.clear();
}

fn settle(context: &Context, app: &mut AlphaDocs) {
    frame(context, app, vec![]);
    frame(context, app, vec![]);
}

fn bounds(context: &Context, id: impl std::hash::Hash + std::fmt::Debug) -> Rect {
    context
        .data_mut(|data| data.get_temp::<Rect>(Id::new(id)))
        .expect("settled UI frame recorded response bounds")
}

fn click(context: &Context, app: &mut AlphaDocs, rect: Rect) {
    let pos = rect.center();
    frame(
        context,
        app,
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
}

fn key(key: Key, modifiers: egui::Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn checklist_blocks() -> Vec<Block> {
    vec![
        Block::Paragraph(Paragraph::from_text("first task")),
        Block::Paragraph(Paragraph::from_text("second task")),
    ]
}

#[test]
fn checklist_toolbar_checkbox_undo_and_readonly_work_in_light_and_dark_frames() {
    for theme in [Theme::Light, Theme::Dark] {
        let (context, mut app) = app_with_blocks(checklist_blocks());
        app.prefs.theme = theme;
        app.session.editor.select_all();
        let original_caret = app.session.editor.caret;
        let original_anchor = app.session.editor.anchor;
        settle(&context, &mut app);

        click(&context, &mut app, bounds(&context, "checklist_toolbar"));
        for block in 0..2 {
            assert_eq!(
                app.session
                    .editor
                    .doc
                    .paragraph(Address {
                        block,
                        ..Address::default()
                    })
                    .unwrap()
                    .list,
                ListKind::Check,
                "toolbar must mark selected paragraphs in {theme:?}"
            );
        }
        settle(&context, &mut app);
        let item = Address::default();
        let first = bounds(&context, ("checklist_bounds", item));
        let next = bounds(
            &context,
            (
                "checklist_bounds",
                Address {
                    block: 1,
                    ..Default::default()
                },
            ),
        );
        assert!(
            first.bottom() <= next.top(),
            "adjacent checklist hit regions must not overlap in {theme:?}: {first:?} {next:?}"
        );
        click(
            &context,
            &mut app,
            bounds(&context, ("checklist_bounds", item)),
        );
        assert!(
            app.session.editor.doc.paragraph(item).unwrap().checked,
            "vector checkbox click must toggle completion in {theme:?}"
        );
        assert_eq!(app.session.editor.caret, original_caret);
        assert_eq!(app.session.editor.anchor, original_anchor);
        frame(
            &context,
            &mut app,
            vec![key(Key::Z, egui::Modifiers::COMMAND)],
        );
        assert!(!app.session.editor.doc.paragraph(item).unwrap().checked);
        assert_eq!(app.session.editor.caret, original_caret);
        frame(
            &context,
            &mut app,
            vec![key(Key::Z, egui::Modifiers::COMMAND)],
        );
        assert!(
            app.session
                .editor
                .doc
                .paragraph(item)
                .is_some_and(|paragraph| paragraph.list == ListKind::None)
        );

        let (context, mut app) = app_with_blocks(checklist_blocks());
        app.prefs.theme = theme;
        app.session.viewing = true;
        settle(&context, &mut app);
        click(&context, &mut app, bounds(&context, "checklist_toolbar"));
        assert!(
            app.session.editor.doc.addresses().iter().all(|address| app
                .session
                .editor
                .doc
                .paragraph(*address)
                .is_some_and(|paragraph| paragraph.list == ListKind::None)),
            "read-only checklist toolbar must not mutate in {theme:?}"
        );
    }
}

fn table_blocks() -> Vec<Block> {
    vec![Block::Table(vec![
        vec![
            Paragraph::from_text("A"),
            Paragraph::from_text("B"),
            Paragraph::from_text("C"),
        ],
        vec![
            Paragraph::from_text("D"),
            Paragraph::from_text("E"),
            Paragraph::from_text("F"),
        ],
    ])]
}

fn cell_center(context: &Context, app: &AlphaDocs, row: usize, cell: usize) -> Pos2 {
    let canvas = context
        .read_response(Id::new("document_canvas"))
        .expect("settled full UI has a document canvas")
        .rect;
    let layout = app.layout.as_ref().expect("settled UI has measured layout");
    let scale = 4.0 / 3.0 * app.prefs.zoom;
    let width = layout.size.x * scale;
    let left = (canvas.left() + (canvas.width() - width) * 0.5).max(canvas.left() + 24.0);
    let object = layout
        .objects
        .iter()
        .find_map(|object| match object {
            Object::Border {
                page: 0,
                address,
                rect,
            } if address.block == 0 && address.row == row && address.cell == cell => Some(*rect),
            _ => None,
        })
        .expect("selected table cell has a measured border");
    pos2(
        left + object.center().x * scale,
        canvas.top() + 12.0 + object.center().y * scale,
    )
}

fn context_click_cell(context: &Context, app: &mut AlphaDocs, row: usize, cell: usize) {
    let pos = cell_center(context, app, row, cell);
    frame(
        context,
        app,
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: egui::PointerButton::Secondary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    frame(
        context,
        app,
        vec![Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(
        egui::Popup::is_any_open(context),
        "table context menu opened"
    );
    settle(context, app);
}

fn choose_context_command(context: &Context, app: &mut AlphaDocs, label: &str) {
    click(context, app, bounds(context, ("menu_bounds", label)));
    settle(context, app);
}

#[test]
fn table_context_row_and_column_selection_copy_format_clear_and_undo_use_full_frames() {
    let (context, mut app) = app_with_blocks(table_blocks());
    settle(&context, &mut app);

    context_click_cell(&context, &mut app, 1, 1);
    choose_context_command(&context, &mut app, "Select row");
    let row_addresses = app.session.editor.selected_table_cell_addresses();
    assert_eq!(row_addresses.len(), 3);
    assert!(row_addresses.iter().all(|address| address.row == 1));
    assert_eq!(app.session.editor.selected_text(), "D\tE\tF");
    let row_clip = app.session.editor.copy_fragment();
    assert_eq!(row_clip.text, "D\tE\tF");
    let [Block::Table(rows)] = row_clip.blocks.as_slice() else {
        panic!("row copy should produce a native table fragment")
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].iter().map(Paragraph::text).collect::<Vec<_>>(),
        ["D", "E", "F"]
    );

    frame(
        &context,
        &mut app,
        vec![key(Key::B, egui::Modifiers::COMMAND)],
    );
    assert!(
        app.session
            .editor
            .selected_table_cell_addresses()
            .iter()
            .all(|address| app
                .session
                .editor
                .doc
                .paragraph(*address)
                .unwrap()
                .chars
                .iter()
                .all(|character| character.style.bold)),
        "formatting should affect every cell and preserve the selection"
    );
    assert_eq!(app.session.editor.selected_table_cell_addresses().len(), 3);

    frame(
        &context,
        &mut app,
        vec![key(Key::Delete, egui::Modifiers::NONE)],
    );
    for cell in 0..3 {
        assert_eq!(
            app.session
                .editor
                .doc
                .paragraph(Address {
                    block: 0,
                    row: 1,
                    cell,
                })
                .unwrap()
                .text(),
            ""
        );
    }
    assert_eq!(
        app.session
            .editor
            .doc
            .paragraph(Address {
                block: 0,
                row: 0,
                cell: 0,
            })
            .unwrap()
            .text(),
        "A"
    );
    frame(
        &context,
        &mut app,
        vec![key(Key::Z, egui::Modifiers::COMMAND)],
    );
    assert_eq!(
        app.session
            .editor
            .doc
            .paragraph(Address {
                block: 0,
                row: 1,
                cell: 0,
            })
            .unwrap()
            .text(),
        "D"
    );

    context_click_cell(&context, &mut app, 0, 2);
    choose_context_command(&context, &mut app, "Select column");
    let column_addresses = app.session.editor.selected_table_cell_addresses();
    assert_eq!(column_addresses.len(), 2);
    assert!(column_addresses.iter().all(|address| address.cell == 2));
    assert_eq!(
        app.session.editor.table_selection(),
        Some(crate::engine::TableSelection::Column {
            block: 0,
            column: 2,
        })
    );
    assert!(context.memory(|memory| memory.has_focus(Id::new("document_canvas"))));
    assert_eq!(app.session.editor.selected_text(), "C\nF");
    let column_clip = app.session.editor.copy_fragment();
    assert_eq!(column_clip.text, "C\nF");
    frame(
        &context,
        &mut app,
        vec![key(Key::Backspace, egui::Modifiers::NONE)],
    );
    for row in 0..2 {
        assert_eq!(
            app.session
                .editor
                .doc
                .paragraph(Address {
                    block: 0,
                    row,
                    cell: 2,
                })
                .unwrap()
                .text(),
            ""
        );
    }
    frame(
        &context,
        &mut app,
        vec![key(Key::Z, egui::Modifiers::COMMAND)],
    );
    assert_eq!(
        app.session
            .editor
            .doc
            .paragraph(Address {
                block: 0,
                row: 0,
                cell: 2,
            })
            .unwrap()
            .text(),
        "C"
    );

    app.session.viewing = true;
    context_click_cell(&context, &mut app, 0, 1);
    choose_context_command(&context, &mut app, "Select row");
    frame(
        &context,
        &mut app,
        vec![key(Key::Delete, egui::Modifiers::NONE)],
    );
    assert_eq!(
        app.session
            .editor
            .doc
            .paragraph(Address {
                block: 0,
                row: 0,
                cell: 1,
            })
            .unwrap()
            .text(),
        "B",
        "read-only table selection cannot clear document contents"
    );
}

#[test]
fn table_cell_selection_does_not_create_zero_length_text_annotations() {
    let (context, mut app) = app_with_blocks(table_blocks());
    settle(&context, &mut app);
    context_click_cell(&context, &mut app, 1, 1);
    choose_context_command(&context, &mut app, "Select row");
    assert!(app.session.editor.selected());
    assert!(!app.session.editor.has_scalar_selection());

    app.begin_comment();
    assert!(!app.comments_open);
    assert!(app.text_comment_range.is_none());

    app.execute(Action::Reaction, &context);
    assert!(app.text_reaction_range.is_none());
    assert!(app.dialog.is_none());
    assert!(
        app.session
            .editor
            .doc
            .addresses()
            .into_iter()
            .all(|address| app
                .session
                .editor
                .doc
                .paragraph(address)
                .is_none_or(|paragraph| paragraph.annotations.is_empty()))
    );
}

#[test]
fn table_formula_direction_cell_selection_and_optional_recalculation_use_real_frames() {
    for theme in [Theme::Light, Theme::Dark] {
        let blocks = vec![Block::Table(vec![
            vec![Paragraph::default(); 3],
            vec![
                Paragraph::from_text("10"),
                Paragraph::from_text("20"),
                Paragraph::default(),
            ],
        ])];
        let (context, mut app) = app_with_blocks(blocks);
        app.prefs.theme = theme;
        settle(&context, &mut app);
        context_click_cell(&context, &mut app, 1, 2);
        choose_context_command(&context, &mut app, "Formula…");
        click(
            &context,
            &mut app,
            bounds(&context, "table_calculate_bounds"),
        );
        settle(&context, &mut app);
        let address = Address {
            block: 0,
            row: 1,
            cell: 2,
        };
        let p = app.session.editor.doc.paragraph(address).unwrap();
        assert_eq!(p.text(), "30");
        assert_eq!(p.formula.as_ref().unwrap().expression, "=SUM(LEFT)");
        let original = app.session.editor.doc.clone();
        let revision = app.session.editor.revision;
        app.dialog = Some(Dialog::Preferences);
        settle(&context, &mut app);
        click(
            &context,
            &mut app,
            bounds(&context, "table_calculation_preference_bounds"),
        );
        settle(&context, &mut app);
        assert!(app.prefs.auto_recalculate_tables);
        assert_eq!(app.session.editor.revision, revision);
        frame(
            &context,
            &mut app,
            vec![key(Key::Escape, egui::Modifiers::NONE)],
        );
        settle(&context, &mut app);
        context_click_cell(&context, &mut app, 1, 1);
        choose_context_command(&context, &mut app, "Select cell");
        assert_eq!(app.session.editor.selected_text(), "20");
        frame(&context, &mut app, vec![Event::Text("40".into())]);
        settle(&context, &mut app);
        assert_eq!(
            app.session.editor.doc.paragraph(address).unwrap().text(),
            "50"
        );
        frame(
            &context,
            &mut app,
            vec![key(Key::Z, egui::Modifiers::COMMAND)],
        );
        settle(&context, &mut app);
        assert_eq!(app.session.editor.doc, original);
        app.dialog = Some(Dialog::Preferences);
        settle(&context, &mut app);
        click(
            &context,
            &mut app,
            bounds(&context, "table_calculation_preference_bounds"),
        );
        settle(&context, &mut app);
        assert!(!app.prefs.auto_recalculate_tables);
        let encoded = serde_json::to_vec(&app.prefs).unwrap();
        let loaded: Preferences = serde_json::from_slice(&encoded).unwrap();
        assert!(!loaded.auto_recalculate_tables);
        frame(
            &context,
            &mut app,
            vec![key(Key::Escape, egui::Modifiers::NONE)],
        );
        settle(&context, &mut app);
        context_click_cell(&context, &mut app, 1, 1);
        choose_context_command(&context, &mut app, "Select cell");
        frame(&context, &mut app, vec![Event::Text("40".into())]);
        settle(&context, &mut app);
        assert_eq!(
            app.session.editor.doc.paragraph(address).unwrap().text(),
            "30"
        );
        assert_eq!(
            app.session
                .editor
                .doc
                .paragraph(Address {
                    block: 0,
                    row: 1,
                    cell: 1
                })
                .unwrap()
                .text(),
            "40"
        );
        context_click_cell(&context, &mut app, 1, 2);
        let command = bounds(&context, ("menu_bounds", "Recalculate table"));
        let clip = bounds(&context, ("menu_clip", "Recalculate table"));
        assert!(
            clip.contains(command.center()),
            "Recalculate table outside clip: {command:?} clip={clip:?}"
        );
        choose_context_command(&context, &mut app, "Recalculate table");
        assert_eq!(
            app.session.editor.doc.paragraph(address).unwrap().text(),
            "50",
            "status={} caret={:?} menu={:?} open={}",
            app.status,
            app.session.editor.caret,
            bounds(&context, ("menu_bounds", "Recalculate table")),
            egui::Popup::is_any_open(&context)
        );
        app.session.viewing = true;
        context_click_cell(&context, &mut app, 0, 0);
        choose_context_command(&context, &mut app, "Select cell");
        assert_eq!(
            app.session.editor.selected_table_cell_addresses(),
            [Address {
                block: 0,
                row: 0,
                cell: 0
            }]
        );
        let before = app.session.editor.doc.clone();
        frame(&context, &mut app, vec![Event::Text("blocked".into())]);
        assert_eq!(app.session.editor.doc, before);
    }
    let legacy: Preferences = serde_json::from_str("{}").unwrap();
    assert!(!legacy.auto_recalculate_tables);
}
