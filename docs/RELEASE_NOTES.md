# AlphaDocs 0.1.6

This release fixes bordered DOCX story paragraphs being replaced by lines and Windows updates failing because the installer retained the installation as its working directory. The update regression exercises the real wait/swap/validate path in a disposable installation, including a retained previous payload.

New editing controls:

- Remove pinned custom colours with their visible trash buttons.
- Set table cell/selected row/column backgrounds, border colours and widths.
- Insert Word-style table formulas and explicitly recalculate the table with Undo.
- Insert/edit equations, text boxes, rectangles, ellipses, lines and arrows; position/resize them on the page.

Native files retain editable sources. DOCX uses standard Word formula fields, Office Math and editable WordprocessingShape objects; PDF uses vectors. Whole-package Open XML validation covers these constructs. Other rich exports use object pictures with a notice. This remains a deliberate document subset: merged cells, true inline objects, arbitrary/custom shapes, tracked changes and standard Word comment interoperability remain unsupported. [Editor audit and compatibility limits](https://github.com/AlphaWorkroom/AlphaDocs/blob/main/docs/EDITOR_COMPATIBILITY.md).

Both devices need this update for live-editing sessions with editable objects. The user’s unavailable MacBook story has not been tested directly; the fixed importer is covered by original Unicode story/table fixtures. Native/runtime and external-reader evidence are recorded separately from cross-platform build/package checks.

Closing Preferences after changing themes, Help, About, word count and other informational dialogs now retains your reading position and selection. Full editor frames verify the shared dialog-closing path in both themes.

The fixes from 0.1.5 are included:

This maintenance update fixes issues found in another native interaction QA pass:

- Fast horizontal scrollbar drags keep the correct pointer owner, including the corner where the two bars meet.
- Fast mouse drags select text correctly even when the gesture arrives in one frame.
- Saving and exporting keep your reading position. Unhandled Escape presses no longer jump to the caret.
- Editor panels no longer cover window resize edges.
- Invalid custom colours cannot pin an older preview value; duplicate/full palettes explain why pinning is disabled.
- Signature drawing stops cleanly when the pointer or window focus is lost.
- The empty homepage accurately reflects the Autosave preference.
- Emoji in body text export with the bundled fallback font, including joined and skin-tone sequences.
- Closely spaced checklist items have distinct click regions and properly sized marks on screen and in PDF.
- Missing or invalid files show an opening error without replacing the current document.

Full UI-frame regression coverage includes both themes, 100%/200% zoom, 960–1920-pixel layouts, selection preservation and undo. Native Windows interaction checks and automated platform checks are distinct from physical Mac/Linux device testing.

The features introduced in 0.1.4 remain included:

- Drag the native document scrollbar normally; text selection and outline navigation remain intact.
- Pick any text/highlight colour using a native colour wheel, brightness rail or HEX/RGB fields, and pin it to your palette.
- Select an entire table row or column from its context menu, then copy, format or clear its cells.
- Turn selected paragraphs into interactive checklists from the toolbar or right-click menu. Completed items dim in both light and dark modes; changes remain undoable. PDF preserves visible checkmarks and DOCX uses standard Word checkbox controls.
- Dictionary searches now use a labelled Wiktionary fallback when the primary provider is unavailable.
- Create local eSignature request files containing every document tab. Signers review the frozen document, type/draw signatures or decline, and return response files. Import responses and export the completed PDF with its signing section and local record. Names and timestamps are self-declared/local; no hosted signing or certified identity service is included. [Workflow](https://github.com/AlphaWorkroom/AlphaDocs/blob/main/docs/SIGNING.md).
- Built-in native offline English dictation replaces OS-shortcut instructions. Explicit first-use setup downloads the verified speech runtime/model separately (about 46–55 MB). Start/Stop, audio level, partial preview, cancellation and undoable insertion are included. Audio/document text stay on the device. No voice-formatting commands or commercial-cloud accuracy comparison are claimed.

Installers retain all supplied and third-party notices and remain subject to the 100 MB package limit. macOS apps use native traffic lights and Command controls; builds remain ad-hoc signed, not Apple-notarised. Windows/macOS/Linux build/package checks do not establish physical microphone or Mac UI verification.

Previous autosave, measured paper sizes, update installation/relaunch and arrow-rendering fixes remain included. [Full changelog](https://github.com/AlphaWorkroom/AlphaDocs/blob/main/CHANGELOG.md) · [Writing tools and limits](https://github.com/AlphaWorkroom/AlphaDocs/blob/main/docs/WRITING_TOOLS.md).
