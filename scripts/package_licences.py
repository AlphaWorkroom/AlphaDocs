"""Bundle resolved dependency notices from Cargo metadata; do not fetch or execute crates."""
import json
import pathlib
import shutil
import sys

root = pathlib.Path(__file__).resolve().parent.parent
metadata = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8-sig"))
destination = root / "dist" / "third-party-licences" / "cargo"
sources = root / "dist" / "third-party-sources"
destination.mkdir(parents=True, exist_ok=True)
sources.mkdir(parents=True, exist_ok=True)
notices = ["# Resolved Cargo dependency notices", "", "Includes platform/build dependencies as well as runtime crates. Licence expressions are the publishers' declarations; original notices accompany each entry. Existing font, artwork, dictionary and vendored egui-winit notices also apply.", ""]
missing = []
archives = []
for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
    if package["source"] is None:
        continue
    source = pathlib.Path(package["manifest_path"]).parent
    tag = package["name"] + "-" + package["version"]
    files = {p for p in source.iterdir() if p.is_file() and p.name.lower().startswith(("license", "licence", "copying", "notice"))}
    for folder in ("licenses", "licences", "fonts"):
        directory = source / folder
        if directory.is_dir():
            files.update(p for p in directory.rglob("*") if p.is_file() and (p.name.lower().startswith(("license", "licence", "copying", "notice")) or p.suffix.lower() in (".txt", ".md") and folder != "fonts"))
    if package.get("license_file"):
        files.add(source / package["license_file"])
    target = destination / tag
    target.mkdir(exist_ok=True)
    for path in files:
        relative = path.relative_to(source)
        output = target / relative
        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, output)
    if not files:
        # The immutable source archive supplies the publisher's complete source/notices.
        missing.append(tag)
    archive = source.parents[1].parent / "cache" / source.parent.name / (tag + ".crate")
    if "MPL" in (package.get("license") or "") or not files:
        if not archive.is_file():
            raise RuntimeError("Missing source archive: " + tag)
        shutil.copyfile(archive, sources / archive.name)
        archives.append(tag)
    notices.append(f"- {tag}: {package.get('license') or 'See original licence file'}. Source: https://crates.io/crates/{package['name']}/{package['version']}")
notices.extend(["", "Unmodified published Cargo source archives for MPL components and crates without a standalone notice are in third-party-sources (.crate is gzip-compressed tar). AlphaDocs's modified egui-winit source is in the repository's vendor directory.", ""])
(root / "dist" / "THIRD-PARTY-NOTICES.md").write_text("\n".join(notices), encoding="utf-8")
print(f"Bundled notices for {len(metadata['packages'])} resolved packages; {len(archives)} source archives. {len(missing)} packages use their full source archive as the original notice source.")
