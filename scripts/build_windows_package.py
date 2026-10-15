"""Build a portable ZIP and per-user Windows installer from the verified dist only."""
from pathlib import Path
import hashlib
import json
import os
import re
import subprocess
import zipfile

root = Path(__file__).resolve().parent.parent
dist = root / "dist"
out = root / "releases"
out.mkdir(exist_ok=True)
portable = out / "AlphaDocs-Windows-x86_64-Portable.zip"
with zipfile.ZipFile(portable, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
    for path in sorted(dist.rglob("*")):
        if path.is_file():
            archive.write(path, path.relative_to(dist).as_posix())
with zipfile.ZipFile(portable) as archive:
    assert archive.testzip() is None
    assert archive.read("LICENSE") == (root / "LICENSE").read_bytes()
    assert archive.read("AlphaDocs.exe") == (dist / "AlphaDocs.exe").read_bytes()
compiler = Path(os.environ["WINDIR"]) / "Microsoft.NET/Framework64/v4.0.30319/csc.exe"
installer = out / "AlphaDocs-Windows-x86_64-Setup.exe"
version = re.search(r'^version = "([0-9]+[.][0-9]+[.][0-9]+)"', (root / "Cargo.toml").read_text(), re.M)[1]
metadata = root / "target/installer-version.cs"
metadata.parent.mkdir(exist_ok=True)
metadata.write_text('[assembly:System.Reflection.AssemblyVersion("' + version + '.0")]\n', encoding="utf-8")
subprocess.run([
    str(compiler), "/nologo", "/target:winexe", "/optimize+",
    "/reference:System.Windows.Forms.dll", "/reference:System.Drawing.dll",
    "/reference:System.IO.Compression.dll", "/reference:System.IO.Compression.FileSystem.dll",
    "/win32icon:" + str(root / "assets/alphadocs.ico"),
    "/resource:" + str(portable) + ",AlphaDocs.Package.zip",
    "/out:" + str(installer), str(root / "scripts/windows_installer.cs"), str(metadata),
], check=True)
report = {}
for path in [portable, installer, dist / "AlphaDocs.exe"]:
    assert path.stat().st_size < 100_000_000, "Package exceeds the requested 100 MB limit"
    report[str(path.relative_to(root))] = {
        "bytes": path.stat().st_size,
        "sha256": hashlib.file_digest(path.open("rb"), "sha256").hexdigest()
        if hasattr(hashlib, "file_digest") else hashlib.sha256(path.read_bytes()).hexdigest(),
    }
(out / "checksums.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
print(json.dumps(report, indent=2))
