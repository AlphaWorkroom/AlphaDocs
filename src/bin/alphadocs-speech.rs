#![forbid(unsafe_code)]

use alphadocs::speech::{WorkerEvent, WorkerEventKind, WorkerRequest};
use cpal::{
    SampleFormat, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use std::{
    env,
    io::{self, BufRead, Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use vosk::{DecodingState, Model, Recognizer};

struct AudioBlock {
    pcm: Vec<i16>,
    level: f32,
}

struct RunningSession {
    session_id: u64,
    stream: Stream,
    recognizer: JoinHandle<Result<String, String>>,
}

const RECOGNIZER_SAMPLE_RATE: u32 = 16_000;

fn main() {
    if let Err(error) = run() {
        let _ = write_event(
            &Arc::new(Mutex::new(io::stdout())),
            WorkerEvent::new(None, WorkerEventKind::Error { message: error }),
        );
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|arg| arg == "--transcribe-wav") {
        if args.len() != 3 {
            return Err("Usage: alphadocs-speech --transcribe-wav <wav> <model-folder>".into());
        }
        let transcript = transcribe_wav(
            PathBuf::from(&args[1]).as_path(),
            PathBuf::from(&args[2]).as_path(),
        )?;
        println!("{transcript}");
        return Ok(());
    }
    if args.len() != 1 {
        return Err("Usage: alphadocs-speech <model-folder>".into());
    }
    let model_dir = PathBuf::from(&args[0]);
    if !model_dir.is_dir() {
        return Err("Speech model folder is missing".into());
    }

    let model = Model::new(model_dir.to_string_lossy().into_owned())
        .ok_or_else(|| "The selected offline speech model could not be loaded".to_owned())?;
    let output = Arc::new(Mutex::new(io::stdout()));
    write_event(&output, WorkerEvent::new(None, WorkerEventKind::Ready))
        .map_err(|error| error.to_string())?;

    let stdin = io::stdin();
    let mut active: Option<RunningSession> = None;
    for line in stdin.lock().lines() {
        let line = line.map_err(|error| error.to_string())?;
        let request: WorkerRequest = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(error) => {
                write_event(
                    &output,
                    WorkerEvent::new(
                        None,
                        WorkerEventKind::Error {
                            message: format!("Invalid speech command: {error}"),
                        },
                    ),
                )
                .map_err(|error| error.to_string())?;
                continue;
            }
        };

        match request {
            WorkerRequest::Start { session_id } => {
                if active.is_some() {
                    send_error(
                        &output,
                        Some(session_id),
                        "A voice session is already running",
                    )?;
                    continue;
                }
                match start_session(session_id, &model, output.clone()) {
                    Ok(session) => {
                        active = Some(session);
                        send_event(&output, Some(session_id), WorkerEventKind::Listening)?;
                    }
                    Err(error) => send_error(&output, Some(session_id), &error)?,
                }
            }
            WorkerRequest::Stop { session_id } => {
                let Some(session) = active.take() else {
                    send_error(&output, Some(session_id), "No voice session is running")?;
                    continue;
                };
                if session.session_id != session_id {
                    active = Some(session);
                    send_error(&output, Some(session_id), "Voice session ID did not match")?;
                    continue;
                }
                match stop_session(session, true) {
                    Ok(transcript) => send_event(
                        &output,
                        Some(session_id),
                        WorkerEventKind::Stopped { transcript },
                    )?,
                    Err(error) => send_error(&output, Some(session_id), &error)?,
                }
            }
            WorkerRequest::Cancel { session_id } => {
                let Some(session) = active.take() else {
                    send_event(&output, Some(session_id), WorkerEventKind::Cancelled)?;
                    continue;
                };
                if session.session_id != session_id {
                    active = Some(session);
                    send_error(&output, Some(session_id), "Voice session ID did not match")?;
                    continue;
                }
                let _ = stop_session(session, false);
                send_event(&output, Some(session_id), WorkerEventKind::Cancelled)?;
            }
            WorkerRequest::Shutdown => {
                if let Some(session) = active.take() {
                    let _ = stop_session(session, false);
                }
                break;
            }
        }
    }
    if let Some(session) = active.take() {
        let _ = stop_session(session, false);
    }
    Ok(())
}

fn start_session(
    session_id: u64,
    model: &Model,
    output: Arc<Mutex<io::Stdout>>,
) -> Result<RunningSession, String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "No microphone input device is available".to_owned())?;
    let supported = device
        .default_input_config()
        .map_err(|error| format!("Could not open microphone configuration: {error}"))?;
    let sample_rate = supported.sample_rate();
    let config: StreamConfig = supported.into();
    let channels = config.channels as usize;
    let (sender, receiver) = mpsc::sync_channel::<AudioBlock>(8);
    let recognizer = Recognizer::new(model, RECOGNIZER_SAMPLE_RATE as f32)
        .ok_or_else(|| "Could not initialize the speech recognizer".to_owned())?;
    let output_for_recognizer = output.clone();
    let recognition_thread = thread::Builder::new()
        .name("alphadocs-speech-recognition".into())
        .spawn(move || {
            let result = recognize(
                session_id,
                recognizer,
                receiver,
                output_for_recognizer.clone(),
                sample_rate,
            );
            if let Err(error) = &result {
                let _ = send_error(&output_for_recognizer, Some(session_id), error);
            }
            result
        })
        .map_err(|error| format!("Could not start speech recognition: {error}"))?;

    let stream_result = match supported.sample_format() {
        SampleFormat::F32 => build_stream::<f32>(
            &device,
            config,
            channels,
            sender,
            output.clone(),
            session_id,
        ),
        SampleFormat::I16 => build_stream::<i16>(
            &device,
            config,
            channels,
            sender,
            output.clone(),
            session_id,
        ),
        SampleFormat::U16 => {
            build_stream::<u16>(&device, config, channels, sender, output, session_id)
        }
        format => Err(format!("Microphone sample format {format} is unsupported")),
    };
    let stream = match stream_result {
        Ok(stream) => stream,
        Err(error) => {
            let _ = recognition_thread.join();
            return Err(format!("Could not start microphone input: {error}"));
        }
    };
    if let Err(error) = stream.play() {
        drop(stream);
        let _ = recognition_thread.join();
        return Err(format!("Could not start microphone input: {error}"));
    }
    Ok(RunningSession {
        session_id,
        stream,
        recognizer: recognition_thread,
    })
}

trait PcmSample: cpal::SizedSample + Copy {
    fn to_pcm16(self) -> i16;
}

impl PcmSample for f32 {
    fn to_pcm16(self) -> i16 {
        (self.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
    }
}

impl PcmSample for i16 {
    fn to_pcm16(self) -> i16 {
        self
    }
}

impl PcmSample for u16 {
    fn to_pcm16(self) -> i16 {
        (self as i32 - 32_768) as i16
    }
}

fn build_stream<T: PcmSample>(
    device: &cpal::Device,
    config: StreamConfig,
    channels: usize,
    sender: mpsc::SyncSender<AudioBlock>,
    output: Arc<Mutex<io::Stdout>>,
    session_id: u64,
) -> Result<Stream, String> {
    let data_callback = move |data: &[T], _: &cpal::InputCallbackInfo| {
        let channel_count = channels.max(1);
        let mut mono = Vec::with_capacity(data.len().div_ceil(channel_count));
        for frame in data.chunks_exact(channel_count) {
            let total: i32 = frame.iter().map(|sample| sample.to_pcm16() as i32).sum();
            mono.push((total / channel_count as i32) as i16);
        }
        if mono.is_empty() {
            return;
        }
        let mean_square = mono
            .iter()
            .map(|sample| {
                let normalized = *sample as f64 / i16::MAX as f64;
                normalized * normalized
            })
            .sum::<f64>()
            / mono.len() as f64;
        let block = AudioBlock {
            pcm: mono,
            level: mean_square.sqrt().clamp(0.0, 1.0) as f32,
        };
        let _ = sender.try_send(block);
    };
    let error_callback = move |error| {
        let _ = write_event(
            &output,
            WorkerEvent::new(
                Some(session_id),
                WorkerEventKind::Error {
                    message: format!("Microphone input stopped: {error}"),
                },
            ),
        );
    };
    device
        .build_input_stream::<T, _, _>(config, data_callback, error_callback, None)
        .map_err(|error| error.to_string())
}

fn recognize(
    session_id: u64,
    mut recognizer: Recognizer,
    receiver: mpsc::Receiver<AudioBlock>,
    output: Arc<Mutex<io::Stdout>>,
    source_rate: u32,
) -> Result<String, String> {
    let mut finalized = String::new();
    let mut last_preview = String::new();
    let mut last_level = -1.0;
    let mut last_sent = Instant::now();
    let mut resampler = LinearResampler::new(source_rate, RECOGNIZER_SAMPLE_RATE)?;
    let started = Instant::now();
    while let Ok(block) = receiver.recv() {
        if started.elapsed() > Duration::from_secs(30 * 60) || finalized.len() > 48_000 {
            return Err("Voice typing reached its session limit. Stop and start a new session for more text.".into());
        }
        let converted = resampler.push(&block.pcm);
        if !converted.is_empty() {
            let decoding_state = recognizer
                .accept_waveform(&converted)
                .map_err(|error| format!("Speech decoding failed: {error}"))?;
            if decoding_state == DecodingState::Finalized {
                let segment = recognizer
                    .result()
                    .single()
                    .map(|result| result.text.trim().to_owned())
                    .unwrap_or_default();
                append_transcript(&mut finalized, &segment);
            }
        }
        let partial = recognizer.partial_result().partial.trim().to_owned();
        let mut transcript = finalized.clone();
        append_transcript(&mut transcript, &partial);
        let time_to_update = last_sent.elapsed() >= Duration::from_millis(100);
        if transcript != last_preview
            || (time_to_update && (block.level - last_level).abs() >= 0.015)
        {
            write_event(
                &output,
                WorkerEvent::new(
                    Some(session_id),
                    WorkerEventKind::Partial {
                        transcript: transcript.clone(),
                        level: block.level,
                    },
                ),
            )
            .map_err(|error| error.to_string())?;
            last_preview = transcript;
            last_level = block.level;
            last_sent = Instant::now();
        }
    }
    let final_segment = recognizer
        .final_result()
        .single()
        .map(|result| result.text.trim().to_owned())
        .unwrap_or_default();
    append_transcript(&mut finalized, &final_segment);
    Ok(finalized)
}

struct LinearResampler {
    source_rate: u32,
    target_rate: u32,
    buffer: Vec<i16>,
    next_position: f64,
}

impl LinearResampler {
    fn new(source_rate: u32, target_rate: u32) -> Result<Self, String> {
        if source_rate == 0 || target_rate == 0 {
            return Err("Audio sample rate must be greater than zero".into());
        }
        Ok(Self {
            source_rate,
            target_rate,
            buffer: Vec::new(),
            next_position: 0.0,
        })
    }

    fn push(&mut self, samples: &[i16]) -> Vec<i16> {
        self.buffer.extend_from_slice(samples);
        let step = self.source_rate as f64 / self.target_rate as f64;
        let mut output = Vec::new();
        while self.next_position + 1.0 < self.buffer.len() as f64 {
            let left = self.next_position.floor() as usize;
            let fraction = (self.next_position - left as f64) as f32;
            let a = self.buffer[left] as f32;
            let b = self.buffer[left + 1] as f32;
            output.push((a + (b - a) * fraction).round() as i16);
            self.next_position += step;
        }
        let consumed =
            (self.next_position.floor() as usize).min(self.buffer.len().saturating_sub(1));
        if consumed > 0 {
            self.buffer.drain(..consumed);
            self.next_position -= consumed as f64;
        }
        output
    }
}

fn read_pcm16_wav(path: &std::path::Path) -> Result<(u32, Vec<i16>), String> {
    let file =
        std::fs::File::open(path).map_err(|error| format!("Could not open WAV file: {error}"))?;
    let mut bytes = Vec::new();
    file.take(64_000_001)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read WAV file: {error}"))?;
    if bytes.len() > 64_000_000 {
        return Err("Speech test WAV exceeds the 64 MB limit".into());
    }
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("Speech test input is not a RIFF/WAVE file".into());
    }
    let mut offset = 12;
    let mut format = None;
    let mut data = None;
    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let start = offset + 8;
        let end = start
            .checked_add(size)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| "Speech test WAV has a truncated chunk".to_owned())?;
        if id == b"fmt " {
            if size < 16 {
                return Err("Speech test WAV format chunk is too short".into());
            }
            format = Some((
                u16::from_le_bytes(bytes[start..start + 2].try_into().unwrap()),
                u16::from_le_bytes(bytes[start + 2..start + 4].try_into().unwrap()),
                u32::from_le_bytes(bytes[start + 4..start + 8].try_into().unwrap()),
                u16::from_le_bytes(bytes[start + 14..start + 16].try_into().unwrap()),
            ));
        } else if id == b"data" {
            data = Some(&bytes[start..end]);
        }
        offset = end + (size & 1);
    }
    let Some((encoding, channels, sample_rate, bits_per_sample)) = format else {
        return Err("Speech test WAV has no format chunk".into());
    };
    if encoding != 1 || bits_per_sample != 16 || channels == 0 {
        return Err("Speech test WAV must contain PCM 16-bit audio".into());
    }
    let pcm = data.ok_or_else(|| "Speech test WAV has no audio data".to_owned())?;
    let channel_count = channels as usize;
    let mut mono = Vec::with_capacity(pcm.len() / 2 / channel_count);
    for frame in pcm.chunks_exact(2 * channel_count) {
        let sum = frame
            .as_chunks::<2>()
            .0
            .iter()
            .map(|sample| i16::from_le_bytes([sample[0], sample[1]]) as i32)
            .sum::<i32>();
        mono.push((sum / channel_count as i32) as i16);
    }
    if mono.is_empty() {
        return Err("Speech test WAV contains no complete audio frames".into());
    }
    Ok((sample_rate, mono))
}

fn transcribe_wav(
    wav_path: &std::path::Path,
    model_path: &std::path::Path,
) -> Result<String, String> {
    let model = Model::new(model_path.to_string_lossy().into_owned())
        .ok_or_else(|| "The selected offline speech model could not be loaded".to_owned())?;
    let (source_rate, pcm) = read_pcm16_wav(wav_path)?;
    let mut resampler = LinearResampler::new(source_rate, RECOGNIZER_SAMPLE_RATE)?;
    let converted = resampler.push(&pcm);
    let mut recognizer = Recognizer::new(&model, RECOGNIZER_SAMPLE_RATE as f32)
        .ok_or_else(|| "Could not initialize the speech recognizer".to_owned())?;
    let mut transcript = String::new();
    for chunk in converted.chunks(4096) {
        if recognizer
            .accept_waveform(chunk)
            .map_err(|error| format!("Speech decoding failed: {error}"))?
            == DecodingState::Finalized
        {
            let segment = recognizer
                .result()
                .single()
                .map(|result| result.text.trim().to_owned())
                .unwrap_or_default();
            append_transcript(&mut transcript, &segment);
        }
    }
    let final_segment = recognizer
        .final_result()
        .single()
        .map(|result| result.text.trim().to_owned())
        .unwrap_or_default();
    append_transcript(&mut transcript, &final_segment);
    Ok(transcript)
}

fn append_transcript(target: &mut String, segment: &str) {
    if segment.is_empty() {
        return;
    }
    if !target.is_empty()
        && !target.chars().last().is_some_and(char::is_whitespace)
        && !segment.chars().next().is_some_and(char::is_whitespace)
    {
        target.push(' ');
    }
    target.push_str(segment);
}

fn stop_session(session: RunningSession, keep_transcript: bool) -> Result<String, String> {
    drop(session.stream);
    let transcript = session
        .recognizer
        .join()
        .map_err(|_| "Speech recognition worker stopped unexpectedly".to_owned())??;
    Ok(if keep_transcript {
        transcript
    } else {
        String::new()
    })
}

fn send_error(
    output: &Arc<Mutex<io::Stdout>>,
    session_id: Option<u64>,
    message: &str,
) -> Result<(), String> {
    send_event(
        output,
        session_id,
        WorkerEventKind::Error {
            message: message.to_owned(),
        },
    )
}

fn send_event(
    output: &Arc<Mutex<io::Stdout>>,
    session_id: Option<u64>,
    event: WorkerEventKind,
) -> Result<(), String> {
    write_event(output, WorkerEvent::new(session_id, event)).map_err(|error| error.to_string())
}

fn write_event(output: &Arc<Mutex<io::Stdout>>, event: WorkerEvent) -> io::Result<()> {
    if let Ok(mut output) = output.lock() {
        serde_json::to_writer(&mut *output, &event)?;
        output.write_all(b"\n")?;
        output.flush()
    } else {
        Err(io::Error::other("speech protocol output is unavailable"))
    }
}

#[cfg(test)]
mod tests {
    use super::{LinearResampler, append_transcript, read_pcm16_wav};
    use std::{fs, path::PathBuf};

    #[test]
    fn transcript_segments_join_without_losing_existing_whitespace() {
        let mut transcript = String::from("hello");
        append_transcript(&mut transcript, "world");
        assert_eq!(transcript, "hello world");
        append_transcript(&mut transcript, "");
        assert_eq!(transcript, "hello world");
        append_transcript(&mut transcript, " next");
        assert_eq!(transcript, "hello world next");
    }

    #[test]
    fn resampler_preserves_chunk_boundaries_and_converts_44100_to_16000() {
        let input = vec![1200_i16; 44_100];
        let mut resampler = LinearResampler::new(44_100, 16_000).unwrap();
        let mut output = Vec::new();
        for chunk in input.chunks(37) {
            output.extend(resampler.push(chunk));
        }
        assert!((15_999..=16_000).contains(&output.len()));
        assert!(output.iter().all(|sample| *sample == 1200));
        assert!(LinearResampler::new(0, 16_000).is_err());
    }

    #[test]
    fn wav_reader_downmixes_pcm16_and_rejects_truncated_input() {
        let mut bytes = Vec::from(&b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x02\0\x80\xBB\0\0\0\xEE\x02\0\x04\0\x10\0data\x04\0\0\0"[..]);
        bytes.extend_from_slice(&1000_i16.to_le_bytes());
        bytes.extend_from_slice(&(-1000_i16).to_le_bytes());
        let size = (bytes.len() - 8) as u32;
        bytes[4..8].copy_from_slice(&size.to_le_bytes());
        let path = std::env::temp_dir().join(format!(
            "alphadocs-speech-wav-{}-{}.wav",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&path, &bytes).unwrap();
        let parsed = read_pcm16_wav(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!(parsed, (48_000, vec![0]));

        let bad_path = PathBuf::from("missing-speech-test.wav");
        assert!(read_pcm16_wav(&bad_path).is_err());
    }
}
