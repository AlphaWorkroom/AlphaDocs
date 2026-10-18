# Stage 29 QA — table selection and calculation updates

18 October 2026. Source: `b692e11709eabbeb633dd3ee85ea191a534ffa7c` (v0.1.7).

- `cargo clippy --locked --all-targets --all-features -- -D warnings`: passed.
- `cargo test --locked --all-targets --all-features`: 275 library, 3 speech-worker and 5 integration tests passed; zero failures. Four unrelated explicit diagnostics remain ignored in this run; their earlier results were not rerun.
- rustfmt and git diff whitespace checks passed. Native Windows debug build passed.
- Engine regressions cover whole-cell copy/clear/Undo, document Select All after a cell selection, dependent formulas during coalesced typing, automatic/manual modes, error recovery and atomic cached results.
- Settled UI frames cover both Light and Dark, an empty header row, numeric-aware LEFT default, formula preview, Preferences persistence, automatic edit/Undo, manual recalculation hit bounds and read-only selection.
- Isolated native Windows smoke check: opened the synthetic two-table fixture, inspected the new context menu, opened Formula in C2 beneath an empty header, verified `=SUM(LEFT)` and Result: 30, then clicked Calculate and observed 30. Captures: `qa/stage29/native-table-menu.jpg`, `native-formula-preview.jpg`, `native-calculated-result.jpg`. These local captures and logs are not bundled in downloads.
- No user documents, profile, clipboard, microphone or physical printer were used. Automatic recalculation is off by default and remains a local preference.

Native Windows/Linux/macOS release builds, installers and update checks are pending in [the tagged release workflow](https://github.com/AlphaWorkroom/AlphaDocs/actions/runs/39098769107). Physical macOS/Linux interaction has not been verified locally. The workflow publishes installers after all required jobs pass; existing release assets were not replaced.
