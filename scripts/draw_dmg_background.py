"""Draw the installer layout's original vector-style background at 1x/2x.
Requires Pillow only when regenerating artwork, never in the packaged app.
"""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

root = Path(__file__).resolve().parent.parent
out = root / "assets/dmg"
out.mkdir(exist_ok=True)
for scale in (1, 2):
    image = Image.new("RGB", (640 * scale, 480 * scale), "#f8fafd")
    draw = ImageDraw.Draw(image)
    def text(point, value, size, color, bold=False):
        font = ImageFont.truetype(str(root / "assets/fonts" / ("NotoSans-Bold.ttf" if bold else "NotoSans-Regular.ttf")), size * scale)
        draw.text(tuple(p * scale for p in point), value, fill=color, font=font)
    text((40, 22), "AlphaDocs", 30, "#202124", True)
    text((40, 70), "Drag the app into Applications.", 17, "#5f6368")
    points = [(286, 184), (337, 184), (325, 172), (333, 164), (360, 190), (333, 216), (325, 208), (337, 196), (286, 196)]
    draw.polygon([(x * scale, y * scale) for x, y in points], fill="#0b57d0")
    text((40, 445), "AlphaDocs by Alpha Workroom", 12, "#5f6368")
    image.save(out / ("install-background.png" if scale == 1 else "install-background@2x.png"))
