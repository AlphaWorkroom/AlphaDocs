# Stage 31 QA — homepage card wrapping

18 October 2026. Source `ba16d9820af0ea82432e82195ea710d42751d498`, public version 0.1.8.1.

Recent cards previously used unconstrained vertical child layouts inside a wrapping horizontal row. Reserving each card's complete width before layout lets the row wrap before the card passes its boundary. Centred content is clamped to its actual available width; a 1130-pixel content width accommodates five 210-pixel cards plus four 20-pixel gaps, and all seven templates on wide windows.

- Full all-feature suite passed: 279 library, 3 speech-worker and 5 integration tests (287 total), zero failures; four explicit diagnostics were not rerun. After the final two-pixel content-width correction, only the three affected homepage tests were rerun and passed. Other passing results were reused.
- Strict all-target/all-feature Clippy, rustfmt, whitespace checks and final native build passed.
- Real UI-frame regression uses 13 synthetic recent documents at 1574, 960, 1920 and 960 pixels in Light and Dark. It checks horizontal bounds, row wrapping, nonoverlap, resize transitions and five cards per wide row. Existing search/header input and card-context-menu checks pass.
- Native Windows visual check showed seven templates and five recent cards across, with additional documents on subsequent rows. Scrolling down and opening a document from the second row succeeded. Captures are retained locally in `qa/stage31/homepage-wrapped.jpg`, `homepage-later-row.jpg` and `wrapped-row-open.jpg`.
- The CLI and Windows FileVersion/ProductVersion report 0.1.8.1. Cargo uses compatible `0.1.8+patch.1` internally. Three/four-part version ordering, canonical tags and exact installer URLs pass regression checks. Python packaging mapping/syntax checks passed.
- macOS retains three-component bundle keys as required by [Apple's bundle version documentation](https://developer.apple.com/documentation/BundleResources/Information-Property-List/CFBundleVersion); the app and public release use 0.1.8.1, while the bundle build encodes the revision. macOS/Linux packages and runtime behavior were not locally verified.
- All native interactions used a synthetic library and isolated profile. No user documents, installed app, caches or clipboard were modified.

[Tagged release packaging and native installer CI](https://github.com/AlphaWorkroom/AlphaDocs/actions/runs/39118010414) is pending. Older app versions require a manual GitHub download for this patch because their update parser only accepts three numeric components. This patch recognises both three- and four-part release numbers.
