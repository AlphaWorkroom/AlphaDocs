//! Original, formatted demonstration documents. Never uses the real library.
use alphadocs::{model::*, persistence};

const INK: [u8; 3] = [33, 48, 65];
const BLUE: [u8; 3] = [35, 101, 162];
fn paragraph(text: &str, size: f32, serif: bool, bold: bool, color: [u8; 3]) -> Paragraph {
    let style = TextStyle {
        font: if serif { "Noto Serif" } else { "Noto Sans" }.into(),
        size,
        bold,
        color,
        ..Default::default()
    };
    Paragraph {
        chars: text
            .chars()
            .map(|ch| StyledChar {
                ch,
                style: style.clone(),
            })
            .collect(),
        style,
        line_spacing: 1.35,
        after: 10.0,
        ..Default::default()
    }
}
fn text(text: &str) -> Block {
    Block::Paragraph(paragraph(text, 11.0, false, false, INK))
}
fn label(text: &str) -> Block {
    Block::Paragraph(paragraph(text, 9.0, false, true, BLUE))
}
fn heading(text: &str, size: f32, serif: bool) -> Block {
    let mut p = paragraph(text, size, serif, true, INK);
    p.heading = 1;
    p.keep_next = true;
    p.after = 18.0;
    Block::Paragraph(p)
}
fn prose(text: &str) -> Block {
    let mut p = paragraph(text, 11.5, true, false, INK);
    p.line_spacing = 1.4;
    Block::Paragraph(p)
}
fn save(mut doc: Document, sections: Vec<(&str, Vec<Block>)>) -> Result<(), String> {
    doc.tabs = sections
        .iter()
        .enumerate()
        .map(|(n, (title, blocks))| ContentTab {
            id: format!("section-{n}"),
            title: (*title).into(),
            blocks: if n == 0 {
                Default::default()
            } else {
                blocks.clone().into()
            },
        })
        .collect();
    doc.blocks = sections[0].1.clone();
    doc.validate()?;
    let path = persistence::library_path(&doc);
    persistence::save_document(&doc, &path)?;
    println!("{}: {}", doc.title, path.display());
    Ok(())
}
fn main() -> Result<(), String> {
    if std::env::var_os("ALPHADOCS_DATA_DIR").is_none() {
        return Err("Set an isolated demonstration profile".into());
    }
    let mut book = Document::new("The Lighthouse Atlas");
    book.page.page_numbers = true;
    let chapters = vec![
        (
            "01 · The blue hour",
            vec![
                label("THE LIGHTHOUSE ATLAS  /  CHAPTER ONE"),
                heading("The blue hour", 28.0, true),
                prose(
                    "By the time the ferry reached the island, the sea had turned the colour of ink. Mara stood at the rail with her grandfather’s atlas pressed beneath her coat. Every harbour in the book had a name. This one had only a small blue circle, drawn in a hand she knew better than her own.",
                ),
                prose(
                    "On the quay, a bicycle leaned against a stack of empty lobster pots. Beyond it, white houses climbed the hill towards a lighthouse that had not shone for twenty years. A woman in a yellow raincoat raised one hand, as if she had been expecting this particular ferry all her life.",
                ),
                prose(
                    "“You brought the book,” she said. It was a statement, not a question. Mara looked down at the atlas. A narrow ribbon of blue light had appeared between its pages.",
                ),
                prose(
                    "They walked uphill without speaking. Behind them the ferry sounded its horn, a long, low note that seemed to travel through the stones beneath their feet. At the last bend, Mara turned to look at the harbour. The blue circle on the map was glowing.",
                ),
                prose(
                    "For the first time since opening her grandfather’s letter, she felt that she was moving towards something, rather than away.",
                ),
            ],
        ),
        (
            "02 · A map of small things",
            vec![
                label("THE LIGHTHOUSE ATLAS  /  CHAPTER TWO"),
                heading("A map of small things", 28.0, true),
                prose(
                    "The keeper’s kitchen smelled of orange peel and rain. On the table lay a map of the island, its coastline stitched in blue thread. Someone had marked the position of every bench, every spring and every place where the wind changed direction.",
                ),
                prose(
                    "“A useful map remembers what people forget,” the woman said, setting two cups beside the atlas. “The large things can look after themselves.”",
                ),
                prose(
                    "Mara opened the book to the harbour. Where the blue circle had been, a tiny drawing of the kitchen now appeared. Two cups. An open window. A girl who had finally arrived.",
                ),
            ],
        ),
        (
            "03 · The light returns",
            vec![
                label("THE LIGHTHOUSE ATLAS  /  CHAPTER THREE"),
                heading("The light returns", 28.0, true),
                prose(
                    "At dusk they carried the atlas up the spiral stair. The lantern room was cold, but the glass held the last of the sun. Mara placed the book on the brass stand and watched its pages turn in a wind she could not feel.",
                ),
                prose(
                    "Far below, the first windows of the village began to shine. Then the lighthouse answered: a single blue beam sweeping across the sea, drawing a new line on a very old map.",
                ),
            ],
        ),
        (
            "Characters & notes",
            vec![
                heading("Between the chapters", 24.0, true),
                text("Mara — a cartographer discovering the places her grandfather left behind."),
                text("The keeper — a patient guardian of the island’s smallest stories."),
                text("The atlas — a map that changes when someone pays attention."),
            ],
        ),
    ];
    save(book, chapters)?;
    let guide = Document::new("Room for your ideas");
    save(
        guide,
        vec![
            (
                "The essentials",
                vec![
                    label("ALPHADOCS  /  A QUIETER WORKSPACE"),
                    heading("Room for your ideas.", 30.0, false),
                    Block::Paragraph(paragraph(
                        "From the first sentence to the final page, make space for the work that matters.",
                        13.0,
                        true,
                        false,
                        BLUE,
                    )),
                    Block::Image(EmbeddedImage {
                        name: "Blue book".into(),
                        data: include_bytes!("../assets/alphadocs-icon.png")
                            .to_vec()
                            .into(),
                        width: 110.0,
                        height: 110.0,
                        ..Default::default()
                    }),
                    heading("Write with familiar tools", 17.0, false),
                    text(
                        "Give your document structure with headings, clear typography and measured pages. Bring text, tables and pictures together without leaving your workspace.",
                    ),
                    heading("Keep the original", 17.0, false),
                    text(
                        "Trim transparent image borders and adjust the result. Your original picture stays embedded, so you can change your mind later.",
                    ),
                    label("WRITE FREELY.  SAVE LOCALLY.  KEEP THE ORIGINAL."),
                ],
            ),
            (
                "Ideas & references",
                vec![
                    heading("A place for everything", 24.0, false),
                    text(
                        "Keep research, references and supporting notes in separate content tabs inside the same document.",
                    ),
                ],
            ),
        ],
    )?;
    let brief = Document::new("Studio launch brief");
    let cell = |s, bold| paragraph(s, 10.0, false, bold, if bold { BLUE } else { INK });
    save(
        brief,
        vec![
            (
                "Launch plan",
                vec![
                    label("STUDIO NOTES  /  PROJECT BRIEF 01"),
                    heading("Good work starts here.", 30.0, false),
                    Block::Paragraph(paragraph(
                        "A small launch. A clear plan. A shared next step.",
                        13.0,
                        true,
                        false,
                        BLUE,
                    )),
                    heading("The idea", 17.0, false),
                    text(
                        "Create a welcoming place for independent makers to share their process. Lead with useful stories, thoughtful details and a simple invitation to take part.",
                    ),
                    heading("Three weeks to launch", 17.0, false),
                    Block::Table(vec![
                        vec![cell("Milestone", true), cell("What we will make", true)],
                        vec![
                            cell("01  ·  Define", false),
                            cell("A clear story and visual direction", false),
                        ],
                        vec![
                            cell("02  ·  Build", false),
                            cell("An editorial page and launch notes", false),
                        ],
                        vec![
                            cell("03  ·  Share", false),
                            cell("A finished collection, ready for readers", false),
                        ],
                    ]),
                    heading("A useful review", 17.0, false),
                    text(
                        "Open a direct sharing session with a teammate on the same LAN or VPN. Review the wording together, with edit or view access, while keeping a local document copy.",
                    ),
                    label("FOCUS ON THE NEXT SMALL STEP."),
                ],
            ),
            (
                "Review notes",
                vec![
                    heading("Make the details count", 24.0, false),
                    text(
                        "Is the purpose clear? Is the next step easy to find? Does every paragraph earn its place?",
                    ),
                ],
            ),
        ],
    )?;
    persistence::save_preferences(&persistence::Preferences {
        theme: persistence::Theme::Light,
        outline: true,
        autosave: false,
        check_updates: false,
        personal_dictionary: vec!["ALPHADOCS".into(), "VPN".into()],
        ..Default::default()
    })?;
    Ok(())
}
