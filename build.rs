fn main() {
    let base = format!(
        "{}.{}.{}",
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR"),
        env!("CARGO_PKG_VERSION_PATCH")
    );
    let build = env!("CARGO_PKG_VERSION")
        .split_once('+')
        .map_or("", |(_, build)| build);
    let version = if let Some(revision) = build.strip_prefix("patch.") {
        let number: u16 = revision.parse().expect("numeric patch revision");
        assert_eq!(number.to_string(), revision, "canonical patch revision");
        format!("{base}.{revision}")
    } else {
        assert!(build.is_empty(), "unsupported release build metadata");
        base
    };
    println!("cargo:rustc-env=ALPHADOCS_VERSION={version}");
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
        let numeric = version
            .split('.')
            .chain(std::iter::repeat("0"))
            .take(4)
            .fold(0u64, |value, part| {
                (value << 16) | u64::from(part.parse::<u16>().expect("Windows version component"))
            });
        winresource::WindowsResource::new()
            .set("ProductVersion", &version)
            .set("FileVersion", &version)
            .set_version_info(winresource::VersionInfo::PRODUCTVERSION, numeric)
            .set_version_info(winresource::VersionInfo::FILEVERSION, numeric)
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
