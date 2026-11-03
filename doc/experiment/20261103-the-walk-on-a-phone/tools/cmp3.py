import json,collections,sys
def load(f):
    m={}
    for d in json.load(open(f)):
        for i in range(0,len(d),12):
            m[tuple(d[i:i+5])]=d[i+5:i+12]
    return m
for x,y in [p.split(',') for p in sys.argv[1:]]:
    try:a=load(f'entries-{x}.json');b=load(f'entries-{y}.json')
    except Exception as e:print(x,y,'missing');continue
    both=set(a)&set(b);bad=[k for k in both if a[k][:3]!=b[k][:3]];same=[k for k in bad if a[k][3:]==b[k][3:]]
    cell=[k for k in same if a[k][0]==1 or b[k][0]==1]
    z=collections.Counter(k[1] for k in bad)
    print(f'{x} / {y}: both {len(both)} differ {len(bad)} ({100*len(bad)/max(1,len(both)):.2f}%), same inheritance {len(same)}, of those a Cell {len(cell)}; zooms {sorted(z.items())[-6:]}')
    for k in cell[:3]:print('    ',k,a[k][:3],b[k][:3])
