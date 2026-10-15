use super::*;
impl Paragraph {
    /// Scalar offsets are supplied by the grapheme-aware engine. Transform
    /// anchors at the actual edit location, never by guessing a text diff.
    pub(crate) fn replace_text(&mut self, range: std::ops::Range<usize>, chars: Vec<StyledChar>) {
        let inserted = chars.len();
        let removed = range.end - range.start;
        for note in &mut self.annotations {
            if removed == 0 {
                if range.start <= note.start {
                    note.start += inserted;
                    note.end += inserted;
                } else if range.start < note.end {
                    note.end += inserted;
                }
            } else if note.end <= range.start {
                // Entirely before the edit.
            } else if note.start >= range.end {
                note.start = note.start - removed + inserted;
                note.end = note.end - removed + inserted;
            } else {
                note.start = note.start.min(range.start);
                note.end = if note.end > range.end {
                    note.end - removed + inserted
                } else {
                    range.start + inserted
                };
                note.end = note.end.max(note.start);
            }
        }
        self.chars.splice(range, chars);
        self.snap_annotations();
    }
    pub(crate) fn split_text(&mut self, offset: usize) -> Self {
        let mut right = self.clone();
        right.chars = self.chars.split_off(offset).into();
        right.annotations.clear();
        let old = std::mem::take(&mut self.annotations);
        for mut note in old {
            if note.start >= offset {
                note.start -= offset;
                note.end -= offset;
                right.annotations.push(note);
            } else if note.end <= offset {
                self.annotations.push(note);
            } else {
                let mut tail = note.clone();
                tail.start = 0;
                tail.end -= offset;
                note.end = offset;
                self.annotations.push(note);
                right.annotations.push(tail);
            }
        }
        right
    }
    pub(crate) fn append_text(&mut self, mut other: Self) {
        let offset = self.chars.len();
        self.chars.extend(std::mem::take(&mut other.chars));
        for mut note in other.annotations {
            note.start += offset;
            note.end += offset;
            self.annotations.push(note);
        }
        self.snap_annotations();
    }
    pub(crate) fn slice_text(&mut self, start: usize, end: usize) {
        self.chars = self.chars[start..end].to_vec().into();
        self.annotations.retain(|n| n.end > start && n.start < end);
        for n in &mut self.annotations {
            n.start = n.start.max(start) - start;
            n.end = n.end.min(end) - start;
        }
    }
    fn snap_annotations(&mut self) {
        if self.annotations.is_empty() {
            return;
        }
        let mut boundaries = vec![0usize];
        for g in self.text().graphemes(true) {
            boundaries.push(boundaries.last().copied().unwrap_or(0) + g.chars().count());
        }
        for n in &mut self.annotations {
            let start = boundaries
                .binary_search(&n.start)
                .unwrap_or_else(|i| i.saturating_sub(1));
            let end = boundaries
                .binary_search(&n.end)
                .unwrap_or_else(|i| i.min(boundaries.len() - 1));
            if n.start == n.end {
                n.start = boundaries[start];
                n.end = n.start;
            } else {
                n.start = boundaries[start];
                n.end = boundaries[end];
            }
        }
    }
}
