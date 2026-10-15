# Make room — the AlphaDocs 10-second trailer

An editable **EffectCraft 0.10** motion project, built through its actual MCP server.
1920 × 1080, 60 fps, 600 video frames. No Adobe After Effects installation is required.

The footage is captured from the real native AlphaDocs Windows application using
the original book, guide and project-brief examples in this repository. Text,
masks, shadows, lighting, transforms, timing and easing remain native editable
EffectCraft layers and keyframes. There are no simulated document controls or
fabricated connected collaborators. Sharing shows the actual LAN/VPN invitation
interface. The renderer is EffectCraft's GPU engine.

## Open and edit

Open `AlphaDocs-Hyper-10s.ecproj` in EffectCraft. Keep the adjacent `assets` folder.
All footage links are relative to the project so the folder can be moved together.
The bundled Noto Sans Regular and Bold fonts are under `assets/fonts`, with their
unchanged SIL Open Font License. If those faces are missing, install the fonts or
launch EffectCraft with `EFFECTCRAFT_FONT_DIRS` pointing to that directory.

The timeline layers use numbered shot names and a separate `SOUND` track. The
music and transition sounds are an original, sample-free 144 BPM electronic ident
at 48 kHz stereo. No reference-video music or Google artwork is included.

| Time | Shot |
| --- | --- |
| 0–1.25 s | Typographic hook and illustrated page |
| 1.25–3.33 s | Three chapters in one book |
| 3.33–5 s | Three documents and the real open-file tabs |
| 5–6.25 s | Reversible transparency crop and control close-up |
| 6.25–7.92 s | Direct LAN/VPN sharing invitation |
| 7.92–10 s | AlphaDocs / Alpha Workroom brand hold |

## Rebuild

Requires Python 3.10+, numpy, EffectCraft 0.10+ and ffmpeg on PATH. From the
repository root:

```powershell
python promo/AlphaDocs-Hyper-10s/build_trailer.py --render
```

From an extracted standalone trailer folder, run `python build_trailer.py --render`.
The extracted project already contains every required asset. Its rendered MP4 is
written beside the project; repository builds write to `screenshots` instead.

For a nonstandard EffectCraft installation, pass `--effectcraft PATH` or set
`EFFECTCRAFT_CLI`. The script talks JSON-RPC over the installed native CLI's stdio
MCP connection, creates the layers, saves the project and renders through
EffectCraft. ffmpeg only remuxes that render with streaming metadata. The native
master contains 10 seconds of video; AAC padding can make the container report
10.005 seconds. This does not add video frames.

First-party project, images and audio follow the repository's AlphaDocs Free Use
& Non-Monetisation Licence 1.1. The font licence remains unchanged. This marketing
folder is not part of the application installer or its runtime payload.
