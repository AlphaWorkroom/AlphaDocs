use super::*;
/// Apply an edited drawing-text draft through the ordinary grapheme-safe
/// engine, preserving styles/annotations outside the actual changed range.
pub fn edit_drawing_text(paragraph: &Paragraph, new_text: &str) -> Paragraph {
    let old = paragraph.text();
    if old == new_text {
        return paragraph.clone();
    }
    let old_parts: Vec<_> = old.graphemes(true).collect();
    let new_parts: Vec<_> = new_text.graphemes(true).collect();
    let prefix = old_parts
        .iter()
        .zip(&new_parts)
        .take_while(|(a, b)| a == b)
        .count();
    let suffix = old_parts[prefix..]
        .iter()
        .rev()
        .zip(new_parts[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let start = old_parts[..prefix].iter().map(|s| s.chars().count()).sum();
    let end = old_parts[..old_parts.len() - suffix]
        .iter()
        .map(|s| s.chars().count())
        .sum();
    let insert = new_parts[prefix..new_parts.len() - suffix].concat();
    let mut doc = Document::new("Drawing draft");
    doc.blocks = vec![Block::Paragraph(paragraph.clone())];
    let mut editor = Editor::new(doc);
    editor.place(
        Position {
            address: Address::default(),
            offset: start,
        },
        false,
    );
    editor.place(
        Position {
            address: Address::default(),
            offset: end,
        },
        true,
    );
    let style = paragraph
        .chars
        .get(start.saturating_sub(1))
        .map(|c| c.style.clone())
        .unwrap_or_else(|| paragraph.style.clone());
    editor.insert_rich(vec![
        insert
            .chars()
            .map(|ch| StyledChar {
                ch,
                style: style.clone(),
            })
            .collect(),
    ]);
    editor.doc.paragraph(Address::default()).unwrap().clone()
}
impl Editor {
    pub fn set_table_formula(&mut self, mut formula: TableFormula) -> Result<(), String> {
        if !self.collaboration_writable() {
            return Err("This document is read-only".into());
        }
        let at = self.caret.address;
        let Some(Block::Table(rows)) = self.doc.blocks.get(at.block) else {
            return Err("Select a table cell first".into());
        };
        // Accept convenient native input, but store a real Word formula field
        // instruction so export/recalculation never turns it into an unknown field.
        formula.expression = formula.expression.trim().to_owned();
        if !formula.expression.starts_with('=') {
            formula.expression.insert(0, '=');
        }
        if !formula.valid() || formula.word_number_format.is_some() {
            return Err("Invalid table formula or number format".into());
        }
        let result = crate::table_formula::evaluate(rows, at.row, at.cell, &formula.expression)?;
        let text = crate::table_formula::formatted(result, formula.decimals);
        self.transaction(|editor| {
            if let Some(p) = editor.doc.paragraph_mut(at) {
                let chars = text
                    .chars()
                    .map(|ch| StyledChar {
                        ch,
                        style: p.style.clone(),
                    })
                    .collect();
                p.replace_text(0..p.chars.len(), chars);
                p.formula = Some(formula);
                editor.caret.offset = p.chars.len();
                editor.anchor = editor.caret;
            }
        });
        self.sync_style();
        Ok(())
    }
    pub fn recalculate_table(&mut self) -> Result<(), String> {
        if !self.collaboration_writable() {
            return Err("This document is read-only".into());
        }
        let block = self.caret.address.block;
        let Some(Block::Table(rows)) = self.doc.blocks.get(block) else {
            return Err("Select a table cell first".into());
        };
        let mut results = vec![];
        for (row, cells) in rows.iter().enumerate() {
            for (cell, p) in cells.iter().enumerate() {
                if let Some(formula) = &p.formula {
                    if formula.word_number_format.is_some() {
                        return Err("This Word number format is preserved with its existing result. Choose Formula and a supported number format to recalculate it.".into());
                    }
                    let result =
                        crate::table_formula::evaluate(rows, row, cell, &formula.expression)?;
                    results.push((
                        Address { block, row, cell },
                        formula.clone(),
                        crate::table_formula::formatted(result, formula.decimals),
                    ));
                }
            }
        }
        self.transaction(|editor| {
            for (address, formula, text) in results {
                if let Some(p) = editor.doc.paragraph_mut(address) {
                    let chars = text
                        .chars()
                        .map(|ch| StyledChar {
                            ch,
                            style: p.style.clone(),
                        })
                        .collect();
                    p.replace_text(0..p.chars.len(), chars);
                    p.formula = Some(formula);
                }
            }
        });
        Ok(())
    }
}
