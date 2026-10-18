# AlphaDocs 0.1.6

- Fixed DOCX imports replacing bordered paragraphs with horizontal lines and losing their text. Paragraph borders, shading and editable content now survive supported Word round trips.
- Fixed Windows updates failing because the installer kept the installation folder in use.
- Added visible delete buttons for saved custom colours.
- Added table cell, row and column backgrounds, border colours and border widths.
- Added table formulas with cell references, ranges and common calculation functions, plus undoable table recalculation.
- Added editable equations, text boxes, rectangles, ellipses, lines and arrows, with text formatting, positioning and resizing.
- Improved Word equation and shape compatibility, horizontal-line geometry and symbol rendering inside text boxes.
- Fixed informational dialogs moving the document away from the current reading position.

DOCX exports use editable Word formula fields, Office Math and standard shapes; PDF exports preserve vector graphics. [Compatibility and supported formats](https://github.com/AlphaWorkroom/AlphaDocs/blob/main/docs/EDITOR_COMPATIBILITY.md).

Both devices must update to this version to collaborate on documents containing editable equations and shapes.
