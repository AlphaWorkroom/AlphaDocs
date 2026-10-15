fn main() {
    println!("cargo:rerun-if-changed=assets/alphadocs.ico");
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
