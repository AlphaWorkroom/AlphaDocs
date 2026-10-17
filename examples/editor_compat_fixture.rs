//! Original, isolated QA fixtures. Never reads a real user document/profile.
use alphadocs::{compat, engine::Editor, fonts, layout::Layout, model::*, persistence};
fn main() -> Result<(), String> {
    let output = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .ok_or("Provide a QA output directory")?;
    std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let mut editor = Editor::new(Document::new("The Last Light"));
    let mut title = Paragraph::from_text("The Last Light");
    title.set_heading(7);
    let mut chapter = Paragraph::from_text("Chapter One — The Watchkeeper");
    chapter.set_heading(2);
    chapter.box_style = Some(BoxStyle {
        fill: None,
        borders: [
            CellBorder {
                width: 0.0,
                ..Default::default()
            },
            CellBorder {
                colour: [11, 87, 208],
                width: 1.0,
            },
            CellBorder {
                width: 0.0,
                ..Default::default()
            },
            CellBorder {
                width: 0.0,
                ..Default::default()
            },
        ],
    });
    let mut body = Paragraph::from_text(
        "Mara reached the lighthouse before the rain. Below the glass, the harbour shone like a folded map. She left her notebook beside the lamp and listened for the small bell at the end of the pier.",
    );
    body.box_style = Some(BoxStyle::unbordered());
    body.first_indent = 18.0;
    body.after = 10.0;
    editor.doc.blocks = vec![
        Block::Paragraph(title),
        Block::Paragraph(chapter),
        Block::Paragraph(body.clone()),
        Block::Paragraph(Paragraph::from_text(
            "The bell rang twice. Someone was coming home.",
        )),
        Block::Paragraph(Paragraph::default()),
    ];
    editor.place(
        Position {
            address: Address {
                block: 4,
                row: 0,
                cell: 0,
            },
            offset: 0,
        },
        false,
    );
    editor.insert_table(4, 3);
    let block = editor.caret.address.block;
    let Block::Table(rows) = &mut editor.doc.blocks[block] else {
        return Err("Missing fixture table".into());
    };
    for (row, data) in rows.iter_mut().zip([
        ["Supplies", "Quantity", "Cost"],
        ["Lamp oil", "2", "12.50"],
        ["Notebooks", "3", "7.50"],
        ["Total", "", ""],
    ]) {
        for (cell, text) in row.iter_mut().zip(data) {
            *cell = Paragraph::from_text(text);
        }
    }
    for cell in &mut rows[0] {
        cell.box_style = Some(BoxStyle {
            fill: Some([211, 227, 253]),
            borders: [CellBorder {
                colour: [11, 87, 208],
                width: 1.0,
            }; 4],
        });
        for c in &mut cell.chars {
            c.style.bold = true;
        }
    }
    editor.place(
        Position {
            address: Address {
                block,
                row: 3,
                cell: 2,
            },
            offset: 0,
        },
        false,
    );
    editor.set_table_formula(TableFormula {
        expression: "=SUM(ABOVE)".into(),
        decimals: Some(2),
        word_number_format: None,
    })?;
    let context = eframe::egui::Context::default();
    editor
        .doc
        .blocks
        .push(Block::Image(alphadocs::editable_objects::prepare(
            &context,
            ObjectSource::Equation {
                latex: r"\frac{-b\pm\sqrt{b^2-4ac}}{2a}".into(),
                size: 18.0,
                colour: [31; 3],
            },
            0.0,
            0.0,
        )?));
    for kind in ShapeKind::ALL {
        let text = if matches!(kind, ShapeKind::Line | ShapeKind::Arrow) {
            ""
        } else {
            "Keep the lamp burning. Someone is coming home."
        };
        editor
            .doc
            .blocks
            .push(Block::Image(alphadocs::editable_objects::prepare(
                &context,
                ObjectSource::Drawing {
                    kind,
                    text: Box::new(Paragraph::from_text(text)),
                    fill: if kind == ShapeKind::Rectangle {
                        Some([211, 227, 253])
                    } else {
                        None
                    },
                    border: CellBorder {
                        colour: [11, 87, 208],
                        width: 1.0,
                    },
                },
                240.0,
                70.0,
            )?));
    }
    editor.doc.validate()?;
    persistence::atomic_write(
        &output.join("story.adocs"),
        &serde_json::to_vec_pretty(&editor.doc).map_err(|e| e.to_string())?,
    )?;
    compat::docx::export(&editor.doc, &output.join("story.docx"))?;
    let (imported, warnings) = compat::docx::import(&output.join("story.docx"))?;
    assert_eq!(imported.text(), editor.doc.text());
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        imported
            .blocks
            .iter()
            .filter(|b| matches!(b,Block::Image(i) if i.editable.is_some()))
            .count(),
        6
    );
    for (before, after) in editor
        .doc
        .blocks
        .iter()
        .filter_map(|b| {
            if let Block::Image(i) = b {
                Some(i)
            } else {
                None
            }
        })
        .zip(imported.blocks.iter().filter_map(|b| {
            if let Block::Image(i) = b {
                Some(i)
            } else {
                None
            }
        }))
    {
        assert_eq!(before.id, after.id);
        assert!(after.editable.is_some());
        if let (ObjectSource::Drawing { text: a, .. }, ObjectSource::Drawing { text: b, .. }) = (
            &before.editable.as_ref().unwrap().source,
            &after.editable.as_ref().unwrap().source,
        ) {
            assert_eq!(a.text(), b.text());
        }
    }
    let ctx = eframe::egui::Context::default();
    fonts::install(&ctx);
    fonts::prepare(&ctx, &imported, &TextStyle::default());
    let mut measured = None;
    let mut frame = ctx.run_ui(Default::default(), |_| {
        measured = Some(Layout::measure(&ctx, &imported))
    });
    frame.textures_delta.clear();
    compat::pdf::export(
        &imported,
        &measured.ok_or("No layout")?,
        &output.join("story.pdf"),
    )?;
    println!(
        "DOCX text/format fixture and measured PDF written to {}",
        output.display()
    );
    Ok(())
}
