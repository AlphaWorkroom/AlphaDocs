use super::*;

fn editor() -> (Context, AlphaDocs) {
    let ctx = Context::default();
    ctx.enable_accesskit();
    fonts::install(&ctx);
    let mut app = AlphaDocs::fresh(&ctx, Preferences::default(), vec![]);
    app.home = false;
    (ctx, app)
}

fn frame(ctx: &Context, app: &mut AlphaDocs, events: Vec<Event>) {
    app.last_autosave = Instant::now();
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

fn settle(ctx: &Context, app: &mut AlphaDocs) {
    frame(ctx, app, vec![]);
    frame(ctx, app, vec![]);
}

fn click_id(ctx: &Context, app: &mut AlphaDocs, id: &str) {
    let rect = ctx
        .data_mut(|d| d.get_temp::<Rect>(Id::new(id)))
        .unwrap_or_else(|| panic!("missing response bounds for {id}"));
    frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(rect.center()),
            Event::PointerButton {
                pos: rect.center(),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            Event::PointerButton {
                pos: rect.center(),
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

#[test]
fn page_setup_cancel_apply_undo_and_read_only_are_real_ui_flows() {
    let (ctx, mut app) = editor();
    let original = app.session.editor.doc.page.clone();

    app.dialog = Some(Dialog::Page);
    settle(&ctx, &mut app);
    click_id(&ctx, &mut app, "page_pageless");
    assert!(app.page_draft.pageless);
    click_id(&ctx, &mut app, "page_cancel");
    assert_eq!(app.session.editor.doc.page, original);
    assert!(app.dialog.is_none());

    app.page_draft = original.clone();
    app.page_draft.paper = Paper::Letter;
    app.dialog = Some(Dialog::Page);
    settle(&ctx, &mut app);
    click_id(&ctx, &mut app, "page_ok");
    assert_eq!(app.session.editor.doc.page.paper, Paper::Letter);
    assert!(app.session.editor.can_undo());
    frame(&ctx, &mut app, vec![key(Key::Z, egui::Modifiers::COMMAND)]);
    assert_eq!(app.session.editor.doc.page, original);

    app.session.viewing = true;
    app.page_draft = original.clone();
    app.page_draft.paper = Paper::Legal;
    app.dialog = Some(Dialog::Page);
    settle(&ctx, &mut app);
    click_id(&ctx, &mut app, "page_ok");
    assert_eq!(app.session.editor.doc.page, original);
    assert!(
        app.dialog.is_some(),
        "a disabled Apply must leave the dialog open"
    );
}

#[test]
fn translation_guard_rejects_disabled_action_without_starting_work() {
    let (ctx, mut app) = editor();
    app.prefs.online_translation = false;
    app.open_tool(tools::Tool::Translate);
    assert!(app.dialog.is_none());
    frame(&ctx, &mut app, vec![]);
}

#[test]
fn translation_stays_hidden_in_tools_and_feature_search_when_disabled() {
    let (ctx, mut app) = editor();
    app.prefs.online_translation = false;
    settle(&ctx, &mut app);
    let tools = ctx
        .data_mut(|d| d.get_temp::<Rect>(Id::new("tools_menu")))
        .expect("Tools menu bounds");
    frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(tools.center()),
            Event::PointerButton {
                pos: tools.center(),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            Event::PointerButton {
                pos: tools.center(),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    settle(&ctx, &mut app);
    assert!(
        ctx.data_mut(|d| d.get_temp::<Rect>(Id::new(("menu_bounds", "Translate document…"))))
            .is_none()
    );

    app.open_feature_search();
    app.feature_query = "translate document".into();
    settle(&ctx, &mut app);
    frame(&ctx, &mut app, vec![key(Key::Enter, egui::Modifiers::NONE)]);
    assert!(app.feature_search_open);
    assert!(app.dialog.is_none());
}

#[test]
fn disabled_tools_rows_keep_the_popup_open_and_do_not_dispatch_actions() {
    for label in ["Review suggested edits", "eSignature", "Linked objects"] {
        let (ctx, mut app) = editor();
        settle(&ctx, &mut app);
        click_id(&ctx, &mut app, "tools_menu");
        settle(&ctx, &mut app);
        assert!(egui::Popup::is_any_open(&ctx));
        let row = ctx
            .data_mut(|d| d.get_temp::<Rect>(Id::new(("menu_bounds", label))))
            .unwrap_or_else(|| panic!("missing disabled menu row {label}"));
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(row.center()),
                Event::PointerButton {
                    pos: row.center(),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                Event::PointerButton {
                    pos: row.center(),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert!(
            egui::Popup::is_any_open(&ctx),
            "disabled {label} dismissed Tools"
        );
        assert!(app.dialog.is_none());
        assert!(app.action.is_none());
    }
}

#[test]
fn citation_ui_adds_source_inserts_and_undoes_text_and_keeps_native_metadata() {
    let (ctx, mut app) = editor();
    app.session.editor.insert("Body text.");
    app.session.editor.place(Position::default(), false);
    app.open_tool(tools::Tool::Citations);
    settle(&ctx, &mut app);

    click_id(&ctx, &mut app, "citations_add");
    settle(&ctx, &mut app);
    click_id(&ctx, &mut app, "citation_title");
    frame(&ctx, &mut app, vec![Event::Text("A Book".into())]);
    click_id(&ctx, &mut app, "citations_save");
    assert_eq!(app.session.editor.doc.citations.len(), 1);
    assert_eq!(app.session.editor.doc.citations[0].title, "A Book");

    settle(&ctx, &mut app);
    click_id(&ctx, &mut app, "citations_cite_0");
    assert!(app.session.editor.doc.text().starts_with("(A Book, n.d.)"));
    frame(&ctx, &mut app, vec![key(Key::Z, egui::Modifiers::COMMAND)]);
    assert_eq!(app.session.editor.doc.text(), "Body text.");

    settle(&ctx, &mut app);
    click_id(&ctx, &mut app, "citations_bibliography");
    assert!(app.session.editor.doc.text().contains("References"));
    frame(&ctx, &mut app, vec![key(Key::Z, egui::Modifiers::COMMAND)]);
    assert_eq!(app.session.editor.doc.text(), "Body text.");
    let bytes = serde_json::to_vec(&app.session.editor.doc).unwrap();
    let reopened: Document = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(reopened.citations, app.session.editor.doc.citations);
}

#[test]
fn personal_dictionary_accepts_unicode_text_events_without_editing_document() {
    let (ctx, mut app) = editor();
    let doc = app.session.editor.doc.clone();
    let revision = app.session.editor.revision;
    app.open_tool(tools::Tool::PersonalDictionary);
    settle(&ctx, &mut app);
    click_id(&ctx, &mut app, "personal_dictionary_input");
    frame(&ctx, &mut app, vec![Event::Text("naïve-café".into())]);
    assert_eq!(app.tools.query_for_test(), "naïve-café");
    assert_eq!(app.session.editor.doc, doc);
    assert_eq!(app.session.editor.revision, revision);
}

#[test]
fn notification_and_accessibility_controls_do_not_edit_a_read_only_document() {
    for tool in [tools::Tool::Notifications, tools::Tool::Accessibility] {
        let (ctx, mut app) = editor();
        app.session.viewing = true;
        let doc = app.session.editor.doc.clone();
        let revision = app.session.editor.revision;
        app.open_tool(tool);
        settle(&ctx, &mut app);
        let id = if tool == tools::Tool::Notifications {
            "notifications_nearby"
        } else {
            "accessibility_text"
        };
        click_id(&ctx, &mut app, id);
        assert_eq!(app.session.editor.doc, doc);
        assert_eq!(app.session.editor.revision, revision);
    }
}
