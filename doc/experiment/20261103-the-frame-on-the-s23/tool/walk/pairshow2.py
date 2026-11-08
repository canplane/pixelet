
import json,re,sys
from PIL import Image,ImageChops
N=lambda a,q:a[min(len(a)-1,int(len(a)*q))] if a else 0
order=sys.argv[1:]
for L in order:
    d=json.load(open(f'pair-{L}.json'));R=json.load(open(f'pair-{L}-rows.json'));n=R['names'];rows=R['rows']
    log=dict((k,v) for k,v in d['log'])
    c=log['circle'];f=re.search(r'fps ([0-9.]+)',c);g=re.search(r'gpu +p50 ([0-9.]+)  p95 ([0-9.]+)',c);ki=re.search(r'kinds +(.*)',c).group(1)[:40]
    print(L,'fps',f.group(1),'gpu',g.group(1),g.group(2),'|',ki,'|',log.get('therm','')[:18],'| errs',log.get('errs','')[:300])
    out=[]
    for s in('light_walk','light_hit','sample','beam','miss','light_sample','light_beam'):
        if s in n:
            i=1+n.index(s);x=sorted(r[i] for r in rows if r[i]>0);out.append('%s %d %.1f/%.1f'%(s,len(x),N(x,.5),N(x,.95)))
    print('  ',' '.join(out))
    if 'counts' in log:
        for l in log['counts'].split('\n'):
            if re.match(r'  (eye|light) ',l):print('  ',re.sub(r' +',' ',l)[:220])
