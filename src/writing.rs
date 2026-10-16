//! Small, explicit writing adapters. Online functions are called only by user actions.
use crate::engine::Editor;
use crate::model::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use unicode_segmentation::UnicodeSegmentation;

const MAX_HTTP_RESPONSE_BYTES: usize = 1_048_576;
const MAX_TRANSLATED_PARAGRAPH_CHARS: usize = 20_000;
const MAX_TRANSLATED_DOCUMENT_CHARS: usize = 100_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct TypingPreferences {
    pub capitalize: bool,
    pub smart_quotes: bool,
    pub detect_links: bool,
    pub detect_lists: bool,
    pub substitutions: bool,
    pub replacements: Vec<Substitution>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Substitution {
    pub enabled: bool,
    pub from: String,
    pub to: String,
}
impl Default for TypingPreferences {
    fn default() -> Self {
        Self {
            capitalize: false,
            smart_quotes: false,
            detect_links: false,
            detect_lists: false,
            substitutions: false,
            replacements: [
                ("(c)", "©"),
                ("(r)", "®"),
                ("tm", "™"),
                ("...", "…"),
                ("1/2", "½"),
                ("1/3", "⅓"),
                ("1/4", "¼"),
                ("->", "→"),
                ("<-", "←"),
            ]
            .into_iter()
            .map(|(a, b)| Substitution {
                enabled: true,
                from: a.into(),
                to: b.into(),
            })
            .collect(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Citation {
    pub author: String,
    pub title: String,
    pub year: String,
    pub publisher: String,
    pub url: String,
}
#[derive(Clone, Copy, Default, PartialEq)]
pub enum CitationStyle {
    #[default]
    Apa,
    Mla,
    Chicago,
}
impl Citation {
    pub fn valid(&self) -> bool {
        !self.title.trim().is_empty()
            && [
                &self.author,
                &self.title,
                &self.year,
                &self.publisher,
                &self.url,
            ]
            .iter()
            .all(|s| s.len() <= 4096 && !s.contains('\0'))
    }
    fn name(&self) -> &str {
        if self.author.trim().is_empty() {
            self.title.trim()
        } else {
            self.author.trim()
        }
    }
    pub fn inline(&self, style: CitationStyle, page: &str) -> String {
        let page = page.trim();
        match style {
            CitationStyle::Mla => format!(
                "({}{})",
                self.name(),
                if page.is_empty() {
                    String::new()
                } else {
                    format!(" {page}")
                }
            ),
            CitationStyle::Apa => format!(
                "({}, {}{})",
                self.name(),
                if self.year.is_empty() {
                    "n.d."
                } else {
                    &self.year
                },
                if page.is_empty() {
                    String::new()
                } else {
                    format!(", p. {page}")
                }
            ),
            CitationStyle::Chicago => format!(
                "({} {}{})",
                self.name(),
                if self.year.is_empty() {
                    "n.d."
                } else {
                    &self.year
                },
                if page.is_empty() {
                    String::new()
                } else {
                    format!(", {page}")
                }
            ),
        }
    }
    pub fn bibliography(&self, style: CitationStyle) -> String {
        let year = if self.year.trim().is_empty() {
            "n.d."
        } else {
            self.year.trim()
        };
        let title = self.title.trim();
        let publisher = self.publisher.trim();
        let author = self.author.trim();
        let main = match style {
            CitationStyle::Apa if author.is_empty() => format!("{title}. ({year})."),
            CitationStyle::Apa => format!("{author}. ({year}). {title}."),
            CitationStyle::Mla if author.is_empty() && publisher.is_empty() => {
                format!("{title}, {year}.")
            }
            CitationStyle::Mla if publisher.is_empty() => {
                format!("{author}. {title}, {year}.")
            }
            CitationStyle::Mla if author.is_empty() => format!("{title}."),
            CitationStyle::Mla => format!("{author}. {title}."),
            CitationStyle::Chicago if author.is_empty() => format!("{title}. {year}."),
            CitationStyle::Chicago => format!("{author}. {year}. {title}."),
        };
        let publisher = if publisher.is_empty() {
            String::new()
        } else if style == CitationStyle::Mla {
            format!(" {publisher}, {year}.")
        } else {
            format!(" {publisher}.")
        };
        format!(
            "{main}{publisher}{}",
            if self.url.trim().is_empty() {
                String::new()
            } else {
                format!(" {}", self.url.trim())
            }
        )
    }
}

/// Text-only report; never represents these paragraphs as tracked revisions.
pub fn compare(before: &Document, after: &Document, author: &str) -> Result<Document, String> {
    let before = before.flattened();
    let after = after.flattened();
    let old = before.text();
    let new = after.text();
    let a: Vec<_> = old.lines().collect();
    let b: Vec<_> = new.lines().collect();
    if (a.len() + 1).saturating_mul(b.len() + 1) > 4_000_000 || old.len() + new.len() > 8_000_000 {
        return Err("This comparison is too large. Compare smaller content tabs or excerpts (up to four million line pairs).".into());
    }
    let n = b.len() + 1;
    let mut lcs = vec![0u32; (a.len() + 1) * n];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i * n + j] = if a[i] == b[j] {
                1 + lcs[(i + 1) * n + j + 1]
            } else {
                lcs[(i + 1) * n + j].max(lcs[i * n + j + 1])
            };
        }
    }
    let mut doc = Document::new(&format!("Comparison — {}", after.title));
    let mut heading = Paragraph::from_text("Document comparison");
    heading.set_heading(1);
    doc.blocks = vec![
        Block::Paragraph(heading),
        Block::Paragraph(Paragraph::from_text(&format!(
            "Original: {}\nCompared with: {}\nAttributed to: {}\nText comparison of all tabs, including table text. Layout, objects, comments and tracked changes are not compared. Red − is removed; green + is added.",
            before.title, after.title, author
        ))),
    ];
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        let (prefix, text, color, strike) = if i < a.len() && j < b.len() && a[i] == b[j] {
            let t = a[i];
            i += 1;
            j += 1;
            ("  ", t, [31, 31, 31], false)
        } else if j < b.len() && (i == a.len() || lcs[i * n + j + 1] >= lcs[(i + 1) * n + j]) {
            let t = b[j];
            j += 1;
            ("+ ", t, [20, 110, 60], false)
        } else {
            let t = a[i];
            i += 1;
            ("− ", t, [175, 30, 40], true)
        };
        let mut p = Paragraph::from_text(&format!("{prefix}{text}"));
        p.style.color = color;
        p.style.strike = strike;
        for c in &mut p.chars {
            c.style = p.style.clone();
        }
        doc.blocks.push(Block::Paragraph(p));
    }
    doc.validate()?;
    Ok(doc)
}
pub fn percent(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn get_json(url: &str) -> Result<serde_json::Value, String> {
    let bytes = get_bytes(url)?;
    serde_json::from_slice(&bytes).map_err(|_| "The service returned an invalid response".into())
}

/// Parse one MyMemory response without contacting the service. The service returns
/// HTML entities inside its JSON string even though AlphaDocs stores plain text.
fn parse_translation_response(bytes: &[u8]) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| "The translation service returned an invalid response".to_owned())?;
    if value["responseStatus"].as_u64() != Some(200)
        || value["quotaFinished"].as_bool() == Some(true)
    {
        let detail = value["responseDetails"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("service quota or language unavailable");
        return Err(format!("Translation stopped: {detail}"));
    }
    let translated = value["responseData"]["translatedText"]
        .as_str()
        .ok_or_else(|| "The translation service returned an invalid response".to_owned())?;
    let decoded = decode_html_entities(translated);
    if decoded.chars().count() > MAX_TRANSLATED_PARAGRAPH_CHARS {
        return Err("Translation response too large".into());
    }
    Ok(decoded)
}

fn decode_html_entities(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find('&') {
        output.push_str(&rest[..start]);
        let after_amp = &rest[start + 1..];
        let Some(end) = after_amp.find(';').filter(|end| *end <= 32) else {
            output.push('&');
            rest = after_amp;
            continue;
        };
        let entity = &after_amp[..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            value if value.starts_with("#x") || value.starts_with("#X") => {
                u32::from_str_radix(&value[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            value if value.starts_with('#') => {
                value[1..].parse::<u32>().ok().and_then(char::from_u32)
            }
            _ => None,
        };
        if let Some(ch) = decoded {
            output.push(ch);
            rest = &after_amp[end + 1..];
        } else {
            output.push('&');
            rest = after_amp;
        }
    }
    output.push_str(rest);
    output
}
pub fn dictionary(word: &str) -> Result<String, String> {
    if word.is_empty()
        || word.chars().count() > 80
        || word
            .chars()
            .any(|c| !c.is_alphabetic() && c != '-' && c != '\'')
    {
        return Err("Enter one English word (up to 80 characters).".into());
    }
    let value = get_json(&format!(
        "https://api.dictionaryapi.dev/api/v2/entries/en/{}",
        percent(word)
    ))?;
    let entries = value.as_array().ok_or("No definition found")?;
    let mut out = String::new();
    for entry in entries.iter().take(4) {
        out.push_str(&format!(
            "{}  {}\n\n",
            entry["word"].as_str().unwrap_or(word),
            entry["phonetic"].as_str().unwrap_or("")
        ));
        if let Some(meanings) = entry["meanings"].as_array() {
            for m in meanings.iter().take(8) {
                out.push_str(&format!("{}\n", m["partOfSpeech"].as_str().unwrap_or("")));
                if let Some(defs) = m["definitions"].as_array() {
                    for (i, d) in defs.iter().take(8).enumerate() {
                        out.push_str(&format!(
                            "{}. {}\n",
                            i + 1,
                            d["definition"].as_str().unwrap_or("")
                        ));
                        if let Some(example) = d["example"].as_str() {
                            out.push_str(&format!("“{example}”\n"));
                        }
                    }
                }
                out.push('\n');
            }
        }
        if let Some(urls) = entry["sourceUrls"].as_array() {
            for url in urls.iter().filter_map(|v| v.as_str()).take(4) {
                out.push_str(&format!("Source: {url}\n"));
            }
        }
        if let Some(name) = entry["license"]["name"].as_str() {
            out.push_str(&format!("Licence: {name}\n"));
        }
    }
    Ok(out)
}
pub const LANGUAGES: [(&str, &str); 14] = [
    ("en", "English"),
    ("fr", "French"),
    ("de", "German"),
    ("es", "Spanish"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("nl", "Dutch"),
    ("pl", "Polish"),
    ("uk", "Ukrainian"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("zh-CN", "Chinese (simplified)"),
    ("ar", "Arabic"),
    ("hi", "Hindi"),
];
/// Byte-bounded chunks never split a grapheme (MyMemory GET limit: 500 UTF-8 bytes).
pub fn chunks(text: &str) -> Result<Vec<&str>, String> {
    let mut result = vec![];
    let mut start = 0;
    let mut end = 0;
    for (offset, g) in text.grapheme_indices(true) {
        if g.len() > 500 {
            return Err("A text cluster exceeds the translation service's 500-byte limit".into());
        }
        if offset + g.len() - start > 500 {
            result.push(&text[start..end]);
            start = offset;
        }
        end = offset + g.len();
    }
    if end > start {
        result.push(&text[start..end]);
    }
    Ok(result)
}
pub fn translate(
    doc: Document,
    title: &str,
    source: &str,
    target: &str,
) -> Result<Document, String> {
    translate_with(doc, title, source, target, |chunk, source, target| {
        let bytes = get_bytes(&format!(
            "https://api.mymemory.translated.net/get?q={}&langpair={}%7C{}",
            percent(chunk),
            percent(source),
            percent(target)
        ))?;
        parse_translation_response(&bytes)
    })
}

fn get_bytes(url: &str) -> Result<Vec<u8>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .into();
    agent
        .get(url)
        .header(
            "User-Agent",
            concat!("AlphaDocs/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|e| format!("Online service unavailable: {e}"))?
        .body_mut()
        .with_config()
        .limit(MAX_HTTP_RESPONSE_BYTES as u64)
        .read_to_vec()
        .map_err(|e| format!("Could not read the service response: {e}"))
}

fn translate_with(
    doc: Document,
    title: &str,
    source: &str,
    target: &str,
    mut translate_chunk: impl FnMut(&str, &str, &str) -> Result<String, String>,
) -> Result<Document, String> {
    let total: usize = (0..doc.tabs.len())
        .map(|i| {
            doc.tab_blocks(i)
                .iter()
                .map(|b| match b {
                    Block::Paragraph(p) => p.chars.len(),
                    Block::Table(rows) => rows.iter().flatten().map(|p| p.chars.len()).sum(),
                    _ => 0,
                })
                .sum::<usize>()
        })
        .sum::<usize>()
        + doc.page.header.chars.len()
        + doc.page.footer.chars.len();
    if total > 5000 {
        return Err("MyMemory's anonymous quota is 5,000 characters per day. This document exceeds that limit; translate a shorter copy.".into());
    }
    if source == target {
        return Err("Choose different source and target languages".into());
    }
    let mut texts = Vec::<String>::new();
    let mut seen_texts = std::collections::HashSet::<String>::new();
    for tab in 0..doc.tabs.len() {
        for block in doc.tab_blocks(tab) {
            match block {
                Block::Paragraph(p) => {
                    if !p.text().trim().is_empty() {
                        let text = p.text();
                        if seen_texts.insert(text.clone()) {
                            texts.push(text);
                        }
                    }
                }
                Block::Table(rows) => {
                    for p in rows.iter().flatten() {
                        if !p.text().trim().is_empty() {
                            let text = p.text();
                            if seen_texts.insert(text.clone()) {
                                texts.push(text);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    for p in [&doc.page.header, &doc.page.footer] {
        if !p.text().trim().is_empty() {
            let text = p.text();
            if seen_texts.insert(text.clone()) {
                texts.push(text);
            }
        }
    }

    let mut replacements = std::collections::HashMap::<String, String>::new();
    let mut translated_total = 0usize;
    for text in texts {
        let mut translated = String::new();
        for chunk in chunks(&text)? {
            translated.push_str(&translate_chunk(chunk, source, target)?);
            if translated.chars().count() > MAX_TRANSLATED_PARAGRAPH_CHARS {
                return Err("Translation response too large".into());
            }
        }
        translated_total = translated_total.saturating_add(translated.chars().count());
        if translated_total > MAX_TRANSLATED_DOCUMENT_CHARS {
            return Err("The translated copy is too large to open safely.".into());
        }
        replacements.insert(text, translated);
    }

    let mut editor = Editor::new(doc);
    editor.transaction(|e| {
        for tab in 0..e.doc.tabs.len() {
            let blocks = if tab == e.doc.active_tab {
                &mut e.doc.blocks
            } else {
                &mut e.doc.tabs[tab].blocks
            };
            for block in blocks {
                match block {
                    Block::Paragraph(p) => apply_translation(p, &replacements),
                    Block::Table(rows) => {
                        for p in rows.iter_mut().flatten() {
                            apply_translation(p, &replacements);
                        }
                    }
                    _ => {}
                }
            }
        }
        apply_translation(&mut e.doc.page.header, &replacements);
        apply_translation(&mut e.doc.page.footer, &replacements);
        e.doc.id = Document::new("").id;
        e.doc.title = title.to_owned();
        e.doc.starred = false;
    });
    editor.doc.validate()?;
    Ok(editor.doc)
}

fn apply_translation(
    paragraph: &mut Paragraph,
    replacements: &std::collections::HashMap<String, String>,
) {
    let text = paragraph.text();
    let Some(translated) = replacements.get(&text) else {
        return;
    };
    // New word boundaries invalidate inline links and text-range anchors in the copy.
    let mut style = paragraph.style.clone();
    style.link = None;
    paragraph.replace_text(
        0..paragraph.chars.len(),
        translated
            .chars()
            .map(|ch| StyledChar {
                ch,
                style: style.clone(),
            })
            .collect(),
    );
    paragraph.annotations.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_response_decodes_text_and_rejects_quota_or_malformed_data() {
        let response = br#"{"responseStatus":200,"responseData":{"translatedText":"Tom &amp; Ana &#x1F600; &unknown;"}}"#;
        assert_eq!(
            parse_translation_response(response).unwrap(),
            "Tom & Ana 😀 &unknown;"
        );
        let quota = br#"{"responseStatus":429,"quotaFinished":true,"responseDetails":"Daily limit reached"}"#;
        assert!(
            parse_translation_response(quota)
                .unwrap_err()
                .contains("Daily limit")
        );
        assert!(parse_translation_response(b"not json").is_err());
    }

    #[test]
    fn bibliography_uses_title_once_when_author_is_missing() {
        let citation = Citation {
            title: "A useful source".into(),
            year: " 2024 ".into(),
            publisher: "Example Press".into(),
            ..Default::default()
        };
        assert_eq!(
            citation.bibliography(CitationStyle::Apa),
            "A useful source. (2024). Example Press."
        );
        assert_eq!(
            citation.bibliography(CitationStyle::Mla),
            "A useful source. Example Press, 2024."
        );
    }

    #[test]
    fn translation_chunks_are_byte_bounded_and_keep_graphemes_whole() {
        let family = "👩‍👩‍👧‍👦";
        let text = family.repeat(80);
        let pieces = chunks(&text).unwrap();
        assert!(pieces.iter().all(|piece| piece.len() <= 500));
        assert_eq!(pieces.concat(), text);
        assert!(
            pieces
                .iter()
                .all(|piece| piece.graphemes(true).all(|g| g == family))
        );

        let oversized_cluster = format!("a{}", "\u{301}".repeat(300));
        assert!(chunks(&oversized_cluster).is_err());
    }

    #[test]
    fn comparison_rejects_a_diff_matrix_over_its_cell_limit() {
        let mut before = Document::new("Before");
        let mut after = Document::new("After");
        before.blocks = (0..2_000)
            .map(|_| Block::Paragraph(Paragraph::from_text("line")))
            .collect();
        after.blocks = before.blocks.clone();
        assert!(compare(&before, &after, "Reviewer").is_err());
    }

    #[test]
    fn translation_is_an_offline_separate_copy_and_preserves_tabs_and_citations() {
        let mut source = Document::new("Original");
        source.blocks = vec![Block::Paragraph(Paragraph::from_text("Hello 👩‍👩‍👧‍👦"))];
        source.tabs.push(ContentTab {
            id: "tab-2".into(),
            title: "Second tab".into(),
            blocks: vec![Block::Paragraph(Paragraph::from_text("Second"))].into(),
        });
        source.page.header = Paragraph::from_text("Header");
        source.citations.push(Citation {
            author: "A. Author".into(),
            title: "A source".into(),
            year: "2024".into(),
            ..Default::default()
        });
        let source_id = source.id.clone();
        let original_text = source.text();
        let copy = translate_with(source.clone(), "Traduit", "en", "fr", |chunk, _, _| {
            Ok(format!("[{}]", chunk))
        })
        .unwrap();

        assert_ne!(copy.id, source_id);
        assert_eq!(copy.title, "Traduit");
        assert_eq!(source.text(), original_text);
        let Block::Paragraph(translated) = &copy.blocks[0] else {
            panic!("paragraph content was not preserved");
        };
        assert_eq!(translated.text(), "[Hello 👩‍👩‍👧‍👦]");
        let Block::Paragraph(second_tab) = &copy.tabs[1].blocks[0] else {
            panic!("second tab paragraph was not preserved");
        };
        assert_eq!(second_tab.text(), "[Second]");
        assert_eq!(copy.page.header.text(), "[Header]");
        assert_eq!(copy.citations, source.citations);
    }

    #[test]
    fn translation_stops_when_service_failure_occurs_without_returning_partial_copy() {
        let mut doc = Document::new("Original");
        doc.blocks = vec![
            Block::Paragraph(Paragraph::from_text("one")),
            Block::Paragraph(Paragraph::from_text("two")),
        ];
        let mut calls = 0;
        let result = translate_with(doc, "Copy", "en", "fr", |chunk, _, _| {
            calls += 1;
            if chunk == "two" {
                Err("Translation stopped: quota".into())
            } else {
                Ok("un".into())
            }
        });
        assert_eq!(calls, 2);
        assert!(result.unwrap_err().contains("quota"));
    }

    #[test]
    fn translation_quota_preflight_makes_no_service_calls() {
        let mut doc = Document::new("Original");
        doc.blocks = vec![Block::Paragraph(Paragraph::from_text(&"x".repeat(5_001)))];
        let mut calls = 0;
        let result = translate_with(doc, "Copy", "en", "fr", |_, _, _| {
            calls += 1;
            Ok("translated".into())
        });
        assert!(result.unwrap_err().contains("5,000 characters"));
        assert_eq!(calls, 0);
    }
}
