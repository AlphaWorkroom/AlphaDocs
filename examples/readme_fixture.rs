//! Original demonstration text, isolated from the user's library.
use alphadocs::{engine::Editor, model::*, persistence};
fn main() -> Result<(), String> {
    if std::env::var_os("ALPHADOCS_DATA_DIR").is_none() {
        return Err("Set an isolated demonstration profile".into());
    }
    for (title, text) in [
        (
            "Your next chapter",
            "A quiet place to write.\nBring your ideas together with familiar tools, clear pages and documents that stay on your computer.",
        ),
        ("Project notes", "Decisions, ideas and the next small step."),
    ] {
        let mut doc = Document::new(title);
        let mut heading = Paragraph::from_text(title);
        heading.set_heading(1);
        let mut intro = Paragraph::from_text(text);
        intro.after = 12.0;
        doc.blocks = vec![
            Block::Paragraph(heading),
            Block::Paragraph(intro),
            Block::Image(EmbeddedImage {
                name: "Blue book".into(),
                data: include_bytes!("../assets/alphadocs-icon.png")
                    .to_vec()
                    .into(),
                width: 180.0,
                height: 180.0,
                ..Default::default()
            }),
            Block::Paragraph(Paragraph::from_text(
                "Write freely. Save locally. Keep the original.",
            )),
        ];
        let mut editor = Editor::new(doc);
        editor.add_tab();
        editor.insert("A separate space for references, notes and supporting ideas.");
        editor.switch_tab(0);
        let path = persistence::library_path(&editor.doc);
        persistence::save_document(&editor.doc, &path)?;
        println!("{}", path.display());
    }
    persistence::save_preferences(&persistence::Preferences {
        theme: persistence::Theme::Light,
        outline: true,
        autosave: false,
        check_updates: false,
        ..Default::default()
    })?;
    Ok(())
}
