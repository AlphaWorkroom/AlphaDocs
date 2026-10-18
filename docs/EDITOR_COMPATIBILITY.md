# Editor audit and document compatibility

18 October 2026 · AlphaDocs 0.1.6

This audit checks ordinary writing workflows against the documented capabilities of [Word](https://learn.microsoft.com/en-us/office365/servicedescriptions/office-online-service-description/word-online) and [Google Docs](https://support.google.com/docs/answer/1696711?hl=en). It does not claim full engine or layout parity. Preserve the original source when importing an unfamiliar document and read the import/export notices.

## Writing and formatting

| Capability | AlphaDocs status |
|---|---|
| Unicode editing, selection, clipboard, undo/redo | Implemented; grapheme boundaries and successive native UI frames tested. |
| Actual installed/bundled font families, sizes, bold/italic/underline/strike, colour/highlight, super/subscript | Implemented; custom wheel/HEX/RGB and up to 24 pinned colours. Each saved swatch now has a visible remove button. |
| Headings, alignment, indents, line/paragraph spacing, page breaks, columns, widow/keep controls | Implemented with measured pagination; paragraph borders/shading now retained from DOCX. |
| Bullets, numbering, interactive checklists, links, find/replace, spelling/limited grammar, word count | Implemented; proofing remains English and conservatively scoped. |
| Paper size/orientation, margins, running text, page numbers, PDF/native printing | Implemented subset; printer capabilities/platform support are documented separately. |
| Multiple files and content tabs, local recovery/autosave, export | Implemented; independent file selection/history retained. |
| Comments, local signatures and live collaboration | Implemented native workflows; standard Word comment threads, certified signatures and tracked changes are separate unsupported features. |

## Tables

Insert/delete/move tables, add/delete rows/columns, select a row/column, copy and format selected cells, and undo are implemented. **Cell background and borders…** changes a cell, selected row or selected column. Fill-only changes preserve each cell’s existing edges. Border colour and width are physical document styling, independent of Light/Dark mode; zero width removes the edges. Screen, PDF, DOCX and rich export table styles use these values.

**Formula…** replaces the current cell with a result. **Recalculate table** updates formula cells together, or leaves the table untouched if any calculation fails. Editing a result replaces its formula; Undo restores it. This follows Word’s explicit table-field workflow, not spreadsheet-style continuous recalculation. [Word table formulas](https://support.microsoft.com/en-us/word/use-a-formula-in-a-word-table).

Supported: arithmetic `+ - * / ^`, parentheses, cell references `A1`, ranges `A1:B3`, `ABOVE/BELOW/LEFT/RIGHT`, and `SUM/PRODUCT/AVERAGE/COUNT/MIN/MAX/ABS/INT/ROUND`. Number formatting is automatic or 0–8 decimals. Circular references, invalid cells, overflow and division by zero produce errors before changes are committed. Inputs/work are bounded. Currency/thousands parsing, Word’s full function/mask language and spreadsheet-style reference adjustment when rows move are not implemented. Imported unfamiliar Word number masks and their cached results are preserved; recalculation asks for an explicitly supported format.

DOCX uses standard Word formula fields and editable table cells. Other formats retain calculated values. Merged cells, nested table geometry, arbitrary column widths and separate paragraph styles within one cell remain unsupported. Nested cell text is retained with a warning; multiple paragraphs become soft line breaks.

## Equations, text boxes and shapes

Insert → **Equation** edits a bounded LaTeX source with a preview, size and colour. Common fractions, roots, powers, sums and integrals are supported. Invalid/unsupported input reports an error instead of silently discarding parts. DOCX writes standard editable Office Math (OMML), with explicit font size/colour; supported OMML imports back into editable source. This is a mathematical expression editor, not a symbolic equation solver. [Word equation workflow](https://support.microsoft.com/en-us/word/training/write-an-equation-or-formula).

Insert → **Text box and shapes** offers text box, rectangle, ellipse, line and arrow. Edit text, font, size, emphasis, colour/alignment, fill, border and dimensions; select **Edit object…** to reopen the source. Drag the object/handles to position/resize it, committing one undoable edit. Resizing a text box reflows its text. DOCX uses standard editable WordprocessingShape objects. Standard geometry/source wins when an external editor changes it. PDF uses vector outlines; other rich exports use pictures with a notice. [Google drawing/text-box workflow](https://support.google.com/docs/answer/179740?hl=en).

These objects use the existing block-anchored placement system. True inline character atoms, arbitrary page anchoring, rotation, groups, arbitrary/custom shapes and objects inside table cells/headers are unsupported. Equations export as standalone left-aligned blocks; advanced wrapping/effects on an equation are not represented in DOCX. Legacy VML shape geometry is unsupported, but text-box contents are retained as editable paragraphs with a warning. Unsupported math retains readable mathematical tokens with a warning. Drawing text uses a paragraph with soft line breaks; independent multi-paragraph styles and full complex-script shaping parity are not claimed. Drawing text is edited through its source dialog; ordinary body Find/proofing does not inspect its internal source.

## DOCX loss prevention and limits

The reported “body pages become lines” investigation found a destructive importer branch: any paragraph with `pBdr` became a horizontal rule, including paragraphs containing story text and empty/nil border overrides. The importer now retains that text and styles and renders its actual border. A narrow empty-paragraph rule representation remains readable. Regression tests import, edit, undo, export and reopen Unicode story/table content.

The export audit also corrected OOXML run, paragraph, section and cell property ordering, border ordering and mandatory table grids. Whole-package Microsoft Open XML SDK validation covers the original story fixture, coloured table/formula, Office Math and all five shapes. Content-control/custom-XML wrappers and supported AlternateContent branches retain text without duplicating fallback content. Multiple drawings/equations in one paragraph are retained as separate objects; inline placement is approximated. Foot/endnote text is retained as labelled editable paragraphs at the end, rather than dynamic page notes. Merged cells, complex sections, tracked-change history and standard Word comments remain unsupported and are reported.

The export includes a modern Word compatibility setting so saving does not invite a downgrade of modern shapes. LibreOffice currently misplaces some WPS text and line geometry; it is not a verified equivalent layout path.

A valid DOCX is not a guarantee of identical layout in every reader. Font availability and each reader’s pagination affect appearance. Keep `.adocs` for the complete native document, sources and annotations. Older AlphaDocs cannot understand schema-11 editable objects; live-editing peers must both update to this version.

## Verification scope

Verification uses synthetic documents covering the supported features. Compatibility with every existing DOCX file is not guaranteed. Native Windows, Microsoft Word, independent LibreOffice rendering, SDK/package checks and Rust regression results are recorded in `docs/QA_STAGE28.md`. ONLYOFFICE runtime and physical Mac/Linux UI verification must not be inferred from those checks.
