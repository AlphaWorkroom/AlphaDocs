# AlphaDocs integration patches to egui-winit 0.36.2

Upstream: https://github.com/emilk/egui/tree/0.36.2/crates/egui-winit . Original MIT/Apache-2.0 licences retained.

1. Always emit Event::Paste for a native paste gesture, even with empty/unavailable text; AlphaDocs consumes HTML and bitmap formats independently. Plain text fields still receive an empty no-op paste.
2. Ctrl+Alt+X is the proofing shortcut. is_cut_command requires Alt to be absent for command-X so the actual key reaches AlphaDocs rather than cutting selected content. Command-X still cuts. Regression: cargo test -p egui-winit --lib alphadocs_shortcut_tests.

No web renderer or browser integration is added.
