//! Offline English spelling and conservative grammar rules. No document text leaves the device.
use crate::{engine::Editor, model::*};
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};
use unicode_segmentation::UnicodeSegmentation;
#[derive(Clone, Debug)]
pub struct Issue {
    pub address: Address,
    pub start: usize,
    pub end: usize,
    pub original: String,
    pub message: &'static str,
    pub suggestions: Vec<String>,
    pub grammar: bool,
}
fn dictionary() -> Result<&'static spellbook::Dictionary, String> {
    static DICTIONARY: OnceLock<Result<spellbook::Dictionary, String>> = OnceLock::new();
    DICTIONARY
        .get_or_init(|| {
            spellbook::Dictionary::new(
                include_str!("../assets/proofing/index.aff"),
                include_str!("../assets/proofing/index.dic"),
            )
            .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)
}
pub fn check(doc: &Document, ignored: &[String]) -> Result<Vec<Issue>, String> {
    check_document(doc, ignored, true)
}
/// Background markers avoid computing suggestions for every misspelling.
pub fn markers(doc: &Document, ignored: &[String]) -> Result<Vec<Issue>, String> {
    check_document(doc, ignored, false)
}
pub fn suggestions(word: &str) -> Result<Vec<String>, String> {
    let mut list = vec![];
    dictionary()?.suggest(&word.replace('’', "'"), &mut list);
    list.truncate(6);
    Ok(list)
}
fn check_document(
    doc: &Document,
    ignored: &[String],
    with_suggestions: bool,
) -> Result<Vec<Issue>, String> {
    let dictionary = dictionary()?;
    let ignored: HashSet<_> = ignored.iter().map(|w| w.to_lowercase()).collect();
    let mut issues = vec![];
    let mut cache: HashMap<String, Vec<String>> = HashMap::new();
    for address in doc.addresses() {
        let Some(p) = doc.paragraph(address) else {
            continue;
        };
        let text = p.text();
        let network = network_ranges(&text);
        let mut scanned_byte = 0;
        let mut scalar_offset = 0;
        let words: Vec<_> = text
            .unicode_word_indices()
            .filter(|(_, w)| w.chars().any(char::is_alphabetic))
            .map(|(byte, word)| {
                scalar_offset += text[scanned_byte..byte].chars().count();
                let start = scalar_offset;
                scalar_offset += word.chars().count();
                scanned_byte = byte + word.len();
                (byte, word, start)
            })
            .collect();
        for (index, &(byte, word, start)) in words.iter().enumerate() {
            if network.iter().any(|r| r.contains(&byte))
                || p.chars
                    .iter()
                    .skip(start)
                    .take(word.chars().count())
                    .any(|c| c.style.link.is_some())
            {
                continue;
            }
            let end = start + word.chars().count();
            let normal = word.replace('’', "'");
            let prefix = words.get(index + 1).is_some_and(|&(next, _, _)| {
                matches!(
                    normal.to_ascii_lowercase().as_str(),
                    "pre"
                        | "post"
                        | "non"
                        | "re"
                        | "co"
                        | "mid"
                        | "sub"
                        | "super"
                        | "anti"
                        | "pro"
                        | "inter"
                        | "intra"
                        | "multi"
                        | "over"
                        | "under"
                ) && matches!(
                    &text[byte + word.len()..next],
                    "-" | "\u{2010}" | "\u{2011}"
                )
            });
            if !ignored.contains(&normal.to_lowercase())
                && !prefix
                && !normal.chars().any(char::is_numeric)
                && !dictionary.check(&normal)
            {
                let suggestions = cache.entry(normal.clone()).or_insert_with(|| {
                    let mut list = vec![];
                    if with_suggestions {
                        dictionary.suggest(&normal, &mut list);
                    }
                    list.truncate(6);
                    list
                });
                issues.push(Issue {
                    address,
                    start,
                    end,
                    original: word.into(),
                    message: "Possible spelling error",
                    suggestions: suggestions.clone(),
                    grammar: false,
                });
            }
            let previous = index.checked_sub(1).map(|i| words[i]);
            if let Some((prev_byte, prev, prev_start)) = previous {
                let gap = &text[prev_byte + prev.len()..byte];
                if !gap.is_empty()
                    && gap.chars().all(char::is_whitespace)
                    && prev.eq_ignore_ascii_case(word)
                {
                    let start = prev_start;
                    issues.push(Issue {
                        address,
                        start,
                        end,
                        original: text[prev_byte..byte + word.len()].into(),
                        message: "Repeated word",
                        suggestions: vec![prev.into()],
                        grammar: true,
                    });
                }
                if gap.chars().all(char::is_whitespace) {
                    let replacement =
                        match (prev.to_lowercase().as_str(), word.to_lowercase().as_str()) {
                            ("he" | "she" | "it", "are") => Some("is"),
                            ("i", "is" | "are") => Some("am"),
                            ("we" | "they" | "you", "is") => Some("are"),
                            ("he" | "she" | "it", "have") => Some("has"),
                            _ => None,
                        };
                    if let Some(replacement) = replacement {
                        issues.push(Issue {
                            address,
                            start,
                            end,
                            original: word.into(),
                            message: "Check subject–verb agreement",
                            suggestions: vec![replacement.into()],
                            grammar: true,
                        });
                    }
                    let article = prev.to_lowercase();
                    let noun = word.to_lowercase();
                    let vowel = noun.starts_with(['a', 'e', 'i', 'o', 'u']);
                    let exception = noun.starts_with("uni")
                        || noun.starts_with("use")
                        || noun.starts_with("one")
                        || noun.starts_with("euro");
                    let silent_h = ["hour", "honest", "honour", "heir"]
                        .iter()
                        .any(|s| noun.starts_with(s));
                    if (article == "a" && ((vowel && !exception) || silent_h))
                        || (article == "an" && ((!vowel && !silent_h) || exception))
                    {
                        let replacement = if article == "a" { "an" } else { "a" };
                        let replacement = if prev.starts_with(char::is_uppercase) {
                            format!("{}{}", replacement[..1].to_uppercase(), &replacement[1..])
                        } else {
                            replacement.into()
                        };
                        let start = prev_start;
                        issues.push(Issue {
                            address,
                            start,
                            end: start + prev.chars().count(),
                            original: prev.into(),
                            message: "Check the indefinite article",
                            suggestions: vec![replacement],
                            grammar: true,
                        });
                    }
                }
            }
            if word == "i" {
                issues.push(Issue {
                    address,
                    start,
                    end,
                    original: word.into(),
                    message: "Capitalise the pronoun I",
                    suggestions: vec!["I".into()],
                    grammar: true,
                });
            }
        }
    }
    issues.sort_by_key(|i| (i.address, i.start, !i.grammar));
    Ok(issues)
}
/// Byte spans are used only for lexical exclusion; issue positions remain scalars.
fn network_ranges(text: &str) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut offset = 0;
    for part in text.split_inclusive(char::is_whitespace) {
        let token = part.trim().trim_matches(|c: char| {
            matches!(
                c,
                '(' | ')' | '[' | ']' | '<' | '>' | '"' | '\'' | ',' | ';' | '!' | '?' | '.'
            )
        });
        let host = token.split(['/', '?', '#']).next().unwrap_or("");
        let domain = host
            .rsplit('@')
            .next()
            .unwrap_or(host)
            .split(':')
            .next()
            .unwrap_or("");
        let labels: Vec<_> = domain.split('.').collect();
        let valid_domain = labels.len() >= 2
            && labels.iter().all(|s| {
                !s.is_empty()
                    && s.len() <= 63
                    && !s.starts_with('-')
                    && !s.ends_with('-')
                    && s.chars().all(|c| c.is_alphanumeric() || c == '-')
            })
            && labels
                .last()
                .is_some_and(|s| s.len() >= 2 && s.chars().all(char::is_alphabetic));
        if token.starts_with("https://") || token.starts_with("http://") || valid_domain {
            ranges.push(offset..offset + part.len());
        }
        offset += part.len();
    }
    ranges
}
pub fn replace(editor: &mut Editor, issue: &Issue, replacement: &str) -> bool {
    let Some(p) = editor.doc.paragraph(issue.address) else {
        return false;
    };
    let actual: String = p
        .chars
        .iter()
        .skip(issue.start)
        .take(issue.end.saturating_sub(issue.start))
        .map(|c| c.ch)
        .collect();
    if actual != issue.original
        || p.snap(issue.start) != issue.start
        || p.snap(issue.end) != issue.end
    {
        return false;
    }
    editor.anchor = Position {
        address: issue.address,
        offset: issue.start,
    };
    editor.caret = Position {
        address: issue.address,
        offset: issue.end,
    };
    editor.sync_style();
    editor.paste(replacement, true);
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn domains_links_and_hyphenated_prefixes_are_not_spelling_errors() {
        let mut doc = Document::new("Proofing exclusions");
        doc.blocks = vec![Block::Paragraph(Paragraph::from_text(
            "😀 northernbeachespokemon.com.au https://example.org/mispeling person@example.com Pre-Launch pre-launch Post‑Launch Pre-mispeling teh",
        ))];
        let issues = markers(&doc, &[]).unwrap();
        assert_eq!(
            issues
                .iter()
                .map(|i| i.original.as_str())
                .collect::<Vec<_>>(),
            ["mispeling", "teh"]
        );
        for issue in &issues {
            assert_eq!(
                doc.paragraph(issue.address)
                    .unwrap()
                    .text()
                    .chars()
                    .skip(issue.start)
                    .take(issue.end - issue.start)
                    .collect::<String>(),
                issue.original
            );
        }
        let mut linked = Paragraph::from_text("mispeling");
        for c in &mut linked.chars {
            c.style.link = Some("https://example.org".into());
        }
        doc.blocks = vec![Block::Paragraph(linked)];
        assert!(markers(&doc, &[]).unwrap().is_empty());
    }
    #[test]
    fn offline_spelling_grammar_unicode_table_replacement_and_undo() {
        let mut doc = Document::new("Proofing");
        doc.blocks = vec![
            Block::Paragraph(Paragraph::from_text(
                "😀 This is teh test. She are ready. A apple. the the i",
            )),
            Block::Table(vec![vec![Paragraph::from_text("mispeling")]]),
        ];
        let issues = check(&doc, &[]).unwrap();
        let typo = issues.iter().find(|i| i.original == "teh").unwrap();
        assert!(typo.suggestions.iter().any(|s| s == "the"));
        assert_eq!(typo.start, 10);
        assert!(issues.iter().any(|i| i.message == "Repeated word"));
        assert!(
            issues
                .iter()
                .any(|i| i.original == "are" && i.suggestions == ["is"])
        );
        assert!(issues.iter().any(|i| i.address.block == 1));
        let mut editor = Editor::new(doc.clone());
        assert!(replace(&mut editor, typo, "the"));
        assert!(!replace(&mut editor, typo, "the"));
        editor.undo();
        assert_eq!(editor.doc, doc);
        assert!(
            !check(&doc, &["teh".into()])
                .unwrap()
                .iter()
                .any(|i| i.original == "teh")
        );
    }
}
