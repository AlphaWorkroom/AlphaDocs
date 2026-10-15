use crate::model::*;
mod annotations;
mod operations;
mod shared;
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
    title: Option<String>,
    starred: Option<bool>,
    caret: Position,
    anchor: Position,
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
            collaboration: None,
        };
        editor.sync_style();
        editor
    }
    fn restore(&mut self, s: Snapshot) -> Snapshot {
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
            title: s.title.as_ref().map(|_| self.doc.title.clone()),
            starred: s.starred.map(|_| self.doc.starred),
            caret: self.caret,
            anchor: self.anchor,
        };
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
        self.caret != self.anchor
    }
    pub fn range(&self) -> (Position, Position) {
        if self.caret < self.anchor {
            (self.caret, self.anchor)
        } else {
            (self.anchor, self.caret)
        }
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
        if !self.selected() {
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
        self.selected()
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
            && !self.selected()
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
        if self.selected() && !extend {
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
        if !self.selected() {
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
    blocks
        .into_iter()
        .map(|b| match b {
            Block::Paragraph(p) => paragraph(p),
            Block::Table(rows) => rows.iter().flatten().map(paragraph).sum(),
            Block::Image(i) => {
                i.data.len()
                    + i.comments
                        .iter()
                        .map(|c| c.text.len() + c.replies.iter().map(String::len).sum::<usize>())
                        .sum::<usize>()
                    + i.reactions.iter().map(String::len).sum::<usize>()
            }
            _ => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
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
