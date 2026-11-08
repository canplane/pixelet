import json,sys,collections as C,glob
for f in sorted(glob.glob('load-*.json')) if len(sys.argv)<2 else sys.argv[1:]:
    J=json.load(open(f));L=J['L'];m=L['marks'];P=L['pipes'];M=L['mods']
    ok=[p for p in P if p.get('e') is not None]
    print('==',f,'world',round(m.get('world',0)/1000,1),'s | canvas',round(m.get('canvas',0)/1000,1),'wasm',[(w['n'][:6],round(w['s']/1000,1),round(w.get('e',0)/1000,1)) for w in L['wasm']][:3],'adapter',round(m.get('adapter_e',0)/1000,1),'device',round(m.get('device_e',0)/1000,1))
    print('   modules n',len(M),'bytes %.1f MB'%(sum(x['bytes'] for x in M)/1e6),'first',round(min((x['s'] for x in M),default=0)/1000,1),'last end',round(max((x['e'] for x in M),default=0)/1000,1),'sum create ms',round(sum(x['e']-x['s'] for x in M)))
    if P:
        print('   pipes n',len(P),'failed',sum(1 for p in P if p.get('err')),'first',round(min(p['s'] for p in P)/1000,1),'last end',round(max(p.get('e',0) for p in P)/1000,1),'sum ms',round(sum(p.get('e',p['s'])-p['s'] for p in P)),'all at',round(m.get('pipes_all',0)/1000,1),'first compute',round(m.get('first_compute',0)/1000,1))
    groups=C.defaultdict(lambda:[0,0,0])
    for p in ok:
        e=p['entry'] or p['frag'];d=p['e']-p['s']
        key='render' if p['kind']=='render' else ('light' if e.startswith('light') else 'decor' if 'decor' in e else 'hand' if p.get('hand') else 'rust-gpu big' if p['bytes']>200000 else 'rust-gpu small')
        g=groups[key];g[0]+=1;g[1]+=d;g[2]+=p['bytes']
    for k,g in sorted(groups.items(),key=lambda kv:-kv[1][1]):print('     %-14s n %3d span sum %8.0f ms bytes %.2f MB'%(k,g[0],g[1],g[2]/1e6))
    top=sorted(ok,key=lambda p:-(p['e']-p['s']))[:6];print('   slowest',[(p['entry'] or p['frag'],round(p['e']-p['s']),round(p['bytes']/1000)) for p in top])
    print('   lines',{k:v for k,v in list(L.get('lines',{}).items())[:14]})
    print('   long tasks',len(L['long']),'sum',sum(x[1] for x in L['long']),'max',max([x[1] for x in L['long']],default=0))
