# AlphaDocs 0.1.3

- All 11 paper presets plus custom dimensions, with portrait/landscape measured layout and a redesigned Page setup. Pages/Pageless, saved defaults and the existing columns/running-text controls are included.
- New native writing tools: manual basic citations/bibliography text, independent text comparison reports, body line numbers, personal dictionary controls, optional typing substitutions and online word lookup. Advanced tracked edits, verified signature requests and externally linked objects remain visibly disabled. [Full scope and limits](https://github.com/AlphaWorkroom/AlphaDocs/blob/main/docs/WRITING_TOOLS.md).
- Translation stays hidden unless enabled in Preferences. Clicking Translate sends document text to MyMemory and creates a separate copy; the anonymous service has a 5,000-character daily quota. Inline styles, links and text annotations reset in the translated copy; source content remains intact.
- Autosave now waits until editing has stopped for 30 seconds. A prominent Preferences card clearly enables/disables it; explicit Save, closing and switching still preserve work.
- macOS uses native traffic lights in the integrated header. Large/double arrows use a bundled real font in the editor and PDF; recognised pasted arrow codes are repaired without changing literal code examples.
- New matching update UI with download byte progress, cancellation, SHA-256 verification, saved-document handoff, platform installation and relaunch. Native browser links work again. Installed layouts and OS authorization requirements are described in [update notes](https://github.com/AlphaWorkroom/AlphaDocs/blob/main/docs/UPDATES.md).

Install this version once using the matching installer if your older app's download button does nothing. The new in-app installer flow applies to future updates from 0.1.3 onward. macOS apps remain ad-hoc signed and **not Apple-notarised**; the updater does not bypass Gatekeeper. Native CI and isolated installer smoke checks are not a claim of physical Mac appearance, assistive-hardware testing or a security audit.

# AlphaDocs 0.1.2

- Native macOS Apple silicon and Intel DMGs: drag the app onto Applications. Packages retain all original licences/notices and stay below 100 MB. Legacy PKGs remain for older update clients.
- Mac Command shortcut labels and undo/redo, Option word movement/deletion, Command line/document navigation and undoable line deletion. Control-click opens real context menus; two-finger secondary clicks remain supported.
- Help now groups shortcuts by task with separate, plain-language descriptions. Help and the GitHub page link to the owner's feedback/bug-report form; nothing is submitted automatically.
- Opt-in nearby-device discovery and encrypted live-editing invitations. Choose a device, send an edit/view invitation and wait for the receiver to choose Join. Copy/paste invitations remain available when discovery is blocked.
- A clearer Working together roster shows names and content tabs; larger, staggered caret/selection badges use matching colours.
- Box-drawing characters and arrows now use the bundled symbol fallback in the editor and PDF. Pasted text diagrams preserve spaces, use a real monospace font and fit the available column using measured advances. Known arrow entities are repaired on paste.
- Proofing skips web addresses and linked text, and recognises hyphenated prefixes such as Pre-Launch while still checking the following word.

macOS apps remain ad-hoc signed, **not Developer ID signed or Apple-notarised**. A DMG does not bypass Gatekeeper. The installation guide includes Apple's per-app Open Anyway steps. Apple signing/notarisation credentials are required to remove the unidentified-publisher warning.

Nearby discovery shares chosen device metadata on the LAN; private invitations and documents use encrypted direct connections. Device names are not verified identities; compare short device IDs. Nearby invitations expire after 60 seconds; underlying session invitations still expire within one hour. No relays, hosted storage or account authentication are configured. New nearby discovery has automated loopback/native-host tests; the existing LAN collaboration was additionally confirmed by the user between a Mac and Windows PC. This does not claim visual or Gatekeeper approval testing on a physical Mac.

Windows printing is implemented; Linux/macOS native print backends remain outstanding. Interchange support remains the documented subset.
