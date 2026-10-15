"""Make the README GIFs and a silent 1080p promo from actual native captures.

Requires Pillow and ffmpeg. UI screenshots are never redrawn or fabricated.
All document text in these captures is original demonstration material.
"""
from pathlib import Path
import subprocess
from PIL import Image, ImageDraw, ImageFont, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "screenshots"
W, H, FPS = 1920, 1080, 24

def shot(name):
    return Image.open(OUT / (name + ".png")).convert("RGB")

def gif(name, names, durations):
    frames = [shot(n).resize((1080, 720), Image.Resampling.LANCZOS) for n in names]
    # One shared palette avoids palette flashes between native UI states.
    sheet = Image.new("RGB", (1080, 720 * len(frames)))
    for index, frame in enumerate(frames):
        sheet.paste(frame, (0, index * 720))
    palette = sheet.quantize(colors=256)
    frames = [frame.quantize(palette=palette, dither=Image.Dither.NONE) for frame in frames]
    frames[0].save(OUT / (name + ".gif"), save_all=True, append_images=frames[1:], duration=durations, loop=0, optimize=True, disposal=2)

def font(size, bold=False):
    return ImageFont.truetype(str(ROOT / "assets/fonts" / ("NotoSans-Bold.ttf" if bold else "NotoSans-Regular.ttf")), size)

background = Image.new("RGB", (W, H))
draw = ImageDraw.Draw(background)
for y in range(H):
    color = tuple(int(a + (b-a)*y/H) for a,b in zip((235,244,255),(248,251,255)))
    draw.line((0,y,W,y), fill=color)
icon = Image.open(ROOT / "assets/alphadocs-icon.png").convert("RGBA")

def card(name, number, title, detail, foot):
    frame = background.copy()
    d = ImageDraw.Draw(frame)
    mark = icon.resize((100,100), Image.Resampling.LANCZOS)
    frame.paste(mark,(82,105),mark)
    d.text((82,239),"ALPHADOCS  /  "+number,font=font(22,True),fill="#2466a3")
    size=67
    while d.multiline_textbbox((0,0),title,font=font(size,True),spacing=4)[2]>510:
        size-=1
    d.multiline_text((78,330),title,font=font(size,True),fill="#203348",spacing=4)
    d.multiline_text((82,575),detail,font=font(28),fill="#536b83",spacing=13)
    d.text((82,922),"AlphaDocs by Alpha Workroom",font=font(21,True),fill="#203348")
    d.text((82,966),foot,font=font(19),fill="#536b83")
    native = shot(name).resize((1200,800),Image.Resampling.LANCZOS)
    mask=Image.new("L",native.size); ImageDraw.Draw(mask).rounded_rectangle((0,0,1199,799),radius=20,fill=255)
    shadow=Image.new("RGBA",(W,H)); sd=ImageDraw.Draw(shadow)
    sd.rounded_rectangle((642,140,1842,940),radius=22,fill=(32,51,72,45))
    frame=Image.alpha_composite(frame.convert("RGBA"),shadow.filter(ImageFilter.GaussianBlur(20))).convert("RGB")
    frame.paste(native,(630,124),mask)
    return frame

def closing():
    frame=background.copy(); d=ImageDraw.Draw(frame)
    mark=icon.resize((210,210),Image.Resampling.LANCZOS);frame.paste(mark,(855,135),mark)
    d.text((W/2,385),"AlphaDocs",font=font(94,True),fill="#203348",anchor="mt")
    d.text((W/2,530),"A familiar editor. Your own documents.",font=font(42),fill="#536b83",anchor="mt")
    d.rounded_rectangle((570,650,1350,744),radius=47,fill="#d3e4ff")
    d.text((W/2,669),"Windows  ·  Linux  ·  macOS",font=font(34,True),fill="#165eaa",anchor="mt")
    d.text((W/2,823),"github.com/AlphaWorkroom/AlphaDocs",font=font(31),fill="#203348",anchor="mt")
    d.text((W/2,930),"Built with Rust  ·  Offline editing  ·  By Alpha Workroom",font=font(22),fill="#536b83",anchor="mt")
    return frame

def main():
    gif("chapters",["book","book-2","book-3"],[2400,2400,2400])
    gif("tabs",["main","brief","book"],[2400,2400,2400])
    gif("crop",["crop-before","crop","crop-before"],[1800,2600,1800])
    gif("sharing",["brief","sharing"],[2200,4200])
    scenes=[
        (4,card("main","01","Room for\nyour ideas.","Familiar tools. Clear pages.\nWrite and save completely offline.","No account needed for local editing.")),
        (4,card("brief","02","Make it\nyour own.","Shape a brief with headings,\ncolour, pictures and editable tables.","Original document. Real native app.")),
        (2,card("book","03","Every chapter.\nOne book.","Give chapters their own\ncontent tabs inside a document.","Separate chapters. One .adocs file.")),
        (2,card("book-2","03","Keep your\nplace.","Move between chapters\nwithout losing the story.","Actual chapter-tab navigation.")),
        (2,card("book-3","03","Build the\nnext chapter.","Keep characters, research\nand notes close at hand.","Book text written for this demonstration.")),
        (2,card("main","04","More documents.\nOne window.","Switch open files with tabs.\nKeep each document’s own history.","Documents and chapter tabs are separate.")),
        (2,card("brief","04","Stay in\nyour flow.","Your brief, book and notes\nare only a tab away.","Native Rust. No embedded browser.")),
        (2,card("crop-before","05","Lose the\nempty edges.","Trim transparent picture borders\nwith one reversible setting.","The original image stays embedded.")),
        (3,card("crop","05","Keep the\noriginal.","Refine the picture.\nChange your mind later.","Reversible transparency crop.")),
        (4,card("sharing","06","Work with\nother people.","Encrypted direct sharing\nwith edit or view access.","LAN/VPN required. Host stays open.")),
        (5,closing()),
    ]
    scenes[0][1].save(OUT/"promo-cover.png")
    total=sum(int(seconds*FPS) for seconds,_ in scenes)
    output=OUT/"AlphaDocs-Promo.mp4"
    command=["ffmpeg","-hide_banner","-loglevel","error","-y","-f","rawvideo","-pix_fmt","rgb24","-s",f"{W}x{H}","-r",str(FPS),"-i","pipe:0","-an","-c:v","libx264","-preset","fast","-crf","21","-pix_fmt","yuv420p","-movflags","+faststart",str(output)]
    process=subprocess.Popen(command,stdin=subprocess.PIPE)
    count=0
    for seconds,scene in scenes:
        n=int(seconds*FPS)
        for index in range(n):
            alpha=min(1.0,index/8,(n-1-index)/8)
            frame=Image.blend(background,scene,max(0,alpha)) if alpha<1 else scene.copy()
            ImageDraw.Draw(frame).rectangle((0,H-5,int(W*(count+1)/total),H),fill="#478fd8")
            process.stdin.write(frame.tobytes())
            count+=1
    process.stdin.close()
    if process.wait(): raise SystemExit("Video encoding failed")
    print(f"Rendered {total/FPS:g}s, {W}x{H}, {output.stat().st_size:,} bytes")

if __name__=="__main__": main()
