fn main() {
    println!("cargo:rerun-if-changed=assets/alphadocs.ico");
    println!("cargo:rerun-if-env-changed=VOSK_LIB_DIR");
    if std::env::var_os("CARGO_FEATURE_SPEECH_WORKER").is_some() {
        let library_dir = std::env::var_os("VOSK_LIB_DIR")
            .expect("VOSK_LIB_DIR must point to the pinned native Vosk library directory");
        println!(
            "cargo:rustc-link-search=native={}",
            std::path::PathBuf::from(library_dir).display()
        );
        match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
            Ok("linux") => println!("cargo:rustc-link-arg-bin=alphadocs-speech=-Wl,-rpath,$ORIGIN"),
            Ok("macos") => {
                println!("cargo:rustc-link-arg-bin=alphadocs-speech=-Wl,-rpath,@loader_path")
            }
            _ => {}
        }
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/alphadocs.ico")
            .set("ProductName", "AlphaDocs")
            .set("FileDescription", "AlphaDocs by Alpha Workroom")
            .set("CompanyName", "Alpha Workroom")
            .set("OriginalFilename", "AlphaDocs.exe")
            .set("LegalCopyright", "Copyright (c) 2026 Alpha Workroom")
            .compile()
            .expect("compile the AlphaDocs Windows icon resource");
    }
}
