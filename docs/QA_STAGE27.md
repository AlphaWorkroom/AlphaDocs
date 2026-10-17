# Interaction QA — 17 October 2026

This pass uses synthetic documents and an isolated AlphaDocs profile. It does not modify the user's open installation, documents or preferences. No physical print jobs, microphone capture or OS privacy prompts are automated.

## Reproduced and corrected

| Scenario | Correction and regression coverage |
| --- | --- |
| Fast horizontal scrollbar drag ending over the vertical bar | Preserve the scrollbar hit by the initial press. Full editor frames check both axes, unchanged selection/content, Light/Dark, 960–1920 layouts and 100%/200% zoom. |
| Entire text-selection drag delivered in one frame | Measure both endpoints, retain the initial anchor and snap it to a Unicode grapheme boundary. Test complete/coalesced and later-release gestures, including linked text in Viewing mode. Selecting a link does not open it. |
| Save while reading away from the caret | Save/Save As/Copy/export retain the viewport; unhandled keys do not force caret scrolling. A temporary native save/reload checks document and selection integrity. Native before/after screenshots verify Save retains the reading position. |
| Window edge gesture covered by editor panels | Register resize handles after the panels and dispatch the native resize command on press. Full editor frames verify separate and coalesced input. |
| Invalid colour entry followed by Pin | Disable pinning for invalid input, duplicates and full palettes; retain document/selection. Native wheel, brightness, pin, Apply and Undo were exercised in Dark mode. |
| Signature stroke interrupted by leaving the window/focus | End the active stroke, retaining points already drawn. Returning without a fresh press adds no connecting line. Full signing workspace frames verify both interruption events and reviewed/disabled input. |
| Empty homepage with autosave disabled | Show the actual preference instead of claiming autosave is enabled. |
| Body emoji exported as empty squares | Select the bundled fallback per complete emoji grapheme; keep joined/modifier sequences together and remove hidden selector glyphs after shaping. PDF extraction checks single, joined, skin-tone, variation-selector and flag emoji. |
| Closely spaced checklist markers and click targets | Keep each hit region within its measured line; share size constraints between canvas and PDF so adjacent marks stay separate. Full Light/Dark frames check nonoverlapping targets, toggle and Undo. |
| Missing/invalid file opened from Documents | Show the existing message dialog instead of a hidden status line. Frame tests retain current content, caret and Undo, and preserve malformed source bytes. |
| Closing Preferences after switching theme reveals an offscreen caret | Informational/preferences dialogs restore focus without forcing a scroll. Light/Dark editor frames exercise Preferences, Help, About, count, rename, messages, export results and the related Tools dialogs; document/selection/viewport remain intact. This final check ships in 0.1.6; published 0.1.5 remains intact. |

## Verification scenarios

- Native Windows: new document, multiline Unicode/arrow input, keyboard and mouse selection, checklist conversion/toggle/undo, content tabs, separate open-file tabs, native Save As, close/reopen/session retention, long-document thumb dragging and track clicking, offscreen Find navigation, table picker/first-cell typing/Tab/empty-cell context targeting/whole-column selection, Preferences scrolling, themes, embedded image resize/Undo, custom colour wheel/pin/Apply/Undo.
- Full-frame/engine regressions: table navigation and appended-row Undo; rectangular selection/copy/format/clear/read-only enforcement; image/table drags and resize gestures; crop/effects/snapping; comments/annotation anchors; compact/narrow chrome; all paper presets/custom geometry; all-tab native/DOCX roundtrips and exports; clipboard worker staleness; proofing; autosave idle/IME/off gates; update modal/verified download helpers; signing validation; voice worker cancellation and stale-anchor rejection.
- Automated checks: `cargo fmt --check`, `cargo clippy --locked --all-targets --all-features -- -D warnings`, `cargo test --locked --all-features`. The four normally ignored diagnostics are also run explicitly with `cargo test --locked --lib --all-features -- --ignored --nocapture --test-threads=1`: two large-document timings, live synthetic dictionary lookup and the public GitHub checksum-asset redirect/hash check.

Local logs, original fixtures, before/after failure logs and native screenshots are retained in `qa/stage27/`. Published artifacts must be checked against the final CI source and their SHA-256 before delivery.

The first exact-release run caught a separate PDF/DOCX command path still revealing the caret after export. Release publication was stopped. The post-command frame regression now covers both original export commands and every newer format; native PDF export is repeated on the corrected final executable.

Local final source result: 259 library tests and 3 speech-worker tests passed; all four normally ignored diagnostics passed when run explicitly (266 total, zero failures). Formatting and strict all-target/all-feature Clippy passed. In the unoptimised timing diagnostics, 28,000 words had a 2.8 ms warm editor-frame median; a single 28,001-word wrapped paragraph measured 54 pages with a 3.2 ms warm median and a 718 ms edit/reflow. These measurements exclude GPU presentation and persistence I/O and are not release-build latency guarantees.

## Verification boundaries

Native interaction evidence is Windows-specific. Platform CI does not establish physical Mac/Linux appearance, microphone quality, cross-device networking, printer finishing or Apple notarisation. Windows OS resize-loop behaviour cannot be certified from coalesced synthetic mouse input; its native command dispatch is covered by full frames. Installed font coverage still matters for scripts outside the bundled fonts; choose a matching installed font for those scripts. Unsupported tracked changes, true inline image atoms and hosted/nearby signing delivery retain their existing documented limits.
