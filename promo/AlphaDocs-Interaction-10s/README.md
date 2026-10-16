# Follow the idea — AlphaDocs interaction trailer

The revised **10-second, 1920 × 1080, 60 fps** trailer is built around app actions.
It replaces the earlier floating-card edit with one continuous camera view of the
native application, an animated vector cursor and click-synchronised UI changes.

Open `AlphaDocs-Interaction-10s.ecproj` in **EffectCraft 0.10+**. Keep `assets`
beside the project; its footage paths are relative. The cursor is an editable
native shape imported from the included original SVG. The source-space cursor,
click rings and screenshots share one nested composition, and one camera layer
zooms/pans that composition. The pointer therefore stays on the actual button
through the move. Camera motion is sampled from smooth curves at 60 fps; short
overshoots settle into each stop. The invitation panel has its own snap animation.

| Time | Action |
| --- | --- |
| 0–1.25 s | Establish the app, zoom into chapter tabs, move to chapter two |
| 1.25 s | Click chapter two; the real captured selection/content changes |
| 1.5–2.3 s | Follow through to the chapter title |
| 2.35–3.1 s | Travel up to the open-file tabs; click the illustrated guide |
| 3.35–4 s | Pull back to the formatted guide |
| 4.1–4.85 s | Follow the cursor to the brief tab and click |
| 5–6.1 s | Track across the toolbar to Share and click |
| 6.1–7.7 s | Follow the opening invitation panel and settle |
| 7.8–10 s | Paper wipe into the AlphaDocs brand ending |

Four separate, editable, original 48 kHz stereo sound stems are included:
**music, clicks, camera whooshes and settling snaps**. Clicks are foreground
transients on the exact interaction frames, rather than quiet details in a music
mix. The music and effects use original procedural synthesis; no stock samples,
Google audio or Google artwork are included. Play the MP4 with sound; the README
GIF is a silent preview.

All UI pixels come from the real AlphaDocs native Windows application, with the
repository's original book, guide and brief documents. The system pointer is
parked below the video crop before capture. The edited vector cursor recreates
the verified navigation gestures; this is an animated UI demonstration, not an
unedited screen recording. Sharing shows native LAN/VPN invitation controls and
does not simulate connected collaborators or expose a real invitation/address.

## Rebuild

Requires EffectCraft 0.10+, Python 3.10+, numpy and ffmpeg. From this directory:

```powershell
python build_trailer.py --preview --render
```

Use `--effectcraft PATH` or `EFFECTCRAFT_CLI` for a nonstandard install. Project
construction uses the actual local EffectCraft stdio MCP. The native renderer
produces the full edit/audio; ffmpeg only remuxes the final MP4 for streaming.
Opening/editing the project does not require Python or ffmpeg. Included Noto Sans
Regular/Bold fonts keep their unchanged SIL Open Font License; install those fonts
or set `EFFECTCRAFT_FONT_DIRS` if they are unavailable in EffectCraft.

First-party assets/project/audio follow the included AlphaDocs Free Use &
Non-Monetisation Licence 1.1. This marketing project is not part of the app's
runtime or installer payload.
