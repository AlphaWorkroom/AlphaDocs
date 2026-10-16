"""Build Linux DEB/tar or macOS app/DMG/legacy PKG using native host tools."""
from pathlib import Path
import hashlib
import json
import os
import platform
import plistlib
import re
import shutil
import subprocess
import sys
import tarfile

root = Path(__file__).resolve().parent.parent
version = re.search(r'^version = "([0-9.]+)"', (root / "Cargo.toml").read_text(), re.M)[1]
out = root / "releases"
out.mkdir(exist_ok=True)
work = root / "target/package"
work.mkdir(parents=True, exist_ok=True)
arch = "aarch64" if platform.machine() in ("arm64", "aarch64") else "x86_64"
outputs = []

if sys.platform == "darwin":
    payload = work / "macos-payload"
    app = payload / "AlphaDocs.app"
    macos = app / "Contents/MacOS"
    resources = app / "Contents/Resources"
    macos.mkdir(parents=True, exist_ok=True)
    resources.mkdir(parents=True, exist_ok=True)
    shutil.copy2(root / "dist/alphadocs", macos / "alphadocs")
    os.chmod(macos / "alphadocs", 0o755)
    speech_files = ("alphadocs-speech",)
    if any(not (root / "dist" / name).is_file() for name in speech_files):
        raise SystemExit("Speech-enabled macOS package is missing its native sidecar")
    for name in speech_files:
        shutil.copy2(root / "dist" / name, macos / name)
        os.chmod(macos / name, 0o755)
    shutil.copytree(root / "dist", resources / "package", dirs_exist_ok=True, ignore=shutil.ignore_patterns("alphadocs"))
    icons = work / "AlphaDocs.iconset"
    icons.mkdir(exist_ok=True)
    for size in (16, 32, 128, 256, 512):
        for scale in (1, 2):
            name = f"icon_{size}x{size}{'@2x' if scale == 2 else ''}.png"
            subprocess.run(["sips", "-z", str(size * scale), str(size * scale), str(root / "assets/alphadocs-icon.png"), "--out", str(icons / name)], check=True, stdout=subprocess.DEVNULL)
    subprocess.run(["iconutil", "-c", "icns", str(icons), "-o", str(resources / "AlphaDocs.icns")], check=True)
    info = {"CFBundleName": "AlphaDocs", "CFBundleDisplayName": "AlphaDocs", "CFBundleIdentifier": "studio.alphaworkroom.alphadocs", "CFBundleExecutable": "alphadocs", "CFBundleIconFile": "AlphaDocs.icns", "CFBundlePackageType": "APPL", "CFBundleShortVersionString": version, "CFBundleVersion": version, "LSMinimumSystemVersion": "14.0", "NSHighResolutionCapable": True,
            "NSMicrophoneUsageDescription": "AlphaDocs uses the microphone only while you explicitly start offline speech input.",
            "CFBundleDocumentTypes": [{"CFBundleTypeName": "AlphaDocs document", "CFBundleTypeExtensions": ["adocs"], "CFBundleTypeRole": "Editor", "LSHandlerRank": "Owner"}]}
    (app / "Contents/Info.plist").write_bytes(plistlib.dumps(info))
    subprocess.run(["codesign", "--force", "--deep", "--sign", "-", str(app)], check=True)
    from macos_dmg import build as build_dmg
    outputs.append(build_dmg(app, out / f"AlphaDocs-macOS-{arch}.dmg"))
    # A relocatable component can silently "install" into the build-tree copy
    # found by Spotlight instead of /Applications. Pin the intended location.
    components = work / "components.plist"
    subprocess.run(["pkgbuild", "--analyze", "--root", str(payload), str(components)], check=True)
    entries = plistlib.loads(components.read_bytes())
    for entry in entries:
        entry["BundleIsRelocatable"] = False
    components.write_bytes(plistlib.dumps(entries))
    pkg = out / f"AlphaDocs-macOS-{arch}.pkg"
    subprocess.run(["pkgbuild", "--root", str(payload), "--component-plist", str(components), "--install-location", "/Applications", "--identifier", "studio.alphaworkroom.alphadocs", "--version", version, str(pkg)], check=True)
    subprocess.run([str(macos / "alphadocs"), "--self-check"], check=True)
    outputs.append(pkg)
    portable = out / f"AlphaDocs-macOS-{arch}.tar.gz"
    with tarfile.open(portable, "w:gz") as archive: archive.add(app, arcname="AlphaDocs.app")
    outputs.append(portable)
elif sys.platform == "linux":
    package = work / "deb"
    app = package / "opt/alphadocs"
    shutil.copytree(root / "dist", app, dirs_exist_ok=True)
    os.chmod(app / "alphadocs", 0o755)
    share = package / "usr/share"
    (share / "applications").mkdir(parents=True, exist_ok=True)
    (share / "applications/alphadocs.desktop").write_text("[Desktop Entry]\nType=Application\nName=AlphaDocs\nComment=Offline document editor by Alpha Workroom\nExec=/opt/alphadocs/alphadocs %f\nIcon=alphadocs\nTerminal=false\nCategories=Office;WordProcessor;\nMimeType=application/x-alphadocs;\n", encoding="utf-8")
    (share / "mime/packages").mkdir(parents=True, exist_ok=True)
    (share / "mime/packages/alphadocs.xml").write_text('<?xml version="1.0" encoding="UTF-8"?><mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info"><mime-type type="application/x-alphadocs"><comment>AlphaDocs document</comment><glob pattern="*.adocs"/></mime-type></mime-info>', encoding="utf-8")
    icon = share / "icons/hicolor/512x512/apps/alphadocs.png"
    icon.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(root / "assets/alphadocs-icon.png", icon)
    control = package / "DEBIAN"
    control.mkdir(exist_ok=True)
    (control / "control").write_text(f"Package: alphadocs\nVersion: {version}\nArchitecture: amd64\nMaintainer: Alpha Workroom\nSection: editors\nPriority: optional\nDepends: libc6 (>= 2.35), libgcc-s1, libstdc++6, libasound2, libfontconfig1, libx11-6, libx11-xcb1, libxcursor1, libxrandr2, libxi6, libxcb1, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libwayland-cursor0, libwayland-egl1, libegl1, libgl1\nRecommends: xdg-desktop-portal, xdg-desktop-portal-gtk\nHomepage: https://github.com/AlphaWorkroom/AlphaDocs\nDescription: Offline native document editor by Alpha Workroom\n", encoding="utf-8")
    deb = out / "AlphaDocs-Linux-x86_64.deb"
    subprocess.run(["dpkg-deb", "--root-owner-group", "--build", str(package), str(deb)], check=True)
    outputs.append(deb)
    portable = out / "AlphaDocs-Linux-x86_64.tar.gz"
    with tarfile.open(portable, "w:gz") as archive: archive.add(app, arcname="AlphaDocs")
    outputs.append(portable)
else:
    raise SystemExit("Use build_windows_package.py on Windows")
report = {}
for path in outputs:
    assert path.stat().st_size < 100_000_000, "Download exceeds 100 MB limit"
    report[path.name] = {"bytes": path.stat().st_size, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
(out / f"checksums-{sys.platform}-{arch}.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
print(json.dumps(report, indent=2))
