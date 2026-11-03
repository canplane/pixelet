import json,re,sys
names=('beam','sample','miss','world','face','generate')
for lab in sys.argv[1:]:
    for i in (1,2,3,4):
        try:d=json.load(open(f'ab-{lab}-{i}.json'))
        except Exception as e:print(lab,i,'missing');continue
        for k,v in d['log']:
            if isinstance(v,str) and v.startswith('=== perf'):
                fr=re.search(r'fps ([0-9.]+)',v).group(1);iv=re.search(r'interval p50 ([0-9.]+)  p95 ([0-9.]+)',v).groups();g=re.search(r'gpu +p50 ([0-9.]+)  p95 ([0-9.]+)',v);m=re.search(r'spans .*',v).group(0);sp=[(re.search(r'\b'+n+r' ([0-9.]+) \(',m) or [0,'-'])[1] for n in names]
                e=re.search(r'eye +cast in (\d+).*?beam ([0-9.]+)  sample ([0-9.]+)  miss ([0-9.]+)',v)
                print(f"{lab:6} {i} {k:14} fps {fr:>5} int {iv[0]:>4}/{iv[1]:>5} gpu {g.group(1):>5}/{g.group(2):>6} {dict(zip(names,sp))} {('lanes '+e.group(2)+' '+e.group(3)+' '+e.group(4)) if e else ''}")
            elif k=='invalid':print(lab,i,'invalid')
