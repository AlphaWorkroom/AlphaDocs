# AlphaDocs 0.1.2

- Native macOS Apple silicon and Intel DMGs: drag the app onto Applications. Packages retain all original licences/notices and stay below 100 MB. Legacy PKGs remain for older update clients.
- Mac Command shortcut labels and undo/redo, Option word movement/deletion, Command line/document navigation and undoable line deletion. Control-click opens real context menus; two-finger secondary clicks remain supported.
- Help now groups shortcuts by task with separate, plain-language descriptions. Help and the GitHub page link to the owner's feedback/bug-report form; nothing is submitted automatically.
- Opt-in nearby-device discovery and encrypted live-editing invitations. Choose a device, send an edit/view invitation and wait for the receiver to choose Join. Copy/paste invitations remain available when discovery is blocked.
- A clearer Working together roster shows names and content tabs; larger, staggered caret/selection badges use matching colours.

macOS apps remain ad-hoc signed, **not Developer ID signed or Apple-notarised**. A DMG does not bypass Gatekeeper. The installation guide includes Apple's per-app Open Anyway steps. Apple signing/notarisation credentials are required to remove the unidentified-publisher warning.

Nearby discovery shares chosen device metadata on the LAN; private invitations and documents use encrypted direct connections. Device names are not verified identities; compare short device IDs. Nearby invitations expire after 60 seconds; underlying session invitations still expire within one hour. No relays, hosted storage or account authentication are configured. New nearby discovery has automated loopback/native-host tests; the existing LAN collaboration was additionally confirmed by the user between a Mac and Windows PC. This does not claim visual or Gatekeeper approval testing on a physical Mac.

Windows printing is implemented; Linux/macOS native print backends remain outstanding. Interchange support remains the documented subset.
