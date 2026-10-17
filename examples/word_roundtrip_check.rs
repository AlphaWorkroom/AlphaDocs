//! Verify an externally edited synthetic Word fixture without changing its source.
use alphadocs::{compat, model::*};
use std::path::PathBuf;
fn main() -> Result<(), String> {
    let source = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Pass the edited QA DOCX")?,
    );
    let output = PathBuf::from(
        std::env::args_os()
            .nth(2)
            .ok_or("Pass a new roundtrip destination")?,
    );
    if output.exists() || source == output {
        return Err("Output must be a new path".into());
    }
    let (doc, warnings) = compat::docx::import(&source)?;
    for text in [
        "Verified",
        "The Last Light",
        "Mara reached the lighthouse",
        "20.00",
    ] {
        if !doc.text().contains(text) {
            return Err(format!("Missing Word-edited text: {text}"));
        }
    }
    let mut kinds = Vec::new();
    let mut equations = 0;
    let mut formulas = 0;
    for block in &doc.blocks {
        match block {
            Block::Image(image) => match &image
                .editable
                .as_ref()
                .ok_or("Object lost editable source")?
                .source
            {
                ObjectSource::Equation { latex, size, .. } => {
                    if latex.is_empty() || (*size - 18.0).abs() > 0.1 {
                        return Err("Equation source or size changed".into());
                    }
                    equations += 1;
                }
                ObjectSource::Drawing { kind, text, .. } => {
                    if !matches!(kind, ShapeKind::Line | ShapeKind::Arrow)
                        && !text.text().contains("Keep the lamp burning.")
                    {
                        return Err("Drawing text was lost".into());
                    }
                    kinds.push(*kind);
                }
            },
            Block::Table(rows) => {
                if rows[0][0].box_style.and_then(|s| s.fill) != Some([211, 227, 253]) {
                    return Err("Cell fill changed".into());
                }
                formulas += rows
                    .iter()
                    .flatten()
                    .filter(|p| p.formula.is_some())
                    .count();
            }
            _ => {}
        }
    }
    if equations != 1 || formulas != 1 || !ShapeKind::ALL.iter().all(|k| kinds.contains(k)) {
        return Err("Word roundtrip lost editable objects or fields".into());
    }
    compat::docx::export(&doc, &output)?;
    let (reopened, _) = compat::docx::import(&output)?;
    if doc.text() != reopened.text() {
        return Err("Second DOCX roundtrip changed body text".into());
    }
    println!(
        "Word edit/save/reimport/export verified: one equation, five editable shapes, formula, cell fill and body text. Import notices: {warnings:?}"
    );
    Ok(())
}
