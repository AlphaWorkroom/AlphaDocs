use super::*;
use crate::writing::TypingPreferences;

impl Editor {
    /// Automatic typing edits share one transaction with their triggering keystroke.
    /// Paste and IME use their existing paths; this never rewrites an entire paragraph.
    pub fn type_text(&mut self, text: &str, prefs: &TypingPreferences) {
        if self.selected()
            || text.graphemes(true).count() != 1
            || text.contains(['\r', '\n'])
            || !(prefs.capitalize
                || prefs.smart_quotes
                || prefs.detect_links
                || prefs.detect_lists
                || prefs.substitutions)
        {
            self.insert(text);
            return;
        }
        let Some(p) = self.doc.paragraph(self.caret.address) else {
            return;
        };
        let before: String = p.chars[..self.caret.offset].iter().map(|c| c.ch).collect();
        let previous = before.chars().next_back();
        let mut insert = text.to_owned();
        if prefs.smart_quotes {
            insert = match text {
                "\"" => if previous.is_none_or(|c| c.is_whitespace() || "([{—".contains(c)) {
                    "“"
                } else {
                    "”"
                }
                .into(),
                "'" => if previous.is_none_or(|c| c.is_whitespace() || "([{—".contains(c)) {
                    "‘"
                } else {
                    "’"
                }
                .into(),
                _ => insert,
            };
        }
        if prefs.capitalize
            && text.chars().all(char::is_alphabetic)
            && (before.trim().is_empty()
                || previous.is_some_and(char::is_whitespace)
                    && before.trim_end().ends_with(['.', '!', '?']))
        {
            insert = text.to_uppercase();
        }
        let combined = format!("{before}{insert}");
        let mut replacement = None;
        if prefs.substitutions {
            let whitespace = insert.chars().all(char::is_whitespace);
            let haystack = if whitespace {
                before.as_str()
            } else {
                combined.as_str()
            };
            for sub in prefs.replacements.iter().filter(|s| {
                s.enabled && !s.from.is_empty() && s.from.len() <= 128 && s.to.len() <= 512
            }) {
                if let Some(prefix) = haystack.strip_suffix(&sub.from)
                    && haystack
                        .grapheme_indices(true)
                        .any(|(offset, _)| offset == prefix.len())
                    && prefix
                        .chars()
                        .next_back()
                        .is_none_or(|c| !c.is_alphanumeric())
                {
                    replacement = Some((
                        prefix.chars().count(),
                        format!("{}{}", sub.to, if whitespace { &insert } else { "" }),
                    ));
                    break;
                }
            }
        }
        let style = self.typing.clone();
        self.transaction(|e| {
            let mut start = e.caret.offset;
            let mut content = insert.clone();
            if let Some((from, value)) = &replacement {
                start = *from;
                content = value.clone();
            }
            let end = e.caret.offset;
            let Some(p) = e.doc.paragraph_mut(e.caret.address) else {
                return;
            };
            p.replace_text(
                start..end,
                content
                    .chars()
                    .map(|ch| StyledChar {
                        ch,
                        style: style.clone(),
                    })
                    .collect(),
            );
            e.caret.offset = start + content.chars().count();
            e.anchor = e.caret;
            if prefs.detect_lists
                && text == " "
                && matches!(before.as_str(), "-" | "*" | "1.")
                && p.list == ListKind::None
            {
                p.list = if before == "1." {
                    ListKind::Number
                } else {
                    ListKind::Bullet
                };
                p.replace_text(0..e.caret.offset, vec![]);
                e.caret.offset = 0;
                e.anchor = e.caret;
            } else if prefs.detect_links && text.chars().all(char::is_whitespace) {
                let start = before
                    .char_indices()
                    .rfind(|(_, c)| c.is_whitespace())
                    .map_or(0, |(i, c)| i + c.len_utf8());
                let token = before[start..].trim_end_matches(['.', ',', ';', ':', '!', ')']);
                if (token.starts_with("https://")
                    || token.starts_with("http://")
                    || token.starts_with("www."))
                    && token.len() <= 2048
                    && token.contains('.')
                {
                    let offset = before[..start].chars().count();
                    let end = offset + token.chars().count();
                    for c in &mut p.chars[offset..end] {
                        c.style.link = Some(if token.starts_with("www.") {
                            format!("https://{token}")
                        } else {
                            token.into()
                        });
                        c.style.underline = true;
                        c.style.color = [11, 87, 208];
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn substitutions_links_lists_are_single_undoable_unicode_edits() {
        let mut e = Editor::new(Document::new("Typing"));
        e.insert("é (c)");
        let before = e.doc.clone();
        let p = TypingPreferences {
            substitutions: true,
            ..Default::default()
        };
        e.type_text(" ", &p);
        assert_eq!(e.doc.text(), "é © ");
        e.undo();
        assert_eq!(e.doc, before);
        e.redo();
        assert_eq!(e.doc.text(), "é © ");
        e.select_all();
        e.insert("https://example.com");
        let before = e.doc.clone();
        e.type_text(
            " ",
            &TypingPreferences {
                detect_links: true,
                ..Default::default()
            },
        );
        assert_eq!(
            e.doc.paragraph(Address::default()).unwrap().chars[0]
                .style
                .link
                .as_deref(),
            Some("https://example.com")
        );
        e.undo();
        assert_eq!(e.doc, before);
        e.select_all();
        e.insert("-");
        let before = e.doc.clone();
        e.type_text(
            " ",
            &TypingPreferences {
                detect_lists: true,
                ..Default::default()
            },
        );
        assert_eq!(
            e.doc.paragraph(Address::default()).unwrap().list,
            ListKind::Bullet
        );
        e.undo();
        assert_eq!(e.doc, before);
    }

    #[test]
    fn substitution_cannot_split_a_combining_grapheme() {
        let mut e = Editor::new(Document::new("Typing"));
        e.insert("a\u{301}");
        let prefs = TypingPreferences {
            substitutions: true,
            replacements: vec![crate::writing::Substitution {
                enabled: true,
                from: "\u{301}".into(),
                to: "x".into(),
            }],
            ..Default::default()
        };
        e.type_text(" ", &prefs);
        assert_eq!(e.doc.text(), "a\u{301} ");
    }
}
