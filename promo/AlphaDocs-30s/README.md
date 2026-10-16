# Follow the idea — AlphaDocs, 30 seconds

**1920 × 1080 · 60 fps · stereo sound · native editable EffectCraft project**

This revision retains the approved 10-second trailer's continuous camera,
curved cursor paths, settling overshoots and snapping invitation panel. The
camera trips are about 25% slower; most of the additional time is a pause for
reading and looking at the application. It also adds a real cursor-led journey
through Preferences into Dark, under **“Tired eyes? Switch to dark mode!”**.
That wording stays on screen for 6.8 seconds, changing colour with the theme.

Open `AlphaDocs-30s.ecproj` in **EffectCraft 0.10+** and keep `assets` alongside
it. Footage paths are relative. Native text, vector cursor/click rings, masks,
camera transforms, keyframes and four audio stems remain editable. Both the
cursor and captured UI live inside the same nested composition, so zooms keep
the pointer attached to its actual target.

| Time | What happens |
| --- | --- |
| 0–4 s | Establish AlphaDocs, follow the cursor into chapter two |
| 4–6.2 s | Hold the chapter title and formatted book |
| 6.2–10.7 s | Switch open-file tabs; show the illustrated guide |
| 10.7–14.6 s | Switch to the project brief and travel to Share |
| 14.6–18.9 s | Snap open the real invitation panel; explain LAN/VPN scope |
| 18.9–22 s | Close sharing, open Preferences and move to Dark |
| 22–25.8 s | Click Dark, close Preferences and hold the dark workspace |
| 25.8–30 s | Paper wipe into the AlphaDocs brand and platform ending |

The chapter, guide and brief are the repository's original example documents.
Every application capture comes from the real native Windows app. A separate
test profile isolates the demonstration from personal documents and preferences.
The system pointer is parked below the video crop; the editable vector cursor
recreates verified UI actions. This is an animated demonstration, rather than an
unedited recording. The sharing panel shows invitation controls without a real
invitation/address or simulated connected people.

The four original **48 kHz stereo** stems are music, clicks, whooshes and snaps.
They are procedurally synthesised, with click transients on the exact action
frames. The music is composed across the full 30 seconds; it is not a slowed
10-second recording. No stock/Google audio, artwork or proprietary document
text is included. The GIF on the repository page is silent; use the MP4 for sound.

## Rebuild

Requires EffectCraft 0.10+, Python 3.10+, numpy and ffmpeg:

```powershell
python build_trailer.py --preview --render
```

Set `EFFECTCRAFT_CLI` or pass `--effectcraft PATH` for another installation.
The script constructs the project through EffectCraft's real local stdio MCP,
then renders video and sound using its native renderer. ffmpeg remuxes that
output for streaming. Opening/editing the project needs neither Python nor
ffmpeg. Install the included Noto Sans faces or set `EFFECTCRAFT_FONT_DIRS` to
`assets/fonts` if those fonts are not available in your EffectCraft installation.

The supplied blue book logo is unchanged. Included Noto Sans Regular/Bold retain
their separate, unchanged SIL Open Font License. First-party project, captures
and audio use the included AlphaDocs Free Use & Non-Monetisation Licence 1.1.
Marketing sources and renders are not bundled into the application installer.
