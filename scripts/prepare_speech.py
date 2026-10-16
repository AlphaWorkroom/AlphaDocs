"""Fetch pinned Vosk native libraries and stage the optional speech worker."""
from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path, PurePosixPath
import platform
import shutil
import sys
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent.parent
VERSION = "0.3.42"
RELEASE = f"https://github.com/alphacep/vosk-api/releases/download/v{VERSION}"
MAX_ASSET_BYTES = 20_000_000
MAX_STAGED_RUNTIME_BYTES = 80_000_000

# Official Vosk GitHub release asset hashes verified from the exact release
# archives. GitHub's release API does not publish digests for these files.
ASSETS = {
    ("linux", "x86_64"): (
        "vosk-linux-x86_64-0.3.42.zip",
        "70480495011a29f957c1194cd460449ef7de8c17ea000e387ddb13fd7f844d42",
        ("libvosk.so",),
    ),
    ("win32", "amd64"): (
        "vosk-win64-0.3.42.zip",
        "9a63e42bd970343041d19e784e545228d3f4703ccec9f2eb1ccc6d5e96c170c3",
        ("libvosk.dll", "libvosk.lib", "libstdc++-6.dll", "libgcc_s_seh-1.dll", "libwinpthread-1.dll"),
    ),
    ("darwin", "x86_64"): (
        "vosk-osx-0.3.42.zip",
        "65395f196c9d0583d79949142b25560acaf9c295f36284e18433097f3adb0ea1",
        ("libvosk.dylib",),
    ),
    ("darwin", "arm64"): (
        "vosk-osx-0.3.42.zip",
        "65395f196c9d0583d79949142b25560acaf9c295f36284e18433097f3adb0ea1",
        ("libvosk.dylib",),
    ),
    ("darwin", "aarch64"): (
        "vosk-osx-0.3.42.zip",
        "65395f196c9d0583d79949142b25560acaf9c295f36284e18433097f3adb0ea1",
        ("libvosk.dylib",),
    ),
}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_archive_path(name: str) -> PurePosixPath:
    path = PurePosixPath(name)
    if path.is_absolute() or not path.parts or any(part in ("", ".", "..") for part in path.parts):
        raise ValueError(f"Unsafe path in native speech archive: {name!r}")
    if "\\" in name or ":" in name:
        raise ValueError(f"Unsafe path in native speech archive: {name!r}")
    return path


def host_key() -> tuple[str, str]:
    machine = platform.machine().lower()
    if sys.platform == "win32" and machine in ("amd64", "x86_64"):
        machine = "amd64"
    return sys.platform, machine


def asset_for_host() -> tuple[str, str, tuple[str, ...]]:
    key = host_key()
    try:
        return ASSETS[key]
    except KeyError as error:
        raise SystemExit(f"No pinned Vosk binary is available for {key}; speech worker cannot be packaged") from error


def prepare_native(destination: Path) -> Path:
    key = host_key()
    name, expected_hash, _ = asset_for_host()

    archive_dir = destination / "archives"
    library_dir = destination / "lib"
    archive_dir.mkdir(parents=True, exist_ok=True)
    library_dir.mkdir(parents=True, exist_ok=True)
    archive = archive_dir / name
    if not archive.exists() or sha256(archive) != expected_hash:
        temporary = archive.with_suffix(archive.suffix + ".download")
        request = urllib.request.Request(f"{RELEASE}/{name}", headers={"User-Agent": "AlphaDocs speech backend build"})
        with urllib.request.urlopen(request, timeout=60) as response, temporary.open("wb") as output:
            size = 0
            while chunk := response.read(1024 * 1024):
                size += len(chunk)
                if size > MAX_ASSET_BYTES:
                    raise SystemExit(f"Vosk runtime archive exceeds the {MAX_ASSET_BYTES}-byte build limit")
                output.write(chunk)
        if sha256(temporary) != expected_hash:
            temporary.unlink(missing_ok=True)
            raise SystemExit(f"Vosk runtime archive digest mismatch for {name}")
        temporary.replace(archive)
    if archive.stat().st_size > MAX_ASSET_BYTES or sha256(archive) != expected_hash:
        raise SystemExit(f"Vosk runtime archive failed pinned size/hash validation: {name}")

    _, _, expected_files = asset_for_host()
    extracted: dict[str, bytes] = {}
    with zipfile.ZipFile(archive) as zipped:
        for item in zipped.infolist():
            safe_archive_path(item.filename)
            mode = item.external_attr >> 16
            if mode & 0o170000 == 0o120000:
                raise SystemExit(f"Vosk archive contains an unexpected symbolic link: {item.filename}")
            if item.is_dir():
                continue
            if item.file_size > MAX_STAGED_RUNTIME_BYTES:
                raise SystemExit(f"Vosk native file exceeds the build limit: {item.filename}")
            filename = PurePosixPath(item.filename).name
            if filename in expected_files:
                extracted[filename] = zipped.read(item)
    missing = set(expected_files) - extracted.keys()
    if missing:
        raise SystemExit(f"Pinned Vosk archive is missing native files: {sorted(missing)}")

    for filename, data in extracted.items():
        target = library_dir / filename
        temporary = target.with_suffix(target.suffix + ".tmp")
        temporary.write_bytes(data)
        temporary.replace(target)
    write_github_environment(library_dir)
    return library_dir


def write_github_environment(library_dir: Path) -> None:
    env_file = os.environ.get("GITHUB_ENV")
    if env_file:
        with Path(env_file).open("a", encoding="utf-8") as output:
            output.write(f"VOSK_LIB_DIR={library_dir.resolve()}\n")
            if sys.platform == "linux":
                output.write(f"LD_LIBRARY_PATH={library_dir.resolve()}:$LD_LIBRARY_PATH\n")
            elif sys.platform == "darwin":
                output.write(f"DYLD_LIBRARY_PATH={library_dir.resolve()}:$DYLD_LIBRARY_PATH\n")
    path_file = os.environ.get("GITHUB_PATH")
    if path_file and sys.platform == "win32":
        with Path(path_file).open("a", encoding="utf-8") as output:
            output.write(f"{library_dir.resolve()}\n")


def stage_worker(native_dir: Path, dist: Path, target: Path) -> None:
    asset_for_host()
    executable_name = "alphadocs-speech.exe" if sys.platform == "win32" else "alphadocs-speech"
    staged_name = "AlphaDocs-Speech.exe" if sys.platform == "win32" else executable_name
    executable = target / "release" / executable_name
    if not executable.is_file():
        raise SystemExit(f"Build the speech worker first; missing {executable}")
    dist.mkdir(parents=True, exist_ok=True)
    shutil.copy2(executable, dist / staged_name)
    # Runtime binaries are fetched directly from upstream only on explicit
    # first-use setup, not redistributed in the AlphaDocs installer.
    print(f"Staged native speech worker ({(dist / staged_name).stat().st_size} bytes); runtime is a first-use download")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-dir", type=Path, default=ROOT / "target/speech-native/vosk-0.3.42")
    parser.add_argument("--prepare-native", action="store_true", help="download and validate pinned native libraries for this build host")
    parser.add_argument("--stage", action="store_true", help="copy the already-built native worker into dist")
    parser.add_argument("--dist", type=Path, default=ROOT / "dist")
    parser.add_argument("--target-dir", type=Path, default=ROOT / "target")
    args = parser.parse_args()
    if not args.prepare_native and not args.stage:
        parser.error("choose --prepare-native, --stage, or both")
    native_dir = prepare_native(args.native_dir) if args.prepare_native else args.native_dir / "lib"
    if args.stage:
        stage_worker(native_dir.parent, args.dist, args.target_dir)


if __name__ == "__main__":
    main()
