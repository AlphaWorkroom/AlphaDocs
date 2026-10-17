# Stage 28 QA — editor compatibility and Windows updater

18 October 2026. Original synthetic writing only. User originals and caches were not changed. The affected MacBook story was unavailable, so exact-file reproduction is not claimed.

## Automated evidence

- Full Rust all-target/all-feature suite: 271 library, 3 speech-worker and 4 independent integration tests passed; all four explicit diagnostics also passed (282 active checks in total). No real microphone, system clipboard, user profile or physical printer was used.
- Strict Clippy all targets/all features and rustfmt passed after the final source refinements.
- Complete DOCX package validation: Microsoft Open XML SDK 3.0.1, Office 2010 schema, zero errors (`qa/stage28/openxml-delivery.log`). Includes story text with paragraph borders, cell fills/edges, a real Word formula field, Office Math and five WPS object geometries.
- DOCX/native source regressions preserve Unicode text/styles, stable object IDs, formulas, original media, resizing and Undo. Full UI frames exercise palette removal, row shading preserving distinct edges, source-dialog Insert/Edit/Cancel, stale/read-only guards and small 960×640 layouts in both themes.
- Collaboration regression edits drawing source on one replica and verifies changed remote preview, unchanged payload sharing and live-session edit/view paths. Protocol registration and request versions share the current ALPN.
- Measured PDF audit: two pages, zero raster images, 191 vector paths, body text and 20.00 formula result present. Independent PNG review confirms readable story, table fills/borders, equation, text boxes, ellipse, horizontal line and arrow.

## Windows update root cause

Real previous installation logs failed at `Directory.InternalMove` after the editor process had exited. A controlled Windows reproduction verified that an inherited working directory inside the old installation prevents its rename. Releasing that working directory permits the move (`qa/stage28/updater-root-cause.json`).

The client launches from staging, and the installer defensively moves its own CWD to a safe existing directory before staging/swapping. `--verify-update` now executes the production `Update(oldPid)` wait/install path: seeds a disposable installation, sets a nested installation CWD, starts the old native executable, waits for it to exit, installs and validates both the new payload and retained backup. It also covers a nonexistent fresh-install parent. The preliminary real-path regression passed (`qa/stage28/updater-live-cwd-verification.json`). Final packaged installer verification is recorded with package hashes below.

## Independent reader scope

Microsoft Word opens the original fixture without a repair prompt and displays its body paragraphs, coloured table and 20.00 formula result. Office Math is recognised as an editable equation; WPS text boxes/shapes are recognised as drawings. A separate latest fixture verifies explicit 18pt math and left alignment rather than Word’s default centred 12pt rendering. The modern-compatibility fixture was edited and saved in Word without a downgrade prompt, then reimported/exported through AlphaDocs: body text, one editable equation, all five shapes, formula and cell fill survive. A later native Word visual check caught diagonal line/arrow geometry; corrected zero-height standard geometry now displays horizontally and has a dedicated integration regression.

LibreOffice headless conversion succeeds with an isolated profile and retains readable body/drawing/math content. It displays the cached SUM result as 20 rather than Word’s 20.00; exact cross-reader number-format identity is not claimed. Reader pagination differs, and LibreOffice places some WPS text outside its shape and renders line angles differently. This is a recorded compatibility limitation, not a successful layout-fidelity result.

ONLYOFFICE runtime verification is not available locally. Windows QA does not verify physical Mac/Linux UI; native CI build/package gates are recorded separately. Advanced unsupported features are listed in [EDITOR_COMPATIBILITY.md](EDITOR_COMPATIBILITY.md).

## Final native/package evidence

Native Windows Light/Dark captures confirm formatted body/table/math content, visible palette remove controls, source dialogs, cell shading Apply/Undo and text-box resize/reflow/Undo. Source-dialog fields are outlined; footer bounds are tested at 960×640. Table context commands scroll at short heights. Evidence remains in qa/stage28. Final exact-release packaging and platform CI are pending; preliminary QA executables are not the delivered package.
