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
    /// This preference is local session state, never document data or an undo entry.
    pub fn set_auto_recalculate_tables(&mut self, enabled: bool) {
        self.auto_recalculate_tables = enabled;
        if !enabled {
            self.table_calculation_result = None;
        }
    }
    pub fn take_table_calculation_result(&mut self) -> Option<Result<(), String>> {
        self.table_calculation_result.take()
    }
    fn write_table_results(&mut self, block: usize, results: Vec<(usize, usize, String)>) {
        for (row, cell, text) in results {
            if let Some(p) = self.doc.paragraph_mut(Address { block, row, cell })
                && p.text() != text
            {
                let formula = p.formula.clone();
                let chars = text
                    .chars()
                    .map(|ch| StyledChar {
                        ch,
                        style: p.style.clone(),
                    })
                    .collect();
                p.replace_text(0..p.chars.len(), chars);
                p.formula = formula;
            }
        }
    }
    pub(super) fn refresh_automatic_table(&mut self, block: usize) -> bool {
        if !self.auto_recalculate_tables {
            return false;
        }
        let Some(Block::Table(rows)) = self.doc.blocks.get(block) else {
            return false;
        };
        if !rows.iter().flatten().any(|p| p.formula.is_some()) {
            return false;
        }
        match crate::table_formula::calculate(rows) {
            Ok(results) => {
                self.write_table_results(block, results);
                self.table_calculation_result = Some(Ok(()));
            }
            Err(error) => self.table_calculation_result = Some(Err(error)),
        }
        true
    }
    pub(super) fn refresh_changed_table_calculations(&mut self, old: &Document) {
        if !self.auto_recalculate_tables
            || old.tabs[old.active_tab].id != self.doc.tabs[self.doc.active_tab].id
        {
            return;
        }
        let changed: Vec<_> = self
            .doc
            .blocks
            .iter()
            .enumerate()
            .filter(|(i, b)| matches!(b, Block::Table(_)) && old.blocks.get(*i) != Some(*b))
            .map(|(i, _)| i)
            .collect();
        let mut error = None;
        for block in changed {
            if self.refresh_automatic_table(block)
                && let Some(Err(message)) = &self.table_calculation_result
            {
                error = Some(message.clone());
            }
        }
        if let Some(error) = error {
            self.table_calculation_result = Some(Err(error));
        }
    }
    pub fn recalculate_table(&mut self) -> Result<(), String> {
        if !self.collaboration_writable() {
            return Err("This document is read-only".into());
        }
        let block = self.caret.address.block;
        let Some(Block::Table(rows)) = self.doc.blocks.get(block) else {
            return Err("Select a table cell first".into());
        };
        let results = crate::table_formula::calculate(rows)?;
        self.transaction(|editor| editor.write_table_results(block, results));
        Ok(())
    }
}

#[cfg(test)]
mod table_updates_tests {
    use super::*;
    fn editor() -> Editor {
        let mut doc = Document::new("Formula updates");
        let mut cells: Vec<_> = ["1", "2", "3"]
            .into_iter()
            .map(Paragraph::from_text)
            .collect();
        for (i, expression) in [(1, "=A1*2"), (2, "=B1+1")] {
            cells[i].formula = Some(TableFormula {
                expression: expression.into(),
                decimals: None,
                word_number_format: None,
            });
        }
        doc.blocks = vec![Block::Table(vec![cells])];
        Editor::new(doc)
    }
    fn texts(e: &Editor) -> Vec<String> {
        let Block::Table(rows) = &e.doc.blocks[0] else {
            panic!()
        };
        rows[0].iter().map(Paragraph::text).collect()
    }
    #[test]
    fn automatic_table_results_share_typing_undo_and_can_be_disabled() {
        let mut e = editor();
        let original = e.doc.clone();
        e.set_auto_recalculate_tables(true);
        assert_eq!(e.doc, original);
        assert!(!e.can_undo());
        e.place(
            Position {
                address: Address::default(),
                offset: 1,
            },
            false,
        );
        e.insert("0");
        e.insert("0"); // second keystroke uses the coalesced typing path
        assert_eq!(texts(&e), ["100", "200", "201"]);
        e.undo();
        assert_eq!(e.doc, original);
        e.redo();
        assert_eq!(texts(&e), ["100", "200", "201"]);
        e.set_auto_recalculate_tables(false);
        e.select_table_cell(Address::default());
        e.insert("7");
        assert_eq!(texts(&e), ["7", "200", "201"]);
        e.recalculate_table().unwrap();
        assert_eq!(texts(&e), ["7", "14", "15"]);
        e.undo();
        assert_eq!(texts(&e), ["7", "200", "201"]);
        e.undo();
        assert_eq!(texts(&e), ["100", "200", "201"]);
    }
    #[test]
    fn invalid_table_input_keeps_cached_results_and_recovers_without_partial_writes() {
        let mut e = editor();
        e.set_auto_recalculate_tables(true);
        e.select_table_cell(Address::default());
        e.insert("oops");
        assert_eq!(texts(&e), ["oops", "2", "3"]);
        assert!(
            e.take_table_calculation_result()
                .unwrap()
                .unwrap_err()
                .contains("B1")
        );
        e.select_table_cell(Address::default());
        e.insert("4");
        assert_eq!(texts(&e), ["4", "8", "9"]);
        assert!(e.take_table_calculation_result().unwrap().is_ok());
        let before = e.doc.clone();
        assert!(
            e.set_table_formula(TableFormula {
                expression: "=A1".into(),
                decimals: None,
                word_number_format: None
            })
            .is_err()
        );
        assert_eq!(e.doc, before);
    }
    #[test]
    fn select_cell_keeps_table_geometry_styles_and_other_cells_during_clear_and_undo() {
        let mut e = editor();
        let address = Address {
            block: 0,
            row: 0,
            cell: 1,
        };
        assert!(e.select_table_cell(address));
        assert_eq!(e.selected_table_cell_addresses(), [address]);
        assert_eq!(e.selected_text(), "2");
        let fragment = e.copy_fragment();
        let [Block::Table(rows)] = fragment.blocks.as_slice() else {
            panic!()
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].len(), 1);
        let before = e.doc.clone();
        assert!(e.clear_selected_table_cells());
        assert_eq!(texts(&e), ["1", "", "3"]);
        e.undo();
        assert_eq!(e.doc, before);
        e.select_table_cell(address);
        e.select_all();
        assert!(e.table_selection().is_none());
        assert_eq!(e.selected_chars().len(), 3);
        assert!(!e.select_table_cell(Address {
            block: 0,
            row: 99,
            cell: 0
        }));
    }
}
