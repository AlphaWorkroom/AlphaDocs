//! Protocol and editor anchoring for optional offline speech recognition.
//!
//! Speech results are previews until a matching session is explicitly stopped.
//! Only a final transcript may be applied, through [`Editor::insert`], and only
//! while the document, tab, revision and selection still match the captured
//! anchor.

use crate::{engine::Editor, model::Position};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    path::{Component, Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError},
    thread,
};
use zip::ZipArchive;
mod native;
pub use native::runtime_download_bytes;

pub const SPEECH_PROTOCOL_VERSION: u16 = 1;
pub const DEFAULT_MODEL_ID: &str = "vosk-model-small-en-us-0.15";
pub const DEFAULT_MODEL_LANGUAGE: &str = "English (United States)";
pub const DEFAULT_MODEL_ARCHIVE_BYTES: u64 = 41_205_931;
pub const DEFAULT_MODEL_ARCHIVE_SHA256: &str =
    "30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498";
pub const DEFAULT_MODEL_URL: &str =
    "https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip";
const MAX_MODEL_ARCHIVE_BYTES: u64 = 80_000_000;
const MAX_MODEL_UNPACKED_BYTES: u64 = 150_000_000;
const MAX_MODEL_FILES: usize = 256;
const MAX_WORKER_EVENT_BYTES: usize = 64 * 1024;
const WORKER_EVENT_QUEUE_CAPACITY: usize = 32;

/// Offline recognition backend selected by the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    /// Compact Apache-licensed Vosk English model.
    VoskSmallEnUs,
}

/// A ready worker command. `args` are engine-owned and must be passed verbatim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledRuntime {
    pub worker_path: PathBuf,
    pub args: Vec<OsString>,
    pub engine: Engine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallPhase {
    DownloadingRuntime,
    DownloadingModel,
    ExtractingModel,
    Ready,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstallProgress {
    pub phase: InstallPhase,
    pub completed_bytes: u64,
    pub total_bytes: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallError {
    Cancelled,
    Failed(String),
}

pub const fn model_languages(engine: Engine) -> &'static [&'static str] {
    match engine {
        Engine::VoskSmallEnUs => &[DEFAULT_MODEL_LANGUAGE],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpeechModelDescriptor {
    pub id: &'static str,
    pub language: &'static str,
    pub archive_bytes: u64,
    pub sha256: &'static str,
    pub download_url: &'static str,
    pub license: &'static str,
}

pub const DEFAULT_ENGLISH_US_MODEL: SpeechModelDescriptor = SpeechModelDescriptor {
    id: DEFAULT_MODEL_ID,
    language: DEFAULT_MODEL_LANGUAGE,
    archive_bytes: DEFAULT_MODEL_ARCHIVE_BYTES,
    sha256: DEFAULT_MODEL_ARCHIVE_SHA256,
    download_url: DEFAULT_MODEL_URL,
    license: "Apache-2.0",
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelDownloadProgress {
    Downloading { received: u64, total: u64 },
    Extracting,
}

/// Download and install the pinned English model after an explicit user action.
/// The archive is hash-checked before it is extracted into the supplied app-data
/// model directory. It performs no work at application startup.
pub fn install_default_english_us_model(
    model_root: &Path,
    mut progress: impl FnMut(ModelDownloadProgress),
) -> Result<PathBuf, String> {
    install_default_english_us_model_with_cancel(model_root, &mut progress, || false)
}

fn install_default_english_us_model_with_cancel(
    model_root: &Path,
    progress: &mut impl FnMut(ModelDownloadProgress),
    cancelled: impl Fn() -> bool,
) -> Result<PathBuf, String> {
    let installed = model_root.join(DEFAULT_MODEL_ID);
    if installed.exists() {
        return validate_model_directory(&installed).map(|()| installed);
    }
    fs::create_dir_all(model_root)
        .map_err(|error| format!("Could not create model folder: {error}"))?;

    let archive_path = model_root.join(format!(".{DEFAULT_MODEL_ID}.zip.part"));
    let stage_path = model_root.join(format!(
        ".{DEFAULT_MODEL_ID}.installing-{}",
        std::process::id()
    ));
    if archive_path.exists() || stage_path.exists() {
        return Err("An incomplete speech-model installation already exists; remove that temporary model folder and retry".into());
    }

    let result = (|| {
        download_model_archive(&archive_path, progress, &cancelled)?;
        if cancelled() {
            return Err("Speech model installation was cancelled".into());
        }
        progress(ModelDownloadProgress::Extracting);
        fs::create_dir(&stage_path)
            .map_err(|error| format!("Could not create model staging folder: {error}"))?;
        extract_pinned_model(&archive_path, &stage_path, &cancelled)?;
        validate_model_directory(&stage_path.join(DEFAULT_MODEL_ID))?;
        fs::rename(stage_path.join(DEFAULT_MODEL_ID), &installed)
            .map_err(|error| format!("Could not install speech model: {error}"))?;
        Ok(installed.clone())
    })();
    let _ = fs::remove_file(&archive_path);
    let _ = fs::remove_dir_all(&stage_path);
    result
}

/// Return the ready-to-run worker for an installed backend, if its executable,
/// model and required runtime files are present. `root` is app-data/AlphaDocs/speech.
pub fn detect_installed_runtime(
    root: &Path,
    engine: Engine,
) -> Result<Option<InstalledRuntime>, String> {
    match engine {
        Engine::VoskSmallEnUs => {
            let model = root.join(DEFAULT_MODEL_ID);
            if !model.exists() {
                return Ok(None);
            }
            validate_model_directory(&model)?;
            let Some(worker) = native::detect(root, &speech_worker_path()?)? else {
                return Ok(None);
            };
            Ok(Some(InstalledRuntime {
                worker_path: worker,
                args: vec![model.into_os_string()],
                engine,
            }))
        }
    }
}

/// Install an engine only after an explicit user action. Runtime files and
/// models live below the supplied application-data folder, never the repo.
pub fn install_runtime(
    root: &Path,
    engine: Engine,
    mut progress: impl FnMut(InstallProgress),
    cancelled: impl Fn() -> bool,
) -> Result<InstalledRuntime, InstallError> {
    if cancelled() {
        return Err(InstallError::Cancelled);
    }
    match engine {
        Engine::VoskSmallEnUs => {
            let worker = speech_worker_path().map_err(InstallError::Failed)?;
            if !worker.is_file() {
                return Err(InstallError::Failed(format!(
                    "The optional speech worker was not packaged beside AlphaDocs: {}",
                    worker.display()
                )));
            }
            let worker = native::install(root, &worker, &mut progress, &cancelled)?;
            let model = install_default_english_us_model_with_cancel(
                root,
                &mut |update| {
                    let item = match update {
                        ModelDownloadProgress::Downloading { received, total } => InstallProgress {
                            phase: InstallPhase::DownloadingModel,
                            completed_bytes: received,
                            total_bytes: Some(total),
                        },
                        ModelDownloadProgress::Extracting => InstallProgress {
                            phase: InstallPhase::ExtractingModel,
                            completed_bytes: 0,
                            total_bytes: None,
                        },
                    };
                    progress(item);
                },
                &cancelled,
            )
            .map_err(|error| {
                if cancelled() || error.contains("cancelled") {
                    InstallError::Cancelled
                } else {
                    InstallError::Failed(error)
                }
            })?;
            if cancelled() {
                return Err(InstallError::Cancelled);
            }
            let installed = InstalledRuntime {
                worker_path: worker,
                args: vec![model.into_os_string()],
                engine,
            };
            progress(InstallProgress {
                phase: InstallPhase::Ready,
                completed_bytes: DEFAULT_MODEL_ARCHIVE_BYTES,
                total_bytes: Some(DEFAULT_MODEL_ARCHIVE_BYTES),
            });
            Ok(installed)
        }
    }
}

fn speech_worker_path() -> Result<PathBuf, String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("Could not locate AlphaDocs: {error}"))?;
    let parent = executable
        .parent()
        .ok_or_else(|| "Could not locate the AlphaDocs application folder".to_owned())?;
    #[cfg(windows)]
    let name = "alphadocs-speech.exe";
    #[cfg(not(windows))]
    let name = "alphadocs-speech";
    Ok(parent.join(name))
}

pub fn validate_model_directory(path: &Path) -> Result<(), String> {
    if !path.is_dir() {
        return Err("Choose a folder containing a Vosk speech model".into());
    }
    for required in ["am/final.mdl", "conf/model.conf"] {
        if !path.join(required).is_file() {
            return Err(format!(
                "Selected folder is missing the Vosk model file `{required}`"
            ));
        }
    }
    let has_hclg = path.join("graph/HCLG.fst").is_file();
    let has_lookahead_graph =
        path.join("graph/HCLr.fst").is_file() && path.join("graph/Gr.fst").is_file();
    if !has_hclg && !has_lookahead_graph {
        return Err("Selected folder is missing a supported Vosk decoding graph".into());
    }
    Ok(())
}

fn download_model_archive(
    destination: &Path,
    progress: &mut impl FnMut(ModelDownloadProgress),
    cancelled: &impl Fn() -> bool,
) -> Result<(), String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(4)
        .timeout_global(Some(std::time::Duration::from_secs(300)))
        // This bounds the entire body transfer, not time between chunks.
        .timeout_recv_body(Some(std::time::Duration::from_secs(300)))
        .build()
        .into();
    let mut response = agent
        .get(DEFAULT_MODEL_URL)
        .header(
            "User-Agent",
            concat!("AlphaDocs/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|error| format!("Could not download the speech model: {error}"))?;
    if response.body().content_length() != Some(DEFAULT_MODEL_ARCHIVE_BYTES) {
        return Err("The speech-model server returned an unexpected archive size".into());
    }
    let reader = response
        .body_mut()
        .with_config()
        .limit(MAX_MODEL_ARCHIVE_BYTES)
        .reader();
    let mut reader = reader;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| format!("Could not create speech-model download: {error}"))?;
    let mut hasher = Sha256::new();
    let mut received = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if cancelled() {
            return Err("Speech model installation was cancelled".into());
        }
        let count = reader
            .read(&mut buffer)
            .map_err(|error| format!("Could not read the speech-model download: {error}"))?;
        if count == 0 {
            break;
        }
        received = received.saturating_add(count as u64);
        if received > DEFAULT_MODEL_ARCHIVE_BYTES {
            return Err("The speech-model download exceeded its pinned size".into());
        }
        hasher.update(&buffer[..count]);
        output
            .write_all(&buffer[..count])
            .map_err(|error| format!("Could not save the speech-model download: {error}"))?;
        progress(ModelDownloadProgress::Downloading {
            received,
            total: DEFAULT_MODEL_ARCHIVE_BYTES,
        });
    }
    output
        .sync_all()
        .map_err(|error| format!("Could not flush speech-model download: {error}"))?;
    let digest = format!("{:x}", hasher.finalize());
    if received != DEFAULT_MODEL_ARCHIVE_BYTES || digest != DEFAULT_MODEL_ARCHIVE_SHA256 {
        return Err("The downloaded speech model failed its pinned size or SHA-256 check".into());
    }
    Ok(())
}

fn extract_pinned_model(
    archive_path: &Path,
    destination: &Path,
    cancelled: &impl Fn() -> bool,
) -> Result<(), String> {
    let file = File::open(archive_path)
        .map_err(|error| format!("Could not open speech model: {error}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("Invalid speech-model archive: {error}"))?;
    if archive.len() > MAX_MODEL_FILES {
        return Err("Speech-model archive contains too many files".into());
    }
    let mut uncompressed = 0_u64;
    for index in 0..archive.len() {
        if cancelled() {
            return Err("Speech model installation was cancelled".into());
        }
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Invalid speech-model archive entry: {error}"))?;
        let Some(relative) = entry.enclosed_name() else {
            return Err("Speech-model archive contains an unsafe path".into());
        };
        if !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        {
            return Err("Speech-model archive contains an unsafe path".into());
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err("Speech-model archive contains an unexpected symbolic link".into());
        }
        let Some(first) = relative.components().next() else {
            return Err("Speech-model archive contains an empty path".into());
        };
        if first.as_os_str() != DEFAULT_MODEL_ID {
            return Err("Speech-model archive has an unexpected top-level folder".into());
        }
        let target = destination.join(&relative);
        if entry.is_dir() {
            fs::create_dir_all(target)
                .map_err(|error| format!("Could not create model folder: {error}"))?;
            continue;
        }
        uncompressed = uncompressed.saturating_add(entry.size());
        if uncompressed > MAX_MODEL_UNPACKED_BYTES {
            return Err("Speech-model archive expands beyond the allowed size".into());
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create model folder: {error}"))?;
        }
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)
            .map_err(|error| format!("Could not write speech model: {error}"))?;
        std::io::copy(&mut entry, &mut output)
            .map_err(|error| format!("Could not extract speech model: {error}"))?;
    }
    Ok(())
}

/// A command sent to the optional speech worker over newline-delimited JSON.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkerRequest {
    Start { session_id: u64 },
    Stop { session_id: u64 },
    Cancel { session_id: u64 },
    Shutdown,
}

/// Worker event sent to the editor over newline-delimited JSON.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkerEvent {
    pub protocol: u16,
    pub session_id: Option<u64>,
    #[serde(flatten)]
    pub event: WorkerEventKind,
}

impl WorkerEvent {
    pub fn new(session_id: Option<u64>, event: WorkerEventKind) -> Self {
        Self {
            protocol: SPEECH_PROTOCOL_VERSION,
            session_id,
            event,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkerEventKind {
    Ready,
    Listening,
    Partial { transcript: String, level: f32 },
    Stopped { transcript: String },
    Cancelled,
    Error { message: String },
}

/// Owns one sidecar speech session. The controller is process-only and never
/// links microphone or Vosk libraries into the application executable.
pub struct SpeechTask {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    events: Receiver<Result<WorkerEvent, String>>,
    session_id: u64,
    reader: Option<thread::JoinHandle<()>>,
}

impl SpeechTask {
    /// Start the selected worker after the user explicitly requests speech.
    /// Runtime paths and arguments come from the installer/detector, never UI
    /// string construction.
    pub fn spawn(runtime: &InstalledRuntime, session_id: u64) -> Result<Self, String> {
        Self::spawn_command(&runtime.worker_path, &runtime.args, session_id)
    }

    fn spawn_command(
        worker_path: &Path,
        worker_args: &[OsString],
        session_id: u64,
    ) -> Result<Self, String> {
        if !worker_path.is_file() {
            return Err("Offline speech worker is not installed with AlphaDocs".into());
        }
        let mut command = Command::new(worker_path);
        hide_console(&mut command);
        let mut child = command
            .args(worker_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("Could not start offline speech worker: {error}"))?;
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            return Err("Offline speech worker did not provide its event channel".into());
        };
        let (sender, events) = mpsc::sync_channel(WORKER_EVENT_QUEUE_CAPACITY);
        let reader = thread::Builder::new()
            .name("alphadocs-speech-events".into())
            .spawn(move || read_worker_events(stdout, sender))
            .map_err(|error| {
                let _ = child.kill();
                format!("Could not create speech event reader: {error}")
            })?;
        Ok(Self {
            stdin: child.stdin.take(),
            child: Some(child),
            events,
            session_id,
            reader: Some(reader),
        })
    }

    pub fn start(&mut self) -> Result<(), String> {
        self.send(WorkerRequest::Start {
            session_id: self.session_id,
        })
    }

    /// Ask the worker to finalize and return a commit-eligible transcript.
    pub fn stop(&mut self) -> Result<(), String> {
        self.send(WorkerRequest::Stop {
            session_id: self.session_id,
        })
    }

    /// Stop capture and discard its transcript.
    pub fn cancel(&mut self) -> Result<(), String> {
        self.send(WorkerRequest::Cancel {
            session_id: self.session_id,
        })
    }

    pub fn poll_event(&self) -> Result<Option<WorkerEvent>, String> {
        match self.events.try_recv() {
            Ok(Ok(event)) => {
                validate_worker_event(&event, self.session_id)?;
                Ok(Some(event))
            }
            Ok(Err(error)) => Err(error),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                Err("Offline speech worker event channel closed unexpectedly".into())
            }
        }
    }

    pub fn is_running(&mut self) -> Result<bool, String> {
        match self
            .child
            .as_mut()
            .ok_or_else(|| "Offline speech worker has already stopped".to_owned())?
            .try_wait()
            .map_err(|error| format!("Could not check offline speech worker: {error}"))?
        {
            None => Ok(true),
            Some(status) if status.success() => Ok(false),
            Some(status) => Err(format!("Offline speech worker exited with status {status}")),
        }
    }

    fn send(&mut self, request: WorkerRequest) -> Result<(), String> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| "Offline speech worker command channel is closed".to_owned())?;
        serde_json::to_writer(&mut *stdin, &request)
            .map_err(|error| format!("Could not send speech command: {error}"))?;
        stdin
            .write_all(b"\n")
            .and_then(|()| stdin.flush())
            .map_err(|error| format!("Could not send speech command: {error}"))
    }
}

impl Drop for SpeechTask {
    fn drop(&mut self) {
        self.stdin.take();
        if let Some(mut child) = self.child.take() {
            let _ = thread::Builder::new()
                .name("alphadocs-speech-reaper".into())
                .spawn(move || {
                    terminate_worker_tree(&mut child);
                });
        }
        // The event reader is intentionally detached. Killing the worker closes
        // stdout, and the bounded channel prevents it retaining unbounded data.
        self.reader.take();
        while self.events.try_recv().is_ok() {}
    }
}

#[cfg(windows)]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000);
}

#[cfg(not(windows))]
fn hide_console(_command: &mut Command) {}

fn terminate_worker_tree(child: &mut Child) {
    // Vosk runs entirely inside this sidecar; it creates no child processes.
    let _ = child.kill();
    let _ = child.wait();
}

fn read_worker_events(stdout: impl Read, sender: SyncSender<Result<WorkerEvent, String>>) {
    let mut reader = BufReader::new(stdout);
    loop {
        let mut line = Vec::with_capacity(512);
        let bytes = match reader
            .by_ref()
            .take((MAX_WORKER_EVENT_BYTES + 1) as u64)
            .read_until(b'\n', &mut line)
        {
            Ok(bytes) => bytes,
            Err(error) => {
                let _ = sender.send(Err(format!("Could not read speech worker event: {error}")));
                return;
            }
        };
        if bytes == 0 {
            return;
        }
        if bytes > MAX_WORKER_EVENT_BYTES {
            let _ = sender.send(Err("Speech worker event exceeded the size limit".into()));
            return;
        }
        let event = serde_json::from_slice::<WorkerEvent>(&line)
            .map_err(|error| format!("Invalid speech worker event: {error}"));
        if sender.send(event).is_err() {
            return;
        }
    }
}

fn validate_worker_event(event: &WorkerEvent, expected_session_id: u64) -> Result<(), String> {
    if event.protocol != SPEECH_PROTOCOL_VERSION {
        return Err(format!(
            "Speech worker protocol {} is unsupported (expected {})",
            event.protocol, SPEECH_PROTOCOL_VERSION
        ));
    }
    match (&event.event, event.session_id) {
        (WorkerEventKind::Ready, None) => Ok(()),
        (WorkerEventKind::Ready, Some(_)) => {
            Err("Speech worker Ready event had a session ID".into())
        }
        (WorkerEventKind::Error { .. }, None) => Ok(()),
        (_, Some(session_id)) if session_id == expected_session_id => Ok(()),
        (_, Some(_)) => Err("Speech worker event belonged to a stale session".into()),
        (_, None) => Err("Speech worker event did not include its session ID".into()),
    }
}

/// The editor state at which a voice session began.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpeechAnchor {
    pub session_id: u64,
    pub document_id: String,
    pub content_tab_id: String,
    pub revision: u64,
    pub caret: Position,
    pub selection_anchor: Position,
}

impl SpeechAnchor {
    /// Capture the current insertion point or selection. Read-only sessions are
    /// rejected before any microphone worker is launched.
    pub fn capture(session_id: u64, editor: &Editor, read_only: bool) -> Result<Self, AnchorError> {
        if read_only {
            return Err(AnchorError::ReadOnly);
        }
        if editor.table_selection().is_some() {
            return Err(AnchorError::TableSelection);
        }
        let Some(tab) = editor.doc.tabs.get(editor.doc.active_tab) else {
            return Err(AnchorError::InvalidDocumentState);
        };
        Ok(Self {
            session_id,
            document_id: editor.doc.id.clone(),
            content_tab_id: tab.id.clone(),
            revision: editor.revision,
            caret: editor.caret,
            selection_anchor: editor.anchor,
        })
    }

    /// True only while the original editable document, tab, revision and
    /// selection are still active. Any change invalidates the transcript.
    pub fn still_matches(&self, editor: &Editor, read_only: bool) -> bool {
        !read_only
            && editor.table_selection().is_none()
            && self.document_id == editor.doc.id
            && editor
                .doc
                .tabs
                .get(editor.doc.active_tab)
                .is_some_and(|tab| tab.id == self.content_tab_id)
            && self.revision == editor.revision
            && self.caret == editor.caret
            && self.selection_anchor == editor.anchor
    }

    /// Apply a final transcript through the editor's normal transaction path.
    /// Partial results must never be passed here.
    pub fn commit(
        &self,
        editor: &mut Editor,
        read_only: bool,
        transcript: &str,
    ) -> Result<(), AnchorError> {
        if !self.still_matches(editor, read_only) {
            return Err(if read_only {
                AnchorError::ReadOnly
            } else {
                AnchorError::Stale
            });
        }
        if transcript.trim().is_empty() {
            return Err(AnchorError::EmptyTranscript);
        }
        editor.insert(transcript);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnchorError {
    ReadOnly,
    TableSelection,
    InvalidDocumentState,
    Stale,
    EmptyTranscript,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Document;

    fn make_editor() -> Editor {
        Editor::new(Document::new("Speech anchor test"))
    }

    #[test]
    fn protocol_round_trips_partial_and_terminal_events() {
        let partial = WorkerEvent::new(
            Some(17),
            WorkerEventKind::Partial {
                transcript: "hello".into(),
                level: 0.25,
            },
        );
        let json = serde_json::to_string(&partial).unwrap();
        assert_eq!(serde_json::from_str::<WorkerEvent>(&json).unwrap(), partial);

        let stop = WorkerRequest::Stop { session_id: 17 };
        let json = serde_json::to_string(&stop).unwrap();
        assert_eq!(serde_json::from_str::<WorkerRequest>(&json).unwrap(), stop);
    }

    #[test]
    fn worker_events_reject_stale_sessions_and_protocol_versions() {
        let ready = WorkerEvent::new(None, WorkerEventKind::Ready);
        assert!(validate_worker_event(&ready, 17).is_ok());
        let startup_error = WorkerEvent::new(
            None,
            WorkerEventKind::Error {
                message: "model failed to load".into(),
            },
        );
        assert!(validate_worker_event(&startup_error, 17).is_ok());
        let matching = WorkerEvent::new(Some(17), WorkerEventKind::Listening);
        assert!(validate_worker_event(&matching, 17).is_ok());
        let stale = WorkerEvent::new(
            Some(16),
            WorkerEventKind::Stopped {
                transcript: "old".into(),
            },
        );
        assert!(validate_worker_event(&stale, 17).is_err());
        let missing_session = WorkerEvent::new(None, WorkerEventKind::Listening);
        assert!(validate_worker_event(&missing_session, 17).is_err());
        let mut incompatible = matching;
        incompatible.protocol += 1;
        assert!(validate_worker_event(&incompatible, 17).is_err());
    }

    #[test]
    fn worker_reader_bounds_event_memory_and_reports_malformed_json() {
        let oversized = vec![b'x'; MAX_WORKER_EVENT_BYTES + 5];
        let (sender, receiver) = mpsc::sync_channel(2);
        read_worker_events(std::io::Cursor::new(oversized), sender);
        assert!(
            receiver
                .try_recv()
                .unwrap()
                .unwrap_err()
                .contains("size limit")
        );

        let (sender, receiver) = mpsc::sync_channel(2);
        read_worker_events(std::io::Cursor::new(b"{invalid}\n"), sender);
        assert!(
            receiver
                .try_recv()
                .unwrap()
                .unwrap_err()
                .contains("Invalid speech worker event")
        );
    }

    #[test]
    fn final_transcript_is_an_undoable_editor_transaction() {
        let mut editor = make_editor();
        let anchor = SpeechAnchor::capture(4, &editor, false).unwrap();
        anchor
            .commit(&mut editor, false, "recognized text")
            .unwrap();
        assert_eq!(editor.doc.text(), "recognized text");
        editor.undo();
        assert_eq!(editor.doc.text(), "");
    }

    #[test]
    fn stale_document_tab_revision_selection_or_read_only_state_rejects_commit() {
        let mut editor = make_editor();
        let original = SpeechAnchor::capture(8, &editor, false).unwrap();
        editor.doc.id.push_str("-changed");
        assert_eq!(
            original.commit(&mut editor, false, "x"),
            Err(AnchorError::Stale)
        );

        let mut editor = make_editor();
        let original = SpeechAnchor::capture(9, &editor, false).unwrap();
        editor.add_tab();
        assert_eq!(
            original.commit(&mut editor, false, "x"),
            Err(AnchorError::Stale)
        );

        let mut editor = make_editor();
        let original = SpeechAnchor::capture(10, &editor, false).unwrap();
        editor.insert("revision changed");
        assert_eq!(
            original.commit(&mut editor, false, "x"),
            Err(AnchorError::Stale)
        );

        let mut editor = make_editor();
        let original = SpeechAnchor::capture(11, &editor, false).unwrap();
        editor.caret.offset = editor.caret.offset.saturating_add(1);
        assert_eq!(
            original.commit(&mut editor, false, "x"),
            Err(AnchorError::Stale)
        );

        let mut editor = make_editor();
        let fresh = SpeechAnchor::capture(12, &editor, false).unwrap();
        assert_eq!(
            fresh.commit(&mut editor, true, "x"),
            Err(AnchorError::ReadOnly)
        );
    }

    #[test]
    fn read_only_and_empty_transcripts_are_rejected_without_mutation() {
        let mut editor = make_editor();
        assert_eq!(
            SpeechAnchor::capture(1, &editor, true),
            Err(AnchorError::ReadOnly)
        );
        let anchor = SpeechAnchor::capture(2, &editor, false).unwrap();
        assert_eq!(
            anchor.commit(&mut editor, false, "  "),
            Err(AnchorError::EmptyTranscript)
        );
        assert_eq!(editor.doc.text(), "");
        assert_eq!(editor.revision, 0);
    }

    #[test]
    fn rectangular_table_selection_cannot_anchor_a_voice_insert() {
        let mut document = Document::new("Table selection speech anchor");
        document.blocks = vec![crate::model::Block::Table(vec![vec![
            crate::model::Paragraph::default(),
        ]])];
        let mut editor = Editor::new(document);
        let anchor = SpeechAnchor::capture(30, &editor, false).unwrap();
        assert!(editor.select_table_row(0, 0));
        assert_eq!(
            anchor.commit(&mut editor, false, "must not land in selected cells"),
            Err(AnchorError::Stale)
        );
        assert_eq!(
            SpeechAnchor::capture(31, &editor, false),
            Err(AnchorError::TableSelection)
        );
    }

    #[test]
    fn default_model_catalog_has_a_pinned_official_source() {
        assert_eq!(DEFAULT_ENGLISH_US_MODEL.id, "vosk-model-small-en-us-0.15");
        assert_eq!(DEFAULT_ENGLISH_US_MODEL.language, "English (United States)");
        assert_eq!(DEFAULT_ENGLISH_US_MODEL.archive_bytes, 41_205_931);
        assert_eq!(DEFAULT_ENGLISH_US_MODEL.sha256.len(), 64);
        assert!(
            DEFAULT_ENGLISH_US_MODEL
                .download_url
                .starts_with("https://alphacephei.com/")
        );
        assert_eq!(DEFAULT_ENGLISH_US_MODEL.license, "Apache-2.0");
    }

    #[test]
    fn local_model_validation_rejects_non_model_folders_without_io_side_effects() {
        let missing = PathBuf::from("speech-model-that-does-not-exist");
        assert!(validate_model_directory(&missing).is_err());
    }
}
