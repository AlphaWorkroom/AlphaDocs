//! Pinned upstream runtime download, performed only on explicit first-use setup.
use super::*;
use std::collections::BTreeMap;

struct Asset {
    name: &'static str,
    bytes: u64,
    sha256: &'static str,
    files: &'static [&'static str],
}
fn asset() -> Result<Asset, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Ok(Asset {
            name: "vosk-win64-0.3.42.zip",
            bytes: 13_097_226,
            sha256: "9a63e42bd970343041d19e784e545228d3f4703ccec9f2eb1ccc6d5e96c170c3",
            files: &[
                "libvosk.dll",
                "libstdc++-6.dll",
                "libgcc_s_seh-1.dll",
                "libwinpthread-1.dll",
            ],
        }),
        ("linux", "x86_64") => Ok(Asset {
            name: "vosk-linux-x86_64-0.3.42.zip",
            bytes: 7_183_608,
            sha256: "70480495011a29f957c1194cd460449ef7de8c17ea000e387ddb13fd7f844d42",
            files: &["libvosk.so"],
        }),
        ("macos", "x86_64" | "aarch64") => Ok(Asset {
            name: "vosk-osx-0.3.42.zip",
            bytes: 4_702_506,
            sha256: "65395f196c9d0583d79949142b25560acaf9c295f36284e18433097f3adb0ea1",
            files: &["libvosk.dylib"],
        }),
        _ => Err("Offline voice typing is unavailable on this processor/platform".into()),
    }
}
pub fn runtime_download_bytes() -> u64 {
    asset().map_or(0, |a| a.bytes)
}
fn directory(root: &Path) -> PathBuf {
    root.join(format!(
        "runtime-0.3.42-{}-{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    ))
}
fn digest(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    version: String,
    archive_sha256: String,
    worker_sha256: String,
    files: BTreeMap<String, String>,
}
pub(super) fn detect(root: &Path, bundled_worker: &Path) -> Result<Option<PathBuf>, String> {
    let asset = asset()?;
    let dir = directory(root);
    let marker = dir.join("runtime.json");
    if !marker.is_file() || !bundled_worker.is_file() {
        return Ok(None);
    }
    let bytes =
        fs::read(&marker).map_err(|e| format!("Could not read speech runtime manifest: {e}"))?;
    if bytes.len() > 8192 {
        return Err("Speech runtime manifest is invalid; reinstall voice typing".into());
    }
    let manifest: Manifest = serde_json::from_slice(&bytes)
        .map_err(|_| "Speech runtime manifest is invalid; reinstall voice typing".to_owned())?;
    if manifest.version != env!("CARGO_PKG_VERSION")
        || manifest.archive_sha256 != asset.sha256
        || manifest.worker_sha256 != digest(bundled_worker)?
    {
        return Ok(None);
    }
    for &name in asset.files {
        if manifest.files.get(name) != Some(&digest(&dir.join(name)).unwrap_or_default()) {
            return Ok(None);
        }
    }
    let worker = dir.join(
        bundled_worker
            .file_name()
            .ok_or("Speech worker path is invalid")?,
    );
    if digest(&worker).ok().as_ref() != Some(&manifest.worker_sha256) {
        return Ok(None);
    }
    Ok(Some(worker))
}
pub(super) fn install(
    root: &Path,
    bundled_worker: &Path,
    progress: &mut impl FnMut(InstallProgress),
    cancelled: &impl Fn() -> bool,
) -> Result<PathBuf, InstallError> {
    if let Some(worker) = detect(root, bundled_worker).map_err(InstallError::Failed)? {
        return Ok(worker);
    }
    let asset = asset().map_err(InstallError::Failed)?;
    fs::create_dir_all(root).map_err(|e| InstallError::Failed(e.to_string()))?;
    let stage = root.join(format!(
        ".runtime-install-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    fs::create_dir(&stage).map_err(|e| InstallError::Failed(e.to_string()))?;
    let result = (|| -> Result<PathBuf, String> {
        let archive = stage.join("upstream.zip");
        let url = format!(
            "https://github.com/alphacep/vosk-api/releases/download/v0.3.42/{}",
            asset.name
        );
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .https_only(true)
            .max_redirects(4)
            .timeout_global(Some(std::time::Duration::from_secs(300)))
            // This bounds the entire body transfer, not time between chunks.
            .timeout_recv_body(Some(std::time::Duration::from_secs(300)))
            .build()
            .into();
        let mut response = agent
            .get(&url)
            .header(
                "User-Agent",
                concat!("AlphaDocs/", env!("CARGO_PKG_VERSION")),
            )
            .call()
            .map_err(|e| format!("Could not download offline speech runtime: {e}"))?;
        let mut reader = response
            .body_mut()
            .with_config()
            .limit(asset.bytes + 1)
            .reader();
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&archive)
            .map_err(|e| e.to_string())?;
        let mut received = 0;
        let mut buffer = [0u8; 65536];
        loop {
            if cancelled() {
                return Err("Cancelled".into());
            }
            let count = reader.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            received += count as u64;
            if received > asset.bytes {
                return Err("Speech runtime exceeded its pinned size".into());
            }
            output
                .write_all(&buffer[..count])
                .map_err(|e| e.to_string())?;
            progress(InstallProgress {
                phase: InstallPhase::DownloadingRuntime,
                completed_bytes: received,
                total_bytes: Some(asset.bytes),
            });
        }
        output.sync_all().map_err(|e| e.to_string())?;
        drop(output);
        if received != asset.bytes || digest(&archive)? != asset.sha256 {
            return Err("Speech runtime failed its pinned SHA-256 or size check".into());
        }
        let mut zip = ZipArchive::new(File::open(&archive).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        if zip.len() > 64 {
            return Err("Speech runtime archive contains too many files".into());
        }
        let mut files = BTreeMap::new();
        let mut total = 0u64;
        for index in 0..zip.len() {
            if cancelled() {
                return Err("Cancelled".into());
            }
            let mut entry = zip.by_index(index).map_err(|e| e.to_string())?;
            let relative = entry
                .enclosed_name()
                .ok_or("Speech runtime contains an unsafe path")?;
            if entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
                return Err("Speech runtime contains an unexpected symbolic link".into());
            }
            let Some(name) = relative.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if !asset.files.contains(&name) {
                continue;
            }
            if files.contains_key(name) {
                return Err("Speech runtime contains duplicate library names".into());
            }
            total = total
                .checked_add(entry.size())
                .ok_or("Speech runtime size is invalid")?;
            if total > 80_000_000 {
                return Err("Speech runtime expands beyond its size limit".into());
            }
            let path = stage.join(name);
            let mut output = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
                .map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut output).map_err(|e| e.to_string())?;
            drop(output);
            files.insert(name.to_owned(), digest(&path)?);
        }
        drop(zip);
        fs::remove_file(archive).map_err(|e| e.to_string())?;
        if asset.files.iter().any(|name| !files.contains_key(*name)) {
            return Err("Speech runtime is missing required libraries".into());
        }
        let worker_name = bundled_worker
            .file_name()
            .ok_or("Speech worker path is invalid")?;
        fs::copy(bundled_worker, stage.join(worker_name)).map_err(|e| e.to_string())?;
        let marker = Manifest {
            version: env!("CARGO_PKG_VERSION").into(),
            archive_sha256: asset.sha256.into(),
            worker_sha256: digest(bundled_worker)?,
            files,
        };
        fs::write(
            stage.join("runtime.json"),
            serde_json::to_vec(&marker).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let destination = directory(root);
        if destination.exists() {
            // Preserve a prior cached runtime on repair or app-version changes.
            let previous = root.join(format!("runtime-previous-{}", rand::random::<u64>()));
            fs::rename(&destination, previous)
                .map_err(|e| format!("Could not retain previous speech runtime: {e}"))?;
        }
        fs::rename(&stage, &destination)
            .map_err(|e| format!("Could not finish speech setup: {e}"))?;
        Ok(destination.join(worker_name))
    })();
    if stage.is_dir()
        && stage.parent() == Some(root)
        && root
            .canonicalize()
            .ok()
            .zip(stage.canonicalize().ok())
            .is_some_and(|(base, target)| target.starts_with(base))
    {
        let _ = fs::remove_dir_all(&stage);
    }
    result.map_err(|error| {
        if cancelled() {
            InstallError::Cancelled
        } else {
            InstallError::Failed(error)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn runtime_catalog_is_pinned_and_detection_never_downloads() {
        let asset = asset().unwrap();
        assert_eq!(asset.sha256.len(), 64);
        assert!(asset.bytes < 20_000_000);
        let root = std::env::temp_dir().join(format!(
            "alphadocs-runtime-missing-{}",
            rand::random::<u64>()
        ));
        assert_eq!(detect(&root, Path::new("missing-worker")).unwrap(), None);
        assert!(!root.exists());
    }
}
