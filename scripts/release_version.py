"""Map Cargo-compatible patch metadata to the public numeric release version."""
from pathlib import Path
import re


def read_version(root: Path) -> str:
    source = (root / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'^version = "([0-9]+\.[0-9]+\.[0-9]+)(?:\+patch\.([0-9]+))?"$', source, re.M)
    if not match:
        raise ValueError("Unsupported package version")
    version = match[1] + ("." + match[2] if match[2] else "")
    if any(str(int(part)) != part or int(part) > 65535 for part in version.split(".")):
        raise ValueError("Version components must be canonical Windows-compatible integers")
    return version


def macos_bundle_versions(version: str) -> tuple[str, str]:
    # Apple bundle keys allow three components. Keep the public patch number
    # in the app/release and encode its revision monotonically in the build.
    parts = [int(part) for part in version.split(".")]
    revision = parts[3] if len(parts) == 4 else 0
    if revision >= 1000:
        raise ValueError("macOS patch revision must be below 1000")
    base = ".".join(str(part) for part in parts[:3])
    return base, f"{parts[0]}.{parts[1]}.{parts[2] * 1000 + revision}"


if __name__ == "__main__":
    print(read_version(Path(__file__).resolve().parent.parent))
