"""Create and verify the native drag-to-Applications DMG on a macOS host.

Does not alter/re-sign the app, remove quarantine, or bypass Gatekeeper.
The full installed payload, including licence/notices, must match the source.
"""
from pathlib import Path
import argparse
import hashlib
import json
import os
import platform
import plistlib
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent


def fingerprint(directory):
    result = {}
    for path in sorted(directory.rglob("*")):
        name = path.relative_to(directory).as_posix()
        if path.is_symlink():
            result[name] = {"link": os.readlink(path)}
        elif path.is_file():
            result[name] = {"sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                            "executable": bool(path.stat().st_mode & 0o111)}
    return result


def verify(dmg, source, temporary):
    subprocess.run(["hdiutil", "verify", str(dmg)], check=True)
    mount = temporary / "mounted"
    mount.mkdir()
    subprocess.run(["hdiutil", "attach", "-readonly", "-nobrowse", "-mountpoint", str(mount), str(dmg)], check=True)
    try:
        assert (mount / "Applications").is_symlink()
        assert os.readlink(mount / "Applications") == "/Applications"
        assert (mount / ".DS_Store").is_file(), "Finder layout missing"
        assert "Open Anyway" in (mount / "Read Me.txt").read_text(encoding="utf-8")
        assert (mount / "Licence.txt").read_bytes() == (source / "Contents/Resources/package/LICENSE").read_bytes()
        assert fingerprint(mount / "AlphaDocs.app") == fingerprint(source), "DMG changed app payload"
        # Reproduce Finder's drag-copy into a temporary Applications directory.
        # Never install over an existing app or read a real user profile.
        installed = temporary / "Applications/AlphaDocs.app"
        installed.parent.mkdir()
        subprocess.run(["ditto", str(mount / "AlphaDocs.app"), str(installed)], check=True)
        assert fingerprint(installed) == fingerprint(source), "Installed payload differs"
        subprocess.run(["codesign", "--verify", "--deep", "--strict", str(installed)], check=True)
        subprocess.run([str(installed / "Contents/MacOS/alphadocs"), "--self-check"], check=True,
                       env=dict(os.environ, ALPHADOCS_DATA_DIR=str(temporary / "profile")))
    finally:
        subprocess.run(["hdiutil", "detach", str(mount)], check=True)


def build(app, output):
    if sys.platform != "darwin":
        raise RuntimeError("DMG creation and verification require macOS")
    import dmgbuild
    app = app.resolve()
    output = output.resolve()
    info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
    assert info["CFBundleIdentifier"] == "studio.alphaworkroom.alphadocs"
    assert app.name == "AlphaDocs.app"
    binary = app / "Contents/MacOS/alphadocs"
    native_arch = "arm64" if platform.machine() in ("arm64", "aarch64") else "x86_64"
    assert native_arch in subprocess.check_output(["lipo", "-archs", str(binary)], text=True).split()
    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(app)], check=True)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="alphadocs-dmg-") as directory:
        temporary = Path(directory)
        readme = temporary / "Read Me.txt"
        readme.write_text(
            f"AlphaDocs {info['CFBundleShortVersionString']} by Alpha Workroom\n\n"
            "INSTALL\nDrag AlphaDocs.app onto Applications, then eject this disk image.\n"
            "Launch AlphaDocs from Applications. macOS 14 or newer is required.\n\n"
            "FIRST OPEN: UNIDENTIFIED DEVELOPER\n"
            "This build is ad-hoc signed, not Developer ID signed or Apple-notarised.\n"
            "The DMG packaging does not remove Apple's publisher warning.\n"
            "After downloading from AlphaWorkroom/AlphaDocs on GitHub and trying to open\n"
            "the installed app, if you trust this source, go to System Settings >\n"
            "Privacy & Security > Open Anyway for AlphaDocs, then confirm Open.\n"
            "This is an exception for this app; do not disable Gatekeeper globally.\n"
            "Apple's instructions: https://support.apple.com/en-us/102445\n\n"
            "MAC CONTROLS\nCommand+C/V/X/A: copy, paste, cut, select all.\n"
            "Command+Z / Command+Shift+Z: undo / redo. Command+S: save.\n"
            "Option+Left/Right: words. Command+Left/Right: start/end of line.\n"
            "Command+Up/Down: document start/end. Add Shift to select.\n"
            "Control-click or two-finger trackpad click: context menu.\n\n"
            "LICENCES\nThe supplied first-party licence is in Licence.txt. All original\n"
            "dependency/font notices and required sources remain inside the app bundle.\n",
            encoding="utf-8")
        licence = temporary / "Licence.txt"
        licence.write_bytes((app / "Contents/Resources/package/LICENSE").read_bytes())
        dmgbuild.build_dmg(str(output), "AlphaDocs", settings={
            "format": "UDZO", "compression_level": 9, "filesystem": "HFS+",
            "files": [str(app), str(readme), str(licence)],
            "symlinks": {"Applications": "/Applications"},
            "icon": str(app / "Contents/Resources/AlphaDocs.icns"),
            "background": str(ROOT / "assets/dmg/install-background.png"),
            "window_rect": ((180, 140), (640, 480)),
            "default_view": "icon-view", "include_icon_view_settings": True,
            "icon_size": 96, "text_size": 14, "grid_spacing": 80,
            "icon_locations": {"AlphaDocs.app": (140, 190), "Applications": (500, 190),
                               "Read Me.txt": (220, 370), "Licence.txt": (420, 370)},
            "show_toolbar": False, "show_sidebar": False, "show_status_bar": False,
        })
        assert output.stat().st_size < 100_000_000, "DMG exceeds 100 MB"
        verify(output, app, temporary)
    report = {"file": output.name, "bytes": output.stat().st_size,
              "sha256": hashlib.sha256(output.read_bytes()).hexdigest(),
              "version": info["CFBundleShortVersionString"], "architecture": native_arch,
              "payloadPreserved": True, "dragCopyVerified": True,
              "signature": "ad-hoc (not Developer ID)", "appleNotarised": False}
    output.with_suffix(".verification.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps(report, indent=2))
    return output


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--app", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    build(args.app, args.out)
