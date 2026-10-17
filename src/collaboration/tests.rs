use super::{live, replica::Replica};
use crate::{engine::Editor, model::*};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
fn fixture() -> Document {
    let mut doc = Document::new("Concurrent QA");
    doc.blocks = vec![
        Block::Paragraph(Paragraph::from_text("Hello world")),
        Block::Table(vec![vec![
            Paragraph::from_text("Cell one"),
            Paragraph::from_text("Cell two"),
        ]]),
    ];
    doc
}
#[test]
fn editable_drawing_sources_refresh_remote_previews_and_share_unchanged_payloads() {
    let context = eframe::egui::Context::default();
    let source = |text: &str| ObjectSource::Drawing {
        kind: ShapeKind::TextBox,
        text: Box::new(Paragraph::from_text(text)),
        fill: None,
        border: CellBorder::default(),
    };
    let mut document = fixture();
    let image = crate::editable_objects::prepare(&context, source("Before"), 200.0, 60.0).unwrap();
    let id = image.id.clone();
    document.blocks.push(Block::Image(image));
    let mut sender = Replica::new(&document).unwrap();
    let mut receiver = Replica::load(&sender.save()).unwrap();
    let before = receiver.project(&document).unwrap();
    let clock = receiver.vector();
    let mut replacement =
        crate::editable_objects::prepare(&context, source("After"), 200.0, 60.0).unwrap();
    replacement.id = id;
    document.blocks[2] = Block::Image(replacement);
    sender.commit(&document).unwrap();
    receiver.accept(&sender.diff(&clock).unwrap()).unwrap();
    let after = receiver.project(&document).unwrap();
    let Block::Image(before_image) = &before.blocks[2] else {
        panic!()
    };
    let Block::Image(after_image) = &after.blocks[2] else {
        panic!()
    };
    assert_ne!(before_image.data, after_image.data);
    assert_eq!(
        after_image.editable,
        document
            .blocks
            .get(2)
            .and_then(|b| if let Block::Image(i) = b {
                i.editable.clone()
            } else {
                None
            })
    );
    let clock = receiver.vector();
    document.paragraph_mut(Address::default()).unwrap().chars[0].ch = 'J';
    sender.commit(&document).unwrap();
    receiver.accept(&sender.diff(&clock).unwrap()).unwrap();
    let unchanged = receiver.project(&document).unwrap();
    let Block::Image(unchanged_image) = &unchanged.blocks[2] else {
        panic!()
    };
    assert_eq!(after_image.data.as_ptr(), unchanged_image.data.as_ptr());
    assert!(Arc::ptr_eq(
        &after_image.editable.as_ref().unwrap().svg,
        &unchanged_image.editable.as_ref().unwrap().svg
    ));
}
fn pair(
    doc: &Document,
) -> (
    Editor,
    Editor,
    Arc<Mutex<live::Room>>,
    Arc<Mutex<live::Room>>,
) {
    let seed = Replica::new(doc).unwrap().save();
    let a = Arc::new(Mutex::new(live::Room::new(
        Replica::load(&seed).unwrap(),
        true,
        "A".into(),
    )));
    let b = Arc::new(Mutex::new(live::Room::new(
        Replica::load(&seed).unwrap(),
        true,
        "B".into(),
    )));
    let (mut x, mut y) = (Editor::new(doc.clone()), Editor::new(doc.clone()));
    x.attach_collaboration(a.clone()).unwrap();
    y.attach_collaboration(b.clone()).unwrap();
    (x, y, a, b)
}
fn exchange(a: &Arc<Mutex<live::Room>>, b: &Arc<Mutex<live::Room>>) {
    let (mut a, mut b) = (a.lock().unwrap(), b.lock().unwrap());
    let au = a.replica.diff(&b.replica.vector()).unwrap();
    let bu = b.replica.diff(&a.replica.vector()).unwrap();
    b.replica.accept(&au).unwrap();
    a.replica.accept(&bu).unwrap();
}
#[test]
fn simultaneous_unicode_edits_converge_and_local_undo_preserves_remote_work() {
    let doc = fixture();
    let (mut x, mut y, a, b) = pair(&doc);
    let p = Position {
        address: Address::default(),
        offset: 6,
    };
    x.place(p, false);
    y.place(p, false);
    x.insert("👩‍🔬 e\u{301} ");
    y.insert("שלום ");
    exchange(&a, &b);
    exchange(&b, &a);
    x.refresh_collaboration(true);
    y.refresh_collaboration(true);
    assert_eq!(x.doc, y.doc);
    assert!(x.doc.text().contains("👩‍🔬 e\u{301} "));
    assert!(x.doc.text().contains("שלום "));
    x.undo();
    exchange(&a, &b);
    y.refresh_collaboration(true);
    assert_eq!(x.doc, y.doc);
    assert!(!x.doc.text().contains("👩‍🔬"));
    assert!(x.doc.text().contains("שלום "));
    x.redo();
    exchange(&b, &a);
    x.refresh_collaboration(true);
    y.refresh_collaboration(true);
    assert_eq!(x.doc, y.doc);
}
#[test]
fn rich_marks_cells_tabs_assets_and_replica_reload_keep_native_content() {
    let mut doc = fixture();
    doc.blocks.push(Block::Image(EmbeddedImage {
        data: include_bytes!("../../assets/alphadocs-icon.png")
            .to_vec()
            .into(),
        width: 72.0,
        height: 72.0,
        ..Default::default()
    }));
    let (mut x, mut y, a, b) = pair(&doc);
    x.anchor = Position {
        address: Address::default(),
        offset: 0,
    };
    x.caret = Position {
        address: Address::default(),
        offset: 5,
    };
    y.anchor = x.anchor;
    y.caret = x.caret;
    x.format(|s| s.bold = true);
    y.format(|s| s.italic = true);
    exchange(&a, &b);
    x.refresh_collaboration(true);
    y.refresh_collaboration(true);
    assert_eq!(x.doc, y.doc);
    let p = x.doc.paragraph(Address::default()).unwrap();
    assert!(p.chars[0].style.bold && p.chars[0].style.italic);
    x.place(
        Position {
            address: Address {
                block: 1,
                row: 0,
                cell: 0,
            },
            offset: 8,
        },
        false,
    );
    y.place(
        Position {
            address: Address {
                block: 1,
                row: 0,
                cell: 1,
            },
            offset: 8,
        },
        false,
    );
    x.insert(" A");
    y.insert(" B");
    exchange(&a, &b);
    x.refresh_collaboration(true);
    y.refresh_collaboration(true);
    assert_eq!(x.doc, y.doc);
    x.add_tab();
    x.insert("Other tab");
    exchange(&a, &b);
    y.refresh_collaboration(true);
    assert_eq!(y.doc.active_tab, 0);
    assert_eq!(x.doc.active_tab, 1);
    let bytes = a.lock().unwrap().replica.save();
    let mut loaded = Replica::load(&bytes).unwrap();
    let reopened = loaded.project(&x.doc).unwrap();
    assert_eq!(reopened, x.doc);
    let Block::Image(image) = &reopened.tabs[0].blocks[2] else {
        panic!("image retained")
    };
    assert_eq!(
        image.data.as_slice(),
        include_bytes!("../../assets/alphadocs-icon.png")
    );
}
#[test]
fn cursors_follow_insertions_and_viewers_cannot_mutate_through_engine() {
    let doc = fixture();
    let (mut x, mut y, a, b) = pair(&doc);
    y.place(
        Position {
            address: Address::default(),
            offset: 6,
        },
        false,
    );
    x.insert("Before ");
    exchange(&a, &b);
    y.refresh_collaboration(true);
    assert_eq!(y.caret.offset, 13);
    b.lock().unwrap().writable = false;
    let before = y.doc.clone();
    y.insert("denied");
    y.undo();
    assert_eq!(y.doc, before);
}
#[test]
fn multibyte_and_surrogate_cursors_comments_and_selection_stay_at_grapheme_boundaries() {
    let mut doc = fixture();
    doc.blocks[0] = Block::Paragraph(Paragraph::from_text("é👩‍🔬e\u{301} café"));
    let (mut x, mut y, a, b) = pair(&doc);
    x.annotate_text(
        (
            Position {
                address: Address::default(),
                offset: 7,
            },
            Position {
                address: Address::default(),
                offset: 11,
            },
        ),
        Some("café"),
        None,
    )
    .unwrap();
    exchange(&a, &b);
    y.refresh_collaboration(true);
    y.place(
        Position {
            address: Address::default(),
            offset: 4,
        },
        false,
    );
    x.place(
        Position {
            address: Address::default(),
            offset: 0,
        },
        false,
    );
    x.insert("שלום ");
    exchange(&a, &b);
    y.refresh_collaboration(true);
    assert_eq!(y.caret.offset, 9);
    x.place(
        Position {
            address: Address::default(),
            offset: 16,
        },
        false,
    );
    x.insert("!");
    exchange(&a, &b);
    y.refresh_collaboration(true);
    let p = y.doc.paragraph(Address::default()).unwrap();
    let n = &p.annotations[0];
    assert_eq!(
        p.chars[n.start..n.end]
            .iter()
            .map(|c| c.ch)
            .collect::<String>(),
        "café"
    );
    assert_eq!(p.snap(y.caret.offset), y.caret.offset);
    y.doc.validate().unwrap();
}
#[test]
fn remote_text_edits_reuse_untouched_rich_payload_and_original_image_storage() {
    let mut doc = fixture();
    doc.blocks.push(Block::Image(EmbeddedImage {
        data: include_bytes!("../../assets/alphadocs-icon.png")
            .to_vec()
            .into(),
        width: 72.0,
        height: 72.0,
        ..Default::default()
    }));
    let (mut x, mut y, a, b) = pair(&doc);
    let Block::Table(rows) = &y.doc.blocks[1] else {
        panic!()
    };
    let chars = rows[0][0].chars.as_ptr();
    let Block::Image(image) = &y.doc.blocks[2] else {
        panic!()
    };
    let media = image.data.as_ptr();
    x.insert("Remote ");
    exchange(&a, &b);
    y.refresh_collaboration(true);
    let Block::Table(rows) = &y.doc.blocks[1] else {
        panic!()
    };
    assert_eq!(chars, rows[0][0].chars.as_ptr());
    let Block::Image(image) = &y.doc.blocks[2] else {
        panic!()
    };
    assert_eq!(media, image.data.as_ptr());
}
#[test]
fn concurrent_comments_keep_stable_ranges_and_replies_survive_reordered_duplicate_updates() {
    let doc = fixture();
    let (mut x, mut y, a, b) = pair(&doc);
    x.anchor = Position {
        address: Address::default(),
        offset: 6,
    };
    x.caret = Position {
        address: Address::default(),
        offset: 11,
    };
    let id = x
        .annotate_text((x.anchor, x.caret), Some("Anchored to world"), None)
        .unwrap();
    assert!(
        !x.doc
            .paragraph(Address::default())
            .unwrap()
            .annotations
            .is_empty(),
        "{}",
        a.lock().unwrap().status
    );
    exchange(&a, &b);
    y.refresh_collaboration(true);
    assert!(
        !y.doc
            .paragraph(Address::default())
            .unwrap()
            .annotations
            .is_empty(),
        "{}; {}",
        a.lock().unwrap().status,
        b.lock().unwrap().status
    );
    let note = y.doc.paragraph(Address::default()).unwrap().annotations[0].clone();
    let cid = note.comments[0].id.clone();
    assert_eq!(note.comments[0].author, "A");
    x.place(
        Position {
            address: Address::default(),
            offset: 0,
        },
        false,
    );
    y.place(
        Position {
            address: Address::default(),
            offset: 0,
        },
        false,
    );
    x.insert("A ");
    y.insert("B ");
    x.reply_annotation_comment(&id, &cid, "first reply");
    y.reply_annotation_comment(&id, &cid, "second reply");
    let au = a.lock().unwrap().replica.save();
    let bu = b.lock().unwrap().replica.save();
    let mut c = Replica::load(&au).unwrap();
    c.accept(&bu).unwrap();
    c.accept(&au).unwrap();
    c.accept(&bu).unwrap();
    exchange(&b, &a);
    exchange(&a, &b);
    x.refresh_collaboration(true);
    y.refresh_collaboration(true);
    assert_eq!(x.doc, y.doc);
    let materialized = c.project(&x.doc).unwrap();
    assert_eq!(materialized, x.doc);
    let p = x.doc.paragraph(Address::default()).unwrap();
    let n = &p.annotations[0];
    assert_eq!(
        p.chars[n.start..n.end]
            .iter()
            .map(|c| c.ch)
            .collect::<String>(),
        "world"
    );
    assert_eq!(n.comments[0].replies.len(), 2);
}
#[test]
fn three_replicas_merge_partitioned_text_deletion_formatting_and_structure() {
    let doc = fixture();
    let (mut x, mut y, a, b) = pair(&doc);
    let seed = a.lock().unwrap().replica.save();
    let c = Arc::new(Mutex::new(live::Room::new(
        Replica::load(&seed).unwrap(),
        true,
        "C".into(),
    )));
    let mut z = Editor::new(doc);
    z.attach_collaboration(c.clone()).unwrap();
    x.insert("Left ");
    y.place(
        Position {
            address: Address::default(),
            offset: 11,
        },
        false,
    );
    y.insert(" Right");
    z.anchor = Position {
        address: Address::default(),
        offset: 0,
    };
    z.caret = Position {
        address: Address::default(),
        offset: 5,
    };
    z.format(|s| s.bold = true);
    x.insert_table(2, 2); // concurrent body structure and text updates
    exchange(&c, &b);
    exchange(&a, &c);
    exchange(&b, &a);
    exchange(&a, &c);
    exchange(&b, &c);
    x.refresh_collaboration(true);
    y.refresh_collaboration(true);
    z.refresh_collaboration(true);
    assert_eq!(x.doc, y.doc);
    assert_eq!(y.doc, z.doc);
    assert!(x.doc.text().contains("Left ") && x.doc.text().contains(" Right"));
    x.doc.validate().unwrap();
}
#[test]
fn real_encrypted_live_session_merges_two_editors_and_revokes_access() {
    let doc = fixture();
    let host = live::host(doc.clone(), "127.0.0.1".parse().unwrap(), "Host".into())
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .unwrap();
    let token = host.edit_invite.as_ref().unwrap().encode().unwrap();
    assert_eq!(
        format!("{:?}", host.edit_invite.as_ref().unwrap()),
        "Invite([redacted])"
    );
    let peer = live::join(live::Invite::parse(&token).unwrap(), "Peer".into())
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .unwrap();
    let (mut x, mut y) = (Editor::new(doc.clone()), Editor::new(doc));
    x.attach_collaboration(host.room.clone()).unwrap();
    y.attach_collaboration(peer.room.clone()).unwrap();
    x.insert("Host ");
    y.insert("Peer ");
    for _ in 0..80 {
        x.refresh_collaboration(true);
        y.refresh_collaboration(true);
        if x.doc == y.doc && x.doc.text().contains("Host ") && x.doc.text().contains("Peer ") {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(x.doc, y.doc);
    assert!(x.doc.text().contains("Peer ") && x.doc.text().contains("Host "));
    host.pause_transport(true);
    y.insert("Offline peer ");
    x.insert("Host during outage ");
    for _ in 0..80 {
        if !peer.room.lock().unwrap().connected {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!peer.room.lock().unwrap().connected);
    host.pause_transport(false);
    for _ in 0..80 {
        x.refresh_collaboration(true);
        y.refresh_collaboration(true);
        if x.doc == y.doc && peer.room.lock().unwrap().connected {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(x.doc, y.doc);
    assert!(x.doc.text().contains("Offline peer ") && x.doc.text().contains("Host during outage "));
    host.revoke();
    for _ in 0..80 {
        if peer.room.lock().unwrap().rejected {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(peer.room.lock().unwrap().rejected);
    let before = y.doc.clone();
    y.insert("denied");
    assert_eq!(y.doc, before);
}
