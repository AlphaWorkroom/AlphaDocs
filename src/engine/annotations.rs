use super::*;

pub(super) fn fresh_ids(blocks: &mut [Block]) {
    let mut ids = std::collections::HashMap::new();
    let mut reset = |p: &mut Paragraph| {
        for n in &mut p.annotations {
            n.id = ids
                .entry(n.id.clone())
                .or_insert_with(new_object_id)
                .clone();
        }
    };
    for b in blocks {
        match b {
            Block::Paragraph(p) => reset(p),
            Block::Table(rows) => {
                for p in rows.iter_mut().flatten() {
                    reset(p);
                }
            }
            _ => {}
        }
    }
}
fn comment(text: &str, author: String) -> ImageComment {
    ImageComment {
        author,
        id: new_object_id(),
        text: text.trim().into(),
        created: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        resolved: false,
        replies: Default::default(),
        reactions: Default::default(),
    }
}
use std::time::{SystemTime, UNIX_EPOCH};
impl Editor {
    pub fn annotate_text(
        &mut self,
        range: (Position, Position),
        text: Option<&str>,
        emoji: Option<&str>,
    ) -> Option<String> {
        let (start, end) = range;
        if start >= end
            || start.address.block >= self.doc.blocks.len()
            || end.address.block >= self.doc.blocks.len()
            || text.is_some_and(|s| s.trim().is_empty())
            || emoji.is_some_and(|s| emojis::get(s).is_none())
            || text.is_none() && emoji.is_none()
        {
            return None;
        }
        let (Some(a), Some(b)) = (
            self.doc.paragraph(start.address),
            self.doc.paragraph(end.address),
        ) else {
            return None;
        };
        if a.snap(start.offset) != start.offset || b.snap(end.offset) != end.offset {
            return None;
        }
        let id = new_object_id();
        let comments: SharedVec<_> = text
            .map(|text| comment(text, self.collaboration_name()))
            .into_iter()
            .collect();
        let reactions: SharedVec<_> = emoji.map(str::to_owned).into_iter().collect();
        self.transaction(|e| {
            let body_len = e.doc.blocks.len();
            for address in e
                .doc
                .addresses()
                .into_iter()
                .filter(|a| *a >= start.address && *a <= end.address && a.block < body_len)
            {
                let p = e.doc.paragraph_mut(address).expect("Body paragraph");
                let lo = if address == start.address {
                    start.offset
                } else {
                    0
                };
                let hi = if address == end.address {
                    end.offset
                } else {
                    p.chars.len()
                };
                if lo < hi {
                    p.annotations.push(TextAnnotation {
                        id: id.clone(),
                        start: lo,
                        end: hi,
                        comments: comments.clone(),
                        reactions: reactions.clone(),
                    });
                }
            }
        });
        Some(id)
    }
    fn annotation_mut(
        &mut self,
        id: &str,
        mut f: impl FnMut(&mut SharedVec<ImageComment>, &mut SharedVec<String>),
    ) {
        for b in &mut self.doc.blocks {
            match b {
                Block::Image(i) if i.id == id => f(&mut i.comments, &mut i.reactions),
                Block::Paragraph(p) if p.annotations.iter().any(|n| n.id == id) => {
                    for n in &mut p.annotations {
                        if n.id == id {
                            f(&mut n.comments, &mut n.reactions);
                        }
                    }
                }
                Block::Table(rows) => {
                    for p in rows.iter_mut().flatten() {
                        if !p.annotations.iter().any(|n| n.id == id) {
                            continue;
                        }
                        for n in &mut p.annotations {
                            if n.id == id {
                                f(&mut n.comments, &mut n.reactions);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    pub fn add_annotation_comment(&mut self, id: &str, text: &str) -> Option<String> {
        if text.trim().is_empty() {
            return None;
        }
        let c = comment(text, self.collaboration_name());
        let cid = c.id.clone();
        let mut found = false;
        self.transaction(|e| {
            e.annotation_mut(id, |comments, _| {
                comments.push(c.clone());
                found = true;
            })
        });
        found.then_some(cid)
    }
    pub fn edit_annotation_comment(&mut self, id: &str, cid: &str, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        self.transaction(|e| {
            e.annotation_mut(id, |cs, _| {
                for c in cs {
                    if c.id == cid {
                        c.text = text.trim().into();
                    }
                }
            })
        });
    }
    pub fn reply_annotation_comment(&mut self, id: &str, cid: &str, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        let name = self.collaboration_name();
        let text = if name == "You" {
            text.trim().to_owned()
        } else {
            format!("{name}: {}", text.trim())
        };
        self.transaction(|e| {
            e.annotation_mut(id, |cs, _| {
                for c in cs {
                    if c.id == cid {
                        c.replies.push(text.clone());
                    }
                }
            })
        });
    }
    pub fn resolve_annotation_comment(&mut self, id: &str, cid: &str, resolved: bool) {
        self.transaction(|e| {
            e.annotation_mut(id, |cs, _| {
                for c in cs {
                    if c.id == cid {
                        c.resolved = resolved;
                    }
                }
            })
        });
    }
    pub fn delete_annotation_comment(&mut self, id: &str, cid: &str) {
        self.transaction(|e| e.annotation_mut(id, |cs, _| cs.retain(|c| c.id != cid)));
    }
    pub fn toggle_annotation_reaction(&mut self, id: &str, cid: Option<&str>, emoji: &str) {
        if emojis::get(emoji).is_none() {
            return;
        }
        let toggle = |rs: &mut SharedVec<String>| {
            if rs.iter().any(|r| r == emoji) {
                rs.retain(|r| r != emoji);
            } else {
                rs.push(emoji.into());
            }
        };
        self.transaction(|e| {
            e.annotation_mut(id, |cs, rs| {
                if let Some(cid) = cid {
                    for c in cs {
                        if c.id == cid {
                            toggle(&mut c.reactions);
                        }
                    }
                } else {
                    toggle(rs);
                }
            })
        });
    }
}
