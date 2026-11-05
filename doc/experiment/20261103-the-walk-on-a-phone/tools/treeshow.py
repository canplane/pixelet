
import json,os
from PIL import Image,ImageChops,ImageStat
for k,v in json.load(open('quick.json'))['log']:
    if k.startswith('view') or k in('slope','world shown s','invalid','therm'):print(k,str(v)[:90])
on=Image.open('tree-on.png').convert('RGB');o1=Image.open('tree-off1.png').convert('RGB');o2=Image.open('tree-off2.png').convert('RGB')
W,H=on.size;print(W,H);box=(0,int(H*.13),W,int(H*.85))
def st(a,b):
    d=ImageChops.difference(a.crop(box),b.crop(box)).convert('L');h=d.histogram();n=sum(h)
    return 'mean %.2f over8 %.3f%% over32 %.3f%%'%(sum(i*c for i,c in enumerate(h))/n,sum(h[9:])/n*100,sum(h[33:])/n*100)
print('on vs off1',st(on,o1));print('off2 vs off1',st(o2,o1))
Lon,Lo1=on.convert('L'),o1.convert('L')
dark=ImageChops.subtract(Lo1,Lon).point(lambda v:min(255,v*4));lit=ImageChops.subtract(Lon,Lo1).point(lambda v:min(255,v*4))
inv=ImageChops.invert(dark);out=Image.merge('RGB',(ImageChops.lighter(inv,lit),inv,inv));out.save('tree-diff.png')
c=Image.new('RGB',(4*300+30,int(300*H/W)),(255,255,255))
for i,f in enumerate(['tree-off1.png','tree-on.png','tree-off2.png','tree-diff.png']):
    c.paste(Image.open(f).convert('RGB').resize((300,int(300*H/W))),(i*310,0))
c.save('tree-all.png')
