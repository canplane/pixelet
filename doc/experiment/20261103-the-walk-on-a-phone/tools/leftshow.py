import json,sys
for d in sys.argv[1:]:
    j=json.load(open(f'pyramid-{d}-left.json'));print('##',d)
    for k,v in j['log']:
        if not k.startswith('pair') :
            if k in('live at s','invalid','done','camera after world') :print(k,str(v)[:100])
            continue
        if 'err' in v:print(k,v);continue
        print(k,'frames',v['frames'],'kept',v['kept0'])
        for n in('beam','sample','miss'):
            p=v[n];z=sorted(p['zooms'].items(),key=lambda kv:-kv[1])[:4]
            print(f"   {n:6} rays {p['rays']:6} distinct {p['distinct']:6} {p['reasons']} deltas(far,near) {sorted(p.get('deltas',{}).items(),key=lambda kv:-kv[1])[:5]}")
