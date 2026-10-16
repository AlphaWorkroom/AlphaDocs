# AlphaDocs

**A familiar document editor. Completely offline.**

Built with Rust · AI-Free editing · Offline documents  
**AlphaDocs by Alpha Workroom**

![AlphaDocs Interface](screenshots/main.png)

## See it in action

[![Watch the AlphaDocs interaction trailer](screenshots/interaction-preview.gif)](screenshots/AlphaDocs-Interaction-10s.mp4)

[Watch the 10-second trailer with sound](screenshots/AlphaDocs-Interaction-10s.mp4) · 1080p / 60 fps MP4. Follow the cursor through real chapter tabs, open documents and direct LAN/VPN sharing, with camera close-ups, click feedback, snapping panels and original click/swish/snap sound design. The GIF above is a silent preview.

The [editable EffectCraft project](promo/AlphaDocs-Interaction-10s) includes the source captures, tracked vector cursor, native camera/keyframes, fonts and four separate sound stems.

Try the editable examples: [the book](examples/documents/The%20Lighthouse%20Atlas.adocs), [the illustrated guide](examples/documents/Room%20for%20your%20ideas.adocs), or [the project brief](examples/documents/Studio%20launch%20brief.adocs). Download the raw `.adocs` file, then open it in AlphaDocs.

## Download

Get installers and portable builds from [GitHub Releases](https://github.com/AlphaWorkroom/AlphaDocs/releases/latest).

| Platform | Installer | Portable |
| --- | --- | --- |
| Windows 10/11, 64-bit | [Windows Setup](https://github.com/AlphaWorkroom/AlphaDocs/releases/latest/download/AlphaDocs-Windows-x86_64-Setup.exe) | [ZIP](https://github.com/AlphaWorkroom/AlphaDocs/releases/latest/download/AlphaDocs-Windows-x86_64-Portable.zip) |
| Linux, x86-64, Ubuntu 22.04+ / compatible Debian systems | [DEB](https://github.com/AlphaWorkroom/AlphaDocs/releases/latest/download/AlphaDocs-Linux-x86_64.deb) | [tar.gz](https://github.com/AlphaWorkroom/AlphaDocs/releases/latest/download/AlphaDocs-Linux-x86_64.tar.gz) |
| macOS 14+, Apple silicon | [PKG](https://github.com/AlphaWorkroom/AlphaDocs/releases/latest/download/AlphaDocs-macOS-aarch64.pkg) | [App bundle](https://github.com/AlphaWorkroom/AlphaDocs/releases/latest/download/AlphaDocs-macOS-aarch64.tar.gz) |
| macOS 14+, Intel | [PKG](https://github.com/AlphaWorkroom/AlphaDocs/releases/latest/download/AlphaDocs-macOS-x86_64.pkg) | [App bundle](https://github.com/AlphaWorkroom/AlphaDocs/releases/latest/download/AlphaDocs-macOS-x86_64.tar.gz) |

The Windows installer requires .NET Framework 4.8. Downloads include the app, its licence and dependency notices; Rust is needed only to build from source. macOS builds currently use ad-hoc app signatures and unsigned installers, and are **not Apple-notarised**. See [installation and platform notes](docs/INSTALLATION.md).

## Features

### Multiple documents. One window.

Keep multiple documents open using tabs, without needing separate windows. Each document keeps its own selection, undo history and save state. Organise sections with document-content tabs in the sidebar.

![Switching between a guide, a project brief and a book](screenshots/tabs.gif)

### Every chapter. One book.

Give chapters their own content tabs inside a single document. Keep character notes and research alongside the story, and use the heading outline to jump to a section. The example below uses serif body text, a chapter title and a small blue running label.

![Navigating three chapters of The Lighthouse Atlas](screenshots/chapters.gif)

### From a first draft to a finished brief.

Use your own type, colour and spacing. Add an editable table for a project plan, or combine headings, text and pictures in an illustrated guide.

![A formatted project brief with milestones and a native editable table](screenshots/brief.png)

### Crop to Transparency

Automatically remove invisible image borders without permanently changing the original image. Adjust crop, transparent padding, opacity, brightness and contrast, or reset the image later.

![Removing and restoring transparent image borders](screenshots/crop.gif)

### Share directly with other people.

Start an encrypted sharing session and privately send an invitation with **edit** or **view** access. Work on the same document with live edits and collaborator selections, or let someone read it while you write. Each participant retains a local document copy.

Sharing currently needs a reachable **LAN or VPN**, and the host must keep AlphaDocs open. It does not use hosted cloud storage, account sign-in or email delivery. [How direct sharing works](docs/COLLABORATION.md).

![The native sharing and join-session interface](screenshots/sharing.gif)

### Your documents stay yours.

Write and edit completely offline, without relying on cloud services, an account or an AI writing service. Native **`.adocs`** files retain editable formatting, content tabs, original pictures and local annotations. Local autosave is optional; Save and version history keep work on your computer.

### Familiar tools, native performance.

- Measured pages, headings, lists, links, editable tables and find/replace.
- Offline Australian English spelling and basic grammar checks, with squiggles and right-click suggestions.
- Reversible image editing, wrap/behind/front placement and measured alignment guides.
- Text/image comments, replies, local reactions and emoji insertion.
- Light/dark/system appearance, feature search and keyboard shortcuts.
- Native `.adocs`, DOCX and plain-text opening; `.adocs`, DOCX, PDF, ODT, TXT, RTF, zipped HTML, EPUB and Markdown downloads.
- Native Windows printing with driver capabilities and preview; PDF preparation on other platforms.
- Optional encrypted direct-peer sharing on a LAN/VPN, without an AlphaDocs account or hosted backend. [Read the collaboration scope](docs/COLLABORATION.md).

Document editing needs no connection. Public GitHub update checks are enabled by default, run at most once a day and can be disabled in Preferences. **Help → Check for updates** checks manually. Only release metadata is requested; document content is never sent. Updates are offered as a matching GitHub download and are installed when you choose to run the installer.

## Compatibility

AlphaDocs is a native Rust desktop application, with no Electron, embedded browser or web editor. Its own measured layout is shared by the canvas and PDF export.

DOCX/HTML interchange supports a documented subset, rather than every Word/browser layout. Rotation, true inline image atoms, merged/nested tables, tracked changes, broad language analysis and macOS/Linux native printer backends are not implemented. Additional export formats do not imply corresponding import support. TXT omits formatting/pictures; web/EPUB reflow, and Markdown uses a companion picture folder. Interfaces and fonts can vary between operating systems.

Release CI builds and tests native installers for each listed platform. Windows has visual interaction QA; automated cross-platform checks are not a claim of identical rendering on every device.

## Build and contribute

```sh
git clone https://github.com/AlphaWorkroom/AlphaDocs.git
cd AlphaDocs
cargo build --locked --release
```

See [development and release instructions](docs/DEVELOPMENT.md) for native dependencies, tests, packaging and the release workflow. Keep build caches out of distribution archives. Report bugs and request features through [GitHub Issues](https://github.com/AlphaWorkroom/AlphaDocs/issues).

## Licence

Copyright © 2026 **Alpha Workroom**. Distributed under the supplied [AlphaDocs Free Use & Non-Monetisation Licence, Version 1.1](LICENSE).

Ordinary personal, educational, workplace and business use is permitted, including earning income from documents you create. Selling or monetising the software itself requires written permission; public downloadable copies must be distributed through GitHub. The full licence controls. This is a source-available project under custom terms, rather than a claim of an OSI-approved open-source licence.

Third-party components retain their own licences. Fonts, dictionary and artwork notices are in `assets/` and `licenses/`; resolved dependency notices and required source archives accompany packaged downloads. AlphaDocs and Alpha Workroom branding remain with their rights holders.
