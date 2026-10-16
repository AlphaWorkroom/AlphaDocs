# Changelog

## 0.1.3 — 16 October 2026

### Focused fixes

- Autosave waits for 30 seconds without local editing. Each edit restarts the wait; IME composition defers saving. Preferences has a dedicated On/Off Autosave card. Explicit Save, switching and closing retain their existing immediate-save behaviour.
- macOS uses native AppKit traffic lights in the integrated header, with space reserved on the left. Windows/Linux keep their existing controls. Native Mac builds and simulated geometry checks are distinct from physical Mac visual testing.
- The update dialog now downloads a platform installer, shows received/total bytes, verifies its declared size and GitHub SHA-256, and hands off installation/relaunch after saving all open documents. Editing is blocked during the download, with cancellation available. Canonical repository URLs and bounded trusted-host redirects are required; failed verification does not run an installer.
- Windows per-user installations use a themed update helper and retain the old installation for recovery. macOS standard Applications bundles are staged, signature/self-check verified and swapped with a retained backup. Linux DEB installations use normal PackageKit/PolicyKit or pkexec/dpkg authorization; supported user-writable layouts swap validated files. Missing permission/tools or unsupported portable paths produce an explicit error and manual release link. Installation helpers retain failure logs and attempt to reopen the old application on failure; Linux package-database rollback is not claimed.
- Restored native browser-link support, which was omitted from the prior build and caused the old download button to do nothing.
- Removed the accidentally included EffectCraft-project sentence from the GitHub README.
- Added real Noto Sans Math glyph fallback for long/double arrows in both editor and PDF, with exact OFL/source notices. Recognised pasted long-arrow names and numeric arrow references become their actual Unicode symbols. Literal code and unrelated entities stay intact; existing document text is not bulk-rewritten.
- Cache audit is reported separately; no development caches or documents were deleted.

### Page setup and writing tools

- Page setup follows the supplied Pages/Pageless reference, with whole-document scope, page colour, four margin fields, an Advanced section and saved defaults for new documents. Defaults never copy document header/footer text.
- All 11 Google Docs paper presets are available: Letter, Tabloid, Legal, Statement, Executive, Folio, A3, A4, A5, ISO B4 and ISO B5. Custom dimensions accept 1–200 inches per side, subject to usable margin/column space. Both orientations use measured layout; native files and DOCX retain dimensions.
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
