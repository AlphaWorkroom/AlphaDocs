"""Assemble native release material from source and resolved Cargo notices."""
from pathlib import Path
import json
import shutil
import subprocess
import sys
import zipfile

root = Path(__file__).resolve().parent.parent
dist = root / "dist"
dist.mkdir(exist_ok=True)
binary = "alphadocs.exe" if sys.platform == "win32" else "alphadocs"
shutil.copyfile(root / "target/release" / binary, dist / ("AlphaDocs.exe" if sys.platform == "win32" else binary))
for source, name in [("LICENSE", "LICENSE"), ("LICENSE", "READ ME - LICENSE.txt"), ("README.md", "README.md"),
                     ("assets/alphadocs-logo.png", "alphadocs-logo.png"), ("assets/alpha-workroom-logo.png", "alpha-workroom-logo.png"),
                     ("assets/alphadocs.ico", "AlphaDocs.ico"), ("assets/BRANDING.txt", "BRANDING.txt")]:
    shutil.copyfile(root / source, dist / name)
for path in (root / "licenses").iterdir():
    if path.is_file(): shutil.copyfile(path, dist / path.name)
# The separately shipped speech worker enables optional Rust dependencies that
# the default editor does not link. Its notices must be packaged as well.
metadata = subprocess.check_output(["cargo", "metadata", "--locked", "--all-features", "--format-version", "1"], cwd=root)
path = root / "target/package-metadata.json"
path.write_bytes(metadata)
subprocess.run([sys.executable, str(root / "scripts/package_licences.py"), str(path)], check=True)
with zipfile.ZipFile(dist / "third-party-sources/egui-winit-modified.zip", "w", zipfile.ZIP_DEFLATED) as archive:
    for path in sorted((root / "vendor/egui-winit").rglob("*")):
        if path.is_file(): archive.write(path, path.relative_to(root / "vendor").as_posix())
print("Prepared native binary, supplied licence, dependency notices and source archives")
