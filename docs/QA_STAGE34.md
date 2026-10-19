# Stage 34 — hyperlinks and deletion focus

20 October 2026. Source `4364db2dd3f01ed2faee5f5490409a1d34bce7e5`, v0.1.11.

Hyperlink activation previously ran for both the raw press target and the completed click target. It now requires a completed primary click. The first text context command is Open to link, using the pointer's measured linked-text position and accepting HTTP, HTTPS or mailto links.

Ordinary character and paragraph-boundary deletion passed the original UI-frame check. The input dispatcher did drop deletion keys when canvas focus was absent and a non-text control owned focus. Backspace/Delete now return input to the document in that case. Text-edit ownership and open popups are checked first; viewing and engine collaboration permissions still reject edits.

- Targeted real UI-frame tests passed for Windows and simulated macOS: separate press/release, coalesced clicks, one open request per gesture, no open on press or idle frames, right-click without navigation, first context action and activating that action.
- Targeted deletion frames passed for both OS input mappings: composed Unicode/emoji deletion, empty paragraph removal, paragraph joins, Undo, and lost canvas focus recovery. Native macOS runtime was not tested locally.
- Native Windows smoke: the link context menu shows Open to link first. Backspace changed `Delete this sample` to `Delete this sampl`; Home then Delete changed it to `elete this sampl`. Evidence: `qa/stage34/link-menu.jpg` and `native-deletion.jpg`.
- Native debug build, rustfmt, whitespace checks and release-workflow YAML/order checks passed. Synthetic documents and an isolated application profile were used; no real clipboard, microphone, printer or user documents were used.
- The complete suite and strict Clippy were deliberately deferred to the post-publication four-platform verify job, following the requested release order. The release still requires native packages and basic installer/update checks before upload. No complete-suite or cross-platform runtime success is claimed here.

[Release build, publication and subsequent verification](https://github.com/AlphaWorkroom/AlphaDocs/actions/runs/39258167822) is pending. Publication is automatic after the packages are ready; the full verify job depends on publication. No CI polling loop was started.
