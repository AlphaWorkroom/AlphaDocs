# Page setup and writing tools

AlphaDocs uses native Rust controls and measured document layout. The Google Docs screenshots are visual references; the application does not embed Google Docs or its cloud tools.

## Page setup

File → Page setup offers Pages and Pageless. Pages has orientation, colour, four margins, and every preset shown in the supplied Google Docs menu: Letter, Tabloid, Legal, Statement, Executive, Folio, A3, A4, A5, ISO B4, ISO B5. Custom accepts physical sides from 1 to 200 inches, provided margins and columns leave usable space. B4/B5 are ISO dimensions, not the larger Japanese variants. Both orientations use the same measured pagination as PDF.

Apply to is whole-document scope across content tabs. Independent sections are not supported. Advanced retains columns, gap, editable repeated headers/footers and page numbers. Set as default saves geometry for new blank documents, omitting the current header/footer text. Cancel leaves the current document unchanged; applying settings is undoable. Page colours belong to the document and do not change with the workspace theme.

Pageless measures one continuous canvas with the retained paper width and one column. It hides manual page breaks, running text, page numbers and line numbers. These remain stored. Printing/PDF measure a separate paged copy; DOCX/ODT/RTF use stored physical paper settings. Pageless mode is native metadata, not a portable Word layout setting.

## What the Tools entries do

| Entry | Implemented behaviour and limits |
| --- | --- |
| Spelling and grammar | Offline English spelling and conservative rule-based grammar review, separate squiggle visibility controls, right-click fixes and personal word exceptions. Does not provide broad multilingual analysis. |
| Word count | Words and characters for the current content tab; Ctrl/Cmd+Shift+C. |
| Review suggested edits | Disabled: no tracked-change engine. Local comments are supported independently. |
| Compare documents | A separate all-tab text report with additions/removals and an attribution label. Paragraph/table text only; no comparison of object geometry, comments or formatting, and no tracked revision fields. Bounded to four million line pairs / 8 MB combined text. |
| Citations | Manual single-author/organisation sources; basic APA 7, MLA 8, Chicago author-date in-text citations and sorted bibliography text. Check final punctuation/ordering against the required style guide. No automatic metadata search, complex multi-author formatting, live fields or standard Word citation interoperability. Sources are native/DOCX private metadata and shared during collaboration; visible citation text remains editable in exports. |
| eSignature | Disabled: no verified signature request/audit service. Insert an existing signature image with Insert → Image if needed. |
| Line numbers | Counts actual measured body lines, continuously or restarting per page, across paged exports. Table cells and running text are excluded. Screen/PDF output and standard DOCX line-number properties; no sections or pageless numbering. Other export formats report the omission. |
| Linked objects | Disabled: external chart/table/slide links and refresh are not implemented. Inserted objects are embedded copies. |
| Dictionary | Explicit online English word lookup using [Free Dictionary API](https://dictionaryapi.dev/), with source/licence information returned by the provider. Only the entered word is sent when Search is clicked; no background document lookups. Ctrl/Cmd+Shift+Y opens the panel. |
| Translate document | Hidden by default; see below. |
| Voice typing | Platform dictation instructions. Recognised OS input uses normal undoable editing. AlphaDocs has no microphone capture, speech engine, language picker or voice editing commands. Windows uses OS dictation, macOS uses its configured Dictation shortcut; Linux depends on an external input method. OS permissions and online speech processing remain controlled by the OS. |
| Notification settings | Nearby invitation prompts and daily GitHub update checks. Offers received while prompts are off are dismissed. No email or hosted comment notifications. |
| Preferences | Theme and local display/save options, optional sentence capitalization, smart quotes, http(s)/www link detection, simple list detection, editable token substitutions. Automatic typing changes are off by default, apply through Editor and undo with their keystroke; paste and IME keep their existing paths. Full Markdown typing, automatic spell replacement and generative Smart Reply are visibly unavailable. |
| Accessibility | Native canvas text exposure and interface scaling. Full rich-text screen-reader navigation, braille and assistive-hardware integration are not claimed. Document zoom is separate. |

## Optional translation and data use

Preferences → General → Enable online document translation adds Tools → Translate document. Enabling it sends nothing. Translate names the provider and sends the document's text from all content tabs, table cells and running headers/footers to `api.mymemory.translated.net` over HTTPS. Images and original media are not uploaded. Do not use it for text you do not want to send to that provider.

[MyMemory API specifications](https://mymemory.translated.net/doc/spec.php) limit each UTF-8 request to 500 bytes; AlphaDocs splits at Unicode grapheme boundaries. [Anonymous usage limits](https://mymemory.translated.net/doc/usagelimits.php) are 5,000 characters/day, shared by public IP. AlphaDocs preflights the document size; the provider may still reject a request because of earlier usage, language availability or service errors. No account, email, API key, paid plan or translation-memory contribution is required by AlphaDocs. Provider [terms and privacy](https://mymemory.translated.net/doc/tos.php) apply. The service can use machine translation; this optional service is distinct from a generative writing assistant.

Translation creates a separate document after the complete operation succeeds. Paragraph styles, tables, tabs and images are retained; inline formatting, links and text-range annotations are reset in the copy because word boundaries change. The source stays intact. Stale document/tab/revision results are rejected. Failure does not open a partial translation or replace source content. Translation quality, quotas and availability are outside AlphaDocs' control.

## Verification scope

Windows native UI checks use original sample text and an isolated profile. Automated tests cover every preset in both orientations, native/custom-size and DOCX roundtrips, PDF physical dimensions and extracted text, measured pageless floating objects, body line numbering, single-transaction typing changes, citation insertion/undo, all-tab translation through an injected offline service, stale results and disabled menu interaction. Cross-platform build/installer results are recorded separately; Windows UI evidence is not a claim of physical Mac/Linux visual or assistive-hardware testing.
