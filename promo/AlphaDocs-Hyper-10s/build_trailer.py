"""Build the editable 10-second trailer using EffectCraft's actual stdio MCP.

No application UI is fabricated: all picture layers are native app captures.
Requires EffectCraft 0.10+, Python 3.10+, numpy; ffmpeg is only used for delivery
metadata/fast-start after the native EffectCraft renderer has produced the edit.
"""
import argparse
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
import wave

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
ASSETS = HERE / 'assets'
PROJECT = HERE / 'AlphaDocs-Hyper-10s.ecproj'
COMP = 'AlphaDocs | Make room | 10s'
B = 60 / 144


class EffectCraft:
    def __init__(self, executable):
        self.process = subprocess.Popen(
            [executable, 'mcp', '--empty', '--gpu'], stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=open(HERE / 'build-engine.log', 'w', encoding='utf-8'),
            text=True, encoding='utf-8',
            env=dict(os.environ, EFFECTCRAFT_FONT_DIRS=str(ASSETS / 'fonts')))
        self.sequence = 0
        self.calls = []
        self.rpc('initialize', {'protocolVersion': '2025-03-26', 'capabilities': {},
                 'clientInfo': {'name': 'AlphaDocs-trailer', 'version': '1.0'}})
        self.process.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
        self.process.stdin.flush()

    def rpc(self, method, params):
        self.sequence += 1
        self.process.stdin.write(json.dumps({'jsonrpc': '2.0', 'id': self.sequence,
                                 'method': method, 'params': params}) + '\n')
        self.process.stdin.flush()
        while True:
            line = self.process.stdout.readline()
            if not line:
                raise RuntimeError('EffectCraft closed its MCP connection')
            result = json.loads(line)
            if result.get('id') == self.sequence:
                if 'error' in result:
                    raise RuntimeError(result['error'])
                return result['result']

    def tool(self, name, **arguments):
        result = self.rpc('tools/call', {'name': name, 'arguments': arguments})
        if result.get('isError'):
            raise RuntimeError(f'{name}: {result}')
        value = json.loads(result['content'][0]['text'])
        self.calls.append({'tool': name, 'arguments': arguments})
        return value

    def cmd(self, command, **params):
        return self.tool('execute_command', command=command, params=params)

    def close(self):
        self.process.stdin.close()
        self.process.wait(timeout=15)


def soundtrack():
    """Original 144 BPM electronic ident; no samples or external music."""
    rate = 48000
    n = rate * 10
    out = np.zeros((n, 2), dtype=np.float64)
    rng = np.random.default_rng(24016)

    def put(t0, sound, gain=1, pan=0):
        offset = round(t0 * rate)
        sound = np.asarray(sound)
        end = min(n, offset + len(sound))
        if end > offset:
            p = min(1, max(-1, pan))
            out[offset:end, 0] += sound[:end-offset] * gain * math.sqrt((1-p)/2)
            out[offset:end, 1] += sound[:end-offset] * gain * math.sqrt((1+p)/2)

    # Airy D-major ninth bed. A soft attack keeps the first transient intentional.
    t = np.arange(n) / rate
    bed = sum(np.sin(2*np.pi*f*t + .16*np.sin(2*np.pi*.19*t))
              for f in (146.832, 220, 293.665, 369.994, 440, 659.255)) / 6
    envelope = np.minimum(1, t/.5) * np.minimum(1, np.maximum(0, (10-t)/.7))
    put(0, bed * envelope, .19)
    for beat in range(24):
        start = beat * B
        if beat < 19:
            k = np.arange(round(.28*rate)) / rate
            phase = 2*np.pi*(48*k + 80*.025*(1-np.exp(-k/.025)))
            put(start, np.sin(phase)*np.exp(-k*19), .36 if beat % 4 == 0 else .23)
            h = np.arange(round(.045*rate))/rate
            noise = rng.standard_normal(len(h))
            high = np.r_[0, np.diff(noise)]
            put(start+B/2, high*np.exp(-h*105), .023, (-1)**beat*.5)
        if beat in (0, 3, 5, 6, 8, 10, 12, 15, 17, 19, 20):
            a = np.arange(round(.45*rate))/rate
            freq = (587.33, 739.989, 880, 1174.659)[beat % 4]
            tone = (np.sin(2*np.pi*freq*a) + .18*np.sin(2*np.pi*freq*2*a))
            tone *= np.minimum(1, a/.003)*np.exp(-a*10)
            put(start, tone, .13, math.sin(beat)*.5)
            put(start+.125, tone, .035, -math.sin(beat)*.5)
    # Transitions have stereo movement and a quiet tactile click on the cut.
    for index, cut in enumerate((1.25, 2.083333, 2.5, 3.333333, 5, 5.7, 6.25, 7.916667)):
        length = .25
        q = np.arange(round(length*rate))/rate
        noise = rng.standard_normal(len(q))
        smooth = np.convolve(noise, np.ones(9)/9, mode='same')
        whoosh = smooth * np.sin(np.pi*q/length)**2
        put(max(0, cut-.19), whoosh, .11, (-1)**index*.65)
        q = np.arange(round(.012*rate))/rate
        put(cut, rng.standard_normal(len(q))*np.exp(-q*500), .065)
    # Final resolved chord, with space to ring out under the brand.
    a = np.arange(round(2.083333*rate))/rate
    chord = sum(np.sin(2*np.pi*f*a)*np.exp(-a*(1.3+i*.15))
                for i, f in enumerate((146.832,293.665,369.994,440,587.33))) / 5
    put(7.916667, chord*np.minimum(1,a/.015), .40)
    out *= np.minimum(1, np.maximum(0,(10-t)/.16))[:,None]
    out = np.tanh(out*1.4)
    out *= .87 / max(.87, np.max(np.abs(out)))
    with wave.open(str(ASSETS/'Make-room-original-144bpm.wav'), 'wb') as f:
        f.setnchannels(2); f.setsampwidth(2); f.setframerate(rate)
        f.writeframes((out*32767).astype('<i2').tobytes())


def make_paths_portable():
    """EffectCraft schema 2 resolves relative footage paths beside the project."""
    project=json.loads(PROJECT.read_text(encoding='utf-8'))
    def visit(value):
        if isinstance(value,dict):
            for key,child in value.items():
                if key=='path' and isinstance(child,str):
                    source=Path(child)
                    if source.is_absolute() and source.is_relative_to(ASSETS):
                        value[key]=source.relative_to(HERE).as_posix()
                else:
                    visit(child)
        elif isinstance(value,list):
            for child in value: visit(child)
    visit(project)
    PROJECT.write_text(json.dumps(project,ensure_ascii=False,indent=2),encoding='utf-8')


def build(executable, previews=True):
    ASSETS.mkdir(parents=True, exist_ok=True)
    (ASSETS/'fonts').mkdir(exist_ok=True)
    def ensure_asset(source,destination):
        if source.is_file() and source.resolve()!=destination.resolve():
            shutil.copy2(source,destination)
        elif not destination.is_file():
            raise FileNotFoundError(f'Missing source asset: {destination}')
    for filename in ('NotoSans-Regular.ttf', 'NotoSans-Bold.ttf', 'OFL.txt'):
        ensure_asset(ROOT/'assets/fonts'/filename, ASSETS/'fonts'/filename)
    ensure_asset(ROOT/'assets/alphadocs-icon.png', ASSETS/'alphadocs-icon.png')
    for filename in ('crop-before.png','crop.png'):
        ensure_asset(ROOT/'screenshots'/filename, ASSETS/filename)
    soundtrack()
    ec = EffectCraft(executable)
    footage = {}
    try:
        ec.cmd('comp.new', name=COMP, width=1920, height=1080,
               frameRate=60, duration=10, background='#07152c',
               shutterAngle=160, motionBlurSamples=8)

        def prop(layer, path, value):
            ec.tool('set_property', layer=layer, path=path, value=value)

        def keys(layer, path, points, ease='easyEase'):
            ec.tool('add_keyframe', layer=layer, path=path, timeBase='comp',
                    interpolation=ease, keys=[{'time':t,'value':v} for t,v in points])

        def span(layer, start, end):
            ec.cmd('layer.timing', layers=[layer], **{'in':start,'out':end})

        def motion(layer, positions=None, scales=None, rotations=None, blur=False):
            if positions: keys(layer,'transform/position',[(t,[x,y,0]) for t,x,y in positions])
            if scales: keys(layer,'transform/scale',[(t,[s,s,100]) for t,s in scales])
            if rotations: keys(layer,'transform/rotation',rotations)
            if blur: ec.cmd('layer.setSwitch',layers=[layer],switch='motionBlur',value=True)

        def fade(layer, start, end, fadein=.1, fadeout=.1):
            values = [(start,0),(start+fadein,100),(end-fadeout,100),(end,0)]
            keys(layer,'transform/opacity',values)

        def text(name, value, size, x, y, start, end, color='#f6f9ff',
                 bold=True, center=False, enter=50, delay=0):
            layer=ec.cmd('layer.newText',name=name,text=value,font='Noto Sans',
                         style='Bold' if bold else 'Regular',size=size,fill=color,
                         position=[x,y],justify='center' if center else 'left')['layer']
            span(layer,start,end)
            motion(layer,[(start,x,y+enter),(start+.27+delay,x,y),
                          (max(start+.3,end-.13),x,y),(end,x,y-24)])
            fade(layer,start,end,.13,.1)
            return layer

        def picture(name, file, roi, start, end, positions, scales, rotations=None,
                    shadow=True, rounding=True, opacity=True):
            if file not in footage:
                result=ec.cmd('file.import',paths=[str((ASSETS/file).resolve())],
                              sequence=False,addToComp=False)
                if result.get('errors'): raise RuntimeError(result)
                footage[file]=result['items'][0]
            layer=ec.cmd('layer.addItem',item=footage[file],duration=10)['layer']
            ec.cmd('layer.rename',layer=layer,name=name)
            span(layer,start,end)
            x,y,w,h=roi
            prop(layer,'transform/anchor',[x+w/2,y+h/2,0])
            # These masks are editable native EffectCraft paths, not baked screenshots.
            if rounding:
                r=min(18,w/8,h/8); k=.55228475*r
                vertices=[[x+r,y],[x+w-r,y],[x+w,y+r],[x+w,y+h-r],
                          [x+w-r,y+h],[x+r,y+h],[x,y+h-r],[x,y+r]]
                ec.cmd('mask.new',layer=layer,vertices=vertices,closed=True,
                       inTangents=[[-k,0],[0,0],[0,-k],[0,0],[k,0],[0,0],[0,k],[0,0]],
                       outTangents=[[0,0],[k,0],[0,0],[0,k],[0,0],[-k,0],[0,0],[0,-k]])
            else:
                ec.cmd('layer.addMask',layer=layer,shape='rect',rect=roi)
            if shadow:
                ec.tool('add_effect',layer=layer,effect='ec.perspective.dropshadow',
                        values={'color':'#00091b','opacity':52,'direction':90,
                                'distance':28,'softness':65})
            motion(layer,positions,scales,rotations,blur=True)
            if opacity: fade(layer,start,end,.09,.08)
            return layer

        # Native gradient field and a slowly travelling light. All remain editable.
        bg=ec.cmd('layer.newSolid',name='01 | Deep blue gradient',color='#07152c',width=1920,height=1080)['layer']
        ramp=ec.tool('add_effect',layer=bg,effect='ec.generate.gradientramp',
                    values={'start':[80,970],'end':[1800,110],
                            'startColor':'#061124','endColor':'#164d78','scatter':1.2})['path']
        keys(bg,ramp+'/end',[(0,[1640,160]),(5,[1350,420]),(10,[1920,0])])
        # Restrained graphic rails: pacing / depth without drawing fake product UI.
        for index,(x,y,w,h) in enumerate(((960,1041,1680,1),(960,40,1680,1))):
            rail=ec.cmd('layer.newShape',kind='rect',name=f'Ambient rail {index}',
                        size=[w,h],fill='#9cceff',position=[x,y])['layer']
            prop(rail,'transform/opacity',14)
        orb=ec.cmd('layer.newShape',kind='ellipse',name='Atmosphere | moving blue light',
                   size=[1000,680],fill='#187cbd',position=[1460,520])['layer']
        ec.tool('add_effect',layer=orb,effect='ec.blur.gaussian',values={'blurriness':220})
        prop(orb,'transform/opacity',22)
        motion(orb,[(0,1590,730),(5,950,200),(10,1490,300)])

        # 00:00 — typographic hook + an actual illustrated page in perspective.
        picture('01 | Guide flies in','guide.jpg',[550,200,795,812],0,1.34,
                [(0,2180,690),(.48,1530,645),(1.05,1505,620),(1.34,1980,350)],
                [(0,80),(.6,89),(1.1,91),(1.34,112)],[(0,-16),(.6,-7),(1.34,8)])
        text('01 | Eyebrow','A QUIETER WORKSPACE',24,120,230,0,1.25,color='#99d7ff',enter=25)
        text('01 | Make room','Make room.',140,112,435,0,1.25,enter=100)
        text('01 | For your ideas','For your ideas.',111,116,585,.13,1.25,color='#83ceff',enter=100)
        text('01 | Offline','Native. Offline. Yours.',29,124,687,.35,1.25,bold=False,enter=28)

        # 00:01.25 — measured-page screenshots; chapters change on beat.
        chapter_cuts=[('book-01.jpg',1.2,2.083333),('book-02.jpg',2.083333,2.5),('book-03.jpg',2.5,3.42)]
        for idx,(file,start,end) in enumerate(chapter_cuts):
            picture(f'02 | Chapter {idx+1}',file,[0,164,1350,846],start,end,
                    [(start,1070 if idx==0 else 1020,740 if idx==0 else 670),
                     (start+.28,1010,660),(end,970,640)],
                    [(start,82 if idx==0 else 98),(start+.28,94),(end,101)],
                    [(start,3 if idx==0 else -.7),(start+.28,0),(end,-.5)],opacity=idx==0)
        text('02 | Chapter headline','Every chapter. One book.',74,120,196,1.25,3.333333,enter=44)
        text('02 | Chapter label','01  /  CHAPTER TABS',21,126,98,1.25,3.333333,color='#9fd7ff',enter=20)

        # 00:03.33 — three documents share one window. Original page crops fan out.
        cards=[('book-03.jpg',440,-8,3.26),('guide.jpg',960,0,3.32),('brief.jpg',1480,8,3.38)]
        for idx,(file,x,rotation,start) in enumerate(cards):
            picture(f'03 | Document card {idx+1}',file,[550,200,795,812],start,5.09,
                    [(start,x+(idx-1)*360,1300),(start+.39,x,735),(4.8,x,710),(5.09,x,420)],
                    [(start,58),(start+.39,66),(4.8,69),(5.09,86)],
                    [(start,rotation*2),(start+.39,rotation),(4.8,rotation),(5.09,rotation*.5)])
        picture('03 | Real open-file tab strip','brief.jpg',[159,0,590,44],3.333333,5.02,
                [(3.333333,960,390),(3.65,960,303),(4.85,960,298),(5.02,960,240)],
                [(3.333333,160),(3.65,195),(5.02,203)],rounding=True)
        text('03 | Tabs headline','All your work. One window.',72,120,190,3.333333,5,enter=45)
        text('03 | Tabs label','02  /  OPEN DOCUMENT TABS',21,126,98,3.333333,5,color='#9fd7ff',enter=20)

        # 00:05 — native reversible crop, including the genuine checkbox detail.
        for file,start,end in [('crop-before.png',4.98,5.7),('crop.png',5.7,6.33)]:
            picture('04 | '+file,file,[318,201,1078,743],start,end,
                    [(start,1170,800),(start+.25,1110,690),(end,1080,670)],
                    [(start,73),(start+.25,88),(end,91)],
                    [(start,-3),(start+.25,-1),(end,0)],opacity=file=='crop-before.png')
        picture('04 | Transparency control macro','crop.png',[1087,582,290,225],5.7,6.32,
                [(5.7,1800,800),(5.92,1500,760),(6.32,1460,730)],
                [(5.7,130),(5.92,160),(6.32,165)])
        text('04 | Crop headline','Keep the original.',86,120,198,5,6.25,enter=45)
        text('04 | Crop label','03  /  REVERSIBLE IMAGE EDITS',21,126,98,5,6.25,color='#9fd7ff',enter=20)

        # 00:06.25 — actual native invitation controls; no fabricated participants.
        picture('05 | Native sharing invitation','sharing.jpg',[708,282,504,370],6.23,8.04,
                [(6.23,2180,580),(6.62,1390,545),(7.75,1365,530),(8.04,1490,330)],
                [(6.23,105),(6.62,134),(7.75,137),(8.04,100)],
                [(6.23,11),(6.62,0),(7.75,-1),(8.04,-5)])
        text('05 | Sharing label','04  /  DIRECT LAN + VPN SHARING',21,126,180,6.25,7.916667,color='#9fd7ff',enter=20)
        text('05 | Better','Better,',126,115,425,6.25,7.916667,enter=65)
        text('05 | Together','together.',126,115,580,6.34,7.916667,color='#83ceff',enter=65)
        text('05 | Sharing detail','Invite. Edit. Keep a local copy.',28,126,691,6.55,7.916667,bold=False,enter=20)

        # 00:07.92 — deliberate brand hold for legibility and a resolved audio hit.
        endbg=ec.cmd('layer.newSolid',name='06 | Brand reveal gradient',color='#07152c',width=1920,height=1080)['layer']
        span(endbg,7.916667,10)
        ec.tool('add_effect',layer=endbg,effect='ec.generate.gradientramp',
                values={'start':[134,1026],'end':[1824,0],
                        'startColor':'#061124','endColor':'#246faa','scatter':1})
        keys(endbg,'transform/opacity',[(7.916667,0),(8.10,100),(10,100)])
        # Existing supplied native book artwork remains unchanged.
        picture('06 | AlphaDocs blue book','alphadocs-icon.png',[0,0,512,512],7.94,10,
                [(7.94,618,415),(8.36,618,433),(10,618,433)],
                [(7.94,6),(8.36,38),(10,38)],[(7.94,-18),(8.36,0),(10,0)],
                shadow=False,rounding=False,opacity=False)
        text('06 | AlphaDocs wordmark','AlphaDocs',142,746,489,7.99,10,enter=45)
        text('06 | Owner','by Alpha Workroom',25,1010,566,8.12,10,color='#a8d9ff',bold=False,center=True,enter=20)
        text('06 | Tagline','Your words. Your workspace.',44,960,696,8.24,10,center=True,enter=25)
        text('06 | Platforms','Windows   /   macOS   /   Linux',26,960,795,8.43,10,color='#b8d9f1',bold=False,center=True,enter=20)
        text('06 | Repository','github.com/AlphaWorkroom/AlphaDocs',23,960,914,8.52,10,color='#c5e6ff',bold=False,center=True,enter=15)

        # Native audio footage is part of the editable timeline, not a later dub.
        result=ec.cmd('file.import',paths=[str((ASSETS/'Make-room-original-144bpm.wav').resolve())],sequence=False,addToComp=False)
        audio=ec.cmd('layer.addItem',item=result['items'][0])['layer']
        ec.cmd('layer.rename',layer=audio,name='SOUND | Original 144 BPM ident + transition design')
        ec.cmd('time.set',time=3.95)
        ec.cmd('comp.setPosterTime')
        ec.tool('save_project',path=str(PROJECT))
        # Assets already live beside the project. Relativise after closing MCP.
        if previews:
            preview=HERE/'previews'; preview.mkdir(exist_ok=True)
            for time in (.65,1.7,2.72,3.95,5.95,7.1,9.2):
                ec.tool('render_frame',time=time,max_side=1280,inline=False,
                        path=str(preview/f'{time:05.2f}.png'))
        (HERE/'build-audit.json').write_text(json.dumps({
            'engine':ec.tool('get_project')['renderer'],
            'composition':ec.tool('get_comp'),'mcpCalls':len(ec.calls),
            'duration':10,'fps':60,'audio':'Original procedural sound design; no samples',
            'captures':'AlphaDocs native Windows app; original public example documents',
            'reference':'https://x.com/Google/status/2110751391899644101/video/1'
        },indent=2),encoding='utf-8')
        print(f'Built {PROJECT.name}: {len(ec.calls)} native MCP calls',flush=True)
    finally:
        ec.close()
    make_paths_portable()


if __name__ == '__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--effectcraft',default=os.environ.get('EFFECTCRAFT_CLI',r'C:\Program Files\EffectCraft\effectcraft-cli.exe'))
    parser.add_argument('--no-previews',action='store_true')
    parser.add_argument('--render',action='store_true',help='Render the native timeline and make a streaming MP4')
    args=parser.parse_args()
    build(args.effectcraft,not args.no_previews)
    if args.render:
        native=HERE/'AlphaDocs-Hyper-10s-native-final.mp4'
        delivery=ROOT/'screenshots/AlphaDocs-Hyper-10s.mp4' if (ROOT/'Cargo.toml').is_file() else HERE/'AlphaDocs-Hyper-10s.mp4'
        subprocess.run([args.effectcraft,'render','--project',str(PROJECT),'--comp',COMP,
                        '--out',str(native),'--format','h264','--start','0','--end','10',
                        '--fps','60','--resolution','full','--quality','best',
                        '--bitrate','24000','--audio','on','--gpu'],check=True)
        subprocess.run(['ffmpeg','-hide_banner','-y','-i',str(native),'-c','copy',
                        '-movflags','+faststart',str(delivery)],check=True)
