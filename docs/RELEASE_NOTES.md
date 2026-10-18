# AlphaDocs 0.1.7

- Added **Select cell** beside Select row and Select column in table menus. Copy, format or clear a complete cell, including an empty one.
- Table calculation commands are grouped near the top of the context menu.
- Formula dialogs show the target cell and a result preview. Suggested sums follow the nearby numbers, including rows beneath an empty header.
- Added **Preferences → General → Automatically recalculate table formulas**. Enable it to update results as table values change; Undo restores the input and its results together. The option is off by default.
- Invalid calculation inputs preserve cached results and display a message. **Recalculate table** remains available for manual updates.

For numbers side by side, use `=SUM(LEFT)`. For numbers in a column, use `=SUM(ABOVE)`.
