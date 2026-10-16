# Install AlphaDocs

Download from the project's [GitHub Releases](https://github.com/AlphaWorkroom/AlphaDocs/releases/latest). Packages include the supplied application licence and third-party notices. Keep native documents as `.adocs` to retain every editable feature.

## Windows

Run the 64-bit Setup executable, read the licence and choose Install. It installs for the current user, without administrator access, under `%LOCALAPPDATA%/Programs/AlphaDocs`. It offers a desktop shortcut and Open-with registration for `.adocs`. .NET Framework 4.8 is required for Setup; the app itself does not require Rust. Alternatively, extract the portable ZIP and run `AlphaDocs.exe`.

Close AlphaDocs before upgrading. Previous install directories remain as recoverable sibling backups. Documents and preferences are separate in the platform application-data directory. Older native files remain readable; Save offers a new `.adocs` target. No user library is bundled with a download.

## Linux

The DEB targets x86-64 Ubuntu 22.04+ and compatible Debian systems with glibc 2.35 or newer. Install using your package manager, for example `sudo apt install ./AlphaDocs-Linux-x86_64.deb`, then launch AlphaDocs from the application menu. It installs under `/opt/alphadocs` and registers the icon and `.adocs` MIME type. Alternatively, extract the tar archive and run `AlphaDocs/alphadocs`. Native display/OpenGL libraries and a working desktop session are required. File dialogs use a desktop portal; install your desktop's XDG portal backend if necessary.

## macOS

Choose Apple silicon (`aarch64`) or Intel (`x86_64`) for macOS 14 or newer. Open the **DMG**, drag **AlphaDocs.app** onto **Applications**, eject the disk image, then open AlphaDocs from Applications. The disk includes a Read Me with installation and Mac controls. The app bundle retains the exact licence and dependency notices. The tar archive and legacy PKG remain available; new update checks prefer the DMG.

The app is **not Developer ID signed or Apple-notarised**. An ad-hoc signature checks code integrity but does not identify an Apple-verified publisher. Changing PKG to DMG does not remove Gatekeeper's warning. After trying to open the installed app, if you trust the official download, go to **System Settings → Privacy & Security → Open Anyway** for AlphaDocs and confirm Open. This is Apple's per-app exception; do not disable Gatekeeper globally. [Apple's opening instructions](https://support.apple.com/en-us/102445). Managed Macs may restrict this option. Seamless distribution requires the owner's Apple Developer ID certificate and notarisation credentials; none are configured or included.

**Mac controls:** Command+C/V/X/A copies, pastes, cuts or selects. Command+Z undoes; Command+Shift+Z redoes. Command+S saves. Option+Left/Right moves by word; Command+Left/Right moves to measured line boundaries; Command+Up/Down moves to content-tab bounds. Add Shift to select. Option+Delete deletes a word; Command+Delete deletes to the start of the line. Command+Shift+H opens Find and replace, preserving Command+H for macOS Hide. Control-click or a two-finger trackpad click opens context menus, including spelling corrections, image options and table commands. Menus, tooltips and Help show Mac shortcut labels.

## Updates and privacy

On startup, automatic checks request the latest public release from the fixed `AlphaWorkroom/AlphaDocs` GitHub repository, at most once per day. They do not send document text, filenames, account details, clipboard content or credentials. GitHub receives an ordinary HTTPS request and app-version User-Agent. Network failures never prevent writing or saving. Preferences turns automatic checks off; Help → Check for updates remains available manually.

Only a newer stable semantic version is offered. Drafts, prereleases, older versions, malformed metadata and foreign release URLs are rejected. The button opens the matching official GitHub installer, or its release page when no matching installer exists. It does not silently download, replace or execute software. Read the release notes, close the app and install the update yourself.

## Platform scope

Windows printer discovery, capability controls and spool submission are implemented. Other platforms retain Save as PDF preparation; native printer backends remain outstanding. Fonts, dialogs and desktop integration depend on the OS. Automated build/self-check/installer tests do not replace visual QA on every desktop.
