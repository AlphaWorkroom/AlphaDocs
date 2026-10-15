//! Compact styled runs on disk, scalar-addressed copy-on-write text in memory.
//! Older native files stored one full style per character; both encodings load.
use super::{SharedVec, StyledChar, TextStyle};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Serialize, Deserialize)]
struct Run {
    text: String,
    style: TextStyle,
}
#[derive(Serialize, Deserialize)]
struct Runs {
    runs: Vec<Run>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum StoredText {
    Compact(Runs),
    Legacy(Vec<StyledChar>),
}

pub fn serialize<S: Serializer>(
    chars: &SharedVec<StyledChar>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut runs: Vec<Run> = vec![];
    for ch in chars {
        if let Some(last) = runs.last_mut()
            && last.style == ch.style
        {
            last.text.push(ch.ch);
        } else {
            runs.push(Run {
                text: ch.ch.to_string(),
                style: ch.style.clone(),
            });
        }
    }
    Runs { runs }.serialize(serializer)
}
pub fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<SharedVec<StyledChar>, D::Error> {
    Ok(match StoredText::deserialize(deserializer)? {
        StoredText::Legacy(chars) => chars.into(),
        StoredText::Compact(data) => data
            .runs
            .into_iter()
            .flat_map(|run| {
                run.text
                    .chars()
                    .map(|ch| StyledChar {
                        ch,
                        style: run.style.clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect(),
    })
}
#[cfg(test)]
mod tests {
    use crate::model::{Baseline, Paragraph};
    #[test]
    fn compact_runs_preserve_unicode_and_mixed_styles() {
        let mut p = Paragraph::from_text("Café e\u{301} 👩‍🔬 H2O");
        p.chars.last_mut().unwrap().style.baseline = Baseline::Subscript;
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"runs\""));
        assert_eq!(json.matches("\"text\"").count(), 2);
        assert_eq!(serde_json::from_str::<Paragraph>(&json).unwrap(), p);
    }
}
