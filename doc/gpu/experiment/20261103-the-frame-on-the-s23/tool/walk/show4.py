import json,sys
d=json.load(open(sys.argv[1]))
for k,v in d['log']:
    if isinstance(v,str) and v.startswith('=== perf'):
        print('=====',k)
        for l in v.split('\n'):
            if l.startswith('tasks n=') : break
            if any(l.strip().startswith(a) for a in('frames','kinds','interval','gpu','busy by','spans','view','eye','light','rays')) : print(l[:640])
    else: print('==',k,json.dumps(v,ensure_ascii=False)[:180] if not isinstance(v,str) else v[:180])
