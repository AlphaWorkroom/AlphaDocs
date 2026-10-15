# Install AlphaDocs

Download from the project's [GitHub Releases](https://github.com/AlphaWorkroom/AlphaDocs/releases/latest). Packages include the supplied application licence and third-party notices. Keep native documents as `.adocs` to retain every editable feature.

## Windows

Run the 64-bit Setup executable, read the licence and choose Install. It installs for the current user, without administrator access, under `%LOCALAPPDATA%/Programs/AlphaDocs`. It offers a desktop shortcut and Open-with registration for `.adocs`. .NET Framework 4.8 is required for Setup; the app itself does not require Rust. Alternatively, extract the portable ZIP and run `AlphaDocs.exe`.

Close AlphaDocs before upgrading. Previous install directories remain as recoverable sibling backups. Documents and preferences are separate in the platform application-data directory. Older native files remain readable; Save offers a new `.adocs` target. No user library is bundled with a download.

## Linux

The DEB targets x86-64 Ubuntu 22.04+ and compatible Debian systems with glibc 2.35 or newer. Install using your package manager, for example `sudo apt install ./AlphaDocs-Linux-x86_64.deb`, then launch AlphaDocs from the application menu. It installs under `/opt/alphadocs` and registers the icon and `.adocs` MIME type. Alternatively, extract the tar archive and run `AlphaDocs/alphadocs`. Native display/OpenGL libraries and a working desktop session are required. File dialogs use a desktop portal; install your desktop's XDG portal backend if necessary.

## macOS

Choose Apple silicon (`aarch64`) or Intel (`x86_64`) for macOS 14 or newer. The PKG installs `AlphaDocs.app` into Applications. The tar archive contains the same app bundle for manual installation.

These initial packages are **not Apple-notarised** and PKG files do not have a Developer ID Installer signature. App bundles are ad-hoc signed for code integrity, which does not establish an Apple-verified publisher. macOS may refuse to open these downloads under its normal Gatekeeper policy. No signing identity or notarisation credentials are bundled or invented. A future Developer ID/notarised release is needed for the usual seamless installation experience.

## Updates and privacy

On startup, automatic checks request the latest public release from the fixed `AlphaWorkroom/AlphaDocs` GitHub repository, at most once per day. They do not send document text, filenames, account details, clipboard content or credentials. GitHub receives an ordinary HTTPS request and app-version User-Agent. Network failures never prevent writing or saving. Preferences turns automatic checks off; Help → Check for updates remains available manually.

Only a newer stable semantic version is offered. Drafts, prereleases, older versions, malformed metadata and foreign release URLs are rejected. The button opens the matching official GitHub installer, or its release page when no matching installer exists. It does not silently download, replace or execute software. Read the release notes, close the app and install the update yourself.

## Platform scope

Windows printer discovery, capability controls and spool submission are implemented. Other platforms retain Save as PDF preparation; native printer backends remain outstanding. Fonts, dialogs and desktop integration depend on the OS. Automated build/self-check/installer tests do not replace visual QA on every desktop.
