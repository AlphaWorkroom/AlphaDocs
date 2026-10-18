# AlphaDocs 0.1.8

- Fixed Windows in-app updates rejecting portable or renamed copies downloaded from GitHub. Updates install into the standard per-user AlphaDocs folder and relaunch the installed app; the original portable folder is retained.
- The update window explains the installation destination and directs portable users to the Start-menu app afterwards.
- Added regression checks for installed, portable and renamed clients, including a fresh installation from a portable copy.

Versions 0.1.6 and 0.1.7 contain the old folder restriction. If one of those versions shows “Install with AlphaDocs Setup first”, download and run the Windows Setup once, then open AlphaDocs from the Start menu. The document library stays in place.
