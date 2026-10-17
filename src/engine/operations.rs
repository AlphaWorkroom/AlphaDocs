use super::*;

impl Editor {
    /// A break image is a body block. An interior drop splits the paragraph at
    /// a measured, grapheme-safe text boundary in the same undo transaction.
    pub fn move_image_to_text(
        &mut self,
        block: usize,
        target: Position,
        placement: ImagePlacement,
    ) -> Option<usize> {
        if !placement.valid()
            || placement.flow != ImageFlow::Break
            || (target.address.row != 0 || target.address.cell != 0)
        {
            return None;
        }
        let Some(Block::Image(image)) = self.doc.blocks.get(block) else {
            return None;
        };
        let image_id = image.id.clone();
        let Some(Block::Paragraph(paragraph)) = self.doc.blocks.get(target.address.block) else {
            return None;
        };
        if target.offset > paragraph.chars.len() || paragraph.snap(target.offset) != target.offset {
            return None;
        }
        if target.offset == 0 || target.offset == paragraph.chars.len() {
            return self.move_image_with_placement(
                block,
                target.address.block + usize::from(target.offset > 0),
                placement,
            );
        }
        let destination = target.address.block - usize::from(block < target.address.block);
        self.transaction(|editor| {
            let mut image = editor.doc.blocks.remove(block);
            if let Block::Image(image) = &mut image {
                image.placement = placement;
            }
            let Block::Paragraph(left) = &mut editor.doc.blocks[destination] else {
                unreachable!()
            };
            let mut right = left.split_text(target.offset);
            left.after = 0.0;
            right.before = 0.0;
            right.page_break_before = false;
            editor.doc.blocks.insert(destination + 1, image);
            editor
                .doc
                .blocks
                .insert(destination + 2, Block::Paragraph(right));
            for position in [&mut editor.caret, &mut editor.anchor] {
                let index = position.address.block;
                if index == usize::MAX {
                    continue;
                }
                if index == block {
                    position.address.block = destination + 1;
                    continue;
                }
                let shifted = index - usize::from(index > block);
                position.address.block = shifted + if shifted > destination { 2 } else { 0 };
                if shifted == destination && position.offset >= target.offset {
                    position.address.block = destination + 2;
                    position.offset -= target.offset;
                }
            }
        });
        matches!(self.doc.blocks.get(destination + 1), Some(Block::Image(i)) if i.id == image_id)
            .then_some(destination + 1)
    }
    pub fn set_image_placement(&mut self, block: usize, placement: ImagePlacement) -> bool {
        if !placement.valid() || !matches!(self.doc.blocks.get(block), Some(Block::Image(_))) {
            return false;
        }
        self.transaction(|e| {
            if let Block::Image(i) = &mut e.doc.blocks[block] {
                i.placement = placement;
            }
        });
        true
    }
    pub fn move_image(&mut self, block: usize, before: usize) -> Option<usize> {
        let Some(Block::Image(image)) = self.doc.blocks.get(block) else {
            return None;
        };
        self.move_image_with_placement(block, before, image.placement)
    }
    pub fn move_image_with_placement(
        &mut self,
        block: usize,
        before: usize,
        placement: ImagePlacement,
    ) -> Option<usize> {
        if !matches!(self.doc.blocks.get(block), Some(Block::Image(_)))
            || before > self.doc.blocks.len()
            || !placement.valid()
        {
            return None;
        }
        let destination = if before > block { before - 1 } else { before };
        self.transaction(|e| {
            if let Block::Image(image) = &mut e.doc.blocks[block] {
                image.placement = placement;
            }
            if destination == block {
                return;
            }
            let image = e.doc.blocks.remove(block);
            e.doc.blocks.insert(destination, image);
            for position in [&mut e.caret, &mut e.anchor] {
                let index = position.address.block;
                if index == usize::MAX {
                    continue;
                }
                position.address.block = if index == block {
                    destination
                } else {
                    let shifted = index - usize::from(index > block);
                    shifted + usize::from(shifted >= destination)
                };
            }
        });
        Some(destination)
    }
    pub fn add_image_comment(&mut self, block: usize, text: &str) -> bool {
        if text.trim().is_empty() || !matches!(self.doc.blocks.get(block), Some(Block::Image(_))) {
            return false;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let comment = ImageComment {
            author: self.collaboration_name(),
            id: format!("{:x}", now.as_nanos()),
            text: text.trim().into(),
            created: now.as_secs(),
            resolved: false,
            replies: Default::default(),
            reactions: Default::default(),
        };
        self.transaction(|e| {
            if let Block::Image(i) = &mut e.doc.blocks[block] {
                i.comments.push(comment);
            }
        });
        true
    }
    pub fn reply_image_comment(&mut self, block: usize, id: &str, text: &str) -> bool {
        if text.trim().is_empty() {
            return false;
        }
        let Some(Block::Image(i)) = self.doc.blocks.get(block) else {
            return false;
        };
        let Some(index) = i.comments.iter().position(|c| c.id == id) else {
            return false;
        };
        let name = self.collaboration_name();
        let text = if name == "You" {
            text.trim().to_owned()
        } else {
            format!("{name}: {}", text.trim())
        };
        self.transaction(|e| {
            if let Block::Image(i) = &mut e.doc.blocks[block] {
                i.comments[index].replies.push(text);
            }
        });
        true
    }
    pub fn resolve_image_comment(&mut self, block: usize, id: &str, resolved: bool) {
        self.transaction(|e| {
            if let Some(Block::Image(i)) = e.doc.blocks.get_mut(block)
                && let Some(comment) = i.comments.iter_mut().find(|c| c.id == id)
            {
                comment.resolved = resolved;
            }
        });
    }
    pub fn edit_image_comment(&mut self, block: usize, id: &str, text: &str) -> bool {
        if text.trim().is_empty() {
            return false;
        }
        let Some(Block::Image(i)) = self.doc.blocks.get(block) else {
            return false;
        };
        let Some(index) = i.comments.iter().position(|c| c.id == id) else {
            return false;
        };
        self.transaction(|e| {
            if let Block::Image(i) = &mut e.doc.blocks[block] {
                i.comments[index].text = text.trim().into();
            }
        });
        true
    }
    pub fn delete_image_comment(&mut self, block: usize, id: &str) {
        self.transaction(|e| {
            if let Some(Block::Image(i)) = e.doc.blocks.get_mut(block) {
                i.comments.retain(|c| c.id != id);
            }
        });
    }
    pub fn toggle_image_reaction(&mut self, block: usize, emoji: &str) -> bool {
        if emojis::get(emoji).is_none()
            || !matches!(self.doc.blocks.get(block), Some(Block::Image(_)))
        {
            return false;
        }
        self.transaction(|e| {
            if let Block::Image(i) = &mut e.doc.blocks[block] {
                if let Some(index) = i.reactions.iter().position(|r| r == emoji) {
                    i.reactions.remove(index);
                } else {
                    i.reactions.push(emoji.into());
                }
            }
        });
        true
    }
    pub fn set_image_effects(&mut self, block: usize, effects: ImageEffects) {
        if !effects.valid() {
            return;
        }
        self.transaction(|e| {
            if let Some(Block::Image(image)) = e.doc.blocks.get_mut(block) {
                image.effects = effects;
            }
        });
    }
}
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipboardContent {
    pub text: String,
    pub blocks: Vec<Block>,
}
impl Editor {
    pub fn move_table(&mut self, block: usize, before: usize) -> bool {
        if !matches!(self.doc.blocks.get(block), Some(Block::Table(_)))
            || before > self.doc.blocks.len()
            || before == block
            || before == block + 1
        {
            return false;
        }
        let destination = if before > block { before - 1 } else { before };
        self.transaction(|e| {
            let table = e.doc.blocks.remove(block);
            e.doc.blocks.insert(destination, table);
            let address = if e.caret.address.block == block {
                Address {
                    block: destination,
                    ..e.caret.address
                }
            } else {
                Address {
                    block: destination,
                    row: 0,
                    cell: 0,
                }
            };
            e.caret = Position {
                address,
                offset: if e.caret.address.block == block {
                    e.caret.offset
                } else {
                    0
                },
            };
            e.anchor = e.caret;
        });
        self.sync_style();
        true
    }
    pub fn trim_image_transparency(&mut self, block: usize, enabled: bool) -> Result<(), String> {
        let Some(Block::Image(image)) = self.doc.blocks.get(block) else {
            return Err("Select an image".into());
        };
        let bounds = if enabled {
            Some(
                crate::image_edit::alpha_bounds(&image.data)?
                    .ok_or("The image has no visible pixels")?,
            )
        } else {
            image.trim_bounds
        };
        self.transaction(|e| {
            if let Some(Block::Image(image)) = e.doc.blocks.get_mut(block) {
                image.trim_transparency = enabled;
                image.trim_bounds = bounds;
            }
        });
        Ok(())
    }
    pub fn resize_image(&mut self, block: usize, width: f32, height: f32) {
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return;
        }
        if !self.collaboration_writable() {
            return;
        }
        let regenerated = if let Some(Block::Image(image)) = self.doc.blocks.get(block)
            && let Some(object) = &image.editable
        {
            let mut source = object.source.clone();
            if let ObjectSource::Equation { size, .. } = &mut source {
                let factor = if (width - image.width).abs() > 0.01 {
                    width / image.width
                } else {
                    height / image.height
                };
                *size = (*size * factor).clamp(6.0, 100.0);
            }
            let Ok(generated) = crate::editable_objects::prepare(
                &eframe::egui::Context::default(),
                source,
                width,
                height,
            ) else {
                return;
            };
            Some(generated)
        } else {
            None
        };
        self.transaction(|e| {
            if let Some(Block::Image(image)) = e.doc.blocks.get_mut(block) {
                if let Some(generated) = regenerated {
                    image.data = generated.data;
                    image.editable = generated.editable;
                    image.width = generated.width;
                    image.height = generated.height;
                } else {
                    image.width = width;
                    image.height = height;
                }
            }
        });
    }
    pub fn delete_image(&mut self, block: usize) {
        if !matches!(self.doc.blocks.get(block), Some(Block::Image(_))) {
            return;
        }
        self.transaction(|e| {
            e.doc.blocks.remove(block);
            if e.doc.addresses().is_empty() {
                e.doc.blocks.push(Block::Paragraph(Paragraph::default()));
            }
            let addresses = e.doc.addresses();
            let address = addresses
                .iter()
                .find(|a| a.block >= block)
                .copied()
                .unwrap_or_else(|| *addresses.last().expect("Editable paragraph"));
            e.caret = Position { address, offset: 0 };
            e.anchor = e.caret;
        });
        self.sync_style();
    }
    pub fn insert_table(&mut self, rows: usize, columns: usize) {
        if rows == 0 || columns == 0 || rows > 100 || columns > 32 {
            return;
        }
        // The flat cell model cannot represent nested tables. Refuse them instead
        // of inserting an outer block in the middle of the current table.
        if matches!(
            self.doc.blocks.get(self.caret.address.block),
            Some(Block::Table(_))
        ) {
            return;
        }
        // Tables are body blocks. Running header/footer paragraphs have sentinel
        // addresses and cannot be replaced by a body table.
        if !matches!(
            self.doc.blocks.get(self.caret.address.block),
            Some(Block::Paragraph(_))
        ) {
            return;
        }
        let cell = Paragraph {
            style: self.typing.clone(),
            after: 0.0,
            ..Default::default()
        };
        self.transaction(|e| {
            e.delete_inner();
            let a = e.caret.address;
            let offset = e.caret.offset;
            let p = e.doc.paragraph_mut(a).expect("Body paragraph");
            let mut tail = p.split_text(offset);
            tail.page_break_before = false;
            let replace_empty = p.chars.is_empty();
            let index = a.block + usize::from(!replace_empty);
            let table = Block::Table(vec![vec![cell; columns]; rows]);
            if replace_empty {
                e.doc.blocks[a.block] = table;
            } else {
                e.doc.blocks.insert(index, table);
            }
            e.doc.blocks.insert(index + 1, Block::Paragraph(tail));
            e.caret = Position {
                address: Address {
                    block: index,
                    row: 0,
                    cell: 0,
                },
                offset: 0,
            };
            e.anchor = e.caret;
        });
        self.sync_style();
    }
    pub fn delete_table(&mut self) {
        let a = self.caret.address;
        if !matches!(self.doc.blocks.get(a.block), Some(Block::Table(_))) {
            return;
        }
        self.transaction(|e| {
            e.doc.blocks[a.block] = Block::Paragraph(Paragraph::default());
            e.caret = Position {
                address: Address {
                    block: a.block,
                    row: 0,
                    cell: 0,
                },
                offset: 0,
            };
            e.anchor = e.caret;
        });
        self.sync_style();
    }
    pub fn copy_fragment(&self) -> ClipboardContent {
        if let Some(selection) = self.table_selection() {
            let table = match selection {
                super::TableSelection::Row { block, row } => {
                    let Some(Block::Table(rows)) = self.doc.blocks.get(block) else {
                        return ClipboardContent {
                            text: String::new(),
                            blocks: vec![],
                        };
                    };
                    let Some(cells) = rows.get(row) else {
                        return ClipboardContent {
                            text: String::new(),
                            blocks: vec![],
                        };
                    };
                    vec![cells.clone()]
                }
                super::TableSelection::Column { block, column } => {
                    let Some(Block::Table(rows)) = self.doc.blocks.get(block) else {
                        return ClipboardContent {
                            text: String::new(),
                            blocks: vec![],
                        };
                    };
                    rows.iter()
                        .map(|row| vec![row.get(column).cloned().unwrap_or_default()])
                        .collect()
                }
            };
            return ClipboardContent {
                text: self.selected_table_text().unwrap_or_default(),
                blocks: vec![Block::Table(table)],
            };
        }
        let (start, end) = self.range();
        let mut blocks = vec![];
        let addresses = self.doc.addresses();
        if self.selected()
            && addresses.first() == Some(&start.address)
            && addresses.last() == Some(&end.address)
            && start.offset == 0
            && self
                .doc
                .paragraph(end.address)
                .is_some_and(|p| end.offset == p.chars.len())
        {
            blocks = self.doc.blocks.clone();
        } else if self.selected()
            && matches!(
                self.doc.blocks.get(start.address.block),
                Some(Block::Paragraph(_))
            )
            && matches!(
                self.doc.blocks.get(end.address.block),
                Some(Block::Paragraph(_))
            )
        {
            blocks = self.doc.blocks[start.address.block..=end.address.block].to_vec();
            if let Some(Block::Paragraph(p)) = blocks.last_mut() {
                p.slice_text(0, end.offset);
            }
            if let Some(Block::Paragraph(p)) = blocks.first_mut() {
                p.slice_text(start.offset, p.chars.len());
            }
        } else {
            for address in self
                .doc
                .addresses()
                .into_iter()
                .filter(|a| *a >= start.address && *a <= end.address)
            {
                let mut p = self
                    .doc
                    .paragraph(address)
                    .expect("Valid paragraph")
                    .clone();
                let last = if address == end.address {
                    end.offset
                } else {
                    p.chars.len()
                };
                let first = if address == start.address {
                    start.offset
                } else {
                    0
                };
                p.slice_text(first, last);
                blocks.push(Block::Paragraph(p));
            }
        }
        ClipboardContent {
            text: self.selected_text(),
            blocks,
        }
    }
    pub fn paste_fragment(&mut self, fragment: &ClipboardContent) {
        self.paste_fragment_replacing_image(fragment, None);
    }
    pub fn paste_fragment_replacing_image(
        &mut self,
        fragment: &ClipboardContent,
        image: Option<usize>,
    ) {
        if fragment.blocks.is_empty() {
            return;
        }
        if image.is_some_and(|block| !matches!(self.doc.blocks.get(block), Some(Block::Image(_)))) {
            return;
        }
        if image.is_none() && self.table_selection().is_some() {
            let mut chars = Vec::new();
            let mut has_content = false;
            for block in &fragment.blocks {
                match block {
                    Block::Paragraph(paragraph) => {
                        if has_content {
                            chars.push(StyledChar {
                                ch: '\n',
                                style: self.typing.clone(),
                            });
                        }
                        chars.extend(paragraph.chars.iter().cloned());
                        has_content = true;
                    }
                    Block::Table(rows) => {
                        for (row_index, row) in rows.iter().enumerate() {
                            if has_content || row_index > 0 {
                                chars.push(StyledChar {
                                    ch: '\n',
                                    style: self.typing.clone(),
                                });
                            }
                            for (cell_index, cell) in row.iter().enumerate() {
                                if cell_index > 0 {
                                    chars.push(StyledChar {
                                        ch: '\t',
                                        style: self.typing.clone(),
                                    });
                                }
                                chars.extend(cell.chars.iter().cloned());
                            }
                            has_content = true;
                        }
                    }
                    Block::Image(_) | Block::PageBreak | Block::Rule => {}
                }
            }
            if !chars.is_empty() {
                self.insert_rich(vec![chars]);
            }
            return;
        }
        // Table cells accept rich text as line-separated paragraphs. Outer tables and images
        // retain structure when pasted into the document body.
        if image.is_none()
            && !self.selection_covers_body()
            && matches!(
                self.doc.blocks.get(self.caret.address.block),
                Some(Block::Table(_))
            )
        {
            let mut text = vec![];
            for b in &fragment.blocks {
                if let Block::Paragraph(p) = b {
                    if !text.is_empty() {
                        text.push(StyledChar {
                            ch: '\n',
                            style: self.typing.clone(),
                        });
                    }
                    text.extend(p.chars.clone());
                }
            }
            self.insert_rich(vec![text]);
            return;
        }
        let mut blocks = fragment.blocks.clone();
        super::annotations::fresh_ids(&mut blocks);
        for block in &mut blocks {
            if let Block::Image(image) = block {
                image.id = new_object_id();
            }
        }
        self.transaction(|e| {
            if let Some(block) = image {
                e.doc.blocks[block] = Block::Paragraph(Paragraph {
                    style: e.typing.clone(),
                    ..Default::default()
                });
                e.caret = Position {
                    address: Address {
                        block,
                        row: 0,
                        cell: 0,
                    },
                    offset: 0,
                };
                e.anchor = e.caret;
            }
            e.delete_inner();
            let address = e.caret.address;
            let offset = e.caret.offset;
            let Some(current) = e.doc.paragraph_mut(address) else {
                return;
            };
            let tail_paragraph = current.split_text(offset);
            let prefix_empty = current.chars.is_empty();
            let mut inserted = blocks;
            let mut first_is_paragraph = false;
            if let Some(Block::Paragraph(first)) = inserted.first_mut() {
                first_is_paragraph = true;
                if prefix_empty {
                    let mut pasted = first.clone();
                    pasted.chars.clear();
                    pasted.annotations.clear();
                    *current = pasted;
                }
                current.append_text(first.clone());
            }
            if first_is_paragraph {
                inserted.remove(0);
            }
            let caret = if inserted.is_empty() {
                let end = current.chars.len();
                current.append_text(tail_paragraph);
                Position {
                    address,
                    offset: end,
                }
            } else if let Some(Block::Paragraph(last)) = inserted.last_mut() {
                let end = last.chars.len();
                last.append_text(tail_paragraph);
                Position {
                    address: Address {
                        block: address.block + inserted.len(),
                        row: 0,
                        cell: 0,
                    },
                    offset: end,
                }
            } else {
                inserted.push(Block::Paragraph(tail_paragraph));
                Position {
                    address: Address {
                        block: address.block + inserted.len(),
                        row: 0,
                        cell: 0,
                    },
                    offset: 0,
                }
            };
            let replace_empty = prefix_empty && !first_is_paragraph;
            let insertion_start = address.block + usize::from(!replace_empty);
            e.doc
                .blocks
                .splice(insertion_start..address.block + 1, inserted);
            let mut caret = caret;
            if replace_empty {
                caret.address.block = caret.address.block.saturating_sub(1);
            }
            e.caret = caret;
            e.anchor = caret;
        });
        self.sync_style();
    }
    pub fn soft_break(&mut self) {
        let style = self.typing.clone();
        self.insert_rich(vec![vec![StyledChar { ch: '\n', style }]]);
    }
    pub fn delete_word(&mut self, back: bool) {
        if self.selected() {
            self.delete_selection();
            return;
        }
        let original = self.caret;
        self.move_horizontal(!back, true, true);
        if self.caret == original {
            self.delete(back);
        } else {
            self.delete_selection();
        }
    }
    pub fn tab(&mut self, back: bool) {
        let a = self.caret.address;
        let Some(Block::Table(rows)) = self.doc.blocks.get(a.block) else {
            if back {
                self.paragraph_format(|p| p.indent = (p.indent - 36.0).max(0.0));
            } else {
                self.insert("\t");
            }
            return;
        };
        let destination = if back {
            if a.cell > 0 {
                Some(Address {
                    cell: a.cell - 1,
                    ..a
                })
            } else if a.row > 0 {
                Some(Address {
                    row: a.row - 1,
                    cell: rows[a.row - 1].len() - 1,
                    ..a
                })
            } else {
                None
            }
        } else if a.cell + 1 < rows[a.row].len() {
            Some(Address {
                cell: a.cell + 1,
                ..a
            })
        } else if a.row + 1 < rows.len() {
            Some(Address {
                row: a.row + 1,
                cell: 0,
                ..a
            })
        } else {
            None
        };
        if let Some(address) = destination {
            self.place(Position { address, offset: 0 }, false);
        } else if !back {
            self.insert_table_row(true);
            self.place(
                Position {
                    address: Address {
                        row: a.row + 1,
                        cell: 0,
                        ..a
                    },
                    offset: 0,
                },
                false,
            );
        }
    }
    pub fn insert_table_row(&mut self, after: bool) {
        let a = self.caret.address;
        self.transaction(|e| {
            if let Some(Block::Table(rows)) = e.doc.blocks.get_mut(a.block)
                && let Some(source) = rows.get(a.row)
            {
                let new = source
                    .iter()
                    .map(|p| {
                        let mut p = p.clone();
                        p.chars.clear();
                        p.annotations.clear();
                        p.checked = false;
                        p
                    })
                    .collect();
                let index = a.row + usize::from(after);
                rows.insert(index, new);
                if !after {
                    e.caret.address.row += 1;
                    e.anchor = e.caret;
                }
            }
        });
    }
    pub fn insert_table_column(&mut self, after: bool) {
        let a = self.caret.address;
        self.transaction(|e| {
            if let Some(Block::Table(rows)) = e.doc.blocks.get_mut(a.block) {
                for row in rows {
                    if let Some(source) = row.get(a.cell) {
                        let mut p = source.clone();
                        p.chars.clear();
                        p.annotations.clear();
                        row.insert(a.cell + usize::from(after), p);
                    }
                }
                if !after {
                    e.caret.address.cell += 1;
                    e.anchor = e.caret;
                }
            }
        });
    }
    pub fn delete_table_row(&mut self) {
        let a = self.caret.address;
        self.transaction(|e| {
            if let Some(Block::Table(rows)) = e.doc.blocks.get_mut(a.block) {
                if rows.len() > 1 {
                    rows.remove(a.row);
                    e.caret.address.row = a.row.min(rows.len() - 1);
                } else {
                    e.doc.blocks[a.block] = Block::Paragraph(Paragraph::default());
                    e.caret.address = Address {
                        block: a.block,
                        row: 0,
                        cell: 0,
                    };
                }
                e.caret.offset = 0;
                e.anchor = e.caret;
            }
        });
        self.sync_style();
    }
    pub fn delete_table_column(&mut self) {
        let a = self.caret.address;
        self.transaction(|e| {
            if let Some(Block::Table(rows)) = e.doc.blocks.get_mut(a.block) {
                if rows.first().is_some_and(|r| r.len() > 1) {
                    for row in rows.iter_mut() {
                        if a.cell < row.len() {
                            row.remove(a.cell);
                        }
                    }
                    e.caret.address.cell = a.cell.min(rows[a.row].len() - 1);
                } else {
                    e.doc.blocks[a.block] = Block::Paragraph(Paragraph::default());
                    e.caret.address = Address {
                        block: a.block,
                        row: 0,
                        cell: 0,
                    };
                }
                e.caret.offset = 0;
                e.anchor = e.caret;
            }
        });
        self.sync_style();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tables_cannot_replace_running_text_and_deletion_updates_typing_style() {
        let mut e = Editor::new(Document::new("Safe tables"));
        e.place(
            Position {
                address: Address {
                    block: usize::MAX,
                    row: 0,
                    cell: 0,
                },
                offset: 0,
            },
            false,
        );
        let before = e.doc.clone();
        e.insert_table(2, 2);
        assert_eq!(e.doc, before);
        assert!(!e.can_undo());
        e.place(Position::default(), false);
        e.insert_table(1, 2);
        e.format(|s| {
            s.bold = true;
            s.size = 24.0;
        });
        e.insert("Styled cell");
        e.delete_table_column();
        assert!(!e.typing.bold);
        assert_eq!(e.typing.size, 11.0);
        e.insert("Replacement");
        assert!(
            e.doc
                .paragraph(e.caret.address)
                .unwrap()
                .chars
                .iter()
                .all(|c| !c.style.bold)
        );
        e.undo();
        e.undo();
        assert_eq!(
            e.doc.paragraph(Address::default()).unwrap().text(),
            "Styled cell"
        );
        assert!(e.typing.bold);
        e.doc.validate().unwrap();
    }
    #[test]
    fn inserting_table_focuses_first_cell_and_preserves_surrounding_text() {
        let mut e = Editor::new(Document::new("Table insertion"));
        e.insert("BeforeAfter");
        e.place(
            Position {
                address: Address::default(),
                offset: 6,
            },
            false,
        );
        let before = e.doc.clone();
        e.insert_table(2, 3);
        assert_eq!(
            e.caret,
            Position {
                address: Address {
                    block: 1,
                    row: 0,
                    cell: 0
                },
                offset: 0
            }
        );
        assert_eq!(
            e.doc.paragraph(Address::default()).unwrap().text(),
            "Before"
        );
        assert_eq!(
            e.doc
                .paragraph(Address {
                    block: 2,
                    ..Default::default()
                })
                .unwrap()
                .text(),
            "After"
        );
        e.insert("Cell one");
        e.tab(false);
        e.insert("Cell two");
        assert_eq!(
            e.doc
                .paragraph(Address {
                    block: 1,
                    row: 0,
                    cell: 1
                })
                .unwrap()
                .text(),
            "Cell two"
        );
        e.undo();
        e.undo();
        e.undo();
        assert_eq!(e.doc, before);
        e.redo();
        e.redo();
        e.redo();
        e.doc.validate().unwrap();
        e.delete_table();
        e.doc.validate().unwrap();
        e.undo();
        assert!(matches!(e.doc.blocks[1], Block::Table(_)));
    }
    #[test]
    fn whole_document_copy_keeps_a_leading_table() {
        let mut e = Editor::new(Document::new("Copy"));
        e.insert_table(1, 2);
        e.insert("Cell");
        e.select_all();
        assert_eq!(e.copy_fragment().blocks, e.doc.blocks);
        let mut target = Editor::new(Document::new("Paste"));
        target.paste_fragment(&e.copy_fragment());
        assert_eq!(target.doc.blocks, e.doc.blocks);
        target.doc.validate().unwrap();
    }
    #[test]
    fn body_clipboard_preserves_headings_tables_and_images() {
        let mut source = Editor::new(Document::new("Source"));
        let mut title = Paragraph::from_text("Heading");
        title.set_heading(1);
        source.doc.blocks = vec![
            Block::Paragraph(title),
            Block::Table(vec![vec![Paragraph::from_text("Cell")]]),
            Block::Image(EmbeddedImage {
                name: "image.png".into(),
                data: vec![1].into(),
                width: 20.0,
                height: 20.0,
                ..Default::default()
            }),
            Block::Paragraph(Paragraph::from_text("End")),
        ];
        source.select_all();
        let payload = source.copy_fragment();
        let mut target = Editor::new(Document::new("Target"));
        target.paste_fragment(&payload);
        let Block::Image(original) = &source.doc.blocks[2] else {
            panic!()
        };
        let Block::Image(copy) = &mut target.doc.blocks[2] else {
            panic!()
        };
        assert_ne!(copy.id, original.id);
        copy.id = original.id.clone();
        assert_eq!(target.doc.blocks, source.doc.blocks);
        target.undo();
        assert_eq!(target.doc.text(), "");
    }
    #[test]
    fn tab_creates_row_and_deleting_cells_is_undoable() {
        let mut e = Editor::new(Document::new("Table"));
        e.doc.blocks = vec![Block::Table(vec![vec![
            Paragraph::from_text("One"),
            Paragraph::from_text("Two"),
        ]])];
        e.place(
            Position {
                address: Address {
                    block: 0,
                    row: 0,
                    cell: 1,
                },
                offset: 3,
            },
            false,
        );
        e.tab(false);
        assert_eq!(e.caret.address.row, 1);
        let Block::Table(rows) = &e.doc.blocks[0] else {
            panic!()
        };
        assert_eq!(rows.len(), 2);
        e.delete_table_column();
        assert_eq!(e.caret.address.cell, 0);
        e.undo();
        let Block::Table(rows) = &e.doc.blocks[0] else {
            panic!()
        };
        assert_eq!(rows[0].len(), 2);
        e.delete_table_row();
        e.undo();
        assert_eq!(
            e.doc
                .paragraph(Address {
                    block: 0,
                    row: 1,
                    cell: 0
                })
                .unwrap()
                .text(),
            ""
        );
    }
    #[test]
    fn caret_snap_and_soft_break_preserve_graphemes() {
        let mut e = Editor::new(Document::new("Unicode"));
        e.insert("e\u{301}👩‍🔬");
        e.place(
            Position {
                address: Address::default(),
                offset: 1,
            },
            false,
        );
        assert_eq!(e.caret.offset, 0);
        e.move_horizontal(true, false, false);
        assert_eq!(e.caret.offset, 2);
        e.soft_break();
        assert_eq!(e.doc.blocks.len(), 1);
        assert_eq!(e.doc.text(), "e\u{301}\n👩‍🔬");
    }
    #[test]
    fn word_delete_keeps_unicode_boundaries() {
        let mut e = Editor::new(Document::new("Unicode"));
        e.insert("naïve café");
        e.delete_word(true);
        assert_eq!(e.doc.text(), "naïve ");
        e.undo();
        assert_eq!(e.doc.text(), "naïve café");
    }
}
