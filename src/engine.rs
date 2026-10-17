use crate::model::*;
mod annotations;
mod calculations;
pub use calculations::edit_drawing_text;
mod operations;
mod shared;
mod typing;
pub use operations::ClipboardContent;
use std::time::{Duration, Instant};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone)]
struct Snapshot {
    tab_id: String,
    full_document: Option<Document>,
    start: usize,
    replace_len: usize,
    blocks: Vec<Block>,
    page: Option<PageSetup>,
    citations: Option<SharedVec<crate::writing::Citation>>,
    title: Option<String>,
    starred: Option<bool>,
    caret: Position,
    anchor: Position,
}

/// A transient selection of a table's complete row or column.
///
/// This is separate from the scalar text selection held by `Editor::caret` and
/// `Editor::anchor`, so selecting cells does not change text-editing semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableSelection {
    Row { block: usize, row: usize },
    Column { block: usize, column: usize },
}

impl Snapshot {
    fn between(before: &Document, after: &Document, caret: Position, anchor: Position) -> Self {
        let mut start = 0;
        while start < before.blocks.len().min(after.blocks.len())
            && before.blocks[start] == after.blocks[start]
        {
            start += 1;
        }
        let mut suffix = 0;
        while suffix
            < before
                .blocks
                .len()
                .saturating_sub(start)
                .min(after.blocks.len().saturating_sub(start))
            && before.blocks[before.blocks.len() - 1 - suffix]
                == after.blocks[after.blocks.len() - 1 - suffix]
        {
            suffix += 1;
        }
        Self {
            tab_id: before.tabs[before.active_tab].id.clone(),
            full_document: (before.tabs != after.tabs).then(|| before.clone()),
            start,
            replace_len: after.blocks.len() - start - suffix,
            blocks: before.blocks[start..before.blocks.len() - suffix].to_vec(),
            page: (before.page != after.page).then(|| before.page.clone()),
            citations: (before.citations != after.citations).then(|| before.citations.clone()),
            title: (before.title != after.title).then(|| before.title.clone()),
            starred: (before.starred != after.starred).then_some(before.starred),
            caret,
            anchor,
        }
    }
}
pub struct Editor {
    pub doc: Document,
    pub caret: Position,
    pub anchor: Position,
    pub typing: TextStyle,
    pub revision: u64,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    pub rich_clipboard: Option<(String, Vec<Vec<StyledChar>>)>,
    last_insert: Option<(Instant, Position, TextStyle)>,
    tab_selections: std::collections::HashMap<String, (Position, Position)>,
    table_selection: Option<TableSelection>,
    collaboration: Option<shared::SharedEditor>,
}
impl Editor {
    pub fn new(doc: Document) -> Self {
        let address = doc.addresses().first().copied().unwrap_or_default();
        let caret = Position { address, offset: 0 };
        let mut editor = Self {
            doc,
            caret,
            anchor: caret,
            typing: TextStyle::default(),
            revision: 0,
            undo: vec![],
            redo: vec![],
            rich_clipboard: None,
            last_insert: None,
            tab_selections: Default::default(),
            table_selection: None,
            collaboration: None,
        };
        editor.sync_style();
        editor
    }
    fn restore(&mut self, s: Snapshot) -> Snapshot {
        self.table_selection = None;
        if let Some(document) = s.full_document {
            let inverse = Snapshot::between(&self.doc, &document, self.caret, self.anchor);
            let inverse = Snapshot {
                full_document: Some(self.doc.clone()),
                ..inverse
            };
            self.doc = document;
            self.caret = s.caret;
            self.anchor = s.anchor;
            self.clamp();
            self.sync_style();
            self.revision += 1;
            return inverse;
        }
        if let Some(index) = self.doc.tabs.iter().position(|t| t.id == s.tab_id) {
            self.switch_tab_inner(index);
        }
        let inserted_len = s.blocks.len();
        let blocks = self
            .doc
            .blocks
            .splice(s.start..s.start + s.replace_len, s.blocks)
            .collect();
        let inverse = Snapshot {
            tab_id: s.tab_id,
            full_document: None,
            start: s.start,
            replace_len: inserted_len,
            blocks,
            page: s.page.as_ref().map(|_| self.doc.page.clone()),
            citations: s.citations.as_ref().map(|_| self.doc.citations.clone()),
            title: s.title.as_ref().map(|_| self.doc.title.clone()),
            starred: s.starred.map(|_| self.doc.starred),
            caret: self.caret,
            anchor: self.anchor,
        };
        if let Some(citations) = s.citations {
            self.doc.citations = citations;
        }
        if let Some(page) = s.page {
            self.doc.page = page;
        }
        if let Some(title) = s.title {
            self.doc.title = title;
        }
        if let Some(starred) = s.starred {
            self.doc.starred = starred;
        }
        self.caret = s.caret;
        self.anchor = s.anchor;
        self.sync_style();
        self.revision += 1;
        inverse
    }
    fn switch_tab_inner(&mut self, index: usize) -> bool {
        self.table_selection = None;
        let id = self.doc.tabs[self.doc.active_tab].id.clone();
        self.tab_selections.insert(id, (self.caret, self.anchor));
        if !self.doc.select_tab(index) {
            return false;
        }
        let selection = self
            .tab_selections
            .get(&self.doc.tabs[index].id)
            .copied()
            .unwrap_or_default();
        (self.caret, self.anchor) = selection;
        self.clamp();
        self.sync_style();
        self.last_insert = None;
        true
    }
    pub fn switch_tab(&mut self, index: usize) -> bool {
        if index >= self.doc.tabs.len() {
            return false;
        }
        if self.switch_tab_inner(index) {
            self.revision += 1;
            true
        } else {
            false
        }
    }
    pub fn tab_selections(&self) -> std::collections::HashMap<String, (Position, Position)> {
        let mut selections = self.tab_selections.clone();
        selections.insert(
            self.doc.tabs[self.doc.active_tab].id.clone(),
            (self.caret, self.anchor),
        );
        selections.retain(|id, _| self.doc.tabs.iter().any(|t| &t.id == id));
        selections
    }
    pub fn restore_tab_selections(
        &mut self,
        selections: std::collections::HashMap<String, (Position, Position)>,
    ) {
        self.tab_selections = selections;
        self.tab_selections
            .retain(|id, _| self.doc.tabs.iter().any(|t| &t.id == id));
    }
    pub fn add_tab(&mut self) {
        let number = (1..)
            .find(|n| !self.doc.tabs.iter().any(|t| t.title == format!("Tab {n}")))
            .unwrap();
        let paragraph = Paragraph {
            style: TextStyle {
                font: self.typing.font.clone(),
                ..Default::default()
            },
            ..Default::default()
        };
        self.transaction(|e| {
            e.doc.tabs.push(ContentTab {
                id: format!("tab-{}", Document::new("").id),
                title: format!("Tab {number}"),
                blocks: vec![Block::Paragraph(paragraph)].into(),
            })
        });
        self.switch_tab(self.doc.tabs.len() - 1);
    }
    pub fn rename_tab(&mut self, index: usize, title: &str) {
        let title = title.trim();
        if index < self.doc.tabs.len() && !title.is_empty() {
            self.transaction(|e| e.doc.tabs[index].title = title.into());
        }
    }
    pub fn duplicate_tab(&mut self, index: usize) {
        if index >= self.doc.tabs.len() {
            return;
        }
        let mut copied_blocks = self.doc.tab_blocks(index).to_vec();
        annotations::fresh_ids(&mut copied_blocks);
        for block in &mut copied_blocks {
            if let Block::Image(image) = block {
                image.id = new_object_id();
            }
        }
        let tab = ContentTab {
            id: format!("tab-{}", Document::new("").id),
            title: format!("{} (copy)", self.doc.tabs[index].title),
            blocks: copied_blocks.into(),
        };
        self.transaction(|e| e.doc.tabs.push(tab));
        self.switch_tab(self.doc.tabs.len() - 1);
    }
    pub fn delete_tab(&mut self, index: usize) {
        if index >= self.doc.tabs.len() || self.doc.tabs.len() == 1 {
            return;
        }
        self.transaction(|e| {
            if index == e.doc.active_tab {
                e.switch_tab_inner(if index == 0 { 1 } else { index - 1 });
            }
            e.doc.tabs.remove(index);
            if e.doc.active_tab > index {
                e.doc.active_tab -= 1;
            }
        });
    }
    pub fn transaction(&mut self, f: impl FnOnce(&mut Self)) {
        if !self.collaboration_writable() {
            return;
        }
        self.last_insert = None;
        let old = self.doc.clone();
        let caret = self.caret;
        let anchor = self.anchor;
        f(self);
        if self.doc != old {
            self.table_selection = None;
            if let Some(shared) = self.collaboration.as_mut() {
                if let Err(error) = shared.commit(&old, &self.doc) {
                    shared.report(error);
                    self.doc = old;
                    self.caret = caret;
                    self.anchor = anchor;
                    self.clamp();
                    return;
                }
            } else {
                self.undo
                    .push(Snapshot::between(&old, &self.doc, caret, anchor));
            }
            while self.undo.len() > 1
                && (self.undo.len() > 100
                    || self.undo.iter().map(snapshot_weight).sum::<usize>() > 64 * 1024 * 1024)
            {
                self.undo.remove(0);
            }
            self.redo.clear();
            self.revision += 1;
        }
        self.clamp();
    }
    pub fn can_undo(&self) -> bool {
        if let Some(shared) = &self.collaboration {
            return shared.replica.can_undo();
        }
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        if let Some(shared) = &self.collaboration {
            return shared.replica.can_redo();
        }
        !self.redo.is_empty()
    }
    pub fn undo(&mut self) {
        if self.shared_undo(false) {
            return;
        }
        self.last_insert = None;
        if let Some(s) = self.undo.pop() {
            let inverse = self.restore(s);
            self.redo.push(inverse);
        }
    }
    pub fn redo(&mut self) {
        if self.shared_undo(true) {
            return;
        }
        self.last_insert = None;
        if let Some(s) = self.redo.pop() {
            let inverse = self.restore(s);
            self.undo.push(inverse);
        }
    }
    pub fn selected(&self) -> bool {
        self.has_scalar_selection() || !self.selected_table_cell_addresses().is_empty()
    }
    pub fn has_scalar_selection(&self) -> bool {
        self.caret != self.anchor
    }
    pub fn range(&self) -> (Position, Position) {
        if self.caret < self.anchor {
            (self.caret, self.anchor)
        } else {
            (self.anchor, self.caret)
        }
    }
    fn paragraphs_in_selection(&self) -> Vec<Address> {
        if let Some(selection) = self.table_selection {
            return self.table_cell_addresses_for(selection);
        }
        let (start, end) = self.range();
        self.doc
            .addresses()
            .into_iter()
            .filter(|address| *address >= start.address && *address <= end.address)
            .collect()
    }
    fn table_cell_addresses_for(&self, selection: TableSelection) -> Vec<Address> {
        match selection {
            TableSelection::Row { block, row } => {
                let Some(Block::Table(rows)) = self.doc.blocks.get(block) else {
                    return vec![];
                };
                let Some(cells) = rows.get(row) else {
                    return vec![];
                };
                (0..cells.len())
                    .map(|cell| Address { block, row, cell })
                    .collect()
            }
            TableSelection::Column { block, column } => {
                let Some(Block::Table(rows)) = self.doc.blocks.get(block) else {
                    return vec![];
                };
                rows.iter()
                    .enumerate()
                    .filter_map(|(row, cells)| {
                        (column < cells.len()).then_some(Address {
                            block,
                            row,
                            cell: column,
                        })
                    })
                    .collect()
            }
        }
    }
    fn transaction_preserving_table_selection(&mut self, f: impl FnOnce(&mut Self)) {
        let selection = self.table_selection;
        self.transaction(f);
        if let Some(selection) = selection
            && !self.table_cell_addresses_for(selection).is_empty()
        {
            self.table_selection = Some(selection);
        }
    }
    /// Select a complete row without changing the scalar text caret or anchor.
    pub fn select_table_row(&mut self, block: usize, row: usize) -> bool {
        if matches!(self.doc.blocks.get(block), Some(Block::Table(rows)) if row < rows.len() && !rows[row].is_empty())
        {
            self.table_selection = Some(TableSelection::Row { block, row });
            true
        } else {
            false
        }
    }
    /// Select a complete column, including its cells in every row that has it.
    pub fn select_table_column(&mut self, block: usize, column: usize) -> bool {
        if matches!(self.doc.blocks.get(block), Some(Block::Table(rows)) if rows.iter().any(|r| column < r.len()))
        {
            self.table_selection = Some(TableSelection::Column { block, column });
            true
        } else {
            false
        }
    }
    pub fn table_selection(&self) -> Option<TableSelection> {
        self.table_selection
    }
    pub fn clear_table_selection(&mut self) {
        self.table_selection = None;
    }
    /// Addresses selected by the current row or column selection, in document order.
    pub fn selected_table_cell_addresses(&self) -> Vec<Address> {
        self.table_selection
            .map(|selection| self.table_cell_addresses_for(selection))
            .unwrap_or_default()
    }
    /// Plain-text copy representation for a row or column selection.
    pub fn selected_table_text(&self) -> Option<String> {
        let selection = self.table_selection?;
        let addresses = self.table_cell_addresses_for(selection);
        let mut lines = Vec::new();
        match selection {
            TableSelection::Row { .. } => {
                lines.push(
                    addresses
                        .iter()
                        .map(|address| {
                            self.doc
                                .paragraph(*address)
                                .map_or_else(String::new, Paragraph::text)
                        })
                        .collect::<Vec<_>>()
                        .join("\t"),
                );
            }
            TableSelection::Column { block, column } => {
                let Some(Block::Table(rows)) = self.doc.blocks.get(block) else {
                    return Some(String::new());
                };
                lines.extend(
                    rows.iter()
                        .map(|row| row.get(column).map_or_else(String::new, Paragraph::text)),
                );
            }
        }
        Some(lines.join("\n"))
    }
    /// Clear text in selected cells as one undoable edit, preserving annotation anchors.
    pub fn clear_selected_table_cells(&mut self) -> bool {
        if self.selected_table_cell_addresses().is_empty() {
            return false;
        }
        let revision = self.revision;
        self.delete_selection();
        self.revision != revision
    }
    fn set_checklist_items(editor: &mut Self, mut addresses: Vec<Address>) {
        // Process body blocks from the end so splitting a paragraph does not
        // invalidate the addresses that are still waiting to be handled.
        addresses.sort_unstable_by(|left, right| right.cmp(left));
        for address in addresses {
            let split_body_hard_lines = address.row == 0
                && address.cell == 0
                && matches!(
                    editor.doc.blocks.get(address.block),
                    Some(Block::Paragraph(_))
                );
            if split_body_hard_lines {
                let Some(Block::Paragraph(mut paragraph)) =
                    editor.doc.blocks.get(address.block).cloned()
                else {
                    continue;
                };
                let mut items = Vec::new();
                while let Some(offset) = paragraph
                    .chars
                    .iter()
                    .position(|character| character.ch == '\n')
                {
                    let mut right = paragraph.split_text(offset);
                    let remainder = right.split_text(1);
                    if paragraph.list != ListKind::Check {
                        paragraph.checked = false;
                    }
                    paragraph.list = ListKind::Check;
                    items.push(Block::Paragraph(paragraph));
                    paragraph = remainder;
                }
                if paragraph.list != ListKind::Check {
                    paragraph.checked = false;
                }
                paragraph.list = ListKind::Check;
                items.push(Block::Paragraph(paragraph));
                editor
                    .doc
                    .blocks
                    .splice(address.block..=address.block, items);
            } else if let Some(paragraph) = editor.doc.paragraph_mut(address) {
                if paragraph.list != ListKind::Check {
                    paragraph.checked = false;
                }
                paragraph.list = ListKind::Check;
            }
        }
    }
    /// Apply checklist formatting to each selected paragraph, treating a partial
    /// text selection as a selection of its whole paragraph.
    pub fn set_checklist_for_selection(&mut self) {
        let addresses = self.paragraphs_in_selection();
        self.transaction_preserving_table_selection(|editor| {
            Self::set_checklist_items(editor, addresses);
        });
    }
    /// Toggle selected paragraphs between checklists and ordinary paragraphs.
    pub fn toggle_checklist_for_selection(&mut self) {
        let addresses = self.paragraphs_in_selection();
        if addresses.is_empty() {
            return;
        }
        let remove = addresses.iter().all(|address| {
            self.doc
                .paragraph(*address)
                .is_some_and(|paragraph| paragraph.list == ListKind::Check)
        });
        self.transaction_preserving_table_selection(|editor| {
            if remove {
                for address in addresses {
                    if let Some(paragraph) = editor.doc.paragraph_mut(address) {
                        paragraph.list = ListKind::None;
                        paragraph.checked = false;
                    }
                }
            } else {
                Self::set_checklist_items(editor, addresses);
            }
        });
    }
    /// Toggle an individual checklist item through the undoable editing engine.
    pub fn toggle_check_checked(&mut self, address: Address) -> bool {
        if !self.collaboration_writable()
            || !self
                .doc
                .paragraph(address)
                .is_some_and(|paragraph| paragraph.list == ListKind::Check)
        {
            return false;
        }
        self.transaction_preserving_table_selection(|editor| {
            if let Some(paragraph) = editor.doc.paragraph_mut(address)
                && paragraph.list == ListKind::Check
            {
                paragraph.checked = !paragraph.checked;
            }
        });
        true
    }
    pub fn clamp(&mut self) {
        let first = self.doc.addresses().first().copied().unwrap_or_default();
        for pos in [&mut self.caret, &mut self.anchor] {
            if let Some(p) = self.doc.paragraph(pos.address) {
                pos.offset = p.snap(pos.offset);
            } else {
                pos.address = first;
                pos.offset = 0;
            }
        }
    }
    pub fn sync_style(&mut self) {
        if let Some(p) = self.doc.paragraph(self.caret.address) {
            self.typing = p
                .chars
                .get(self.caret.offset.saturating_sub(1))
                .map(|c| c.style.clone())
                .unwrap_or_else(|| p.style.clone());
        }
    }
    pub fn place(&mut self, pos: Position, extend: bool) {
        self.last_insert = None;
        self.table_selection = None;
        self.caret = pos;
        self.clamp();
        if !extend {
            self.anchor = self.caret;
        }
        self.sync_style();
    }
    pub fn select_all(&mut self) {
        let a = self.doc.addresses();
        if let (Some(first), Some(last)) = (a.first(), a.last()) {
            self.anchor = Position {
                address: *first,
                offset: 0,
            };
            self.caret = Position {
                address: *last,
                offset: self.doc.paragraph(*last).map_or(0, |p| p.chars.len()),
            };
        }
    }
    pub fn selected_chars(&self) -> Vec<Vec<StyledChar>> {
        if self.table_selection.is_some() {
            return self
                .selected_table_cell_addresses()
                .into_iter()
                .filter_map(|address| {
                    self.doc
                        .paragraph(address)
                        .map(|paragraph| paragraph.chars.to_vec())
                })
                .collect();
        }
        let (s, e) = self.range();
        self.doc
            .addresses()
            .into_iter()
            .filter(|a| *a >= s.address && *a <= e.address)
            .map(|a| {
                let p = self.doc.paragraph(a).expect("Valid address");
                let start = if a == s.address { s.offset } else { 0 };
                let end = if a == e.address {
                    e.offset
                } else {
                    p.chars.len()
                };
                p.chars[start.min(p.chars.len())..end.min(p.chars.len())].to_vec()
            })
            .collect()
    }
    pub fn selected_text(&self) -> String {
        if let Some(text) = self.selected_table_text() {
            return text;
        }
        self.selected_chars()
            .iter()
            .map(|c| c.iter().map(|c| c.ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn copy(&mut self) -> String {
        let text = self.selected_text();
        self.rich_clipboard = Some((text.clone(), self.selected_chars()));
        text
    }
    fn delete_inner(&mut self) {
        if self.table_selection.is_some() {
            let addresses = self.selected_table_cell_addresses();
            let Some(first) = addresses.first().copied() else {
                return;
            };
            for address in addresses {
                if let Some(paragraph) = self.doc.paragraph_mut(address) {
                    paragraph.replace_text(0..paragraph.chars.len(), vec![]);
                }
            }
            self.caret = Position {
                address: first,
                offset: 0,
            };
            self.anchor = self.caret;
            return;
        }
        if !self.has_scalar_selection() {
            return;
        }
        if self.selection_covers_body() {
            self.doc.blocks = vec![Block::Paragraph(Paragraph::default())];
            self.caret = Position::default();
            self.anchor = self.caret;
            return;
        }
        let (s, e) = self.range();
        if s.address == e.address {
            if let Some(p) = self.doc.paragraph_mut(s.address) {
                p.replace_text(s.offset..e.offset, vec![]);
            }
        } else {
            let join = matches!(
                self.doc.blocks.get(s.address.block),
                Some(Block::Paragraph(_))
            ) && matches!(
                self.doc.blocks.get(e.address.block),
                Some(Block::Paragraph(_))
            );
            if join {
                let mut tail = self.doc.paragraph(e.address).expect("paragraph").clone();
                tail.replace_text(0..e.offset, vec![]);
                let p = self.doc.paragraph_mut(s.address).expect("paragraph");
                p.replace_text(s.offset..p.chars.len(), vec![]);
                p.append_text(tail);
                self.doc.blocks.drain(s.address.block + 1..=e.address.block);
            } else {
                for a in self.doc.addresses() {
                    if a >= s.address && a <= e.address {
                        let p = self.doc.paragraph_mut(a).expect("paragraph");
                        let start = if a == s.address { s.offset } else { 0 };
                        let end = if a == e.address {
                            e.offset
                        } else {
                            p.chars.len()
                        };
                        p.replace_text(start..end, vec![]);
                    }
                }
            }
        }
        self.caret = s;
        self.anchor = s;
    }
    pub fn delete_selection(&mut self) {
        self.transaction(|e| e.delete_inner());
    }
    pub fn selection_covers_body(&self) -> bool {
        let (start, end) = self.range();
        let addresses = self.doc.addresses();
        self.has_scalar_selection()
            && addresses.first() == Some(&start.address)
            && addresses.last() == Some(&end.address)
            && start.offset == 0
            && self
                .doc
                .paragraph(end.address)
                .is_some_and(|p| end.offset == p.chars.len())
    }
    pub fn insert(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let style = self.typing.clone();
        let single = !text.contains('\n') && text.graphemes(true).count() == 1;
        let merge = self.collaboration.is_none()
            && single
            && !self.has_scalar_selection()
            && self.table_selection.is_none()
            && self
                .last_insert
                .as_ref()
                .is_some_and(|(time, pos, previous)| {
                    time.elapsed() < Duration::from_millis(750)
                        && *pos == self.caret
                        && *previous == style
                });
        if merge && let Some(p) = self.doc.paragraph_mut(self.caret.address) {
            let chars: Vec<_> = text
                .chars()
                .map(|ch| StyledChar {
                    ch,
                    style: style.clone(),
                })
                .collect();
            let len = chars.len();
            p.replace_text(self.caret.offset..self.caret.offset, chars);
            self.caret.offset += len;
            self.anchor = self.caret;
            self.revision += 1;
            self.redo.clear();
            self.last_insert = Some((Instant::now(), self.caret, style));
            return;
        }
        let parts = text
            .split('\n')
            .map(|s| {
                s.chars()
                    .map(|ch| StyledChar {
                        ch,
                        style: style.clone(),
                    })
                    .collect()
            })
            .collect();
        self.insert_rich(parts);
        if single {
            self.last_insert = Some((Instant::now(), self.caret, style));
        }
    }
    pub fn paste(&mut self, text: &str, plain: bool) {
        if !plain
            && let Some((cached, parts)) = &self.rich_clipboard
            && cached == text
        {
            self.insert_rich(parts.clone());
            return;
        }
        self.insert(text);
    }
    pub fn insert_rich(&mut self, parts: Vec<Vec<StyledChar>>) {
        self.transaction(|e| {
            e.delete_inner();
            for (i, part) in parts.into_iter().enumerate() {
                if i > 0 {
                    e.split_inner();
                }
                if let Some(p) = e.doc.paragraph_mut(e.caret.address) {
                    let len = part.len();
                    p.replace_text(e.caret.offset..e.caret.offset, part);
                    e.caret.offset += len;
                    e.anchor = e.caret;
                }
            }
        });
    }
    fn split_inner(&mut self) {
        let a = self.caret.address;
        if matches!(self.doc.blocks.get(a.block), Some(Block::Table(_))) {
            if let Some(p) = self.doc.paragraph_mut(a) {
                p.replace_text(
                    self.caret.offset..self.caret.offset,
                    vec![StyledChar {
                        ch: '\n',
                        style: self.typing.clone(),
                    }],
                );
                self.caret.offset += 1;
            }
        } else if let Some(p) = self.doc.paragraph_mut(a) {
            let mut next = p.split_text(self.caret.offset);
            next.page_break_before = false;
            if p.heading > 0 && next.chars.is_empty() {
                next.set_heading(0);
            }
            self.doc.blocks.insert(a.block + 1, Block::Paragraph(next));
            self.caret = Position {
                address: Address {
                    block: a.block + 1,
                    row: 0,
                    cell: 0,
                },
                offset: 0,
            };
        }
        self.anchor = self.caret;
    }
    pub fn enter(&mut self) {
        self.transaction(|e| {
            e.delete_inner();
            let p = e.doc.paragraph_mut(e.caret.address);
            if let Some(p) = p
                && p.list != ListKind::None
                && p.chars.is_empty()
            {
                p.list = ListKind::None;
            } else {
                e.split_inner();
            }
        });
        self.sync_style();
    }
    pub fn delete(&mut self, back: bool) {
        if self.selected() {
            self.delete_selection();
            return;
        }
        let Some(p) = self.doc.paragraph(self.caret.address) else {
            return;
        };
        if (back && self.caret.offset > 0) || (!back && self.caret.offset < p.chars.len()) {
            let offset = p.boundary(self.caret.offset, !back);
            self.anchor.offset = offset;
            self.delete_selection();
        } else {
            let a = self.doc.addresses();
            if let Some(i) = a.iter().position(|a| *a == self.caret.address) {
                let next = if back {
                    i.checked_sub(1)
                } else {
                    (i + 1 < a.len()).then_some(i + 1)
                };
                if let Some(j) = next {
                    let address = a[j];
                    let offset = if back {
                        self.doc.paragraph(address).map_or(0, |p| p.chars.len())
                    } else {
                        0
                    };
                    if self.caret.address.block == address.block
                        && matches!(self.doc.blocks[address.block], Block::Table(_))
                    {
                        self.place(Position { address, offset }, false);
                        return;
                    }
                    self.anchor = Position { address, offset };
                    self.delete_selection();
                }
            }
        }
    }
    pub fn move_horizontal(&mut self, forward: bool, extend: bool, word: bool) {
        if self.table_selection.is_some() {
            self.table_selection = None;
        }
        if self.has_scalar_selection() && !extend {
            let (s, e) = self.range();
            self.place(if forward { e } else { s }, false);
            return;
        }
        let Some(p) = self.doc.paragraph(self.caret.address) else {
            return;
        };
        let mut pos = self.caret;
        if (forward && pos.offset < p.chars.len()) || (!forward && pos.offset > 0) {
            pos.offset = if word {
                let text = p.text();
                let mut boundaries: Vec<usize> = text
                    .unicode_word_indices()
                    .map(|(i, _)| text[..i].chars().count())
                    .collect();
                boundaries.push(p.chars.len());
                if forward {
                    boundaries
                        .into_iter()
                        .find(|n| *n > pos.offset)
                        .unwrap_or(p.chars.len())
                } else {
                    boundaries
                        .into_iter()
                        .rfind(|n| *n < pos.offset)
                        .unwrap_or(0)
                }
            } else {
                p.boundary(pos.offset, forward)
            };
        } else {
            let a = self.doc.addresses();
            if let Some(i) = a.iter().position(|a| *a == pos.address) {
                let j = if forward {
                    (i + 1 < a.len()).then_some(i + 1)
                } else {
                    i.checked_sub(1)
                };
                if let Some(j) = j {
                    pos.address = a[j];
                    pos.offset = if forward {
                        0
                    } else {
                        self.doc.paragraph(pos.address).map_or(0, |p| p.chars.len())
                    };
                }
            }
        }
        self.place(pos, extend);
    }
    pub fn format(&mut self, f: impl Fn(&mut TextStyle)) {
        f(&mut self.typing);
        if self.table_selection.is_some() {
            let addresses = self.selected_table_cell_addresses();
            self.transaction_preserving_table_selection(|editor| {
                for address in addresses {
                    if let Some(paragraph) = editor.doc.paragraph_mut(address) {
                        f(&mut paragraph.style);
                        for character in &mut paragraph.chars {
                            f(&mut character.style);
                        }
                    }
                }
            });
            return;
        }
        if !self.has_scalar_selection() {
            if self
                .doc
                .paragraph(self.caret.address)
                .is_some_and(|p| p.chars.is_empty())
            {
                let style = self.typing.clone();
                self.transaction(|e| {
                    if let Some(p) = e.doc.paragraph_mut(e.caret.address) {
                        p.style = style;
                    }
                });
            }
            return;
        }
        let (s, e) = self.range();
        self.transaction(|ed| {
            for a in ed.doc.addresses() {
                if a >= s.address && a <= e.address {
                    let p = ed.doc.paragraph_mut(a).expect("paragraph");
                    let start = if a == s.address { s.offset } else { 0 };
                    let end = if a == e.address {
                        e.offset
                    } else {
                        p.chars.len()
                    };
                    if start == 0 && end == p.chars.len() {
                        f(&mut p.style);
                    }
                    for c in &mut p.chars[start..end] {
                        f(&mut c.style);
                    }
                }
            }
        });
    }
    pub fn paragraph_format(&mut self, f: impl Fn(&mut Paragraph)) {
        if self.table_selection.is_some() {
            let addresses = self.selected_table_cell_addresses();
            self.transaction_preserving_table_selection(|editor| {
                for address in addresses {
                    if let Some(paragraph) = editor.doc.paragraph_mut(address) {
                        f(paragraph);
                    }
                }
            });
            self.sync_style();
            return;
        }
        let (s, e) = self.range();
        self.transaction(|ed| {
            for a in ed.doc.addresses() {
                if a >= s.address && a <= e.address {
                    f(ed.doc.paragraph_mut(a).expect("paragraph"));
                }
            }
        });
        self.sync_style();
    }
    pub fn insert_block(&mut self, b: Block) {
        if self.caret.address.block == usize::MAX {
            return;
        }
        self.transaction(|e| {
            e.delete_inner();
            e.split_inner();
            let idx = e.caret.address.block;
            e.doc.blocks.insert(idx, b);
            e.caret.address.block += 1;
            e.anchor = e.caret;
        });
    }
    pub fn find(&self, query: &str, case: bool) -> Vec<(Position, Position)> {
        if query.is_empty() {
            return vec![];
        }
        let mut result = vec![];
        for a in self.doc.addresses() {
            let p = self.doc.paragraph(a).expect("paragraph");
            let source = p.text();
            let needle: Vec<char> = query.chars().collect();
            let chars: Vec<char> = source.chars().collect();
            let mut i = 0;
            while i + needle.len() <= chars.len() {
                let hay: String = chars[i..i + needle.len()].iter().collect();
                if (if case {
                    hay == query
                } else {
                    hay.to_lowercase() == query.to_lowercase()
                }) && p.snap(i) == i
                    && p.snap(i + needle.len()) == i + needle.len()
                {
                    result.push((
                        Position {
                            address: a,
                            offset: i,
                        },
                        Position {
                            address: a,
                            offset: i + needle.len(),
                        },
                    ));
                    i += needle.len();
                } else {
                    i += 1;
                }
            }
        }
        result
    }
    pub fn replace_all(&mut self, query: &str, replacement: &str, case: bool) -> usize {
        let matches = self.find(query, case);
        let count = matches.len();
        self.transaction(|e| {
            for (s, end) in matches.into_iter().rev() {
                e.anchor = s;
                e.caret = end;
                e.delete_inner();
                let p = e.doc.paragraph_mut(s.address).expect("paragraph");
                let style = p
                    .chars
                    .get(s.offset)
                    .map(|c| c.style.clone())
                    .unwrap_or_else(|| p.style.clone());
                let chars: Vec<_> = replacement
                    .chars()
                    .map(|ch| StyledChar {
                        ch,
                        style: style.clone(),
                    })
                    .collect();
                p.replace_text(s.offset..s.offset, chars);
            }
        });
        self.clamp();
        count
    }
}

fn snapshot_weight(snapshot: &Snapshot) -> usize {
    let citation_weight = |citations: &SharedVec<crate::writing::Citation>| {
        citations.capacity() * std::mem::size_of::<crate::writing::Citation>()
            + citations
                .iter()
                .map(|citation| {
                    citation.author.capacity()
                        + citation.title.capacity()
                        + citation.year.capacity()
                        + citation.publisher.capacity()
                        + citation.url.capacity()
                })
                .sum::<usize>()
    };
    let paragraph = |p: &Paragraph| {
        p.chars.len() * (std::mem::size_of::<StyledChar>() + 24)
            + p.annotations
                .iter()
                .map(|n| {
                    n.comments
                        .iter()
                        .map(|c| c.text.len() + c.replies.iter().map(String::len).sum::<usize>())
                        .sum::<usize>()
                        + n.reactions.iter().map(String::len).sum::<usize>()
                })
                .sum::<usize>()
    };
    let blocks: Vec<&Block> = if let Some(doc) = &snapshot.full_document {
        (0..doc.tabs.len())
            .flat_map(|i| doc.tab_blocks(i))
            .collect()
    } else {
        snapshot.blocks.iter().collect()
    };
    let block_weight = blocks
        .into_iter()
        .map(|b| match b {
            Block::Paragraph(p) => paragraph(p),
            Block::Table(rows) => rows.iter().flatten().map(paragraph).sum(),
            Block::Image(i) => {
                i.data.len()
                    + i.editable.as_ref().map_or(0, |object| {
                        object.svg.len()
                            + match &object.source {
                                ObjectSource::Equation { latex, .. } => latex.len(),
                                ObjectSource::Drawing { text, .. } => paragraph(text),
                            }
                    })
                    + i.comments
                        .iter()
                        .map(|c| c.text.len() + c.replies.iter().map(String::len).sum::<usize>())
                        .sum::<usize>()
                    + i.reactions.iter().map(String::len).sum::<usize>()
            }
            _ => 0,
        })
        .sum::<usize>();
    let citations = snapshot
        .full_document
        .as_ref()
        .map(|doc| &doc.citations)
        .or(snapshot.citations.as_ref());
    block_weight + citations.map_or(0, citation_weight)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn checklist_selection_is_undoable_and_preserves_rich_paragraph_data() {
        let mut first = Paragraph::from_text("A👩‍🔬B");
        first.chars[0].style.bold = true;
        first.annotations = vec![TextAnnotation {
            id: "thread-one".into(),
            start: 1,
            end: 4,
            comments: Default::default(),
            reactions: Default::default(),
        }]
        .into();
        let second = Paragraph::from_text("cafe\u{301}");
        let mut doc = Document::new("Checklist selection");
        doc.blocks = vec![Block::Paragraph(first), Block::Paragraph(second)];
        let unchanged = doc.clone();
        let mut editor = Editor::new(doc);
        editor.anchor = Position {
            address: Address::default(),
            offset: 1,
        };
        editor.caret = Position {
            address: Address {
                block: 1,
                ..Address::default()
            },
            offset: 2,
        };

        editor.toggle_checklist_for_selection();
        for block in 0..2 {
            assert_eq!(
                editor
                    .doc
                    .paragraph(Address {
                        block,
                        ..Address::default()
                    })
                    .unwrap()
                    .list,
                ListKind::Check
            );
        }
        let first_after = editor.doc.paragraph(Address::default()).unwrap();
        assert_eq!(first_after.text(), "A👩‍🔬B");
        assert!(first_after.chars[0].style.bold);
        assert_eq!(first_after.annotations[0].start, 1);
        assert_eq!(first_after.annotations[0].end, 4);
        assert_eq!(
            unchanged.paragraph(Address::default()).unwrap().list,
            ListKind::None
        );

        assert!(editor.toggle_check_checked(Address::default()));
        assert!(editor.doc.paragraph(Address::default()).unwrap().checked);
        editor.undo();
        assert!(!editor.doc.paragraph(Address::default()).unwrap().checked);
        editor.undo();
        assert_eq!(
            editor.doc.paragraph(Address::default()).unwrap().list,
            ListKind::None
        );
        assert_eq!(
            editor
                .doc
                .paragraph(Address {
                    block: 1,
                    ..Address::default()
                })
                .unwrap()
                .list,
            ListKind::None
        );
        editor.redo();
        editor.redo();
        assert!(editor.doc.paragraph(Address::default()).unwrap().checked);
    }

    #[test]
    fn checklist_splits_body_hard_lines_and_preserves_styling_and_annotations() {
        let mut paragraph = Paragraph::from_text("one\n👩‍🔬\nthree");
        paragraph.chars[4].style.bold = true;
        paragraph.annotations = vec![TextAnnotation {
            id: "line-note".into(),
            start: 4,
            end: 7,
            comments: Default::default(),
            reactions: Default::default(),
        }]
        .into();
        let mut doc = Document::new("Hard lines");
        doc.blocks = vec![Block::Paragraph(paragraph)];
        let original = doc.clone();
        let mut editor = Editor::new(doc);

        editor.set_checklist_for_selection();
        assert_eq!(editor.doc.blocks.len(), 3);
        let items = editor
            .doc
            .blocks
            .iter()
            .map(|block| match block {
                Block::Paragraph(paragraph) => paragraph,
                _ => panic!("hard lines remain body paragraphs"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            items
                .iter()
                .map(|paragraph| paragraph.text())
                .collect::<Vec<_>>(),
            ["one", "👩‍🔬", "three"]
        );
        assert!(
            items
                .iter()
                .all(|paragraph| paragraph.list == ListKind::Check && !paragraph.checked)
        );
        assert!(items[1].chars[0].style.bold);
        assert_eq!(items[1].annotations[0].id, "line-note");
        assert_eq!(
            (items[1].annotations[0].start, items[1].annotations[0].end),
            (0, 3)
        );
        assert_eq!(original.blocks.len(), 1);

        editor.undo();
        assert_eq!(editor.doc, original);
        editor.redo();
        assert_eq!(editor.doc.blocks.len(), 3);
    }

    #[test]
    fn table_row_and_column_selection_support_copy_format_clear_and_undo() {
        let mut table = vec![
            vec![Paragraph::from_text("a"), Paragraph::from_text("b")],
            vec![Paragraph::from_text("c"), Paragraph::from_text("d")],
        ];
        table[1][0].annotations = vec![TextAnnotation {
            id: "cell-thread".into(),
            start: 0,
            end: 1,
            comments: Default::default(),
            reactions: Default::default(),
        }]
        .into();
        let mut doc = Document::new("Table selection");
        doc.blocks = vec![Block::Table(table)];
        let mut editor = Editor::new(doc);
        editor.place(
            Position {
                address: Address {
                    block: 0,
                    row: 0,
                    cell: 1,
                },
                offset: 1,
            },
            false,
        );
        let scalar_selection = editor.range();

        assert!(editor.select_table_row(0, 1));
        assert_eq!(editor.range(), scalar_selection);
        assert_eq!(
            editor.selected_table_cell_addresses(),
            vec![
                Address {
                    block: 0,
                    row: 1,
                    cell: 0
                },
                Address {
                    block: 0,
                    row: 1,
                    cell: 1
                },
            ]
        );
        assert_eq!(editor.selected_table_text().as_deref(), Some("c\td"));
        editor.format(|style| style.bold = true);
        let Block::Table(rows) = &editor.doc.blocks[0] else {
            panic!("table remains a table")
        };
        assert!(!rows[0][0].chars[0].style.bold);
        assert!(rows[1].iter().all(|cell| cell.chars[0].style.bold));
        assert_eq!(
            editor.table_selection(),
            Some(TableSelection::Row { block: 0, row: 1 })
        );
        let copied = editor.copy_fragment();
        assert_eq!(copied.text, "c\td");
        let [Block::Table(copied_rows)] = copied.blocks.as_slice() else {
            panic!("selected row copied as a table")
        };
        assert_eq!(copied_rows.len(), 1);
        assert!(copied_rows[0].iter().all(|cell| cell.chars[0].style.bold));
        editor.undo();
        let Block::Table(rows) = &editor.doc.blocks[0] else {
            panic!("table remains a table")
        };
        assert!(rows[1].iter().all(|cell| !cell.chars[0].style.bold));
        assert_eq!(editor.range(), scalar_selection);

        assert!(editor.select_table_column(0, 1));
        assert_eq!(editor.selected_table_text().as_deref(), Some("b\nd"));
        editor.paste_fragment(&copied);
        let Block::Table(rows) = &editor.doc.blocks[0] else {
            panic!("table remains a table")
        };
        assert_eq!(rows[0][1].text(), "c\td");
        assert_eq!(rows[1][1].text(), "");
        editor.undo();
        let Block::Table(rows) = &editor.doc.blocks[0] else {
            panic!("table remains a table")
        };
        assert_eq!(rows[0][1].text(), "b");
        assert_eq!(rows[1][1].text(), "d");

        assert!(editor.select_table_column(0, 1));
        editor.insert("typed");
        let Block::Table(rows) = &editor.doc.blocks[0] else {
            panic!("table remains a table")
        };
        assert_eq!(rows[0][1].text(), "typed");
        assert_eq!(rows[1][1].text(), "");
        editor.undo();
        let Block::Table(rows) = &editor.doc.blocks[0] else {
            panic!("table remains a table")
        };
        assert_eq!(rows[0][1].text(), "b");
        assert_eq!(rows[1][1].text(), "d");

        assert!(editor.select_table_column(0, 1));
        let revision = editor.revision;
        assert!(editor.clear_selected_table_cells());
        assert_eq!(editor.revision, revision + 1);
        let Block::Table(rows) = &editor.doc.blocks[0] else {
            panic!("table remains a table")
        };
        assert_eq!(rows[0][1].text(), "");
        assert_eq!(rows[1][1].text(), "");
        editor.undo();
        let Block::Table(rows) = &editor.doc.blocks[0] else {
            panic!("table remains a table")
        };
        assert_eq!(rows[0][1].text(), "b");
        assert_eq!(rows[1][1].text(), "d");
        assert_eq!(rows[1][0].annotations[0].start, 0);
        assert_eq!(rows[1][0].annotations[0].end, 1);
    }

    #[test]
    fn read_only_collaboration_rejects_checklist_and_selected_cell_mutations() {
        let mut doc = Document::new("Read-only edits");
        doc.blocks = vec![
            Block::Paragraph(Paragraph::from_text("item")),
            Block::Table(vec![vec![Paragraph::from_text("cell")]]),
        ];
        let mut editor = Editor::new(doc);
        let room = Arc::new(Mutex::new(crate::collaboration::live::Room::new(
            crate::collaboration::replica::Replica::new(&editor.doc).unwrap(),
            false,
            "Viewer".into(),
        )));
        editor.attach_collaboration(room).unwrap();
        let before = editor.doc.clone();
        editor.set_checklist_for_selection();
        assert!(!editor.toggle_check_checked(Address::default()));
        assert!(editor.select_table_row(1, 0));
        assert!(!editor.clear_selected_table_cells());
        assert_eq!(editor.doc, before);
    }

    #[test]
    fn citation_snapshot_payload_counts_toward_history_budget() {
        let mut doc = Document::new("Citation history");
        let oversized_url = "x".repeat(64 * 1024 * 1024 + 1);
        doc.citations = vec![crate::writing::Citation {
            url: oversized_url,
            ..Default::default()
        }]
        .into();
        let citation_only = Snapshot {
            tab_id: doc.tabs[0].id.clone(),
            full_document: None,
            start: 0,
            replace_len: 0,
            blocks: vec![],
            page: None,
            citations: Some(doc.citations.clone()),
            title: None,
            starred: None,
            caret: Position::default(),
            anchor: Position::default(),
        };
        assert!(snapshot_weight(&citation_only) > 64 * 1024 * 1024);

        let full_document = Snapshot {
            full_document: Some(doc.clone()),
            citations: None,
            ..citation_only
        };
        assert!(snapshot_weight(&full_document) > 64 * 1024 * 1024);
    }

    #[test]
    fn content_tabs_preserve_selection_and_history_across_navigation_and_management() {
        let mut e = Editor::new(Document::new("One file"));
        e.insert("First tab");
        e.select_all();
        let selection = e.range();
        e.add_tab();
        e.insert("Second tab 👋");
        e.switch_tab(0);
        assert_eq!(e.range(), selection);
        assert_eq!(e.doc.text(), "First tab");
        e.undo(); // Undo the latest edit, even though it belongs to another tab.
        assert_eq!(e.doc.active_tab, 1);
        assert_eq!(e.doc.text(), "");
        e.redo();
        assert_eq!(e.doc.text(), "Second tab 👋");
        e.rename_tab(1, "Research");
        e.duplicate_tab(1);
        assert_eq!(e.doc.tabs.len(), 3);
        assert_eq!(e.doc.text(), "Second tab 👋");
        e.place(
            Position {
                address: Address::default(),
                offset: e.doc.text().chars().count(),
            },
            false,
        );
        e.insert(" copied");
        let document = e.doc.clone();
        e.delete_tab(1);
        e.doc.validate().unwrap();
        e.undo();
        assert_eq!(e.doc, document);
        e.redo();
        assert_eq!(e.doc.tabs.len(), 2);
        e.undo();
        e.undo(); // Edit in the duplicated tab.
        e.undo(); // Duplicate tab.
        assert_eq!(e.doc.tabs.len(), 2);
        e.undo(); // Rename.
        assert_eq!(e.doc.tabs[1].title, "Tab 2");
        e.undo(); // Second tab typing.
        e.undo(); // Add tab.
        assert_eq!(e.doc.tabs.len(), 1);
        assert_eq!(e.doc.text(), "First tab");
        e.doc.validate().unwrap();
        for _ in 0..5 {
            e.redo();
            e.doc.validate().unwrap();
        }
        assert_eq!(e.doc, document);
        let loaded: Document =
            serde_json::from_slice(&serde_json::to_vec(&e.doc).unwrap()).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded, e.doc);
        assert!(loaded.flattened().text().contains("First tab"));
        assert!(loaded.flattened().text().contains("Second tab 👋 copied"));
    }
    #[test]
    fn empty_paragraph_formatting_is_saved_and_undoable() {
        let mut e = Editor::new(Document::new("Empty formatting"));
        e.format(|s| {
            s.font = "Noto Serif".into();
            s.size = 19.0;
            s.bold = true;
        });
        assert!(e.can_undo());
        let loaded: Document =
            serde_json::from_slice(&serde_json::to_vec(&e.doc).unwrap()).unwrap();
        let restored = Editor::new(loaded);
        assert_eq!(restored.typing, e.typing);
        e.undo();
        assert_eq!(e.typing, TextStyle::default());
        e.redo();
        assert_eq!(e.typing.size, 19.0);
    }
    #[test]
    fn transactions_share_unchanged_content_and_keep_external_snapshots_immutable() {
        let mut doc = Document::new("Sharing");
        doc.blocks = vec![
            Block::Paragraph(Paragraph::from_text("First")),
            Block::Paragraph(Paragraph::from_text(&"Unchanged ".repeat(1000))),
        ];
        let snapshot = doc.clone();
        let original_pointer = snapshot
            .paragraph(Address {
                block: 1,
                ..Default::default()
            })
            .unwrap()
            .chars
            .as_ptr();
        let mut editor = Editor::new(doc);
        editor.insert("Changed ");
        assert_eq!(
            editor
                .doc
                .paragraph(Address {
                    block: 1,
                    ..Default::default()
                })
                .unwrap()
                .chars
                .as_ptr(),
            original_pointer
        );
        assert_eq!(
            snapshot.paragraph(Address::default()).unwrap().text(),
            "First"
        );
        editor.undo();
        assert_eq!(editor.doc, snapshot);
        editor.redo();
        assert_eq!(
            editor.doc.paragraph(Address::default()).unwrap().text(),
            "Changed First"
        );
    }
    #[test]
    fn continuous_typing_groups_undo_and_navigation_ends_group() {
        let mut e = Editor::new(Document::new("Test"));
        e.insert("a");
        e.insert("b");
        e.insert("c");
        e.undo();
        assert_eq!(e.doc.text(), "");
        e.redo();
        assert_eq!(e.doc.text(), "abc");
        e.move_horizontal(false, false, false);
        e.insert("x");
        e.undo();
        assert_eq!(e.doc.text(), "abc");
    }
    #[test]
    fn deleting_page_break_and_nonoverlapping_replacement() {
        let mut e = Editor::new(Document::new("Test"));
        e.insert("one");
        e.insert_block(Block::PageBreak);
        e.delete(true);
        assert_eq!(e.doc.blocks.len(), 1);
        e.select_all();
        e.insert("aaa");
        assert_eq!(e.replace_all("aa", "x", true), 1);
        assert_eq!(e.doc.text(), "xa");
    }
    #[test]
    fn grapheme_delete_undo() {
        let mut e = Editor::new(Document::new("Test"));
        e.insert("a👨‍👩‍👧‍👦e\u{301}");
        e.delete(true);
        assert_eq!(e.doc.text(), "a👨‍👩‍👧‍👦");
        e.delete(true);
        assert_eq!(e.doc.text(), "a");
        e.undo();
        assert_eq!(e.doc.text(), "a👨‍👩‍👧‍👦");
        e.redo();
        assert_eq!(e.doc.text(), "a");
    }
    #[test]
    fn rich_selection_roundtrip() {
        let mut e = Editor::new(Document::new("Test"));
        e.insert("one\ntwo");
        e.select_all();
        e.format(|s| s.bold = true);
        assert!(
            e.doc.paragraph(Address::default()).unwrap().chars[0]
                .style
                .bold
        );
        let text = e.copy();
        e.place(e.caret, false);
        e.enter();
        e.paste(&text, false);
        assert_eq!(e.doc.text(), "one\ntwo\none\ntwo");
        e.undo();
        assert_eq!(e.doc.text(), "one\ntwo\n");
    }
    #[test]
    fn cross_paragraph_delete_and_replace() {
        let mut e = Editor::new(Document::new("Test"));
        e.insert("Hello\nworld");
        e.select_all();
        e.insert("Unicode café café");
        assert_eq!(e.doc.blocks.len(), 1);
        assert_eq!(e.replace_all("café", "茶", true), 2);
        assert_eq!(e.doc.text(), "Unicode 茶 茶");
        e.undo();
        assert_eq!(e.doc.text(), "Unicode café café");
    }
    #[test]
    fn table_edit_is_structured() {
        let mut e = Editor::new(Document::new("Test"));
        e.insert_block(Block::Table(vec![vec![Paragraph::default(); 2]; 2]));
        let address = Address {
            block: 1,
            row: 1,
            cell: 1,
        };
        e.place(Position { address, offset: 0 }, false);
        e.insert("Cell");
        e.enter();
        e.insert("second line");
        assert_eq!(
            e.doc.paragraph(address).unwrap().text(),
            "Cell\nsecond line"
        );
        e.undo();
        assert_eq!(e.doc.paragraph(address).unwrap().text(), "Cell\n");
    }
}
