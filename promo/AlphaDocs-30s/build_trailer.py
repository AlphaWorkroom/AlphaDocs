"""30-second revision of the approved cursor-led native EffectCraft trailer. Real UI states; authored motion and sound.

The app/cursor/clicks share one nested composition, so camera zooms cannot detach
the pointer from its target. All motion remains native editable keyframes.
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

HERE=Path(__file__).resolve().parent
ASSETS=HERE/'assets'
ROOT=HERE.parent.parent
PROJECT=HERE/'AlphaDocs-30s.ecproj'
COMP='AlphaDocs | Follow the idea | 30s'
APP='Real app + tracked vector cursor'
CLICKS=[(3.30,128,303),(7.60,450,23),(12.10,650,23),(14.55,1864,80),
        (18.90,1145,691),(20.00,1578,80),(22.00,806,450),(23.50,811,726)]

class MCP:
    def __init__(self,exe,gpu=False):
        self.p=subprocess.Popen([exe,'mcp','--empty']+(['--gpu'] if gpu else []),
            stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=open(HERE/'engine.log','w',encoding='utf-8'),
            text=True,encoding='utf-8',env=dict(os.environ,EFFECTCRAFT_FONT_DIRS=str(ASSETS/'fonts')))
        self.n=0;self.calls=0
        self.rpc('initialize',{'protocolVersion':'2025-03-26','capabilities':{},'clientInfo':{'name':'AlphaDocs-interaction-trailer','version':'2'}})
        self.p.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n');self.p.stdin.flush()
    def rpc(self,method,params):
        self.n+=1
        self.p.stdin.write(json.dumps({'jsonrpc':'2.0','id':self.n,'method':method,'params':params})+'\n');self.p.stdin.flush()
        while True:
            line=self.p.stdout.readline()
            if not line:raise RuntimeError('EffectCraft MCP closed')
            r=json.loads(line)
            if r.get('id')==self.n:
                if 'error' in r:raise RuntimeError(r['error'])
                return r['result']
    def tool(self,name,**args):
        r=self.rpc('tools/call',{'name':name,'arguments':args});self.calls+=1
        if r.get('isError'):raise RuntimeError((name,r))
        return json.loads(r['content'][0]['text'])
    def cmd(self,command,**params):return self.tool('execute_command',command=command,params=params)
    def close(self):self.p.stdin.close();self.p.wait(timeout=15)

def ease(u):
    u=max(0,min(1,u));return u*u*u*(10+u*(-15+u*6))

# A single camera path, in source/app pixels. Each trip ends with a short settle.
CAMERA=[(0,960,450,.90),(.30,960,450,.92),
 (.98,404,330,2.44),(1.10,410,330,2.40),(1.44,410,330,2.40),
 (1.98,960,390,1.53),(2.12,960,390,1.50),(2.34,960,390,1.50),
 (2.86,490,150,2.22),(3.00,490,150,2.18),(3.30,490,150,2.18),
 (3.86,960,430,1.14),(4.02,960,430,1.12),
 (4.54,610,160,2.18),(4.67,610,160,2.14),(4.98,610,160,2.14),
 (5.64,1580,190,2.42),(5.80,1580,190,2.38),(6.15,1580,190,2.38),
 (6.74,990,470,1.45),(6.88,990,470,1.40),(7.45,990,470,1.40),
 (8.06,960,450,.84),(8.30,960,450,.84),(10,960,450,.84)]

def approved_camera(t):
    for a,b in zip(CAMERA,CAMERA[1:]):
        if a[0]<=t<=b[0]:
            u=ease((t-a[0])/(b[0]-a[0]));return [a[i]+(b[i]-a[i])*u for i in (1,2,3)]
    return list(CAMERA[-1][1:])

CURSOR=[(0,[660,580]),(.24,[660,580]),(1.03,[128,303]),(2.35,[128,303]),
 (2.96,[450,23]),(3.35,[450,23]),(3.89,[770,445]),(4.04,[770,445]),
 (4.64,[650,23]),(5.02,[650,23]),(5.76,[1864,80]),(6.36,[1864,80]),
 (7.27,[1310,810]),(8.3,[1310,810])]

def approved_pointer(t):
    for index,(a,b) in enumerate(zip(CURSOR,CURSOR[1:])):
        if a[0]<=t<=b[0]:
            u=ease((t-a[0])/(b[0]-a[0]));x=a[1][0]+(b[1][0]-a[1][0])*u;y=a[1][1]+(b[1][1]-a[1][1])*u
            # Gentle arcs, with zero deviation at each exact click target.
            curve=(-1 if index%2 else 1)*min(65,math.dist(a[1],b[1])*.12)*math.sin(math.pi*u)
            return [x,y+curve,0]
    return [*CURSOR[-1][1],0]

# Keep the approved camera/cursor curves. Trips are only ~25% slower; most of
# the extra duration is a still hold so the copy and real UI remain readable.
RETIME=[(0,0),(.30,1.70),(.98,2.55),(1.10,2.70),(1.25,3.30),(1.44,4.00),
 (1.98,4.68),(2.12,4.85),(2.34,6.20),(2.86,6.85),(3.00,7.03),(3.10,7.60),
 (3.30,8.45),(3.86,9.15),(4.02,10.65),(4.54,11.30),(4.67,11.47),(4.85,12.10),
 (4.98,12.85),(5.64,13.68),(5.80,13.88),(6.10,14.55),(6.15,15.10),
 (6.74,15.84),(6.88,16.02),(7.45,18.15)]

def old_time(t):
    for a,b in zip(RETIME,RETIME[1:]):
        if a[1]<=t<=b[1]:return a[0]+(b[0]-a[0])*(t-a[1])/(b[1]-a[1])
    return RETIME[-1][0]

THEME_CAMERA=[(18.15,990,470,1.40),(19.10,990,470,1.40),
 (19.90,1500,190,2.38),(20.15,1500,190,2.38),
 (20.95,960,510,1.48),(21.10,960,510,1.46),(23.60,960,510,1.46),
 (24.40,960,450,.94),(24.55,960,450,.92),(30,960,450,.92)]
THEME_CURSOR=[(18.15,[1310,810]),(18.70,[1145,691]),(19.10,[1145,691]),
 (19.85,[1578,80]),(20.15,[1578,80]),(21.40,[806,450]),(22.20,[806,450]),
 (23.25,[811,726]),(23.70,[811,726]),(24.65,[1400,800]),(30,[1400,800])]

def camera(t):
    if t<=18.15:return approved_camera(old_time(t))
    for a,b in zip(THEME_CAMERA,THEME_CAMERA[1:]):
        if a[0]<=t<=b[0]:
            u=ease((t-a[0])/(b[0]-a[0]));return [a[i]+(b[i]-a[i])*u for i in (1,2,3)]
    return list(THEME_CAMERA[-1][1:])

def pointer(t):
    if t<=18.15:return approved_pointer(old_time(t))
    for index,(a,b) in enumerate(zip(THEME_CURSOR,THEME_CURSOR[1:])):
        if a[0]<=t<=b[0]:
            u=ease((t-a[0])/(b[0]-a[0]));x=a[1][0]+(b[1][0]-a[1][0])*u;y=a[1][1]+(b[1][1]-a[1][1])*u
            curve=(-1 if index%2 else 1)*min(65,math.dist(a[1],b[1])*.12)*math.sin(math.pi*u)
            return [x,y+curve,0]
    return [*THEME_CURSOR[-1][1],0]

def sound():
    rate=48000;n=rate*30;rng=np.random.default_rng(20261016)
    stems={k:np.zeros((n,2),dtype=np.float64) for k in ('music','clicks','whooshes','snaps')}
    def add(stem,start,samples,gain=1,pan=0):
        offset=round(start*rate);end=min(n,offset+len(samples));samples=samples[:end-offset]*gain
        if len(samples)==0:return
        stems[stem][offset:end,0]+=samples*np.sqrt((1-pan)/2)
        stems[stem][offset:end,1]+=samples*np.sqrt((1+pan)/2)
    t=np.arange(n)/rate
    pad=sum(np.sin(2*np.pi*f*t+.04*np.sin(2*np.pi*.2*t)) for f in (146.832,220,293.665,369.994))/4
    add('music',0,pad*np.minimum(1,t/.15)*np.minimum(1,(30-t)/.6),.115)
    for beat in range(72):
        a=np.arange(round(.3*rate))/rate
        freq=(587.33,739.99,880,1174.66)[beat%4]
        pluck=(np.sin(2*np.pi*freq*a)+.12*np.sin(4*np.pi*freq*a))*np.exp(-a*16)*np.minimum(1,a/.003)
        add('music',beat*60/144,pluck,.055,math.sin(beat)*.25)
        if beat%2==0 and beat<63:
            kick=np.sin(2*np.pi*(52*a+65*.02*(1-np.exp(-a/.02))))*np.exp(-a*24)
            add('music',beat*60/144,kick,.10)
    for stamp,_,_ in CLICKS:
        q=np.arange(round(.055*rate))/rate
        click=(rng.standard_normal(len(q))*.18+np.sin(2*np.pi*1800*q)*.75)*np.exp(-q*170)
        add('clicks',stamp,click,.62)
        release=np.sin(2*np.pi*1350*q)*np.exp(-q*200)
        add('clicks',stamp+.075,release,.22)
    for index,stamp in enumerate((1.78,4.10,6.25,8.50,10.70,12.90,15.16,19.15,20.20,23.65,25.85)):
        length=.375;q=np.arange(round(length*rate))/rate
        noise=rng.standard_normal(len(q));filtered=np.convolve(noise,np.ones(7)/7,mode='same')
        swish=filtered*np.sin(np.pi*q/length)**2
        add('whooshes',stamp,swish,.24,(-1)**index*.58)
        # A low airy sweep makes each camera trip audible on laptop speakers.
        tonal=np.sin(2*np.pi*(190*q+620*q*q))*np.sin(np.pi*q/length)**3
        add('whooshes',stamp,tonal,.028,(-1)**index*.2)
    for index,stamp in enumerate((2.70,4.85,7.03,9.30,11.47,13.88,14.95,16.02,20.00,21.10,24.55,26.35)):
        q=np.arange(round(.13*rate))/rate
        pop=(np.sin(2*np.pi*(240*q+230*.013*(1-np.exp(-q/.013))))+.17*np.sin(2*np.pi*1200*q))*np.exp(-q*40)
        add('snaps',stamp,pop,.30 if stamp<26 else .42,(-1)**index*.1)
    a=np.arange(round(1.78*rate))/rate
    ident=sum(np.sin(2*np.pi*f*a)*np.exp(-a*(2+i*.1)) for i,f in enumerate((293.665,369.994,440,587.33)))/4
    add('music',26.35,ident*np.minimum(1,a/.008),.28)
    total=sum(stems.values());gain=min(1,.78/max(.001,np.abs(total).max()))
    for name,value in stems.items():
        value*=gain*np.minimum(1,np.maximum(0,(30-t)/.09))[:,None]
        with wave.open(str(ASSETS/f'{name}-original.wav'),'wb') as f:
            f.setnchannels(2);f.setsampwidth(2);f.setframerate(rate);f.writeframes((value*32767).astype('<i2').tobytes())

def portable():
    data=json.loads(PROJECT.read_text(encoding='utf-8'))
    def visit(o):
        if isinstance(o,dict):
            for k,v in o.items():
                if k=='path' and isinstance(v,str) and Path(v).is_absolute() and Path(v).is_relative_to(ASSETS):o[k]=Path(v).relative_to(HERE).as_posix()
                else:visit(v)
        elif isinstance(o,list):
            for v in o:visit(v)
    visit(data);PROJECT.write_text(json.dumps(data,ensure_ascii=False,indent=2),encoding='utf-8')

def build(exe):
    ASSETS.mkdir(exist_ok=True);(ASSETS/'fonts').mkdir(exist_ok=True)
    for name in ('NotoSans-Regular.ttf','NotoSans-Bold.ttf','OFL.txt'):
        src=ROOT/'assets/fonts'/name
        if src.is_file():shutil.copy2(src,ASSETS/'fonts'/name)
    for src,dest in [(ROOT/'assets/alphadocs-icon.png',ASSETS/'alphadocs-icon.png'),
                     (ROOT/'LICENSE',HERE/'LICENSE')]:
        if src.is_file():shutil.copy2(src,dest)
    sound();ec=MCP(exe);items={}
    def setp(layer,path,value):ec.tool('set_property',layer=layer,path=path,value=value)
    def keys(layer,path,values,easing='linear'):
        ec.tool('add_keyframe',layer=layer,path=path,keys=[{'time':t,'value':v} for t,v in values],timeBase='comp',interpolation=easing)
    def span(layer,start,end):ec.cmd('layer.timing',layers=[layer],**{'in':start,'out':end})
    def footage(file,name,start=0,end=30,roi=None):
        if file not in items:
            result=ec.cmd('file.import',paths=[str(ASSETS/file)],sequence=False,addToComp=False)
            if result.get('errors'):raise RuntimeError(result)
            items[file]=result['items'][0]
        layer=ec.cmd('layer.addItem',item=items[file],duration=30)['layer'];ec.cmd('layer.rename',layer=layer,name=name);span(layer,start,end)
        if roi:
            x,y,w,h=roi;setp(layer,'transform/anchor',[x+w/2,y+h/2,0]);ec.cmd('layer.addMask',layer=layer,shape='rect',rect=roi)
        return layer
    def title(name,value,size,x,y,start,end,color='#202124',center=False,bold=False):
        layer=ec.cmd('layer.newText',name=name,text=value,font='Noto Sans',style='Bold' if bold else 'Regular',size=size,fill=color,position=[x,y],justify='center' if center else 'left')['layer'];span(layer,start,end)
        keys(layer,'transform/position',[(start,[x,y+18,0]),(start+.22,[x,y,0]),(end,[x,y,0])],'easyEase')
        opacity=[(start,0),(start+.13,100),(end,100)] if end>=30 else [(start,0),(start+.13,100),(end-.12,100),(end,0)]
        keys(layer,'transform/opacity',opacity,'easyEase');return layer
    try:
        app=ec.cmd('comp.new',name=APP,width=1920,height=900,frameRate=60,duration=30,background='#f8faff',preserveResolution=True)['comp']
        for name,start,end in [('book-before',0,3.30),('book-after',3.30,7.60),('guide',7.60,12.10),
                               ('brief',12.10,20.00),('preferences-light',20.00,22.00),
                               ('preferences-dark',22.00,23.50),('dark-brief',23.50,30)]:
            layer=footage(name+'.jpg','CAPTURE | '+name,start,end,[0,0,1920,900]);setp(layer,'transform/position',[960,450,0])
        # Native captured modal, over the same current brief. It is a UI demo,
        # not a manufactured connected peer or an invitation containing a secret.
        dim=ec.cmd('layer.newSolid',name='Share | native backdrop',color='#202124',width=1920,height=900)['layer'];span(dim,14.55,18.90)
        keys(dim,'transform/opacity',[(14.55,0),(14.80,30),(18.90,30)],'easyEase')
        panel=footage('sharing-full.jpg','Share | actual invitation panel',14.55,18.90,[708,282,504,468]);setp(panel,'transform/position',[960,516,0])
        # Replace the rectangular crop mask with a native rounded path so the
        # captured modal's old dimmed corner pixels do not form square patches.
        # A second intersecting mask preserves every pixel inside the actual UI.
        x,y,w,h,r=708,282,504,468,24;k=.55228475*r
        ec.cmd('mask.new',layer=panel,mode='Intersect',closed=True,
            vertices=[[x+r,y],[x+w-r,y],[x+w,y+r],[x+w,y+h-r],[x+w-r,y+h],[x+r,y+h],[x,y+h-r],[x,y+r]],
            inTangents=[[-k,0],[0,0],[0,-k],[0,0],[k,0],[0,0],[0,k],[0,0]],
            outTangents=[[0,0],[k,0],[0,0],[0,k],[0,0],[-k,0],[0,0],[0,-k]])
        keys(panel,'transform/scale',[(14.55,[88,88,100]),(14.83,[101.5,101.5,100]),(14.97,[100,100,100]),(18.90,[100,100,100])],'easyEase')
        keys(panel,'transform/position',[(14.55,[960,537,0]),(14.83,[960,513,0]),(14.97,[960,516,0]),(18.90,[960,516,0])],'easyEase')
        keys(panel,'transform/opacity',[(14.55,0),(14.70,100),(18.90,100)],'easyEase')
        ec.tool('add_effect',layer=panel,effect='ec.perspective.dropshadow',values={'color':'#172b4d','opacity':22,'distance':12,'softness':28})
        for index,(stamp,x,y) in enumerate(CLICKS):
            ring=ec.cmd('layer.newShape',name=f'Click feedback {index+1}',kind='ellipse',size=[30,30],fillOpacity=0,fill='#1a73e8',stroke='#1a73e8',strokeWidth=1.6,position=[x,y])['layer'];span(ring,stamp,stamp+.32)
            keys(ring,'transform/scale',[(stamp,[25,25,100]),(stamp+.32,[170,170,100])],'easyEaseOut')
            keys(ring,'transform/opacity',[(stamp,46),(stamp+.32,0)],'easyEaseOut')
        svg='<svg xmlns="http://www.w3.org/2000/svg" width="36" height="44" viewBox="0 0 36 44"><path d="M 3 2 L 3 31 L 11 24 L 18 39 L 26 35 L 18 21 L 31 21 Z" fill="#202124" stroke="#ffffff" stroke-width="2.2" stroke-linejoin="round"/></svg>'
        (ASSETS/'cursor.svg').write_text(svg,encoding='utf-8')
        cursor=ec.cmd('edit.pasteSvg',svg=svg,name='CURSOR | native vector, source-space tracked')['layer'];setp(cursor,'transform/anchor',[3,2,0]);span(cursor,0,25.8)
        keys(cursor,'transform/position',[(f/60,pointer(f/60)) for f in range(1549)])
        press=[(0,[100,100,100])]
        for stamp,_,_ in CLICKS:press.extend([(stamp-.05,[100,100,100]),(stamp,[79,79,100]),(stamp+.10,[104,104,100]),(stamp+.19,[100,100,100])])
        press.append((25.8,[100,100,100]));keys(cursor,'transform/scale',press,'easyEase')
        keys(cursor,'transform/opacity',[(0,100),(24.65,100),(25.00,0),(25.80,0)],'easyEase')
        # Camera acts on this precomp, including the cursor. No independent
        # screen-space cursor approximations: zoom, pan and click coordinates agree.
        main=ec.cmd('comp.new',name=COMP,width=1920,height=1080,frameRate=60,duration=30,background='#f8faff',shutterAngle=120,motionBlurSamples=4)['comp']
        ec.cmd('layer.newSolid',name='Paper-white stage',color='#f8faff',width=1920,height=1080)
        view=ec.cmd('layer.addItem',item=app,duration=30)['layer'];ec.cmd('layer.rename',layer=view,name='CAMERA | continuous source-space follow');span(view,0,26.4)
        keys(view,'transform/position',[(0,[960,600,0]),(19.10,[960,600,0]),(20.95,[960,650,0]),(23.60,[960,650,0]),(24.40,[960,600,0]),(30,[960,600,0])],'easyEase')
        keys(view,'transform/anchor',[(f/60,[*camera(f/60)[:2],0]) for f in range(1801)])
        keys(view,'transform/scale',[(f/60,[camera(f/60)[2]*100,camera(f/60)[2]*100,100]) for f in range(1801)])
        ec.cmd('layer.setSwitch',layers=[view],switch='motionBlur',value=True)
        title('Opening','Meet AlphaDocs.',40,120,110,0,2.00,color='#1a73e8')
        # Quiet editorial headroom keeps the copy readable during close-ups.
        def strip(name,start,end,color='#f8faff'):
            l=ec.cmd('layer.newSolid',name=name,color=color,width=1920,height=164)['layer']
            setp(l,'transform/position',[960,82,0]);span(l,start,end);return l
        strip('Tabs | editorial margin',5.95,8.65)
        title('Tabs label','Everything. One window.',46,120,125,5.95,8.65)
        strip('Share | editorial margin',13.15,16.25)
        title('Share label','Bring someone in.',46,120,125,13.15,16.25)
        pill=ec.cmd('layer.newShape',name='Sharing scope | caption background',kind='rounded',size=[600,64],fill='#f8faff',position=[960,1000])['layer'];span(pill,16.00,18.30)
        keys(pill,'transform/opacity',[(16.00,0),(16.17,100),(18.13,100),(18.30,0)],'easyEase')
        title('Direct scope','Direct LAN / VPN sharing',27,960,1010,16.00,18.30,color='#5f6368',center=True)
        # Same caption throughout the feature. Only its colour changes on the
        # actual Dark click; no second entrance or shortened reading interval.
        strip('Theme | light editorial margin',19.05,22.00)
        strip('Theme | dark editorial margin',22.00,25.85,color='#202124')
        light_id=title('Theme | light caption','Tired eyes? Switch to dark mode!',46,120,125,19.05,22.00)
        label=ec.cmd('layer.newText',name='Theme | dark caption',text='Tired eyes? Switch to dark mode!',font='Noto Sans',style='Regular',size=46,fill='#a8c7fa',position=[120,125],justify='left')['layer'];span(label,22.00,25.85)
        # Keep the light wording solid until the colour switch, then use one
        # final fade only as the paper wipe begins.
        keys(light_id,'transform/opacity',[(21.88,100),(22.00,100)])
        keys(label,'transform/opacity',[(22.00,100),(25.72,100),(25.85,0)],'easyEase')
        wipe=ec.cmd('layer.newSolid',name='Brand | upward paper wipe',color='#f8faff',width=1920,height=1080)['layer'];span(wipe,25.80,30)
        keys(wipe,'transform/position',[(25.80,[960,1620,0]),(26.35,[960,540,0]),(30,[960,540,0])],'easyEase')
        logo=footage('alphadocs-icon.png','Brand | supplied blue book',26.25,30,[0,0,512,512]);setp(logo,'transform/position',[625,435,0])
        keys(logo,'transform/scale',[(26.25,[18,18,100]),(26.55,[34,34,100]),(26.72,[33,33,100]),(30,[33,33,100])],'easyEase')
        keys(logo,'transform/opacity',[(26.25,0),(26.43,100),(30,100)],'easyEase')
        title('Brand | AlphaDocs','AlphaDocs',118,755,485,26.35,30,bold=True)
        title('Brand | Owner','by Alpha Workroom',25,1000,565,26.48,30,color='#5f6368',center=True)
        title('Brand | Thought','Your words. Your workspace.',43,960,710,26.62,30,center=True)
        title('Brand | Platforms','Windows   ·   macOS   ·   Linux',25,960,866,26.78,30,color='#5f6368',center=True)
        for name in ('music','whooshes','snaps','clicks'):
            audio=footage(name+'-original.wav','SOUND | '+name)
        ec.cmd('time.set',time=22.5);ec.tool('save_project',path=str(PROJECT))
        (HERE/'build-audit.json').write_text(json.dumps({'calls':ec.calls,'comps':[ec.tool('get_comp',comp=app),ec.tool('get_comp',comp=main)],'clicks':CLICKS,'camera':CAMERA},indent=2),encoding='utf-8')
    finally:ec.close()
    portable();print('Native interaction project built',flush=True)

def preview(exe):
    d=HERE/'previews';d.mkdir(exist_ok=True)
    for t in (.8,2.8,3.4,5.4,7.65,9.8,12.2,14.65,16.7,18.8,19.95,20.6,21.8,22.3,23.4,24.8,26.3,27.5,29.95):
        subprocess.run([exe,'render-frame','--project',str(PROJECT),'--comp',COMP,'--time',str(t),'--max-side','1280','--out',str(d/f'{t:05.2f}.png'),'--json'],check=True,stdout=subprocess.DEVNULL)

if __name__=='__main__':
    ap=argparse.ArgumentParser();ap.add_argument('--effectcraft',default=os.environ.get('EFFECTCRAFT_CLI',r'C:\Program Files\EffectCraft\effectcraft-cli.exe'));ap.add_argument('--render',action='store_true');ap.add_argument('--preview',action='store_true');a=ap.parse_args()
    build(a.effectcraft)
    if a.preview:preview(a.effectcraft)
    if a.render:
        subprocess.run([a.effectcraft,'render','--project',str(PROJECT),'--comp',COMP,'--out',str(HERE/'native-master.mp4'),'--format','h264','--fps','60','--start','0','--end','30','--resolution','full','--quality','best','--bitrate','22000','--audio','on','--gpu','--json'],check=True)
        delivery=ROOT/'screenshots/AlphaDocs-30s.mp4' if (ROOT/'Cargo.toml').exists() else HERE/'AlphaDocs-30s.mp4'
        subprocess.run(['ffmpeg','-hide_banner','-y','-i',str(HERE/'native-master.mp4'),'-c','copy','-movflags','+faststart',str(delivery)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
