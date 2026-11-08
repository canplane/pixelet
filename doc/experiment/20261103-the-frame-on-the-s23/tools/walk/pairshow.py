
import json,re,glob
from PIL import Image,ImageChops
N=lambda a,q:a[min(len(a)-1,int(len(a)*q))] if a else 0
order=['1-f9f66682e','2-e8d616ba4','3-f9f66682e','4-e8d616ba4']
for L in order:
    d=json.load(open(f'pair-{L}.json'));R=json.load(open(f'pair-{L}-rows.json'));n=R['names'];rows=R['rows']
    log=dict((k,v) for k,v in d['log'])
    c=log['circle'];f=re.search(r'frames n=(\d+)\s+over ([0-9.]+)s\s+fps ([0-9.]+)',c);g=re.search(r'gpu +p50 ([0-9.]+)  p95 ([0-9.]+)',c);ki=re.search(r'kinds +(.*)',c).group(1)[:40]
    b=sorted(r[0] for r in rows)
    print(L,'fps',f.group(3),'gpu',g.group(1),g.group(2),'|',ki,'| view',log['view rest'][-14:],log['view stop'][-14:],'|',log['therm'][:18])
    out=[]
    for s in('sample','beam','miss','light_walk','light_sample','light_beam'):
        i=1+n.index(s);x=sorted(r[i] for r in rows if r[i]>0);out.append('%s %d %.1f/%.1f'%(s,len(x),N(x,.5),N(x,.95)))
    print('  ',' '.join(out))
    for l in log['counts'].split('\n'):
        m=re.match(r'  (eye|light) +cast in (\d+) frames +nodes a cast: beam ([0-9.]+) +sample ([0-9.]+) +miss ([0-9.]+) +cast again ([0-9.]+)%: took ([0-9.]+)% +walked ([0-9.]+)% \(([0-9.]+) nodes each\)',l)
        if m:print('  ',m.group(1),'casts',m.group(2),'beam',round(float(m.group(3))),'sample',round(float(m.group(4))),'miss',round(float(m.group(5))),'again',m.group(6),'took',m.group(7),'walked',m.group(8),'each',m.group(9))
def st(a,b):
    A,B=[Image.open(x).convert('RGB') for x in(a,b)];W,H=A.size;box=(0,int(H*.13),W,int(H*.85))
    d=ImageChops.difference(A.crop(box),B.crop(box)).convert('L');h=d.histogram();t=sum(h)
    return 'mean %.2f over8 %.3f%% over32 %.3f%%'%(sum(i*c for i,c in enumerate(h))/t,sum(h[9:])/t*100,sum(h[33:])/t*100)
for a,b in(('1-f9f66682e','3-f9f66682e'),('2-e8d616ba4','4-e8d616ba4'),('1-f9f66682e','2-e8d616ba4'),('3-f9f66682e','4-e8d616ba4')):
    print('rest',a,b,st(f'pair-{a}-rest.png',f'pair-{b}-rest.png'))
A=Image.open('pair-1-f9f66682e-rest.png').convert('RGB');B=Image.open('pair-2-e8d616ba4-rest.png').convert('RGB')
ImageChops.difference(A,B).point(lambda v:min(255,v*8)).save('pair-diff-rest.png')
W,H=A.size;fs=[f'pair-{L}-{k}.png' for k in('rest','stop') for L in order]
c=Image.new('RGB',(4*270+30,2*int(270*H/W)+10),(255,255,255))
for i,f in enumerate(fs):
    c.paste(Image.open(f).convert('RGB').resize((270,int(270*H/W))),((i%4)*280,(i//4)*(int(270*H/W)+10)))
c.save('pair-all.png')
