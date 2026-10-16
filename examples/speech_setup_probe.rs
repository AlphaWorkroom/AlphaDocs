fn main() -> Result<(), String> {
    let root = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Pass a QA speech folder")?,
    );
    let mut last = None;
    let runtime = alphadocs::speech::install_runtime(
        &root,
        alphadocs::speech::Engine::VoskSmallEnUs,
        |p| {
            let key = (p.phase, p.completed_bytes / 10_000_000);
            if last != Some(key) {
                println!("{:?}: {} / {:?}", p.phase, p.completed_bytes, p.total_bytes);
                last = Some(key);
            }
        },
        || false,
    )
    .map_err(|e| format!("{e:?}"))?;
    println!("Ready: {}", runtime.worker_path.display());
    Ok(())
}
