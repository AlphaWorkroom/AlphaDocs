use crate::{compat, engine::Editor, model::*, persistence};
fn pos(block: usize, offset: usize) -> Position {
    Position {
        address: Address {
            block,
            row: 0,
            cell: 0,
        },
        offset,
    }
}
fn editor(text: &str) -> Editor {
    let mut d = Document::new("Annotation regression");
    d.blocks = vec![Block::Paragraph(Paragraph::from_text(text))];
    Editor::new(d)
}
#[test]
fn annotations_transform_at_actual_edits_split_join_and_undo_without_losing_threads() {
    let mut e = editor("abcde");
    let id = e
        .annotate_text((pos(0, 1), pos(0, 4)), Some("Range comment"), Some("😀"))
        .unwrap();
    let before = e.doc.clone();
    e.place(pos(0, 0), false);
    e.insert("X");
    let n = &e.doc.paragraph(Address::default()).unwrap().annotations[0];
    assert_eq!((n.start, n.end), (2, 5));
    e.undo();
    assert_eq!(e.doc, before);
    e.place(pos(0, 2), false);
    e.enter();
    let a = &e.doc.paragraph(pos(0, 0).address).unwrap().annotations[0];
    let b = &e.doc.paragraph(pos(1, 0).address).unwrap().annotations[0];
    assert_eq!((a.start, a.end, b.start, b.end), (1, 2, 0, 2));
    assert_eq!(a.id, b.id);
    let cid = a.comments[0].id.clone();
    e.reply_annotation_comment(&id, &cid, "Reply café");
    e.toggle_annotation_reaction(&id, Some(&cid), "👍");
    for block in [0, 1] {
        let n = &e.doc.paragraph(pos(block, 0).address).unwrap().annotations[0];
        assert_eq!(n.comments[0].replies[0], "Reply café");
        assert_eq!(n.comments[0].reactions[0], "👍");
    }
    e.place(pos(1, 0), false);
    e.delete(true);
    assert_eq!(e.doc.text(), "abcde");
    e.doc.validate().unwrap();
    let before_delete = e.doc.clone();
    e.anchor = pos(0, 1);
    e.caret = pos(0, 4);
    e.delete_selection();
    e.doc.validate().unwrap();
    let p = e.doc.paragraph(Address::default()).unwrap();
    assert_eq!(p.text(), "ae");
    assert!(p.annotations.iter().all(|n| n.start == n.end));
    assert!(!p.annotations[0].comments.is_empty());
    e.undo();
    assert_eq!(e.doc, before_delete);
}
#[test]
fn annotation_graphemes_copies_tabs_and_native_docx_roundtrips_are_preserved() {
    let mut e = editor("é🙂xyz");
    let id = e
        .annotate_text(
            (pos(0, 0), pos(0, 1)),
            Some("Accented range & quotes"),
            None,
        )
        .unwrap();
    e.place(pos(0, 1), false);
    e.insert("\u{301}");
    e.doc.validate().unwrap();
    assert_eq!(
        e.doc.paragraph(Address::default()).unwrap().annotations[0].end,
        2
    );
    e.anchor = pos(0, 0);
    e.caret = pos(0, 2);
    let fragment = e.copy_fragment();
    e.place(pos(0, 6), false);
    e.paste_fragment(&fragment);
    e.doc.validate().unwrap();
    assert!(
        e.doc
            .paragraph(Address::default())
            .unwrap()
            .annotations
            .iter()
            .any(|n| n.id != id)
    );
    e.duplicate_tab(0);
    let copied = &e.doc.paragraph(Address::default()).unwrap().annotations[0];
    assert_ne!(copied.id, id);
    let path = std::env::temp_dir().join(format!("annotation-{}.adocs", e.doc.id));
    persistence::save_document(&e.doc, &path).unwrap();
    let d = persistence::load_document(&path).unwrap();
    assert_eq!(d, e.doc);
    let word = path.with_extension("docx");
    compat::docx::export(&d, &word).unwrap();
    let (reopened, warnings) = compat::docx::import(&word).unwrap();
    assert!(warnings.is_empty());
    for i in 0..d.tabs.len() {
        assert_eq!(d.tab_blocks(i), reopened.tab_blocks(i));
    }
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(word).unwrap();
}
#[test]
fn cell_annotations_follow_table_moves_and_refuse_running_text_or_invalid_graphemes() {
    let mut e = editor("Header paragraph");
    e.insert_table(2, 2);
    e.insert("Cell 😀 text");
    let a = e.caret.address;
    let id = e
        .annotate_text(
            (
                Position {
                    address: a,
                    offset: 0,
                },
                Position {
                    address: a,
                    offset: 6,
                },
            ),
            Some("Cell comment"),
            None,
        )
        .unwrap();
    let original = e.doc.clone();
    e.move_table(a.block, e.doc.blocks.len());
    let (_, notes) = e
        .doc
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::Table(rows) => Some(((), &rows[0][0].annotations)),
            _ => None,
        })
        .unwrap();
    assert_eq!(notes[0].id, id);
    e.undo();
    assert_eq!(e.doc, original);
    assert!(
        e.annotate_text((pos(usize::MAX, 0), pos(usize::MAX, 1)), Some("No"), None)
            .is_none()
    );
    let mut e = editor("a\u{301}b");
    assert!(
        e.annotate_text((pos(0, 1), pos(0, 2)), Some("Invalid boundary"), None)
            .is_none()
    );
}
