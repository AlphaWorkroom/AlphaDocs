# Development and releases

AlphaDocs uses Rust 2024 and egui/eframe. Keep `Cargo.lock`; use a current stable toolchain supporting its dependencies. No web UI/browser runtime is used.

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked
cargo build --locked --release
```

Debian/Ubuntu native dependencies are listed in `.github/workflows/release.yml`: X11, Wayland, EGL/OpenGL, fontconfig, DBus and XCB development libraries. macOS needs Xcode command-line tools. Windows needs Rust's MSVC toolchain and C++ build tools/Windows SDK.

`model` stores versioned documents with shared copy-on-write payloads. `engine` owns Unicode-safe edits, selection and undo. `layout` shapes/measures/paginates for screen and PDF. `persistence` writes atomically into platform app data. `compat` implements explicit interchange adapters. `ui` draws native controls/canvas. `updates` requests bounded public metadata only. Preserve original images and all content tabs; unsupported constructs need notices, not claims of support.

`ALPHADOCS_DATA_DIR` selects an isolated profile. Tests must not consume the real clipboard, rewrite user documents or send physical print jobs. Native visual QA is a separate release gate. `--version` and `--self-check` inspect a packaged executable without opening documents or modifying a profile.

## Packaging

After a native release build, run `python scripts/prepare_dist.py`. It assembles the binary, verbatim application licence, branding, dependency notices and required dependency source archives. Then run `python scripts/build_windows_package.py` on Windows or `python scripts/package_unix.py` on Linux/macOS. Tools use Python 3.10+ and native host tools (.NET compiler, `dpkg-deb`, or `sips`/`iconutil`/`codesign`/`pkgbuild`). Outputs must stay below 100 MB and include SHA-256 manifests. macOS output is ad-hoc signed, not notarised.

Do not publish `target`, local profiles, QA documents, proprietary reference screenshots, authentication material or development logs. Dev/test profiles omit large debug symbols/incremental copies. `cargo clean` removes regenerable output; preserve verified artifacts first.

## Publishing

Push to `main` to run four native installer builds: Windows x86-64, Linux x86-64, macOS Apple silicon and macOS Intel. Review workflow artifacts. A version tag matching Cargo.toml runs the same gates and creates a public GitHub Release **only after every native build and installer check succeeds**. Apple signing/notarisation is not implied.

Update Cargo's version and `docs/RELEASE_NOTES.md` before tagging. Keep asset names aligned with `src/updates.rs` and the README. Do not repoint published tags; use a new version for fixes. The GitHub workflow token is scoped to release publication, and no persistent token is committed.
