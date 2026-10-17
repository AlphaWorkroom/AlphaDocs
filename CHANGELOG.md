# Changelog

## 0.1.5 — 17 October 2026

- Fixed fast horizontal scrollbar drags ending at the vertical scrollbar corner. The scrollbar receiving the press keeps pointer ownership; scrolling preserves the document selection.
- Fast text-selection drags retain their measured press and release positions when Windows coalesces the gesture into one frame, rather than collapsing to a click.
- Save, Save As, Copy and export retain the reading viewport rather than jumping to an offscreen caret. Unhandled keys such as Escape no longer trigger a caret scroll.
- Window resize edges register after the editor panels, preventing the canvas/status area from covering them. Native resizing starts on the press rather than waiting for a drag threshold.
- Custom colour pinning rejects invalid HEX/RGB input, already-pinned colours and a full palette, with an explanatory disabled tooltip. It no longer pins an old preview value after an invalid edit.
- Drawn signatures end their current stroke when the pointer leaves the window or the window loses focus, preventing an unwanted connecting line on return.
- The empty Documents homepage now reflects whether autosave is enabled.
- PDF body text uses the bundled emoji fallback for missing glyphs and keeps joined/modifier sequences together, including variation selectors without duplicate searchable text.
- Closely spaced checklists have separate click regions; checkbox marks fit their measured line height on screen and in PDF instead of touching adjacent marks.
- Opening a missing or invalid document shows a visible error from both Documents and the editor while retaining the current document and original file.
- Added full UI-frame regressions for interrupted signatures, palette validation, save/selection/viewport integrity, window resize gestures and both scrollbar axes across light/dark themes, window sizes and zoom levels.

## 0.1.4 — 17 October 2026

- Fixed native scrollbar pointer ownership so dragging the thumb does not become a document selection. Mouse-wheel scrolling and outline navigation retain their measured scroll geometry.
- Dictionary searches fall back to English Wiktionary when Free Dictionary API is unavailable. Capitalized searches retry the lowercase English entry when needed. Results identify their source and licence; only the explicitly searched word is sent.
- Text and highlight palettes now include a native hue/saturation wheel, brightness rail, HEX/RGB inputs and up to 24 pinned colours shared between both palettes. Applying a colour is undoable; pinning does not edit the document.
- Table context menus select a complete row or column. Copy retains editable cells, formatting applies across the selection, and Delete/Backspace clears its contents in one undoable edit.
- Added a checklist button immediately beside Add comment and a text context-menu action. Selected paragraphs and explicit body-text line breaks become independent checkable items; completed text is dimmed in light and dark themes without changing its stored colour.
- Checklist PDF/printing markers use vectors, preserving checked and unchecked states without depending on font symbol coverage. DOCX uses standard Word checkbox content controls and imports their current checked state, including table cells and every document tab.
- Added local eSignature request and response files. Requests freeze every document tab, assign named fields, accept typed/drawn signatures or a decline, validate returned responses against the original request manifest, and export a completed PDF with a separate signing section and local audit record. Names are self-declared; no hosted delivery or certificate-backed identity verification is provided.
- Added built-in offline English microphone dictation with Start/Stop, input level, partial preview and undoable final insertion. First-use setup downloads the pinned upstream recognition runtime and model separately (about 46–55 MB, depending on platform); no microphone audio or document text is uploaded. Physical microphone checks remain human verification.

## 0.1.3 — 16 October 2026

### Focused fixes

- Autosave waits for 30 seconds without local editing. Each edit restarts the wait; IME composition defers saving. Preferences has a dedicated On/Off Autosave card. Explicit Save, switching and closing retain their existing immediate-save behaviour.
- macOS uses native AppKit traffic lights in the integrated header, with space reserved on the left. Windows/Linux keep their existing controls. Native Mac builds and simulated geometry checks are distinct from physical Mac visual testing.
- The update dialog now downloads a platform installer, shows received/total bytes, verifies its declared size and GitHub SHA-256, and hands off installation/relaunch after saving all open documents. Editing is blocked during the download, with cancellation available. Canonical repository URLs and bounded trusted-host redirects are required; failed verification does not run an installer.
- Windows per-user installations use a themed update helper and retain the old installation for recovery. macOS standard Applications bundles are staged, signature/self-check verified and swapped with a retained backup. Linux DEB installations use normal PackageKit/PolicyKit or pkexec/dpkg authorization; supported user-writable layouts swap validated files. Missing permission/tools or unsupported portable paths produce an explicit error and manual release link. Installation helpers retain failure logs and attempt to reopen the old application on failure; Linux package-database rollback is not claimed.
- Restored native browser-link support, which was omitted from the prior build and caused the old download button to do nothing.
- Added real Noto Sans Math glyph fallback for long/double arrows in both editor and PDF, with exact OFL/source notices. Recognised pasted long-arrow names and numeric arrow references become their actual Unicode symbols. Literal code and unrelated entities stay intact; existing document text is not bulk-rewritten.
- Cache audit is reported separately; no development caches or documents were deleted.

### Page setup and writing tools

- Page setup follows the supplied Pages/Pageless reference, with whole-document scope, page colour, four margin fields, an Advanced section and saved defaults for new documents. Defaults never copy document header/footer text.
- All 11 paper presets are available: Letter, Tabloid, Legal, Statement, Executive, Folio, A3, A4, A5, ISO B4 and ISO B5. Custom dimensions accept 1–200 inches per side, subject to usable margin/column space. Both orientations use measured layout; native files and DOCX retain dimensions.
- Pageless is a continuous measured canvas. Paper settings and page breaks are retained for paged printing/PDF and other page-based exports.
- Tools includes spelling/grammar visibility switches and a personal dictionary editor; word count and online English dictionary lookup have keyboard shortcuts.
- Compare documents creates an independent text report covering all content tabs and table text, with additions/removals and attribution. It is not a tracked-change engine; formatting, objects and comments are excluded.
- Manual citation sources support basic APA 7, MLA 8 and Chicago author-date text and bibliography insertion. Sources survive native/DOCX roundtrips and collaboration; complex sources, live fields and standard Word citation interoperability are not implemented.
- Line numbers count measured body lines continuously or per page, excluding tables and running text. They render in the editor/PDF and use standard DOCX line-number properties. Section numbering and pageless numbering are unavailable; other exports report omitted numbering.
- Online translation is hidden until explicitly enabled in Preferences. Clicking Translate sends document text to MyMemory and creates a separate copy. Free anonymous usage is limited by the provider to 5,000 characters/day. Objects and paragraph styles are retained; inline formatting, links and text annotations reset in the copy. Failures or stale source revisions leave the original intact.
- Preferences adds optional sentence capitalization, smart quotes, link/list detection and editable substitutions. Automatic typing edits stay undoable; pasted/IME text retains its existing path. These typing options are off until enabled.
- Notification settings control nearby offers and daily update checks. Accessibility exposes document text through the native bridge and lets users enlarge the interface; complete rich-text screen-reader and braille navigation are not claimed.
- Voice typing opens platform dictation guidance; AlphaDocs has no built-in microphone/recognition engine. Review suggested edits, verified eSignature requests and linked external objects remain visibly disabled with reasons. Disabled Tools choices keep the popup open.

## 0.1.2 — 15 October 2026

See the [0.1.2 release notes](https://github.com/AlphaWorkroom/AlphaDocs/releases/tag/v0.1.2) for macOS DMGs and shortcuts, nearby live-editing invitations, clearer collaborator badges, Unicode diagram support and proofing improvements.
