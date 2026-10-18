//! Verified update download and platform handoff. This module never starts an
//! installer during download; the UI receives a `PreparedUpdate` and launches
//! its helper only after saving state and explicitly closing the app.
use crate::{
    persistence,
    updates::{self, Available, ReleaseAsset},
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use url::Url;

pub const MAX_DOWNLOAD_BYTES: u64 = updates::MAX_ASSET_BYTES;
const CHUNK_SIZE: usize = 64 * 1024;
const MAX_REDIRECTS: usize = 4;

#[derive(Debug)]
pub enum Event {
    Progress { received: u64, total: u64 },
    Ready(PreparedUpdate),
    Error(String),
    Cancelled,
}

pub struct UpdateTask {
    pub receiver: Receiver<Event>,
    cancelled: Arc<AtomicBool>,
}

impl UpdateTask {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

#[derive(Debug)]
pub struct PreparedUpdate {
    pub version: String,
    pub asset: PathBuf,
    pub log: PathBuf,
    expected_size: u64,
    expected_sha256: String,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    current_exe: PathBuf,
}

#[derive(Clone, Debug)]
pub struct InstallHandle {
    pub log: PathBuf,
    pub pid: u32,
}

/// Start a bounded, cancellable download. Automatic redirects are disabled;
/// only a small HTTPS GitHub release-asset allowlist is followed manually.
/// The response must exactly match the byte count and SHA-256 from metadata.
pub fn start(available: Available, current_exe: PathBuf) -> UpdateTask {
    let (sender, receiver) = mpsc::channel();
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = cancelled.clone();
    thread::spawn(move || {
        let result = prepare(available, current_exe, &worker_cancelled, &sender);
        match result {
            Ok(prepared) => {
                if sender.send(Event::Ready(prepared)).is_err() {
                    // The user dismissed the updater; retain a verified staged
                    // artifact for retry instead of deleting a completed file.
                }
            }
            Err(DownloadError::Cancelled) => {
                let _ = sender.send(Event::Cancelled);
            }
            Err(DownloadError::Failed(error)) => {
                let _ = sender.send(Event::Error(error));
            }
        }
    });
    UpdateTask {
        receiver,
        cancelled,
    }
}

#[derive(Debug)]
enum DownloadError {
    Cancelled,
    Failed(String),
}

impl From<String> for DownloadError {
    fn from(value: String) -> Self {
        Self::Failed(value)
    }
}

fn prepare(
    available: Available,
    current_exe: PathBuf,
    cancelled: &AtomicBool,
    sender: &Sender<Event>,
) -> Result<PreparedUpdate, DownloadError> {
    if cancelled.load(Ordering::SeqCst) {
        return Err(DownloadError::Cancelled);
    }
    let asset = available.asset.ok_or_else(|| {
        available
            .install_error
            .unwrap_or_else(|| "No verified installer is available for this platform.".into())
    })?;
    validate_asset(&available.version, &asset)?;
    validate_install_layout(&current_exe, &asset)?;

    let update_root = persistence::data_dir().join("updates");
    fs::create_dir_all(&update_root)
        .map_err(|e| format!("Could not create the update staging folder: {e}"))?;
    make_private_directory(&update_root)
        .map_err(|e| format!("Could not protect the update staging folder: {e}"))?;
    let staging = update_root.join(format!(
        "stage-{}-{}-{}",
        available.version,
        std::process::id(),
        nonce()
    ));
    fs::create_dir(&staging)
        .map_err(|e| format!("Could not create a private update staging folder: {e}"))?;
    if let Err(error) = make_private_directory(&staging) {
        let _ = fs::remove_dir(&staging);
        return Err(format!("Could not protect the update staging folder: {error}").into());
    }
    let final_path = staging.join(&asset.name);
    let part_path = staging.join(format!("{}.part", asset.name));

    let downloaded = download_http(&asset, &part_path, cancelled, |received| {
        let _ = sender.send(Event::Progress {
            received,
            total: asset.size,
        });
    });
    if let Err(error) = downloaded {
        let _ = fs::remove_dir(&staging);
        return Err(error);
    }
    if let Err(error) = fs::rename(&part_path, &final_path) {
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("Could not atomically stage the verified update: {error}").into());
    }

    Ok(PreparedUpdate {
        version: available.version,
        asset: final_path,
        log: staging.join("install.log"),
        expected_size: asset.size,
        expected_sha256: asset.sha256,
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        current_exe,
    })
}

fn validate_asset(version: &str, asset: &ReleaseAsset) -> Result<(), DownloadError> {
    updates::stable_version(version).map_err(|_| "Invalid update version metadata".to_owned())?;
    let expected_name = updates::installer(std::env::consts::OS, std::env::consts::ARCH)
        .ok_or_else(|| "Automatic installation is unavailable on this platform.".to_owned())?;
    if asset.name != expected_name {
        return Err("The release asset does not match this platform"
            .to_owned()
            .into());
    }
    let expected_url = format!(
        "https://github.com/AlphaWorkroom/AlphaDocs/releases/download/v{version}/{expected_name}"
    );
    if asset.url != expected_url {
        return Err(
            "The update asset is not hosted at the canonical AlphaDocs release URL"
                .to_owned()
                .into(),
        );
    }
    if asset.size == 0 || asset.size > MAX_DOWNLOAD_BYTES {
        return Err(
            "The update asset is empty or exceeds the 100 MB download limit"
                .to_owned()
                .into(),
        );
    }
    if !updates::is_sha256(&asset.sha256) {
        return Err(
            "The update has no valid SHA-256 digest; installation is blocked"
                .to_owned()
                .into(),
        );
    }
    Ok(())
}

fn download_http(
    asset: &ReleaseAsset,
    part_path: &Path,
    cancelled: &AtomicBool,
    progress: impl FnMut(u64),
) -> Result<(), DownloadError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .timeout_global(Some(std::time::Duration::from_secs(180)))
        .build()
        .into();
    let mut response = release_asset_response(&agent, &asset.url)?;
    if response.body().content_length() != Some(asset.size) {
        return Err(
            "The update server's Content-Length does not match GitHub metadata"
                .to_owned()
                .into(),
        );
    }
    let reader = response
        .body_mut()
        .with_config()
        .limit(MAX_DOWNLOAD_BYTES)
        .reader();
    download_reader(reader, asset, part_path, cancelled, progress)
}

fn release_asset_response(
    agent: &ureq::Agent,
    initial_url: &str,
) -> Result<ureq::http::Response<ureq::Body>, DownloadError> {
    let mut current =
        Url::parse(initial_url).map_err(|_| "The canonical update URL is invalid".to_owned())?;
    for redirects in 0..=MAX_REDIRECTS {
        let response = agent
            .get(current.as_str())
            .header("Accept", "application/octet-stream")
            .header(
                "User-Agent",
                concat!("AlphaDocs/", env!("CARGO_PKG_VERSION")),
            )
            .call()
            .map_err(|e| format!("Could not download the update: {e}"))?;
        if !response.status().is_redirection() {
            return Ok(response);
        }
        if redirects == MAX_REDIRECTS {
            return Err("The update server redirected too many times"
                .to_owned()
                .into());
        }
        let location = response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| "The update server returned an invalid redirect".to_owned())?;
        let next = current
            .join(location)
            .map_err(|_| "The update server returned an invalid redirect URL".to_owned())?;
        validate_release_redirect(&next)?;
        current = next;
    }
    unreachable!("redirect loop always returns")
}

fn validate_release_redirect(url: &Url) -> Result<(), DownloadError> {
    let host = url.host_str().unwrap_or_default();
    let allowed_host = matches!(
        host,
        "release-assets.githubusercontent.com" | "objects.githubusercontent.com"
    );
    if url.scheme() != "https"
        || !allowed_host
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
    {
        return Err("The update server redirected to an untrusted location"
            .to_owned()
            .into());
    }
    Ok(())
}

fn download_reader<R: Read>(
    mut reader: R,
    asset: &ReleaseAsset,
    part_path: &Path,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(u64),
) -> Result<(), DownloadError> {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(part_path)
        .map_err(|e| format!("Could not create the update staging file: {e}"))?;
    let result = (|| {
        let mut hash = Sha256::new();
        let mut received = 0u64;
        let mut buffer = [0u8; CHUNK_SIZE];
        loop {
            if cancelled.load(Ordering::SeqCst) {
                return Err(DownloadError::Cancelled);
            }
            let count = reader
                .read(&mut buffer)
                .map_err(|e| format!("The update download ended unexpectedly: {e}"))?;
            if count == 0 {
                break;
            }
            received = received.saturating_add(count as u64);
            if received > asset.size || received > MAX_DOWNLOAD_BYTES {
                return Err("The update exceeded its declared size".to_owned().into());
            }
            output
                .write_all(&buffer[..count])
                .map_err(|e| format!("Could not write the update to disk: {e}"))?;
            hash.update(&buffer[..count]);
            progress(received);
        }
        if received != asset.size {
            return Err("The downloaded update size does not match GitHub metadata"
                .to_owned()
                .into());
        }
        let digest = format!("{:x}", hash.finalize());
        if !digest.eq_ignore_ascii_case(&asset.sha256) {
            return Err(
                "The update SHA-256 digest does not match GitHub metadata; installation is blocked"
                    .to_owned()
                    .into(),
            );
        }
        output
            .sync_all()
            .map_err(|e| format!("Could not flush the verified update to disk: {e}"))?;
        Ok(())
    })();
    drop(output);
    if result.is_err() {
        let _ = fs::remove_file(part_path);
    }
    result
}

impl PreparedUpdate {
    pub fn launch(self, old_pid: u32, dark_mode: bool) -> Result<InstallHandle, String> {
        if old_pid == 0 {
            return Err("The verified update is no longer available".into());
        }
        verify_staged_asset(&self.asset, self.expected_size, &self.expected_sha256)?;
        let mut command = platform_helper(&self, old_pid, dark_mode)?;
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = command
            .spawn()
            .map_err(|e| format!("Could not start the update helper: {e}"))?;
        Ok(InstallHandle {
            log: self.log,
            pid: child.id(),
        })
    }
}

fn verify_staged_asset(
    path: &Path,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| "The verified update is no longer available".to_owned())?;
    if !metadata.file_type().is_file()
        || metadata.len() != expected_size
        || expected_size == 0
        || expected_size > MAX_DOWNLOAD_BYTES
    {
        return Err("The staged update changed after download verification".into());
    }
    let mut file =
        fs::File::open(path).map_err(|e| format!("Could not reopen the verified update: {e}"))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; CHUNK_SIZE];
    let mut received = 0u64;
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|e| format!("Could not recheck the staged update: {e}"))?;
        if count == 0 {
            break;
        }
        received = received.saturating_add(count as u64);
        if received > expected_size {
            return Err("The staged update changed after download verification".into());
        }
        hash.update(&buffer[..count]);
    }
    if received != expected_size
        || !format!("{:x}", hash.finalize()).eq_ignore_ascii_case(expected_sha256)
    {
        return Err("The staged update failed its SHA-256 recheck".into());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn validate_install_layout(current_exe: &Path, asset: &ReleaseAsset) -> Result<(), DownloadError> {
    let local = std::env::var_os("LOCALAPPDATA")
        .ok_or_else(|| "Could not locate your Windows application-data folder".to_owned())?;
    validate_windows_update_source(current_exe, asset, Path::new(&local))
}

#[cfg(target_os = "windows")]
fn validate_windows_update_source(
    current_exe: &Path,
    asset: &ReleaseAsset,
    local: &Path,
) -> Result<(), DownloadError> {
    if asset.name != "AlphaDocs-Windows-x86_64-Setup.exe" {
        return Err(
            "This Windows installer does not match the running application"
                .to_owned()
                .into(),
        );
    }
    if !local.is_absolute() || !current_exe.is_absolute() || !current_exe.is_file() {
        return Err("Could not resolve the running Windows application or installation destination. No files were changed.".to_owned().into());
    }
    // Setup always targets LOCALAPPDATA/Programs/AlphaDocs. A portable or
    // renamed executable is a valid source, not an arbitrary replacement
    // destination. Its original folder and the document library are retained.
    Ok(())
}

#[cfg(target_os = "macos")]
fn validate_install_layout(current_exe: &Path, asset: &ReleaseAsset) -> Result<(), DownloadError> {
    if !asset.name.starts_with("AlphaDocs-macOS-") || !asset.name.ends_with(".dmg") {
        return Err(
            "This macOS installer does not match the running application"
                .to_owned()
                .into(),
        );
    }
    let app = current_exe
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or_else(|| {
            "The running application is not inside an AlphaDocs.app bundle".to_owned()
        })?;
    if app.file_name().is_none_or(|name| name != "AlphaDocs.app")
        || !current_exe.ends_with("Contents/MacOS/alphadocs")
    {
        return Err(
            "Automatic macOS updates require the standard AlphaDocs.app layout"
                .to_owned()
                .into(),
        );
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let resolved_app = fs::canonicalize(app)
        .map_err(|_| "Could not resolve the installed AlphaDocs application bundle".to_owned())?;
    let system_parent =
        fs::canonicalize("/Applications").unwrap_or_else(|_| PathBuf::from("/Applications"));
    let user_parent =
        fs::canonicalize(home.join("Applications")).unwrap_or_else(|_| home.join("Applications"));
    let allowed = system_parent.join("AlphaDocs.app");
    let user_app = user_parent.join("AlphaDocs.app");
    if !same_path(&resolved_app, &allowed) && !same_path(&resolved_app, &user_app) {
        return Err(
            "Install AlphaDocs in /Applications or ~/Applications to use automatic updates."
                .to_owned()
                .into(),
        );
    }
    check_writable_parent(resolved_app.parent().unwrap())?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn validate_install_layout(current_exe: &Path, asset: &ReleaseAsset) -> Result<(), DownloadError> {
    if asset.name != "AlphaDocs-Linux-x86_64.deb" {
        return Err(
            "This Linux installer does not match the running application"
                .to_owned()
                .into(),
        );
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let system = PathBuf::from("/opt/alphadocs/alphadocs");
    let user = home.join(".local/opt/alphadocs/alphadocs");
    if !same_path(current_exe, &system) && !same_path(current_exe, &user) {
        return Err("Automatic Linux updates support AlphaDocs installed at /opt/alphadocs or ~/.local/opt/alphadocs.".to_owned().into());
    }
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn validate_install_layout(_: &Path, _: &ReleaseAsset) -> Result<(), DownloadError> {
    Err(
        "Automatic installation is unavailable on this operating system."
            .to_owned()
            .into(),
    )
}

#[cfg(target_os = "windows")]
fn platform_helper(
    update: &PreparedUpdate,
    old_pid: u32,
    dark_mode: bool,
) -> Result<Command, String> {
    let mut command = Command::new(&update.asset);
    command
        // Shortcuts start AlphaDocs inside its installation folder. A helper
        // inheriting that CWD locks the directory it must rename on Windows.
        .current_dir(update.asset.parent().ok_or("Invalid update staging path")?)
        .arg("--update")
        .arg(old_pid.to_string())
        .arg(&update.log);
    if dark_mode {
        command.arg("--dark");
    }
    Ok(command)
}

#[cfg(target_os = "macos")]
fn platform_helper(
    update: &PreparedUpdate,
    old_pid: u32,
    _dark_mode: bool,
) -> Result<Command, String> {
    let script = write_helper(&update.log, include_str!("../scripts/update_macos.sh"))?;
    let app = update
        .current_exe
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let app = fs::canonicalize(app)
        .map_err(|e| format!("Could not resolve the installed AlphaDocs app bundle: {e}"))?;
    let mut command = Command::new("/bin/sh");
    command.args([
        script.as_os_str(),
        std::ffi::OsStr::new(&old_pid.to_string()),
        update.asset.as_os_str(),
        app.as_os_str(),
        update.log.as_os_str(),
    ]);
    Ok(command)
}

#[cfg(target_os = "linux")]
fn platform_helper(
    update: &PreparedUpdate,
    old_pid: u32,
    _dark_mode: bool,
) -> Result<Command, String> {
    let script = write_helper(&update.log, include_str!("../scripts/update_linux.sh"))?;
    let install_dir = update.current_exe.parent().unwrap();
    let mut command = Command::new("/bin/sh");
    command.args([
        script.as_os_str(),
        std::ffi::OsStr::new(&old_pid.to_string()),
        update.asset.as_os_str(),
        install_dir.as_os_str(),
        update.current_exe.as_os_str(),
        update.log.as_os_str(),
    ]);
    Ok(command)
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn platform_helper(_: &PreparedUpdate, _: u32, _: bool) -> Result<Command, String> {
    Err("Automatic installation is unavailable on this operating system.".into())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn write_helper(log: &Path, content: &str) -> Result<PathBuf, String> {
    let parent = log.parent().ok_or("Invalid update staging path")?;
    let path = parent.join("apply-update.sh");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| format!("Could not create the update helper script: {e}"))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("Could not write the update helper script: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("Could not protect the update helper script: {e}"))?;
    }
    Ok(path)
}

fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn make_private_directory(path: &Path) -> std::io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_dir() {
        return Err(std::io::Error::other(
            "update staging path must be a real directory",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(any(not(windows), test))]
fn same_path(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        normalize_windows_path(left).eq_ignore_ascii_case(&normalize_windows_path(right))
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

#[cfg(all(windows, test))]
fn normalize_windows_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('/', "\\");
    let normalized = normalized
        .strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .or_else(|| normalized.strip_prefix(r"\\?\").map(str::to_owned))
        .unwrap_or(normalized);
    if normalized.len() > 3 {
        normalized.trim_end_matches('\\').to_owned()
    } else {
        normalized
    }
}

#[cfg(target_os = "macos")]
fn check_writable_parent(path: &Path) -> Result<(), DownloadError> {
    let probe = path.join(format!(".alphadocs-update-probe-{}", nonce()));
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|_| {
            "The AlphaDocs installation folder is not writable by this user; no files were changed."
                .to_owned()
        })?;
    fs::remove_file(probe).map_err(|e| {
        format!("Could not remove the temporary installation permission check: {e}")
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[cfg(windows)]
    #[test]
    fn windows_update_helper_uses_staging_directory_not_installed_working_directory() {
        let update = PreparedUpdate {
            version: "1.2.3".into(),
            asset: PathBuf::from(r"C:\isolated\updates\stage\Setup.exe"),
            log: PathBuf::from(r"C:\isolated\updates\stage\install.log"),
            expected_size: 1,
            expected_sha256: "0".repeat(64),
        };
        let command = platform_helper(&update, 123, true).unwrap();
        assert_eq!(command.get_current_dir(), update.asset.parent());
        assert_eq!(command.get_program(), update.asset.as_os_str());
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["--update", "123", update.log.to_str().unwrap(), "--dark"]
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_update_accepts_installed_portable_and_renamed_sources() {
        let dir = temporary_dir();
        let local = dir.join("Local");
        let mut installer = asset(b"verified separately");
        installer.name = "AlphaDocs-Windows-x86_64-Setup.exe".into();
        for relative in [
            "Local/Programs/AlphaDocs/AlphaDocs.exe",
            "Downloads/AlphaDocs/AlphaDocs.exe",
            "qa/native/AlphaDocs-QA.exe",
        ] {
            let source = dir.join(relative);
            fs::create_dir_all(source.parent().unwrap()).unwrap();
            fs::write(&source, b"unchanged original").unwrap();
            assert!(validate_windows_update_source(&source, &installer, &local).is_ok());
            assert_eq!(fs::read(&source).unwrap(), b"unchanged original");
        }
        let source = dir.join("qa/native/AlphaDocs-QA.exe");
        assert!(validate_windows_update_source(&source, &installer, Path::new("")).is_err());
        assert!(
            validate_windows_update_source(&dir.join("missing.exe"), &installer, &local).is_err()
        );
        installer.name = "AlphaDocs-macOS-aarch64.dmg".into();
        assert!(validate_windows_update_source(&source, &installer, &local).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    fn temporary_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "alphadocs-updater-test-{}-{}",
            std::process::id(),
            nonce()
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    fn asset(bytes: &[u8]) -> ReleaseAsset {
        ReleaseAsset {
            name: "AlphaDocs-test.bin".into(),
            url: "https://github.com/AlphaWorkroom/AlphaDocs/releases/download/v1.2.3/AlphaDocs-test.bin".into(),
            size: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
        }
    }

    #[test]
    fn four_part_patch_keeps_exact_installer_and_tag_validation() {
        let name = updates::installer(std::env::consts::OS, std::env::consts::ARCH).unwrap();
        let mut meta = asset(b"verified download");
        meta.name = name.into();
        meta.url =
            format!("https://github.com/AlphaWorkroom/AlphaDocs/releases/download/v0.1.8.1/{name}");
        assert!(validate_asset("0.1.8.1", &meta).is_ok());
        assert!(validate_asset("0.1.8.01", &meta).is_err());
        meta.url = meta.url.replace("v0.1.8.1/", "v0.1.8/");
        assert!(validate_asset("0.1.8.1", &meta).is_err());
    }

    #[test]
    fn verified_download_writes_atomically_and_reports_progress() {
        let dir = temporary_dir();
        let target = dir.join("update.part");
        let bytes = b"synthetic verified package";
        let meta = asset(bytes);
        let cancel = AtomicBool::new(false);
        let mut progress = 0;
        download_reader(Cursor::new(bytes), &meta, &target, &cancel, |n| {
            progress = n
        })
        .unwrap();
        assert_eq!(progress, bytes.len() as u64);
        assert_eq!(fs::read(&target).unwrap(), bytes);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn size_digest_oversize_and_cancel_fail_closed_and_remove_partial_files() {
        let dir = temporary_dir();
        let bytes = b"synthetic package";
        let cancel = AtomicBool::new(false);
        let mut wrong_hash = asset(bytes);
        wrong_hash.sha256 = "0".repeat(64);
        let path = dir.join("wrong-hash.part");
        assert!(download_reader(Cursor::new(bytes), &wrong_hash, &path, &cancel, |_| {}).is_err());
        assert!(!path.exists());

        let mut wrong_size = asset(bytes);
        wrong_size.size += 1;
        let path = dir.join("wrong-size.part");
        assert!(download_reader(Cursor::new(bytes), &wrong_size, &path, &cancel, |_| {}).is_err());
        assert!(!path.exists());

        let mut oversized = asset(bytes);
        oversized.size = MAX_DOWNLOAD_BYTES + 1;
        assert!(validate_asset("1.2.3", &oversized).is_err());

        let path = dir.join("cancel.part");
        let stopped = AtomicBool::new(true);
        assert!(matches!(
            download_reader(Cursor::new(bytes), &asset(bytes), &path, &stopped, |_| {}),
            Err(DownloadError::Cancelled)
        ));
        assert!(!path.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn staged_installer_is_rechecked_before_handoff() {
        let dir = temporary_dir();
        let path = dir.join("AlphaDocs-test.bin");
        let bytes = b"synthetic verified package";
        fs::write(&path, bytes).unwrap();
        let valid = asset(bytes);
        assert!(verify_staged_asset(&path, valid.size, &valid.sha256).is_ok());

        fs::write(&path, b"tampered").unwrap();
        assert!(verify_staged_asset(&path, valid.size, &valid.sha256).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn redirects_accept_only_https_github_release_asset_hosts() {
        for url in [
            "https://release-assets.githubusercontent.com/github-production-release-asset/file?token=opaque",
            "https://objects.githubusercontent.com/github-production-release-asset/file?token=opaque",
        ] {
            assert!(validate_release_redirect(&Url::parse(url).unwrap()).is_ok());
        }
        for url in [
            "http://release-assets.githubusercontent.com/file",
            "https://evil.example/file",
            "https://user@release-assets.githubusercontent.com/file",
            "https://release-assets.githubusercontent.com:444/file",
            "https://release-assets.githubusercontent.com/file#fragment",
        ] {
            assert!(validate_release_redirect(&Url::parse(url).unwrap()).is_err());
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_install_path_comparison_normalizes_separators_prefixes_and_case() {
        assert!(same_path(
            Path::new(r"C:\Users\Alice\AppData\Local\Programs\AlphaDocs\AlphaDocs.exe"),
            Path::new(r"c:/users/alice/appdata/local/Programs/AlphaDocs/AlphaDocs.exe")
        ));
        assert!(same_path(
            Path::new(r"\\?\C:\Users\Alice\Programs\AlphaDocs.exe"),
            Path::new(r"c:\users\alice\programs\alphadocs.exe")
        ));
        assert!(!same_path(
            Path::new(r"C:\Users\Alice\Programs\AlphaDocs.exe"),
            Path::new(r"C:\Users\Alice\Programs\Other.exe")
        ));
    }

    #[test]
    #[ignore = "explicit public-network smoke test; not part of offline test runs"]
    fn live_release_asset_redirect_smoke() {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .https_only(true)
            .max_redirects(0)
            .timeout_global(Some(std::time::Duration::from_secs(30)))
            .build()
            .into();
        let mut response = agent
            .get("https://api.github.com/repos/AlphaWorkroom/AlphaDocs/releases/latest")
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "AlphaDocs-update-smoke")
            .call()
            .unwrap();
        let metadata = response
            .body_mut()
            .with_config()
            .limit(1024 * 1024)
            .read_to_vec()
            .unwrap();
        let release: serde_json::Value = serde_json::from_slice(&metadata).unwrap();
        let tag = release["tag_name"].as_str().unwrap();
        let asset = release["assets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|asset| asset["name"] == "checksums.json")
            .expect("latest public release should include the small checksum manifest");
        let name = asset["name"].as_str().unwrap();
        let url = asset["browser_download_url"].as_str().unwrap();
        assert_eq!(
            url,
            format!("https://github.com/AlphaWorkroom/AlphaDocs/releases/download/{tag}/{name}")
        );
        let meta = ReleaseAsset {
            name: name.to_owned(),
            url: url.to_owned(),
            size: asset["size"].as_u64().unwrap(),
            sha256: asset["digest"]
                .as_str()
                .unwrap()
                .strip_prefix("sha256:")
                .unwrap()
                .to_owned(),
        };
        assert!(meta.size < 100_000);
        let dir = temporary_dir();
        let path = dir.join("checksums.json.part");
        download_http(&meta, &path, &AtomicBool::new(false), |_| {}).unwrap();
        assert!(serde_json::from_slice::<serde_json::Value>(&fs::read(&path).unwrap()).is_ok());
        fs::remove_dir_all(dir).unwrap();
    }
}
