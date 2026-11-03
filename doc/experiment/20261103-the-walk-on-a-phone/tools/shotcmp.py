from PIL import Image,ImageChops
import warnings,sys;warnings.simplefilter('ignore')
def rep(name,a,b):
    A=Image.open(a).convert('RGB');B=Image.open(b).convert('RGB')
    if A.size!=B.size:print(name,'sizes',A.size,B.size);return
    d=ImageChops.difference(A,B).convert('RGB');W,H=d.size;g=list(d.get_flattened_data() if hasattr(d,'get_flattened_data') else d.getdata())
    n=gt0=gt2=gt8=gt32=0;xs=[];ys=[];S=0
    for i,p in enumerate(g):
        v=max(p);x=i%W;y=i//W
        if 180<=y and 400<=x<620 and y<1200: continue  # the companion
        n+=1;S+=v;gt0+=v>0;gt2+=v>2;gt8+=v>8;gt32+=v>32
        if v>32:xs.append(x);ys.append(y)
    print(f'{name}: differ {100*gt0/n:.3f}% >2 {100*gt2/n:.3f}% >8 {100*gt8/n:.3f}% >32 {100*gt32/n:.4f}% mean {S/n:.3f}', (min(xs),max(xs),min(ys),max(ys)) if xs else '')
import itertools,os
sets=sys.argv[1:]
for tail in ('1','w','x'):
    print('-- pose',tail)
    names=[f'{l}{r}' for l in sets for r in 'ab' if os.path.exists(f'shot-s23-{l}{r}-{tail}.png')]
    for x,y in itertools.combinations(names,2):
        rep(f'{x} / {y}',f'shot-s23-{x}-{tail}.png',f'shot-s23-{y}-{tail}.png')
