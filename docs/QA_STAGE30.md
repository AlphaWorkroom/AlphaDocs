# Stage 30 QA — Windows portable update handoff

18 October 2026. Source `a047da20d41957c3a627a5fec4d1d025fbc4b973` (v0.1.8).

The Windows updater previously compared the running executable against the one fixed per-user installation path. This rejected portable and renamed copies before downloading, even when those copies came from an official GitHub release. Setup already installs to a fixed per-user destination; accepting another source path does not let it replace arbitrary folders.

- All-target/all-feature tests: 276 library, 3 speech-worker and 5 integration tests passed (284 total), zero failures. Four unrelated explicit diagnostics were not rerun.
- Strict all-target/all-feature Clippy, rustfmt, whitespace checks and native debug build passed.
- Windows source-layout regression covers standard, portable and renamed executables, missing executables, invalid local-data paths and wrong installer types. Existing digest/size/redirect and staged-handoff checks passed.
- Full UI frames verify idle and downloading update dialogs fit 960 by 640 in both themes. The idle notice explains the retained portable folder, managed install and Start-menu launch.
- Compiled the production C# installer source against the previously verified v0.1.6 CI payload. Its isolated `--verify-update` passed both installed-app replacement with a nested working directory and a fresh installation from a renamed portable source. SHA-256 of the portable executable stayed unchanged; installed payloads passed native self-checks. No actual user installation, shortcuts or associations were changed by these verification modes.
- Native v0.1.8 launch and Help > Check for updates smoke passed using a synthetic document and isolated profile. GitHub returned no newer stable release, as expected before publication. The new available-update notice is covered by UI frames; a real v0.1.8-to-newer release update was not performed. Local capture: `qa/stage30/native-updater.jpg`.

[Tagged installer build and platform verification](https://github.com/AlphaWorkroom/AlphaDocs/actions/runs/39100082806) is pending. The workflow reruns the installer regression against the actual v0.1.8 payload before publishing. macOS/Linux runtime behavior was not verified locally and their update logic was unchanged.

Older clients containing the rejected-path guard need a one-time Windows Setup installation; a new release cannot change code inside an already-running old client. The document library and original portable directory remain in place.
