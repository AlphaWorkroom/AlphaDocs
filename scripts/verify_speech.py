"""Exercise explicit offline setup and PCM recognition without microphone access."""
from pathlib import Path
import os
import shutil
import subprocess
import sys
import tempfile
import wave

root = Path(__file__).resolve().parent.parent
suffix = ".exe" if sys.platform == "win32" else ""
examples = root / "target/release/examples"
examples.mkdir(parents=True, exist_ok=True)
worker = "alphadocs-speech" + suffix
shutil.copy2(root / "target/release" / worker, examples / worker)
destination = Path(tempfile.mkdtemp(prefix="alphadocs-speech-qa-", dir=os.environ.get("RUNNER_TEMP")))
subprocess.run(["cargo", "run", "--locked", "--release", "--example", "speech_setup_probe", "--", str(destination)], cwd=root, check=True)
runtimes = list(destination.glob("runtime-0.3.42-*"))
assert len(runtimes) == 1, "Setup did not create exactly one platform runtime"
silence = destination / "silence.wav"
with wave.open(str(silence), "wb") as output:
    output.setnchannels(1)
    output.setsampwidth(2)
    output.setframerate(16000)
    output.writeframes(b"\x00\x00" * 16000)
model = destination / "vosk-model-small-en-us-0.15"
result = subprocess.run([str(runtimes[0] / worker), "--transcribe-wav", str(silence), str(model)], capture_output=True, text=True, timeout=60)
if result.returncode:
    raise SystemExit(f"Offline speech runtime failed: {result.stderr[-2000:]}")
assert not result.stdout.strip(), "Silence unexpectedly produced words"
print("Pinned runtime/model setup and native PCM recognition passed; no microphone was opened")
