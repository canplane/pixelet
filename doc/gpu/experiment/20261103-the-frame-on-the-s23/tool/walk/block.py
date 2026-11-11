import re,collections,importlib.util as u,io,contextlib
s=u.spec_from_file_location('r','./reach.py');r=u.module_from_spec(s)
with contextlib.redirect_stdout(io.StringIO()): s.loader.exec_module(r)
P=r.P;short=r.short;defs=r.defs;oo=r.oo;SS=r.sharedsyms;R=r.R;cat=r.cat
isfn=lambda z:z in cat and z.endswith('().')
L=lambda z:defs[z][1][2]-defs[z][1][0]+1
gpu=R['gpu'];gfn={z for z in SS if isfn(z) and z in gpu};cl=R['client'];se=R['server']|R['tool']
def run(prefixes,label,show=False):
    blocked={z for z,d in defs.items() if any(d[0].startswith(p) for p in prefixes)}
    saved=dict(r.adj)
    for z in blocked: r.adj.pop(z,None)
    start={z for path,o,z,l in oo if r.group(path)=='client' and not r.istest(path,o) and not any(path.startswith(p) for p in prefixes)}-blocked
    rr=r.reach(start)
    r.adj.clear();r.adj.update(saved)
    both=[z for z in gfn if z in rr];lost=[z for z in gfn if z in cl and z not in rr]
    print(label,'| still on client & GPU',sum(L(z) for z in both),'| GC lost',sum(L(z) for z in lost),'| of which GPU-only then',sum(L(z) for z in lost if z not in se))
    if show:
        by=collections.Counter()
        for z in both: by[defs[z][0].split('/src/')[1]]+=L(z)
        print(dict(by.most_common()))
        print(sorted(short(z)[1] for z in both if L(z)>=15))
    return both
q='client/kernel/src/domain/query/'
run([q],'no kernel query layer',True)
for sub in ['ray','node','scatter','decor','support','surface','limit','world','entry','place','changes']:
    run([q+sub],'no query/'+sub)
run(['client/kernel/src/domain/layer/terrain'],'no kernel terrain layer')
run(['client/render/'],'no render')
print('--- spec/src direct to shared')
c=collections.defaultdict(set)
for path,o,z,l in oo:
    if path.startswith('spec/src') and not r.istest(path,o) and isfn(z):
        c[short(z)[1]].add(path.replace('spec/src/',''))
for k in sorted(c): print(' ',k,'<-',','.join(sorted(c[k]))[:70])
