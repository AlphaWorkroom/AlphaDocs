use super::*;
#[test]
fn diagram_paste_and_arrow_repair_are_one_undoable_ui_action() {
    let (ctx, mut app) = editor();
    app.session.editor.insert("Original text");
    let original = app.session.editor.doc.clone();
    app.session.editor.select_all();
    ctx.memory_mut(|m| m.request_focus(Id::new("document_canvas")));
    let diagram = "┌─────────────────────┐\n│  Pre-Launch &rightarrow&  │\n└─────────────────────┘";
    image_frame(&ctx, &mut app, vec![Event::Paste(diagram.into())]);
    assert!(app.session.editor.doc.text().contains("Pre-Launch →"));
    assert!(app.session.editor.doc.blocks.iter().all(|b| matches!(b, Block::Paragraph(p) if p.chars.iter().all(|c| c.style.font == "Noto Sans Mono"))));
    image_frame(&ctx, &mut app, vec![key(Key::Z, egui::Modifiers::COMMAND)]);
    assert_eq!(app.session.editor.doc, original);
}
fn mac_command() -> egui::Modifiers {
    egui::Modifiers {
        mac_cmd: true,
        command: true,
        ..Default::default()
    }
}
#[test]
fn mac_command_formatting_undo_redo_and_find_use_real_frames() {
    let (ctx, mut app) = editor();
    ctx.set_os(egui::os::OperatingSystem::Mac);
    app.session.editor.insert("A café with words 🧑🏽‍💻");
    image_frame(&ctx, &mut app, vec![]);
    image_frame(&ctx, &mut app, vec![]);
    ctx.memory_mut(|m| m.request_focus(Id::new("document_canvas")));
    for k in [Key::A, Key::B] {
        image_frame(&ctx, &mut app, vec![key(k, mac_command())]);
    }
    assert!(
        app.session
            .editor
            .doc
            .paragraph(app.session.editor.caret.address)
            .unwrap()
            .chars
            .iter()
            .all(|c| c.style.bold)
    );
    image_frame(&ctx, &mut app, vec![key(Key::Z, mac_command())]);
    assert!(
        app.session
            .editor
            .doc
            .paragraph(app.session.editor.caret.address)
            .unwrap()
            .chars
            .iter()
            .all(|c| !c.style.bold)
    );
    image_frame(
        &ctx,
        &mut app,
        vec![key(Key::Z, mac_command() | egui::Modifiers::SHIFT)],
    );
    assert!(
        app.session
            .editor
            .doc
            .paragraph(app.session.editor.caret.address)
            .unwrap()
            .chars
            .iter()
            .all(|c| c.style.bold)
    );
    image_frame(&ctx, &mut app, vec![key(Key::H, mac_command())]);
    assert!(app.dialog.is_none(), "Cmd+H is reserved for macOS Hide");
    image_frame(
        &ctx,
        &mut app,
        vec![key(Key::H, mac_command() | egui::Modifiers::SHIFT)],
    );
    assert!(matches!(app.dialog, Some(Dialog::Find)));
}
#[test]
fn mac_word_line_document_navigation_and_deletion_are_distinct_and_undoable() {
    let (ctx, mut app) = editor();
    ctx.set_os(egui::os::OperatingSystem::Mac);
    app.session
        .editor
        .insert("First café line.\nSecond line with emoji 🧑🏽‍💻 and words.");
    image_frame(&ctx, &mut app, vec![]);
    image_frame(&ctx, &mut app, vec![]);
    ctx.memory_mut(|m| m.request_focus(Id::new("document_canvas")));
    let end = app.session.editor.caret;
    let start = app.layout.as_ref().unwrap().boundary(end, false);
    image_frame(
        &ctx,
        &mut app,
        vec![key(Key::ArrowLeft, egui::Modifiers::ALT)],
    );
    assert!(
        app.session.editor.caret.offset > start.offset
            && app.session.editor.caret.offset < end.offset
    );
    image_frame(&ctx, &mut app, vec![key(Key::ArrowLeft, mac_command())]);
    assert_eq!(app.session.editor.caret, start);
    image_frame(&ctx, &mut app, vec![key(Key::ArrowDown, mac_command())]);
    assert_eq!(app.session.editor.caret, end);
    let original = app.session.editor.doc.clone();
    image_frame(&ctx, &mut app, vec![key(Key::Backspace, mac_command())]);
    assert_ne!(app.session.editor.doc, original);
    image_frame(&ctx, &mut app, vec![key(Key::Z, mac_command())]);
    assert_eq!(app.session.editor.doc, original);
    image_frame(&ctx, &mut app, vec![key(Key::ArrowUp, mac_command())]);
    assert_eq!(app.session.editor.caret.offset, 0);
    image_frame(
        &ctx,
        &mut app,
        vec![key(Key::ArrowDown, mac_command() | egui::Modifiers::SHIFT)],
    );
    assert!(app.session.editor.selected_text().contains("🧑🏽‍💻"));
}
#[test]
fn mac_control_click_opens_image_menu_without_starting_a_primary_drag() {
    let (ctx, mut app) = image_app();
    ctx.set_os(egui::os::OperatingSystem::Mac);
    select_image(&ctx, &mut app);
    let original = app.session.editor.doc.clone();
    let pos = ctx
        .read_response(Id::new(("image", &0usize)))
        .unwrap()
        .rect
        .center();
    image_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::CTRL,
            },
        ],
    );
    image_frame(
        &ctx,
        &mut app,
        vec![Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(egui::Popup::is_any_open(&ctx));
    assert_eq!(app.image_block, Some(0));
    assert!(app.image_drag.is_none());
    assert_eq!(app.session.editor.doc, original);
}
#[test]
fn feature_search_enter_opens_real_preferences_and_click_dismisses_destination_pulse() {
    let (ctx, mut app) = editor();
    app.feature_query = "autosave".into();
    app.open_feature_search();
    image_frame(&ctx, &mut app, vec![]);
    image_frame(&ctx, &mut app, vec![]);
    image_frame(&ctx, &mut app, vec![key(Key::Enter, egui::Modifiers::NONE)]);
    assert!(!app.feature_search_open);
    assert!(matches!(app.dialog, Some(Dialog::Preferences)));
    image_frame(&ctx, &mut app, vec![]);
    image_frame(&ctx, &mut app, vec![]);
    image_frame(&ctx, &mut app, vec![]);
    assert!(app.feature_pulse.is_some());
    assert!(app.feature_rect.is_some());
    let autosave = ctx
        .data_mut(|d| d.get_temp::<Rect>(Id::new("autosave_preference_bounds")))
        .expect("Autosave checkbox must be in Preferences");
    image_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(autosave.center()),
            pointer(autosave.center(), egui::PointerButton::Primary, true),
            pointer(autosave.center(), egui::PointerButton::Primary, false),
        ],
    );
    assert!(!app.prefs.autosave);
    let pos = pos2(20.0, 500.0);
    image_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(pos),
            pointer(pos, egui::PointerButton::Primary, true),
            pointer(pos, egui::PointerButton::Primary, false),
        ],
    );
    assert!(app.feature_pulse.is_none());
}
#[test]
fn disabled_autosave_gates_periodic_writes_and_legacy_preferences_default_to_enabled() {
    let legacy: Preferences = serde_json::from_str("{}").unwrap();
    assert!(legacy.autosave);
    let (ctx, mut app) = editor();
    app.prefs.autosave = false;
    app.session.editor.insert("Unsaved background change");
    let saved = app.session.saved_library_revision;
    app.last_local_edit = Instant::now() - Duration::from_secs(40);
    app.last_autosave = Instant::now() - Duration::from_secs(3);
    let before = app.last_autosave;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0))),
            ..Default::default()
        },
        |ui| app.update_ui(ui),
    );
    output.textures_delta.clear();
    assert_eq!(app.last_autosave, before);
    assert_eq!(app.session.saved_library_revision, saved);
    let prefs: Preferences =
        serde_json::from_str(&serde_json::to_string(&app.prefs).unwrap()).unwrap();
    assert!(!prefs.autosave);
}

fn autosave_frame_at(ctx: &Context, app: &mut AlphaDocs, now: Instant, events: Vec<Event>) {
    app.autosave_test_now = Some(now);
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0))),
            events,
            ..Default::default()
        },
        |ui| app.update_ui(ui),
    );
    output.textures_delta.clear();
}

#[test]
fn full_ui_autosave_waits_for_thirty_seconds_after_the_last_local_input() {
    let (ctx, mut app) = editor();
    let base = Instant::now();
    app.prefs.autosave = true;
    app.session.saved_library_revision = app.session.editor.revision;
    app.last_local_edit = base;
    app.last_history = base;
    app.autosave_test_now = Some(base);
    autosave_frame_at(&ctx, &mut app, base, vec![]);
    autosave_frame_at(&ctx, &mut app, base, vec![]);

    for (seconds, text) in [(0, "a"), (10, "b"), (20, "c")] {
        ctx.memory_mut(|m| m.request_focus(Id::new("document_canvas")));
        autosave_frame_at(
            &ctx,
            &mut app,
            base + Duration::from_secs(seconds),
            vec![Event::Text(text.into())],
        );
        assert_eq!(app.autosave_test_saves, 0);
    }
    assert_eq!(app.session.editor.revision, 3);

    let preedit = Event::Ime(egui::ImeEvent::Preedit {
        text: "é".into(),
        active_range_chars: Some(0..1),
    });
    autosave_frame_at(
        &ctx,
        &mut app,
        base + Duration::from_secs(50),
        vec![preedit],
    );
    assert_eq!(app.autosave_test_saves, 0);
    ctx.memory_mut(|m| m.request_focus(Id::new("document_canvas")));
    autosave_frame_at(
        &ctx,
        &mut app,
        base + Duration::from_secs(51),
        vec![Event::Ime(egui::ImeEvent::Commit("é".into()))],
    );
    assert_eq!(app.autosave_test_saves, 0);

    autosave_frame_at(&ctx, &mut app, base + Duration::from_secs(80), vec![]);
    assert_eq!(app.autosave_test_saves, 0);
    autosave_frame_at(&ctx, &mut app, base + Duration::from_secs(81), vec![]);
    assert_eq!(app.autosave_test_saves, 1);
    assert_eq!(
        app.session.saved_library_revision,
        app.session.editor.revision
    );
}

#[test]
fn autosave_gate_requires_dirty_idle_enabled_and_no_active_ime() {
    let (_ctx, mut app) = editor();
    let now = Instant::now();
    app.prefs.autosave = true;
    app.last_local_edit = now - Duration::from_secs(29);
    app.last_autosave = now - Duration::from_secs(3);
    app.session.saved_library_revision = app.session.editor.revision.wrapping_add(1);
    assert!(!app.autosave_due(now));
    assert!(app.autosave_due(now + Duration::from_secs(1)));
    app.ime_preedit = "composition".into();
    assert!(!app.autosave_due(now + Duration::from_secs(1)));
    app.ime_preedit.clear();
    app.prefs.autosave = false;
    assert!(!app.autosave_due(now + Duration::from_secs(1)));
    app.prefs.autosave = true;
    app.session.saved_library_revision = app.session.editor.revision;
    assert!(!app.autosave_due(now + Duration::from_secs(1)));
}

#[test]
fn content_tab_navigation_does_not_reset_the_local_edit_idle_clock() {
    let (_ctx, mut app) = editor();
    app.session.editor.add_tab();
    app.session.saved_library_revision = app.session.editor.revision;
    let before = app.autosave_revision();
    assert!(app.session.editor.switch_tab(0));
    let after_navigation = app.autosave_revision();
    assert!(!AlphaDocs::local_revision_changed(
        &before,
        &after_navigation
    ));
    app.session.editor.insert("Local edit");
    let after_edit = app.autosave_revision();
    assert!(AlphaDocs::local_revision_changed(&before, &after_edit));
}
#[test]
fn double_click_content_tab_opens_rename_without_creating_another_document() {
    let (ctx, mut app) = editor();
    app.prefs.outline = true;
    let doc_id = app.session.editor.doc.id.clone();
    image_frame(&ctx, &mut app, vec![]);
    image_frame(&ctx, &mut app, vec![]);
    let tab_id = app.session.editor.doc.tabs[0].id.clone();
    let rect = ctx
        .read_response(Id::new(("content_tab", &tab_id)))
        .unwrap()
        .rect;
    for _ in 0..2 {
        image_frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(rect.center()),
                pointer(rect.center(), egui::PointerButton::Primary, true),
                pointer(rect.center(), egui::PointerButton::Primary, false),
            ],
        );
    }
    assert!(app.tab_rename.is_some());
    assert_eq!(app.session.editor.doc.id, doc_id);
    assert_eq!(app.session.editor.doc.tabs.len(), 1);
    image_frame(&ctx, &mut app, vec![]);
    image_frame(&ctx, &mut app, vec![]);
    image_frame(&ctx, &mut app, vec![key(Key::A, egui::Modifiers::COMMAND)]);
    image_frame(&ctx, &mut app, vec![Event::Text("Review tab".into())]);
    assert_eq!(
        app.tab_rename.as_ref().unwrap().1,
        "Review tab",
        "Focus {:?}",
        ctx.memory(|m| m.focused())
    );
    let blocks = app.session.editor.doc.blocks.clone();
    image_frame(&ctx, &mut app, vec![key(Key::Enter, egui::Modifiers::NONE)]);
    assert!(
        app.tab_rename.is_none(),
        "Focus {:?}, draft {:?}",
        ctx.memory(|m| m.focused()),
        app.tab_rename
    );
    assert_eq!(app.session.editor.doc.tabs[0].title, "Review tab");
    assert_eq!(app.session.editor.doc.blocks, blocks);
    app.session.editor.undo();
    assert_eq!(app.session.editor.doc.tabs[0].title, "Tab 1");
}
#[test]
fn activating_a_comment_keeps_its_card_compact_in_full_ui_frames() {
    let (ctx, mut app) = image_app();
    app.session
        .editor
        .add_image_comment(0, "A short image comment");
    let Block::Image(i) = &app.session.editor.doc.blocks[0] else {
        panic!()
    };
    let id = i.id.clone();
    let cid = i.comments[0].id.clone();
    image_frame(&ctx, &mut app, vec![]);
    image_frame(&ctx, &mut app, vec![]);
    let collapsed = ctx
        .data_mut(|d| d.get_temp::<Rect>(Id::new(("annotation_thread_bounds", &id))))
        .unwrap();
    assert!(collapsed.height() < 180.0);
    app.active_comment = Some((id.clone(), cid));
    image_frame(&ctx, &mut app, vec![]);
    image_frame(&ctx, &mut app, vec![]);
    let expanded = ctx
        .data_mut(|d| d.get_temp::<Rect>(Id::new(("annotation_thread_bounds", &id))))
        .unwrap();
    assert!(
        expanded.height() < 300.0,
        "Expanded card must not fill the viewport: {expanded:?}"
    );
    assert!(expanded.height() > collapsed.height());
    assert!(
        (expanded.width() - collapsed.width()).abs() < 1.0,
        "Collapsed {collapsed:?}, expanded {expanded:?}"
    );
}
#[test]
fn text_selection_comment_shortcut_posts_through_real_ui_without_changing_text() {
    let (ctx, mut app) = editor();
    app.prefs.outline = false;
    app.prefs.ruler = false;
    app.session.editor.doc.blocks = vec![Block::Paragraph(Paragraph::from_text(
        "Select this text and keep the rest.",
    ))];
    app.session.editor.anchor = Position::default();
    app.session.editor.caret = Position {
        offset: 16,
        ..Default::default()
    };
    image_frame(&ctx, &mut app, vec![]);
    image_frame(
        &ctx,
        &mut app,
        vec![key(Key::M, egui::Modifiers::COMMAND | egui::Modifiers::ALT)],
    );
    assert!(app.comments_open);
    assert!(app.text_comment_range.is_some());
    image_frame(
        &ctx,
        &mut app,
        vec![Event::Text("An anchored text comment".into())],
    );
    assert_eq!(app.comment_draft, "An anchored text comment");
    image_frame(
        &ctx,
        &mut app,
        vec![key(Key::Enter, egui::Modifiers::COMMAND)],
    );
    image_frame(&ctx, &mut app, vec![]);
    assert!(!app.comments_open);
    assert!(app.active_comment.is_some());
    let p = app
        .session
        .editor
        .doc
        .paragraph(Address::default())
        .unwrap();
    assert_eq!(p.text(), "Select this text and keep the rest.");
    assert_eq!(
        p.annotations[0].comments[0].text,
        "An anchored text comment"
    );
    assert_eq!((p.annotations[0].start, p.annotations[0].end), (0, 16));
    assert!(!app.session.editor.selected());
    app.session.editor.undo();
    assert!(
        app.session
            .editor
            .doc
            .paragraph(Address::default())
            .unwrap()
            .annotations
            .is_empty()
    );
    image_frame(&ctx, &mut app, vec![]);
}

#[test]
fn full_image_edges_use_resize_cursors_and_resize_in_one_undo_step() {
    let (ctx, mut app) = image_app();
    select_image(&ctx, &mut app);
    let rect = ctx.read_response(Id::new(("image", &0usize))).unwrap().rect;
    for (point, cursor) in [
        (
            pos2(rect.left() + rect.width() * 0.25, rect.top()),
            egui::CursorIcon::ResizeVertical,
        ),
        (
            pos2(rect.right(), rect.top() + rect.height() * 0.25),
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            pos2(rect.left() + rect.width() * 0.25, rect.bottom()),
            egui::CursorIcon::ResizeVertical,
        ),
        (
            pos2(rect.left(), rect.top() + rect.height() * 0.25),
            egui::CursorIcon::ResizeHorizontal,
        ),
        (rect.left_top(), egui::CursorIcon::ResizeNwSe),
        (rect.right_top(), egui::CursorIcon::ResizeNeSw),
    ] {
        image_frame(&ctx, &mut app, vec![Event::PointerMoved(point)]);
        assert_eq!(image_frame_cursor(&ctx, &mut app, vec![]), cursor);
    }
    let before = app.session.editor.doc.clone();
    let start = pos2(rect.right(), rect.top() + rect.height() * 0.25);
    let end = start + vec2(40.0, 0.0);
    image_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(start),
            pointer(start, egui::PointerButton::Primary, true),
            Event::PointerMoved(end),
        ],
    );
    assert_eq!(app.session.editor.doc, before);
    image_frame(
        &ctx,
        &mut app,
        vec![pointer(end, egui::PointerButton::Primary, false)],
    );
    assert!(
        matches!(&app.session.editor.doc.blocks[0], Block::Image(i) if i.width > 240.0 && i.height == 160.0)
    );
    app.session.editor.undo();
    assert_eq!(app.session.editor.doc, before);
    assert!(!app.session.editor.can_undo());
}

#[test]
fn floating_line_snapping_uses_measured_lines_in_full_ui_and_commits_once() {
    for mode in [
        ImageSnap::Freeform,
        ImageSnap::TextLines,
        ImageSnap::NearTextLines,
    ] {
        for coalesced in [false, true] {
            let (ctx, mut app) = image_app();
            if let Block::Image(i) = &mut app.session.editor.doc.blocks[0] {
                i.placement.flow = ImageFlow::Front;
                i.placement.snap = mode;
            }
            let mut p = Paragraph::from_text("First line\nSecond line\nThird line\nFourth line");
            p.style.size = 24.0;
            for ch in &mut p.chars {
                ch.style.size = 24.0;
            }
            app.session.editor.doc.blocks[1] = Block::Paragraph(p);
            select_image(&ctx, &mut app);
            let original = app.session.editor.doc.clone();
            let layout = app.layout.as_ref().unwrap();
            let image_top = layout
                .objects
                .iter()
                .find_map(|o| match o {
                    Object::Image { rect, .. } => Some(rect.top()),
                    _ => None,
                })
                .unwrap();
            let line = layout
                .lines
                .iter()
                .filter(|l| l.address.block == 1)
                .nth(1)
                .unwrap()
                .rect
                .top();
            let scale = 4.0 / 3.0 * app.prefs.zoom;
            let from = ctx
                .read_response(Id::new(("image_move", 0usize)))
                .unwrap()
                .rect
                .center();
            let to = from + vec2(25.0, (line - image_top) * scale + 3.0);
            let mut events = vec![
                Event::PointerMoved(from),
                pointer(from, egui::PointerButton::Primary, true),
                Event::PointerMoved(to),
            ];
            if coalesced {
                events.push(pointer(to, egui::PointerButton::Primary, false));
            }
            image_frame(&ctx, &mut app, events);
            if !coalesced {
                assert_eq!(app.session.editor.doc, original);
                image_frame(
                    &ctx,
                    &mut app,
                    vec![pointer(to, egui::PointerButton::Primary, false)],
                );
            }
            let Block::Image(i) = &app.session.editor.doc.blocks[0] else {
                panic!()
            };
            let expected_y = line - image_top
                + if mode == ImageSnap::Freeform {
                    3.0 / scale
                } else {
                    0.0
                };
            assert!(
                (i.placement.offset[1] - expected_y).abs() < 0.1,
                "mode {mode:?}: {:?}, expected {expected_y}",
                i.placement
            );
            assert!((i.placement.offset[0] - 25.0 / scale).abs() < 0.1);
            app.session.editor.undo();
            assert_eq!(app.session.editor.doc, original);
            assert!(!app.session.editor.can_undo());
        }
    }
}

#[test]
fn dragging_a_clipped_legacy_offset_commits_the_visible_preview_position() {
    let (ctx, mut app) = image_app();
    if let Block::Image(i) = &mut app.session.editor.doc.blocks[0] {
        i.placement.flow = ImageFlow::Front;
        i.placement.offset = [900.0, -100.0];
    }
    select_image(&ctx, &mut app);
    let before = app.session.editor.doc.clone();
    let original = ctx.read_response(Id::new(("image", &0usize))).unwrap().rect;
    let start = ctx
        .read_response(Id::new(("image_move", 0usize)))
        .unwrap()
        .rect
        .center();
    let end = start + vec2(-40.0, 30.0);
    image_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(start),
            pointer(start, egui::PointerButton::Primary, true),
            Event::PointerMoved(end),
            pointer(end, egui::PointerButton::Primary, false),
        ],
    );
    image_frame(&ctx, &mut app, vec![]);
    image_frame(&ctx, &mut app, vec![]);
    let actual = ctx.read_response(Id::new(("image", &0usize))).unwrap().rect;
    assert!(
        (actual.left() - original.left() + 40.0).abs() < 0.1,
        "original {original:?}, actual {actual:?}"
    );
    assert!((actual.top() - original.top() - 30.0).abs() < 0.1);
    app.session.editor.undo();
    assert_eq!(app.session.editor.doc, before);
}

#[test]
fn break_image_can_be_dragged_horizontally_without_reordering_or_extra_undo_steps() {
    let (ctx, mut app) = image_app();
    select_image(&ctx, &mut app);
    let before = app.session.editor.doc.clone();
    let from = ctx
        .read_response(Id::new(("image_move", 0usize)))
        .unwrap()
        .rect
        .center();
    let to = from + vec2(50.0, 0.0);
    image_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(from),
            pointer(from, egui::PointerButton::Primary, true),
            Event::PointerMoved(to),
            pointer(to, egui::PointerButton::Primary, false),
        ],
    );
    let Block::Image(i) = &app.session.editor.doc.blocks[0] else {
        panic!()
    };
    assert!(i.placement.offset[0] > 0.0);
    app.session.editor.undo();
    assert_eq!(app.session.editor.doc, before);
    assert!(!app.session.editor.can_undo());
}

#[test]
fn undoing_selected_last_image_insertion_clears_selection_before_drawing_toolbar() {
    let (ctx, mut app) = image_app();
    let image = app.session.editor.doc.blocks.remove(0);
    app.session.editor.caret = Position::default();
    app.session.editor.anchor = app.session.editor.caret;
    app.session.editor.insert_block(image);
    let block = app
        .session
        .editor
        .doc
        .blocks
        .iter()
        .position(|b| matches!(b, Block::Image(_)))
        .unwrap();
    let Block::Image(i) = &app.session.editor.doc.blocks[block] else {
        panic!()
    };
    app.image_block = Some(block);
    app.image_selection_id = Some(i.id.clone());
    image_frame(&ctx, &mut app, vec![]);
    image_frame(&ctx, &mut app, vec![key(Key::Z, egui::Modifiers::COMMAND)]);
    image_frame(&ctx, &mut app, vec![]);
    assert!(app.image_block.is_none());
    assert!(app.image_selection_id.is_none());
    assert!(
        !app.session
            .editor
            .doc
            .blocks
            .iter()
            .any(|b| matches!(b, Block::Image(_)))
    );
}

#[test]
fn image_context_click_opens_real_object_commands() {
    let (ctx, mut app) = image_app();
    select_image(&ctx, &mut app);
    let pos = ctx
        .read_response(Id::new(("image", &0usize)))
        .unwrap()
        .rect
        .center();
    image_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(pos),
            pointer(pos, egui::PointerButton::Secondary, true),
        ],
    );
    image_frame(
        &ctx,
        &mut app,
        vec![pointer(pos, egui::PointerButton::Secondary, false)],
    );
    assert!(egui::Popup::is_any_open(&ctx));
    assert_eq!(app.image_block, Some(0));
}

#[test]
#[ignore = "Diagnostic timing; run explicitly with --ignored --nocapture"]
fn wrapped_long_paragraph_performance_diagnostic() {
    let (ctx, mut app) = image_app();
    let Block::Image(image) = &mut app.session.editor.doc.blocks[0] else {
        panic!()
    };
    image.placement.flow = ImageFlow::Wrap;
    image.placement.offset = [95.0, 0.0];
    image.width = 170.0;
    app.session.editor.doc.blocks[1] = Block::Paragraph(Paragraph::from_text(
        &"Measured Unicode café text flows beside the image. ".repeat(3500),
    ));
    let cold = Instant::now();
    image_frame(&ctx, &mut app, vec![]);
    let cold = cold.elapsed();
    let mut warm = vec![];
    for _ in 0..10 {
        let timer = Instant::now();
        image_frame(&ctx, &mut app, vec![]);
        warm.push(timer.elapsed());
    }
    warm.sort();
    let edited = Instant::now();
    app.session.editor.place(
        Position {
            address: Address {
                block: 1,
                row: 0,
                cell: 0,
            },
            offset: 0,
        },
        false,
    );
    app.session.editor.insert("Edit ");
    image_frame(&ctx, &mut app, vec![]);
    let edited = edited.elapsed();
    println!(
        "Wrapped long paragraph, {} words / {} measured pages: cold full UI {:?}; warm median {:?}; edit + full UI {:?}. Excludes GPU presentation and persistence I/O.",
        app.session.editor.doc.counts().0,
        app.layout.as_ref().unwrap().pages,
        cold,
        warm[5],
        edited
    );
}

#[test]
fn comment_drafts_are_anchored_to_each_image_instead_of_following_selection() {
    let (ctx, mut app) = image_app();
    let Block::Image(first) = &app.session.editor.doc.blocks[0] else {
        panic!()
    };
    let first_id = first.id.clone();
    let mut second = first.clone();
    second.id = new_object_id();
    let second_id = second.id.clone();
    app.session
        .editor
        .doc
        .blocks
        .insert(1, Block::Image(second));
    app.session.editor.caret.address.block = 2;
    app.session.editor.anchor = app.session.editor.caret;
    app.image_block = Some(0);
    app.image_selection_id = Some(first_id.clone());
    app.comments_open = true;
    app.comment_focus = true;
    image_frame(&ctx, &mut app, vec![]);
    image_frame(
        &ctx,
        &mut app,
        vec![Event::Text("First image draft".into())],
    );
    assert_eq!(app.comment_draft, "First image draft");
    app.image_block = Some(1);
    app.image_selection_id = Some(second_id);
    image_frame(&ctx, &mut app, vec![]);
    assert_eq!(app.comment_draft, "");
    image_frame(
        &ctx,
        &mut app,
        vec![Event::Text("Second image draft".into())],
    );
    assert_eq!(app.comment_draft, "Second image draft");
    app.image_block = Some(0);
    app.image_selection_id = Some(first_id);
    image_frame(&ctx, &mut app, vec![]);
    assert_eq!(app.comment_draft, "First image draft");
}

#[test]
fn overlapping_images_select_the_front_layer_and_background_text_remains_editable() {
    let (ctx, mut app) = image_app();
    let Block::Image(first) = &mut app.session.editor.doc.blocks[0] else {
        panic!()
    };
    first.placement.flow = ImageFlow::Front;
    let mut second = first.clone();
    second.id = new_object_id();
    second.placement.flow = ImageFlow::Behind;
    app.session
        .editor
        .doc
        .blocks
        .insert(1, Block::Image(second));
    app.session.editor.caret.address.block = 2;
    app.session.editor.anchor = app.session.editor.caret;
    select_image(&ctx, &mut app);
    assert_eq!(app.image_block, Some(0));
    let id = app.image_selection_id.clone();
    app.session.editor.move_image(0, 3);
    image_frame(&ctx, &mut app, vec![]);
    assert_eq!(app.image_selection_id, id);
    assert_eq!(app.image_block, Some(2));
    let (ctx, mut app) = image_app();
    let Block::Image(image) = &mut app.session.editor.doc.blocks[0] else {
        panic!()
    };
    image.placement.flow = ImageFlow::Behind;
    app.session.editor.doc.blocks[1] = Block::Paragraph(Paragraph::from_text(
        &"Editable text over a background image. ".repeat(12),
    ));
    image_frame(&ctx, &mut app, vec![]);
    let pos = ctx
        .read_response(Id::new(("image", &0usize)))
        .unwrap()
        .rect
        .left_top()
        + vec2(30.0, 20.0);
    image_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(pos),
            pointer(pos, egui::PointerButton::Primary, true),
        ],
    );
    image_frame(
        &ctx,
        &mut app,
        vec![pointer(pos, egui::PointerButton::Primary, false)],
    );
    assert!(app.image_block.is_none());
    image_frame(&ctx, &mut app, vec![Event::Text("Typed ".into())]);
    assert!(matches!(app.session.editor.doc.blocks[0], Block::Image(_)));
    assert!(app.session.editor.doc.text().contains("Typed"));
}

#[test]
fn homepage_document_preview_and_vector_options_open_real_menus() {
    let (ctx, mut app) = editor();
    app.home = true;
    let doc = Document::new("Context menu QA");
    let id = doc.id.clone();
    app.recents = vec![Recent {
        path: PathBuf::from("qa/no-file-opened-by-this-test.adocs"),
        doc,
        modified: 0,
    }];
    image_frame(&ctx, &mut app, vec![]);
    let r = ctx
        .read_response(Id::new(("recent_document", &id)))
        .unwrap()
        .rect;
    image_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(r.center()),
            pointer(r.center(), egui::PointerButton::Secondary, true),
        ],
    );
    image_frame(
        &ctx,
        &mut app,
        vec![pointer(r.center(), egui::PointerButton::Secondary, false)],
    );
    assert!(egui::Popup::is_any_open(&ctx));
    assert!(app.home);
    image_frame(
        &ctx,
        &mut app,
        vec![key(Key::Escape, egui::Modifiers::NONE)],
    );
    image_frame(&ctx, &mut app, vec![]);
    let options = ctx
        .read_response(Id::new(("recent_document_options", &id)))
        .unwrap()
        .rect
        .center();
    image_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(options),
            pointer(options, egui::PointerButton::Primary, true),
        ],
    );
    image_frame(
        &ctx,
        &mut app,
        vec![pointer(options, egui::PointerButton::Primary, false)],
    );
    assert!(egui::Popup::is_any_open(&ctx));
    assert!(app.home);
}

#[test]
fn image_body_drag_reorders_and_float_drag_positions_with_one_undo_for_fast_events() {
    for coalesced in [false, true] {
        let (ctx, mut app) = image_app();
        select_image(&ctx, &mut app);
        let original = app.session.editor.doc.clone();
        let from = ctx
            .read_response(Id::new(("image_move", 0usize)))
            .unwrap()
            .rect
            .center();
        let to = ctx
            .read_response(Id::new("document_canvas"))
            .unwrap()
            .rect
            .left_top()
            + vec2(180.0, 500.0);
        let mut events = vec![
            Event::PointerMoved(from),
            pointer(from, egui::PointerButton::Primary, true),
            Event::PointerMoved(to),
        ];
        if coalesced {
            events.push(pointer(to, egui::PointerButton::Primary, false));
        }
        image_frame(&ctx, &mut app, events);
        if !coalesced {
            image_frame(
                &ctx,
                &mut app,
                vec![pointer(to, egui::PointerButton::Primary, false)],
            );
        }
        assert!(matches!(app.session.editor.doc.blocks[1], Block::Image(_)));
        assert_eq!(app.image_block, Some(1));
        app.session.editor.undo();
        assert_eq!(app.session.editor.doc, original);
        assert!(!app.session.editor.can_undo());
        image_frame(&ctx, &mut app, vec![]);
        let placement = ImagePlacement {
            flow: ImageFlow::Wrap,
            ..Default::default()
        };
        app.session.editor.set_image_placement(0, placement);
        image_frame(&ctx, &mut app, vec![]);
        let before = app.session.editor.doc.clone();
        let from = ctx
            .read_response(Id::new(("image_move", 0usize)))
            .unwrap()
            .rect
            .center();
        let to = from + vec2(55.0, 36.0);
        image_frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(from),
                pointer(from, egui::PointerButton::Primary, true),
                Event::PointerMoved(to),
                pointer(to, egui::PointerButton::Primary, false),
            ],
        );
        let Block::Image(i) = &app.session.editor.doc.blocks[0] else {
            panic!()
        };
        assert!(i.placement.offset[0] > 0.0);
        assert!(i.placement.offset[1] > 0.0);
        app.session.editor.undo();
        assert_eq!(app.session.editor.doc, before);
    }
}

#[test]
fn image_comment_shortcut_opens_sidebar_and_typing_does_not_delete_image() {
    let (ctx, mut app) = image_app();
    select_image(&ctx, &mut app);
    let original = app.session.editor.doc.clone();
    image_frame(
        &ctx,
        &mut app,
        vec![key(Key::M, egui::Modifiers::COMMAND | egui::Modifiers::ALT)],
    );
    image_frame(&ctx, &mut app, vec![]);
    assert!(app.comments_open);
    assert!(!app.image_options_open);
    ctx.memory_mut(|m| m.request_focus(Id::new("new_image_comment")));
    image_frame(
        &ctx,
        &mut app,
        vec![Event::Text("Image comment typed safely".into())],
    );
    assert_eq!(app.comment_draft, "Image comment typed safely");
    assert_eq!(app.session.editor.doc, original);
    assert!(ctx.read_response(Id::new("new_image_comment")).is_some());
}

fn editor() -> (Context, AlphaDocs) {
    let ctx = Context::default();
    ctx.enable_accesskit();
    fonts::install(&ctx);
    let mut app = AlphaDocs::fresh(&ctx, Preferences::default(), vec![]);
    app.home = false;
    (ctx, app)
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
fn send(ctx: &Context, app: &mut AlphaDocs, events: Vec<Event>) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            events,
            ..Default::default()
        },
        |_| {
            ctx.memory_mut(|m| m.request_focus(Id::new("document_canvas")));
            app.input(ctx);
        },
    );
    output.textures_delta.clear();
}
#[test]
fn formatting_and_undo_shortcuts_operate_on_selection() {
    let (ctx, mut app) = editor();
    app.session.editor.insert("Café");
    send(
        &ctx,
        &mut app,
        vec![
            key(Key::A, egui::Modifiers::COMMAND),
            key(Key::B, egui::Modifiers::COMMAND),
        ],
    );
    assert!(
        app.session
            .editor
            .doc
            .paragraph(Address::default())
            .unwrap()
            .chars
            .iter()
            .all(|c| c.style.bold)
    );
    send(&ctx, &mut app, vec![key(Key::Z, egui::Modifiers::COMMAND)]);
    assert!(
        app.session
            .editor
            .doc
            .paragraph(Address::default())
            .unwrap()
            .chars
            .iter()
            .all(|c| !c.style.bold)
    );
    send(
        &ctx,
        &mut app,
        vec![key(
            Key::Z,
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        )],
    );
    assert!(app.session.editor.typing.bold);
    send(
        &ctx,
        &mut app,
        vec![key(
            Key::Num2,
            egui::Modifiers::COMMAND | egui::Modifiers::ALT,
        )],
    );
    assert_eq!(
        app.session
            .editor
            .doc
            .paragraph(Address::default())
            .unwrap()
            .heading,
        2
    );
}
#[test]
fn table_tab_and_shift_enter_preserve_the_table() {
    let (ctx, mut app) = editor();
    app.session.editor.insert_table(1, 2);
    send(
        &ctx,
        &mut app,
        vec![
            Event::Text("One".into()),
            key(Key::Tab, egui::Modifiers::NONE),
            Event::Text("Two".into()),
        ],
    );
    assert_eq!(app.session.editor.caret.address.cell, 1);
    send(
        &ctx,
        &mut app,
        vec![
            key(Key::Enter, egui::Modifiers::SHIFT),
            Event::Text("lines".into()),
        ],
    );
    assert_eq!(
        app.session
            .editor
            .doc
            .paragraph(app.session.editor.caret.address)
            .unwrap()
            .text(),
        "Two\nlines"
    );
    send(&ctx, &mut app, vec![key(Key::Tab, egui::Modifiers::NONE)]);
    assert_eq!(app.session.editor.caret.address.row, 1);
    app.session.editor.doc.validate().unwrap();
}
#[test]
fn plain_paste_intent_survives_modifier_release_before_clipboard_delivery() {
    let (_, mut source) = editor();
    source.session.editor.format(|s| s.bold = true);
    source.session.editor.insert("Copied");
    source.session.editor.select_all();
    let (ctx, mut target) = editor();
    target.clipboard = Some(source.session.editor.copy_fragment());
    send(
        &ctx,
        &mut target,
        vec![key(
            Key::V,
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        )],
    );
    send(&ctx, &mut target, vec![Event::Paste("Copied".into())]);
    assert_eq!(target.session.editor.doc.text(), "Copied");
    assert!(
        target
            .session
            .editor
            .doc
            .paragraph(Address::default())
            .unwrap()
            .chars
            .iter()
            .all(|c| !c.style.bold)
    );
}
#[test]
fn viewing_mode_allows_selection_but_rejects_edits() {
    let (ctx, mut app) = editor();
    app.session.editor.insert("Keep this text");
    app.session.viewing = true;
    let before = app.session.editor.doc.clone();
    send(
        &ctx,
        &mut app,
        vec![
            key(Key::A, egui::Modifiers::COMMAND),
            Event::Text("Wrong".into()),
            Event::Paste("Wrong".into()),
            key(Key::Backspace, egui::Modifiers::NONE),
            key(Key::Z, egui::Modifiers::COMMAND),
        ],
    );
    assert_eq!(app.session.editor.doc, before);
    assert!(app.session.editor.selected());
    app.execute(Action::Undo, &ctx);
    app.execute(Action::Table(2, 2), &ctx);
    assert_eq!(app.session.editor.doc, before);
}
#[test]
fn tab_retains_canvas_focus_across_real_ui_frames() {
    let (ctx, mut app) = editor();
    app.session.editor.insert_table(1, 2);
    app.refocus = true;
    let raw = |events| egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 960.0))),
        events,
        ..Default::default()
    };
    for _ in 0..3 {
        let mut output = ctx.run_ui(raw(vec![]), |ui| {
            app.input(&ctx);
            app.editor_ui(ui);
        });
        output.textures_delta.clear();
    }
    assert!(ctx.memory(|m| m.has_focus(Id::new("document_canvas"))));
    let mut output = ctx.run_ui(raw(vec![key(Key::Tab, egui::Modifiers::NONE)]), |ui| {
        app.input(&ctx);
        app.editor_ui(ui);
    });
    output.textures_delta.clear();
    assert_eq!(app.session.editor.caret.address.cell, 1);
    assert!(ctx.memory(|m| m.has_focus(Id::new("document_canvas"))));
    let mut output = ctx.run_ui(raw(vec![Event::Text("Second cell".into())]), |ui| {
        app.input(&ctx);
        app.editor_ui(ui);
    });
    output.textures_delta.clear();
    assert_eq!(
        app.session
            .editor
            .doc
            .paragraph(app.session.editor.caret.address)
            .unwrap()
            .text(),
        "Second cell"
    );
}

fn pointer(pos: Pos2, button: egui::PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn table_picker_mouse_activation_inserts_the_chosen_size_and_accepts_typing() {
    let (ctx, mut app) = editor();
    let frame = |app: &mut AlphaDocs, events| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0))),
                events,
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| app.table_picker(ui));
            },
        );
        output.textures_delta.clear();
    };
    frame(&mut app, vec![]);
    let pos = ctx
        .read_response(Id::new(("table_size", 1usize, 2usize)))
        .unwrap()
        .rect
        .center();
    frame(
        &mut app,
        vec![
            Event::PointerMoved(pos),
            pointer(pos, egui::PointerButton::Primary, true),
        ],
    );
    frame(
        &mut app,
        vec![pointer(pos, egui::PointerButton::Primary, false)],
    );
    assert!(matches!(app.action, Some(Action::Table(2, 3))));
    let action = app.action.take().unwrap();
    app.execute(action, &ctx);
    assert!(
        matches!(&app.session.editor.doc.blocks[0], Block::Table(rows) if rows.len() == 2 && rows[0].len() == 3)
    );
    send(&ctx, &mut app, vec![Event::Text("First cell".into())]);
    assert_eq!(
        app.session
            .editor
            .doc
            .paragraph(Address::default())
            .unwrap()
            .text(),
        "First cell"
    );
    app.session.editor.undo();
    app.session.editor.undo();
    assert!(matches!(
        app.session.editor.doc.blocks[0],
        Block::Paragraph(_)
    ));
}

#[test]
fn table_picker_size_can_be_activated_from_keyboard_focus() {
    let (ctx, mut app) = editor();
    let frame = |app: &mut AlphaDocs, events| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| app.table_picker(ui));
            },
        );
        output.textures_delta.clear();
    };
    frame(&mut app, vec![]);
    ctx.memory_mut(|m| m.request_focus(Id::new(("table_size", 2usize, 3usize))));
    frame(&mut app, vec![key(Key::Enter, egui::Modifiers::NONE)]);
    assert!(matches!(app.action, Some(Action::Table(3, 4))));
}

#[test]
fn unchanged_frames_reuse_measurements_but_edits_undo_and_dpi_invalidate_them() {
    let (ctx, mut app) = editor();
    let mut output = ctx.run_ui(Default::default(), |_| {});
    output.textures_delta.clear();
    app.session.editor.insert("Original");
    app.ensure_layout(&ctx);
    let pointer = app.layout.as_ref().unwrap().lines.as_ptr();
    let revision = app.layout_revision;
    for _ in 0..10 {
        app.ensure_layout(&ctx);
    }
    assert_eq!(pointer, app.layout.as_ref().unwrap().lines.as_ptr());
    assert_eq!(revision, app.layout_revision);
    app.session.editor.insert(" more");
    app.ensure_layout(&ctx);
    assert_eq!(app.layout.as_ref().unwrap().lines[0].end, 13);
    app.session.editor.undo();
    app.ensure_layout(&ctx);
    assert_eq!(app.layout.as_ref().unwrap().lines[0].end, 8);
    // Simulate a DPI change without relying on a physical second monitor.
    app.layout_dpi = -1.0;
    app.ensure_layout(&ctx);
    assert_eq!(app.layout_dpi, ctx.pixels_per_point());
}

fn image_app() -> (Context, AlphaDocs) {
    let (ctx, mut app) = editor();
    app.prefs.outline = false;
    app.prefs.ruler = false;
    let pixels = image::RgbaImage::from_pixel(8, 8, image::Rgba([40, 120, 210, 255]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    app.session.editor.doc.blocks = vec![
        Block::Image(EmbeddedImage {
            name: "QA image".into(),
            data: bytes.into_inner().into(),
            width: 240.0,
            height: 160.0,
            ..Default::default()
        }),
        Block::Paragraph(Paragraph::default()),
    ];
    app.session.editor.place(
        Position {
            address: Address {
                block: 1,
                row: 0,
                cell: 0,
            },
            offset: 0,
        },
        false,
    );
    (ctx, app)
}
fn image_frame(ctx: &Context, app: &mut AlphaDocs, events: Vec<Event>) {
    image_frame_cursor(ctx, app, events);
}
fn image_frame_cursor(ctx: &Context, app: &mut AlphaDocs, events: Vec<Event>) -> egui::CursorIcon {
    // Exercise the same chrome, input dispatch, fonts, panels and canvas as the
    // production app, while keeping autosave I/O out of isolated UI tests.
    app.last_autosave = Instant::now();
    let mut raw_input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(
            Pos2::ZERO,
            vec2(1000.0, if app.home { 1100.0 } else { 800.0 }),
        )),
        events,
        ..Default::default()
    };
    eframe::App::raw_input_hook(app, ctx, &mut raw_input);
    let mut output = ctx.run_ui(raw_input, |ui| {
        app.update_ui(ui);
    });
    if let Some(update) = &output.platform_output.accesskit_update {
        assert!(
            update.nodes.iter().any(|(id, _)| *id == update.focus),
            "Focused accessibility node must be present in the rendered UI tree"
        );
    }
    output.textures_delta.clear();
    output.platform_output.cursor_icon
}
fn select_image(ctx: &Context, app: &mut AlphaDocs) {
    image_frame(ctx, app, vec![]);
    let pos = ctx
        .read_response(Id::new(("image", &0usize)))
        .unwrap()
        .rect
        .center();
    image_frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(pos),
            pointer(pos, egui::PointerButton::Primary, true),
        ],
    );
    image_frame(
        ctx,
        app,
        vec![pointer(pos, egui::PointerButton::Primary, false)],
    );
    image_frame(ctx, app, vec![]);
}
#[test]
fn single_image_click_selects_without_a_modal_and_dragging_creates_one_undo_step() {
    let (ctx, mut app) = image_app();
    select_image(&ctx, &mut app);
    assert_eq!(app.image_block, Some(0));
    assert!(app.dialog.is_none());
    let pos = ctx
        .read_response(Id::new(("image_resize", 0usize, 4usize)))
        .unwrap()
        .rect
        .center();
    let before = app.session.editor.doc.clone();
    let revision = app.session.editor.revision;
    image_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(pos),
            pointer(pos, egui::PointerButton::Primary, true),
        ],
    );
    image_frame(
        &ctx,
        &mut app,
        vec![Event::PointerMoved(pos + vec2(60.0, 40.0))],
    );
    image_frame(
        &ctx,
        &mut app,
        vec![Event::PointerMoved(pos + vec2(120.0, 80.0))],
    );
    assert_eq!(
        app.session.editor.revision, revision,
        "Drag previews must not edit history"
    );
    image_frame(
        &ctx,
        &mut app,
        vec![pointer(
            pos + vec2(120.0, 80.0),
            egui::PointerButton::Primary,
            false,
        )],
    );
    assert_eq!(app.session.editor.revision, revision + 1);
    let Block::Image(image) = &app.session.editor.doc.blocks[0] else {
        panic!("Image retained");
    };
    assert!(image.width > 240.0);
    assert!((image.width / image.height - 1.5).abs() < 0.001);
    app.session.editor.undo();
    assert_eq!(app.session.editor.doc, before);
}
#[test]
fn selected_image_keyboard_copy_delete_and_undo_preserve_embedded_bytes() {
    let (ctx, mut app) = image_app();
    select_image(&ctx, &mut app);
    let before = app.session.editor.doc.clone();
    send(&ctx, &mut app, vec![Event::Copy]);
    assert!(
        matches!(&app.clipboard.as_ref().unwrap().blocks[0], Block::Image(i) if i.data == match &before.blocks[0] { Block::Image(i) => i.data.clone(), _ => unreachable!() })
    );
    send(
        &ctx,
        &mut app,
        vec![key(Key::Delete, egui::Modifiers::NONE)],
    );
    assert!(app.image_block.is_none());
    assert!(
        !app.session
            .editor
            .doc
            .blocks
            .iter()
            .any(|b| matches!(b, Block::Image(_)))
    );
    app.session.editor.doc.validate().unwrap();
    app.session.editor.undo();
    assert_eq!(app.session.editor.doc, before);
}

#[test]
fn enter_creates_a_close_paragraph_and_shift_enter_a_line_break() {
    let (ctx, mut app) = editor();
    send(
        &ctx,
        &mut app,
        vec![
            Event::Text("First".into()),
            key(Key::Enter, egui::Modifiers::NONE),
            Event::Text("Second".into()),
        ],
    );
    assert_eq!(app.session.editor.doc.blocks.len(), 2);
    assert_eq!(
        app.session
            .editor
            .doc
            .paragraph(Address::default())
            .unwrap()
            .after,
        0.0
    );
    app.ensure_layout(&ctx);
    let paragraph_step = app.layout.as_ref().unwrap().lines[1].rect.top()
        - app.layout.as_ref().unwrap().lines[0].rect.top();
    let (_, mut soft) = editor();
    send(
        &ctx,
        &mut soft,
        vec![
            Event::Text("First".into()),
            key(Key::Enter, egui::Modifiers::SHIFT),
            Event::Text("Second".into()),
        ],
    );
    assert_eq!(soft.session.editor.doc.blocks.len(), 1);
    assert_eq!(soft.session.editor.doc.text(), "First\nSecond");
    soft.ensure_layout(&ctx);
    let soft_step = soft.layout.as_ref().unwrap().lines[1].rect.top()
        - soft.layout.as_ref().unwrap().lines[0].rect.top();
    assert!((paragraph_step - soft_step).abs() < 0.1);
}

#[test]
fn table_drag_moves_the_block_and_preserves_cells_with_one_undo_step() {
    let (ctx, mut app) = editor();
    app.prefs.outline = false;
    app.prefs.ruler = false;
    app.session.editor.insert("Before");
    app.session.editor.insert_table(2, 2);
    app.session.editor.insert("Keep cell text");
    app.session
        .editor
        .doc
        .paragraph_mut(Address {
            block: 2,
            row: 0,
            cell: 0,
        })
        .unwrap()
        .chars = Paragraph::from_text("After").chars;
    image_frame(&ctx, &mut app, vec![]);
    let from = ctx
        .read_response(Id::new(("table_move", 1usize, 0usize)))
        .unwrap()
        .rect
        .center();
    let canvas = ctx.read_response(Id::new("document_canvas")).unwrap().rect;
    let l = app.layout.as_ref().unwrap();
    let scale = 4.0 / 3.0;
    let left =
        (canvas.left() + (canvas.width() - l.size.x * scale) * 0.5).max(canvas.left() + 24.0);
    let after = l
        .lines
        .iter()
        .find(|line| line.address.block == 2)
        .unwrap()
        .rect;
    let to = pos2(
        left + after.left() * scale + 20.0,
        canvas.top() + 12.0 + after.bottom() * scale + 10.0,
    );
    let before = app.session.editor.doc.clone();
    let revision = app.session.editor.revision;
    image_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(from),
            pointer(from, egui::PointerButton::Primary, true),
        ],
    );
    image_frame(&ctx, &mut app, vec![Event::PointerMoved(to)]);
    assert_eq!(app.session.editor.revision, revision);
    image_frame(
        &ctx,
        &mut app,
        vec![pointer(to, egui::PointerButton::Primary, false)],
    );
    assert!(
        matches!(&app.session.editor.doc.blocks[2], Block::Table(rows) if rows[0][0].text() == "Keep cell text")
    );
    assert_eq!(app.session.editor.revision, revision + 1);
    assert_eq!(app.session.editor.caret.address.block, 2);
    app.session.editor.doc.validate().unwrap();
    app.session.editor.undo();
    assert_eq!(app.session.editor.doc, before);
    app.session.editor.redo();
    assert!(matches!(app.session.editor.doc.blocks[2], Block::Table(_)));
}

#[test]
fn sidebar_plus_adds_content_to_the_same_document_and_focuses_its_canvas() {
    let (ctx, mut app) = editor();
    app.prefs.outline = true;
    app.prefs.ruler = false;
    app.session.editor.insert("Original tab");
    app.session.editor.paragraph_format(|p| p.set_heading(1));
    let id = app.session.editor.doc.id.clone();
    image_frame(&ctx, &mut app, vec![]);
    let plus = ctx
        .read_response(Id::new("add_document_tab"))
        .unwrap()
        .rect
        .center();
    image_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(plus),
            pointer(plus, egui::PointerButton::Primary, true),
        ],
    );
    image_frame(
        &ctx,
        &mut app,
        vec![pointer(plus, egui::PointerButton::Primary, false)],
    );
    image_frame(&ctx, &mut app, vec![]);
    assert_eq!(app.session.editor.doc.id, id);
    assert_eq!(app.session.editor.doc.tabs.len(), 2);
    assert_eq!(app.session.editor.doc.active_tab, 1);
    assert_eq!(app.session.editor.typing.size, 11.0);
    assert!(!app.session.editor.typing.bold);
    assert!(ctx.memory(|m| m.has_focus(Id::new("document_canvas"))));
    image_frame(&ctx, &mut app, vec![Event::Text("New tab content".into())]);
    assert_eq!(app.session.editor.doc.text(), "New tab content");
    app.session.editor.switch_tab(0);
    image_frame(&ctx, &mut app, vec![]);
    assert_eq!(app.session.editor.doc.text(), "Original tab");
    image_frame(&ctx, &mut app, vec![key(Key::Z, egui::Modifiers::COMMAND)]);
    assert_eq!(app.session.editor.doc.active_tab, 1);
    assert_eq!(app.session.editor.doc.text(), "");
    assert_eq!(app.session.editor.doc.tabs.len(), 2);
}

#[test]
fn disabled_sidebar_plus_does_not_mutate_a_document_in_viewing_mode() {
    let (ctx, mut app) = editor();
    app.prefs.outline = true;
    app.prefs.ruler = false;
    app.session.viewing = true;
    image_frame(&ctx, &mut app, vec![]);
    let plus = ctx
        .read_response(Id::new("add_document_tab"))
        .unwrap()
        .rect
        .center();
    let original = app.session.editor.doc.clone();
    image_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(plus),
            pointer(plus, egui::PointerButton::Primary, true),
        ],
    );
    image_frame(
        &ctx,
        &mut app,
        vec![pointer(plus, egui::PointerButton::Primary, false)],
    );
    assert_eq!(app.session.editor.doc, original);
}

#[test]
fn complete_pointer_gestures_in_one_frame_move_tables_and_resize_images() {
    coalesced_gestures(false);
}

#[test]
fn press_and_move_in_one_frame_then_release_move_tables_and_resize_images() {
    coalesced_gestures(true);
}

fn coalesced_gestures(split_release: bool) {
    let (ctx, mut app) = editor();
    app.prefs.outline = false;
    app.prefs.ruler = false;
    app.session.editor.insert_table(1, 2);
    app.session.editor.insert("Keep this cell");
    app.session
        .editor
        .doc
        .paragraph_mut(Address {
            block: 1,
            row: 0,
            cell: 0,
        })
        .unwrap()
        .chars = Paragraph::from_text("Following paragraph").chars;
    image_frame(&ctx, &mut app, vec![]);
    let start = ctx
        .read_response(Id::new(("table_move", 0usize, 0usize)))
        .unwrap()
        .rect
        .center();
    let canvas = ctx.read_response(Id::new("document_canvas")).unwrap().rect;
    let l = app.layout.as_ref().unwrap();
    let scale = 4.0 / 3.0;
    let left =
        (canvas.left() + (canvas.width() - l.size.x * scale) * 0.5).max(canvas.left() + 24.0);
    let rect = l
        .lines
        .iter()
        .find(|line| line.address.block == 1)
        .unwrap()
        .rect;
    let end = pos2(
        left + rect.left() * scale + 20.0,
        canvas.top() + 12.0 + rect.bottom() * scale + 16.0,
    );
    let before = app.session.editor.doc.clone();
    let mut events = vec![
        Event::PointerMoved(start),
        pointer(start, egui::PointerButton::Primary, true),
        Event::PointerMoved(end),
    ];
    if !split_release {
        events.push(pointer(end, egui::PointerButton::Primary, false));
    }
    image_frame(&ctx, &mut app, events);
    if split_release {
        assert_eq!(app.session.editor.doc, before);
        image_frame(
            &ctx,
            &mut app,
            vec![pointer(end, egui::PointerButton::Primary, false)],
        );
    }
    assert!(
        matches!(&app.session.editor.doc.blocks[1], Block::Table(rows) if rows[0][0].text() == "Keep this cell")
    );
    app.session.editor.undo();
    assert_eq!(app.session.editor.doc, before);

    let (ctx, mut app) = image_app();
    select_image(&ctx, &mut app);
    let start = ctx
        .read_response(Id::new(("image_resize", 0usize, 4usize)))
        .unwrap()
        .rect
        .center();
    let before = app.session.editor.doc.clone();
    let end = start + vec2(60.0, 40.0);
    let mut events = vec![
        Event::PointerMoved(start),
        pointer(start, egui::PointerButton::Primary, true),
        Event::PointerMoved(end),
    ];
    if !split_release {
        events.push(pointer(end, egui::PointerButton::Primary, false));
    }
    image_frame(&ctx, &mut app, events);
    if split_release {
        assert_eq!(app.session.editor.doc, before);
        image_frame(
            &ctx,
            &mut app,
            vec![pointer(end, egui::PointerButton::Primary, false)],
        );
    }
    assert!(matches!(&app.session.editor.doc.blocks[0], Block::Image(i) if i.width > 240.0));
    app.session.editor.undo();
    assert_eq!(app.session.editor.doc, before);
}

#[test]
#[ignore = "Diagnostic timing; run explicitly with --ignored --nocapture"]
fn full_editor_frame_performance_diagnostic() {
    let (ctx, mut app) = editor();
    let paragraph = Paragraph::from_text(
        &"Measured typography should stay responsive while scrolling. ".repeat(50),
    );
    app.session.editor.doc.blocks = vec![Block::Paragraph(paragraph); 80];
    let words = app.session.editor.doc.counts().0;
    let frame = |app: &mut AlphaDocs| {
        let start = Instant::now();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 960.0))),
                ..Default::default()
            },
            |ui| app.editor_ui(ui),
        );
        output.textures_delta.clear();
        start.elapsed()
    };
    let cold = frame(&mut app);
    for _ in 0..4 {
        frame(&mut app);
    }
    let mut timings: Vec<_> = (0..30).map(|_| frame(&mut app)).collect();
    timings.sort();
    app.session.editor.insert("x");
    let edited = frame(&mut app);
    println!(
        "Full editor UI: {words} words, cold {cold:?}, warm median {:?}, p95 {:?}, edit {edited:?}",
        timings[15], timings[28]
    );
}

#[test]
fn mouse_clicks_select_empty_cells_and_context_click_targets_the_clicked_row() {
    let (ctx, mut app) = editor();
    app.prefs.outline = false;
    app.prefs.ruler = false;
    app.session.editor.insert_table(2, 3);
    let frame = |app: &mut AlphaDocs, events| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0))),
                events,
                ..Default::default()
            },
            |ui| {
                app.input(&ctx);
                egui::CentralPanel::default().show(ui, |ui| {
                    app.ensure_layout(&ctx);
                    app.canvas(ui);
                });
            },
        );
        output.textures_delta.clear();
    };
    frame(&mut app, vec![]);
    let canvas = ctx.read_response(Id::new("document_canvas")).unwrap().rect;
    let layout = app.layout.as_ref().unwrap();
    let scale = 4.0 / 3.0;
    let left =
        (canvas.left() + (canvas.width() - layout.size.x * scale) * 0.5).max(canvas.left() + 24.0);
    let cell_center = |row, cell| {
        let rect = layout
            .objects
            .iter()
            .find_map(|o| match o {
                Object::Border { address, rect, .. }
                    if address.row == row && address.cell == cell =>
                {
                    Some(*rect)
                }
                _ => None,
            })
            .unwrap();
        pos2(
            left + rect.center().x * scale,
            canvas.top() + 12.0 + rect.center().y * scale,
        )
    };
    let clicked = cell_center(1, 2);
    let context = cell_center(0, 1);
    frame(
        &mut app,
        vec![
            Event::PointerMoved(clicked),
            pointer(clicked, egui::PointerButton::Primary, true),
        ],
    );
    frame(
        &mut app,
        vec![pointer(clicked, egui::PointerButton::Primary, false)],
    );
    assert_eq!(
        app.session.editor.caret.address,
        Address {
            block: 0,
            row: 1,
            cell: 2
        }
    );
    frame(&mut app, vec![Event::Text("Last cell".into())]);
    assert_eq!(
        app.session
            .editor
            .doc
            .paragraph(app.session.editor.caret.address)
            .unwrap()
            .text(),
        "Last cell"
    );
    frame(
        &mut app,
        vec![
            Event::PointerMoved(context),
            pointer(context, egui::PointerButton::Secondary, true),
        ],
    );
    frame(
        &mut app,
        vec![pointer(context, egui::PointerButton::Secondary, false)],
    );
    assert_eq!(
        app.session.editor.caret.address,
        Address {
            block: 0,
            row: 0,
            cell: 1
        }
    );
}

#[test]
fn mixed_native_paste_completion_replaces_select_all_and_rejects_stale_selection() {
    for stale in [false, true] {
        let (ctx, mut app) = editor();
        app.session.editor.doc.blocks = vec![
            Block::Paragraph(Paragraph::from_text("Old content")),
            Block::Table(vec![vec![Paragraph::from_text("Last cell")]]),
        ];
        app.session.editor.select_all();
        let before = app.session.editor.doc.clone();
        let (tx, receiver) = std::sync::mpsc::channel();
        app.paste_state = Some(proofing::PasteState {
            receiver,
            image: None,
            source: (
                app.session.editor.doc.id.clone(),
                app.session.editor.doc.tabs[0].id.clone(),
                app.session.editor.revision,
                app.session.editor.anchor,
                app.session.editor.caret,
            ),
        });
        let import = crate::clipboard::parse_html(
            "<p>New <b>text</b></p><table><tr><td>New cell</td></tr></table>",
            &TextStyle::default(),
            450.0,
            None,
        )
        .unwrap();
        tx.send(Ok(import)).ok().unwrap();
        if stale {
            app.session.editor.move_horizontal(false, false, false);
        }
        image_frame(&ctx, &mut app, vec![]);
        if stale {
            assert_eq!(app.session.editor.doc, before);
            assert!(app.status.contains("cancelled"));
        } else {
            assert_eq!(app.session.editor.doc.blocks.len(), 3);
            assert!(app.session.editor.doc.text().contains("New cell"));
            app.session.editor.undo();
            assert_eq!(app.session.editor.doc, before);
            assert!(!app.session.editor.can_undo());
        }
    }
}
#[test]
fn proofing_shortcut_opens_native_tool_and_empty_paste_preserves_selection() {
    let (ctx, mut app) = editor();
    app.session
        .editor
        .insert("This is teh test. She are ready.");
    app.session.editor.select_all();
    let before = app.session.editor.doc.clone();
    app.paste_text("");
    assert_eq!(app.session.editor.doc, before);
    image_frame(
        &ctx,
        &mut app,
        vec![key(Key::X, egui::Modifiers::COMMAND | egui::Modifiers::ALT)],
    );
    assert!(app.proof_state.is_some());
    assert!(app.dialog.is_none());
    assert!(app.proof_state.is_some());
    for _ in 0..3 {
        image_frame(&ctx, &mut app, vec![]);
    }
    assert_eq!(app.session.editor.doc, before);
}

#[test]
fn selected_image_paste_is_atomic_and_import_errors_leave_original_untouched() {
    let (ctx, mut app) = image_app();
    select_image(&ctx, &mut app);
    let before = app.session.editor.doc.clone();
    app.clipboard = Some(ClipboardContent {
        text: "Replacement".into(),
        blocks: vec![
            Block::Paragraph(Paragraph::from_text("Replacement")),
            Block::Table(vec![vec![Paragraph::from_text("Pasted cell")]]),
        ],
    });
    image_frame(&ctx, &mut app, vec![Event::Paste("Replacement".into())]);
    assert!(
        !app.session
            .editor
            .doc
            .blocks
            .iter()
            .any(|b| matches!(b, Block::Image(_)))
    );
    assert!(app.session.editor.doc.text().contains("Pasted cell"));
    app.session.editor.undo();
    assert_eq!(app.session.editor.doc, before);
    assert!(!app.session.editor.can_undo());
    app.image_block = Some(0);
    let (tx, receiver) = std::sync::mpsc::channel();
    app.paste_state = Some(proofing::PasteState {
        receiver,
        image: Some(0),
        source: (
            app.session.editor.doc.id.clone(),
            app.session.editor.doc.tabs[0].id.clone(),
            app.session.editor.revision,
            app.session.editor.anchor,
            app.session.editor.caret,
        ),
    });
    tx.send(Err("Synthetic unsupported clipboard".into()))
        .ok()
        .unwrap();
    image_frame(&ctx, &mut app, vec![]);
    assert_eq!(app.session.editor.doc, before);
    assert!(app.status.contains("Could not paste"));
}

fn click_toolbar_control(ctx: &Context, app: &mut AlphaDocs, rect: Rect) {
    let at = rect.center();
    image_frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(at),
            pointer(at, egui::PointerButton::Primary, true),
        ],
    );
    image_frame(
        ctx,
        app,
        vec![pointer(at, egui::PointerButton::Primary, false)],
    );
    image_frame(ctx, app, vec![]);
    image_frame(ctx, app, vec![]);
}
#[test]
fn compact_menu_shortcut_and_restore_button_change_chrome_without_changing_document() {
    for theme in [Theme::Light, Theme::Dark] {
        let (ctx, mut app) = editor();
        app.prefs.theme = theme;
        app.session
            .editor
            .insert("Keep the selected text and undo history");
        app.session.editor.select_all();
        image_frame(&ctx, &mut app, vec![]);
        image_frame(&ctx, &mut app, vec![]);
        let before = app.session.editor.doc.clone();
        let range = app.session.editor.range();
        let revision = app.session.editor.revision;
        let top = ctx
            .read_response(Id::new("mode_selector"))
            .unwrap()
            .rect
            .top();
        image_frame(
            &ctx,
            &mut app,
            vec![key(
                Key::F,
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            )],
        );
        image_frame(&ctx, &mut app, vec![]);
        assert!(app.prefs.compact_menus);
        assert!(app.dialog.is_none());
        let compact_top = ctx
            .read_response(Id::new("mode_selector"))
            .unwrap()
            .rect
            .top();
        assert!(
            top - compact_top > 40.0,
            "Compact mode must reclaim the title/menu header"
        );
        assert_eq!(app.session.editor.doc, before);
        assert_eq!(app.session.editor.range(), range);
        assert_eq!(app.session.editor.revision, revision);
        let restore = ctx
            .read_response(Id::new("toggle_menus_button"))
            .unwrap()
            .rect;
        click_toolbar_control(&ctx, &mut app, restore);
        assert!(!app.prefs.compact_menus);
        let restored_top = ctx
            .read_response(Id::new("mode_selector"))
            .unwrap()
            .rect
            .top();
        assert!(
            (restored_top - top).abs() < 1.0,
            "Toolbar before {top}, compact {compact_top}, restored {restored_top}"
        );
        assert_eq!(app.session.editor.doc, before);
        assert_eq!(app.session.editor.range(), range);
        app.session.editor.undo();
        assert!(app.session.editor.doc.text().is_empty());
        assert!(!app.session.editor.can_undo());
        let legacy: Preferences = serde_json::from_str("{}").unwrap();
        assert!(!legacy.compact_menus);
        app.prefs.compact_menus = true;
        let saved: Preferences =
            serde_json::from_str(&serde_json::to_string(&app.prefs).unwrap()).unwrap();
        assert!(saved.compact_menus);
    }
}
#[test]
fn mode_selector_rows_switch_real_viewing_and_disabled_suggesting_cannot_activate() {
    let (ctx, mut app) = editor();
    app.session.editor.insert("Read only text");
    image_frame(&ctx, &mut app, vec![]);
    image_frame(&ctx, &mut app, vec![]);
    let original = app.session.editor.doc.clone();
    let revision = app.session.editor.revision;
    let selector = ctx.read_response(Id::new("mode_selector")).unwrap().rect;
    click_toolbar_control(&ctx, &mut app, selector);
    let suggesting = ctx
        .read_response(Id::new(("document_mode", 1)))
        .unwrap()
        .rect;
    click_toolbar_control(&ctx, &mut app, suggesting);
    assert!(!app.session.viewing);
    assert_eq!(app.session.editor.revision, revision);
    let viewing = ctx
        .read_response(Id::new(("document_mode", 2)))
        .unwrap()
        .rect;
    click_toolbar_control(&ctx, &mut app, viewing);
    assert!(app.session.viewing);
    image_frame(
        &ctx,
        &mut app,
        vec![
            Event::Text("Must not edit".into()),
            Event::Paste("Must not paste".into()),
        ],
    );
    assert_eq!(app.session.editor.doc, original);
    click_toolbar_control(
        &ctx,
        &mut app,
        ctx.read_response(Id::new("mode_selector")).unwrap().rect,
    );
    let editing = ctx
        .read_response(Id::new(("document_mode", 0)))
        .unwrap()
        .rect;
    click_toolbar_control(&ctx, &mut app, editing);
    assert!(!app.session.viewing);
    assert_eq!(app.session.editor.doc, original);
    assert_eq!(app.session.editor.revision, revision);
}
#[test]
fn image_toolbar_keeps_mode_and_menu_restore_on_screen_in_compact_layout() {
    let (ctx, mut app) = image_app();
    select_image(&ctx, &mut app);
    let before = app.session.editor.doc.clone();
    for compact in [false, true] {
        app.prefs.compact_menus = compact;
        image_frame(&ctx, &mut app, vec![]);
        image_frame(&ctx, &mut app, vec![]);
        let mode = ctx.read_response(Id::new("mode_selector")).unwrap().rect;
        let toggle = ctx
            .read_response(Id::new("toggle_menus_button"))
            .unwrap()
            .rect;
        assert!(mode.left() > 0.0 && mode.right() < 1000.0);
        assert!(toggle.right() < 1000.0 && toggle.left() > mode.right());
        assert_eq!(app.image_block, Some(0));
        assert_eq!(app.session.editor.doc, before);
    }
}

#[test]
fn outline_click_scrolls_to_a_heading_on_an_unpainted_page() {
    let (ctx, mut app) = editor();
    let mut heading = Paragraph::from_text("Distant chapter");
    heading.set_heading(1);
    app.session.editor.doc.blocks = vec![
        Block::Paragraph(Paragraph::from_text("Start")),
        Block::PageBreak,
        Block::Paragraph(Paragraph::from_text("Middle")),
        Block::PageBreak,
        Block::Paragraph(heading),
    ];
    app.prefs.outline = true;
    for _ in 0..3 {
        image_frame(&ctx, &mut app, vec![]);
    }
    let address = Address {
        block: 4,
        ..Default::default()
    };
    let original = app.session.editor.doc.clone();
    let response = ctx
        .read_response(Id::new(("outline_heading", &address)))
        .unwrap();
    let point = response.rect.center();
    image_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(point),
            pointer(point, egui::PointerButton::Primary, true),
            pointer(point, egui::PointerButton::Primary, false),
        ],
    );
    assert_eq!(app.session.editor.caret.address, address);
    assert!(
        !app.scroll_caret,
        "Offscreen caret geometry must schedule scrolling in the click frame"
    );
    for _ in 0..30 {
        image_frame(&ctx, &mut app, vec![]);
    }
    for time in [2.0, 3.0, 10.0] {
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(time),
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0))),
                ..Default::default()
            },
            |ui| app.update_ui(ui),
        );
        output.textures_delta.clear();
        let area = ctx.read_response(Id::new("document_canvas")).unwrap().rect;
        let line = app
            .layout
            .as_ref()
            .unwrap()
            .caret_line(app.session.editor.caret)
            .unwrap();
        let scale = 4.0 / 3.0 * app.prefs.zoom;
        let y = area.top()
            + 24.0
            + 12.0
            + line.page as f32 * (app.layout.as_ref().unwrap().size.y * scale + 12.0)
            + line.rect.top() * scale;
        assert!(
            (166.0..780.0).contains(&y),
            "Heading must remain visible after delayed native frames: {y}"
        );
    }
    assert_eq!(
        app.session.editor.doc, original,
        "Outline navigation must never apply heading formatting to another paragraph"
    );
}

#[test]
fn narrow_window_comment_cards_stay_beyond_the_page_in_the_horizontal_rail() {
    let (ctx, mut app) = image_app();
    app.session.editor.add_annotation_comment(
        &match &app.session.editor.doc.blocks[0] {
            Block::Image(i) => i.id.clone(),
            _ => panic!(),
        },
        "Beside the page",
    );
    app.prefs.outline = true;
    for _ in 0..3 {
        image_frame(&ctx, &mut app, vec![]);
    }
    let image = ctx.read_response(Id::new(("image", &0usize))).unwrap().rect;
    assert!(app.annotation_rail_x > image.right());
    assert!(
        app.annotation_rail_x > 1000.0,
        "The rail must remain off-screen until horizontally scrolled, not clamped over paper"
    );
}

#[test]
fn alphadocs_about_ownership_and_licence_fit_both_themes_without_document_changes() {
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
        app.session.editor.insert("Keep café 😀 and its styles");
        let original = app.session.editor.doc.clone();
        app.dialog = Some(Dialog::About);
        app.feature_pulse = Some((Instant::now(), feature_search::Target::Dialog));
        let viewport = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 640.0));
        for _ in 0..3 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(viewport),
                    ..Default::default()
                },
                |ui| app.update_ui(ui),
            );
            output.textures_delta.clear();
        }
        let bounds = app.feature_rect.expect("About dialog is shown");
        assert!(
            viewport.contains_rect(bounds),
            "About overflow: {theme:?} {bounds:?}"
        );
        assert_eq!(app.session.editor.doc, original);
    }
}
#[test]
fn previous_native_files_open_without_becoming_a_save_overwrite_target() {
    for extension in [crate::legacy::EXTENSION, "adocs"] {
        let (ctx, mut app) = editor();
        let doc = Document::new("Previous native document café");
        let path = std::env::temp_dir().join(format!("alphadocs-legacy-{}.{extension}", doc.id));
        persistence::save_document(&doc, &path).unwrap();
        let original = std::fs::read(&path).unwrap();
        app.open_path(path.clone());
        assert_eq!(app.session.editor.doc, doc);
        assert_eq!(app.session.source, Some(path.clone()));
        assert_eq!(
            app.session.file,
            if extension == "adocs" {
                Some(path.clone())
            } else {
                None
            }
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert_eq!(
            persistence::library_path(&doc).extension().unwrap(),
            "adocs"
        );
        drop(ctx);
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn homepage_search_and_actions_stay_separate_during_narrow_resizes_and_accept_input() {
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
        let original = app.session.editor.doc.clone();
        let frame = |app: &mut AlphaDocs, width: f32, events: Vec<Event>| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 800.0))),
                    events,
                    ..Default::default()
                },
                |ui| app.update_ui(ui),
            );
            output.textures_delta.clear();
        };
        for width in [1920.0, 1440.0, 1000.0, 960.0, 1280.0, 960.0] {
            for _ in 0..3 {
                frame(&mut app, width, vec![]);
                let b = ctx
                    .data_mut(|d| {
                        d.get_temp::<homepage::HeaderBounds>(Id::new("homepage_header_bounds"))
                    })
                    .unwrap();
                assert!(
                    b.row.expand(0.1).contains_rect(b.search),
                    "Search outside row: {theme:?} {width}: {:?} {:?}",
                    b.row,
                    b.search
                );
                assert!(b.search.width() > 80.0);
                assert!(
                    b.brand.right() + 12.0 <= b.search.left(),
                    "Brand/search overlap at {width}"
                );
                assert!(
                    b.search.right() + 12.0 <= b.actions[0].left(),
                    "Search/Open overlap at {width}"
                );
                for (i, rect) in b.actions.iter().enumerate() {
                    assert!(
                        b.row.expand(0.1).contains_rect(*rect),
                        "Action outside row at {width}: {rect:?}"
                    );
                    for other in &b.actions[i + 1..] {
                        assert!(!rect.intersects(*other), "Action overlap at {width}");
                    }
                }
            }
        }
        let b = ctx
            .data_mut(|d| d.get_temp::<homepage::HeaderBounds>(Id::new("homepage_header_bounds")))
            .unwrap();
        let pos = b.search.center();
        frame(
            &mut app,
            960.0,
            vec![
                Event::PointerMoved(pos),
                pointer(pos, egui::PointerButton::Primary, true),
                pointer(pos, egui::PointerButton::Primary, false),
            ],
        );
        frame(&mut app, 960.0, vec![Event::Text("meeting notes".into())]);
        assert_eq!(app.search, "meeting notes");
        let pos = b.actions[2].center();
        frame(
            &mut app,
            960.0,
            vec![
                Event::PointerMoved(pos),
                pointer(pos, egui::PointerButton::Primary, true),
                pointer(pos, egui::PointerButton::Primary, false),
            ],
        );
        assert!(matches!(app.dialog, Some(Dialog::About)));
        assert_eq!(app.session.editor.doc, original);
    }
}

#[test]
fn break_image_drop_inside_wrapped_paragraph_keeps_text_styles_and_one_undo() {
    for coalesced in [false, true] {
        let (ctx, mut app) = image_app();
        let text = "A measured paragraph with enough styled Unicode text café e\u{301} to wrap across many lines. ".repeat(8);
        let mut paragraph = Paragraph::from_text(&text);
        paragraph.chars[2].style.bold = true;
        app.session.editor.doc.blocks[1] = Block::Paragraph(paragraph);
        select_image(&ctx, &mut app);
        let original = app.session.editor.doc.clone();
        let layout = app.layout.as_ref().unwrap();
        let target = layout
            .lines
            .iter()
            .filter(|l| l.address.block == 1)
            .nth(2)
            .unwrap();
        let offset = target.start;
        assert!(offset > 0);
        let image_rect = ctx.read_response(Id::new(("image", &0usize))).unwrap().rect;
        let logical_image = layout
            .objects
            .iter()
            .find_map(|o| match o {
                Object::Image { rect, .. } => Some(*rect),
                _ => None,
            })
            .unwrap();
        let scale = 4.0 / 3.0;
        let origin = image_rect.min - logical_image.min.to_vec2() * scale;
        let to = origin + target.rect.left_top().to_vec2() * scale + vec2(16.0, 2.0);
        let from = ctx
            .read_response(Id::new(("image_move", 0usize)))
            .unwrap()
            .rect
            .center();
        let mut events = vec![
            Event::PointerMoved(from),
            pointer(from, egui::PointerButton::Primary, true),
            Event::PointerMoved(to),
        ];
        if coalesced {
            events.push(pointer(to, egui::PointerButton::Primary, false));
        }
        image_frame(&ctx, &mut app, events);
        if !coalesced {
            assert_eq!(app.session.editor.doc, original);
            image_frame(
                &ctx,
                &mut app,
                vec![pointer(to, egui::PointerButton::Primary, false)],
            );
        }
        let blocks = &app.session.editor.doc.blocks;
        assert!(matches!(blocks[1], Block::Image(_)));
        let (Block::Paragraph(left), Block::Paragraph(right)) = (&blocks[0], &blocks[2]) else {
            panic!()
        };
        assert_eq!(left.chars.len(), offset);
        assert_eq!(format!("{}{}", left.text(), right.text()), text);
        assert!(left.chars[2].style.bold);
        app.session.editor.doc.validate().unwrap();
        app.session.editor.undo();
        assert_eq!(app.session.editor.doc, original);
        assert!(!app.session.editor.can_undo());
        app.session.editor.redo();
        assert!(matches!(app.session.editor.doc.blocks[1], Block::Image(_)));
    }
}
