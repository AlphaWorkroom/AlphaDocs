# Stage 33 — release blocker and document-ID correction

18 October 2026. Source `dad2e7b74a147d0715463a03b3a783eea4d3e0c6`, v0.1.10.

The v0.1.9 publication workflow did not publish: Windows, Linux and Intel macOS jobs passed, but Apple Silicon failed `homepage_recent_cards_wrap_inside_window_after_resizing`. Its failure reported identical rectangles for adjacent document widget IDs. New document identities were derived solely from wall-clock nanoseconds; clock resolution can cause distinct rapid creations to receive an identical identity. The source now uses the existing rand dependency to generate random 128-bit document IDs. Existing IDs and saved files are unchanged; no schema migration is needed.

- Local all-feature suite: 280 library, 3 speech-worker and 5 integration tests passed (288 total), zero failures; four unrelated explicit diagnostics not rerun.
- New regression creates 1024 documents on four parallel threads and checks distinct, valid IDs. The original homepage resizing/wrapping test is unchanged and passes.
- Strict all-target/all-feature Clippy, rustfmt and whitespace checks passed. Public version helper returns 0.1.10.
- Existing native homepage visual/scroll/open evidence from Stage 31 is reused: no UI geometry was changed. Native 0.1.10 installers, launch/self-checks and the previously failing Apple Silicon job are pending [tagged platform CI](https://github.com/AlphaWorkroom/AlphaDocs/actions/runs/39122605276). No cross-platform success is claimed before that run completes.
- Failed v0.1.9 tag and prior published releases are retained. Version 0.1.10 uses three numeric components readable by existing updaters. No user documents, installation, profiles, clipboard or caches were changed.

The update dialog correctly reports no newer release while GitHub's latest published release remains v0.1.8. v0.1.10 becomes discoverable after all required native build/installer jobs succeed and publication completes.
