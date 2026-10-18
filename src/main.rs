#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    let command = std::env::args().nth(1);
    if command.as_deref() == Some("--version") {
        println!("AlphaDocs {}", env!("ALPHADOCS_VERSION"));
        return Ok(());
    }
    if command.as_deref() == Some("--self-check") {
        let ctx = eframe::egui::Context::default();
        alphadocs::fonts::install(&ctx);
        let doc = alphadocs::model::Document::new("Native package self-check");
        let mut pages = 0;
        for _ in 0..2 {
            let mut output = ctx.run_ui(Default::default(), |_| {
                pages = alphadocs::layout::Layout::measure(&ctx, &doc).pages;
            });
            output.textures_delta.clear();
        }
        assert!(pages > 0);
        assert!(!app_icon().rgba.is_empty());
        println!(
            "AlphaDocs {} native fonts, image and measured layout OK",
            env!("ALPHADOCS_VERSION")
        );
        return Ok(());
    }
    let viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([1440.0, 960.0])
        .with_min_inner_size([960.0, 640.0])
        .with_icon(std::sync::Arc::new(app_icon()));
    #[cfg(target_os = "macos")]
    let viewport = viewport
        .with_decorations(true)
        .with_fullsize_content_view(true)
        .with_title_shown(false)
        .with_titlebar_shown(false);
    #[cfg(not(target_os = "macos"))]
    let viewport = viewport.with_decorations(false);
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "AlphaDocs",
        options,
        Box::new(|cc| Ok(Box::new(alphadocs::ui::app::AlphaDocs::new(cc)))),
    )
}

fn app_icon() -> eframe::egui::IconData {
    let image = image::load_from_memory(include_bytes!("../assets/alphadocs-icon.png"))
        .expect("bundled AlphaDocs icon is valid PNG")
        .to_rgba8();
    eframe::egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    }
}
