import importlib.util as u,re,collections,pickle,sys
s=u.spec_from_file_location('scip','./scip.py');m=u.module_from_spec(s);s.loader.exec_module(m)
P='rust-analyzer cargo '
def short(z):
    if not z.startswith(P): return None
    a=z[len(P):].split(' ',2)
    return (a[0],a[2]) if len(a)==3 else None
SH={'pxl-engine','pxl-content'}
def istest(path,sym=''):
    return '.test.rs' in path or '/tests/' in path or re.search(r'(^|/)tests?/',sym) is not None
def rng(e):
    return (e[0],e[1],e[2],e[3]) if len(e)==4 else (e[0],e[1],e[0],e[2])
def process(docs,keep):
    defs={};edges=collections.defaultdict(set);occ_owner=[]
    for path,occ in docs.items():
        if not keep(path): continue
        ds=[]
        for l,c,z,ro,e in occ:
            if ro&1 and z.startswith(P) and e and not z.endswith('/'):
                ds.append((rng(e),z))
                defs.setdefault(z,(path,rng(e)))
        ds.sort(key=lambda t:(t[0][2]-t[0][0],t[0][3]-t[0][1]))
        for l,c,z,ro,e in occ:
            if ro&1 or not z.startswith(P) or z.endswith('/'): continue
            owner=None
            for r,o in ds:
                if (r[0],r[1])<=(l,c)<=(r[2],r[3]): owner=o;break
            if owner: occ_owner.append((path,owner,z,l))
    return defs,occ_owner
main=m.load('main.scip');shad=m.load('shader.scip')
keepmain=lambda p:not p.startswith('../') and not p.startswith('/')
defs,oo=process(main,keepmain)
adj=collections.defaultdict(set)
for path,o,z,l in oo: adj[o].add(z)
def crate(z): return short(z)[0]
def group(path):
    if path.startswith('client/render/build.rs') or path.endswith('build.rs'): return 'build'
    if path.startswith('client/') or path.startswith('client-bracket/'): return 'client'
    if path.startswith('server'): return 'server'
    if path.startswith('tool'): return 'tool'
    if path.startswith('spec/src') : return 'spec'
    if path.startswith('spec/content') or path.startswith('spec/engine'): return 'shared'
    return 'other'
roots=collections.defaultdict(set);rootsites=collections.defaultdict(lambda:collections.defaultdict(set))
for path,o,z,l in oo:
    g=group(path)
    if g in('client','server','tool','build') and not istest(path,o):
        roots[g].add(z); rootsites[g][z].add(path)
sdefs,soo=process(shad,lambda p:p.startswith('src/') or p.startswith('macro/'))
for path,o,z,l in soo:
    if crate(z) in SH: roots['gpu'].add(z); rootsites['gpu'][z].add(path)
# symbol index
sharedsyms={z:d for z,d in defs.items() if crate(z) in SH and not istest(d[0],short(z)[1])}
pat_impl=re.compile(r'^(.*/)?impl#\[`?([A-Za-z0-9_]+)[^\]]*\](?:\[`?([A-Za-z0-9_]+)[^\]]*\])?(.+)$')
byname=collections.defaultdict(list)
for z in defs:
    t=short(z)[1]
    mm=re.match(r'^(.*/)?([A-Za-z0-9_]+)#$',t)
    if mm: byname[mm.group(2)].append(z)
impls=[]
traitm=collections.defaultdict(set)
for z in defs:
    t=short(z)[1]; mm=pat_impl.match(t)
    if mm: impls.append((z,mm.group(2),mm.group(3),mm.group(4)))
    mm2=re.match(r'^(.*/)?([A-Za-z0-9_]+)#([A-Za-z0-9_]+\(\)\.)$',t)
    if mm2: traitm[(mm2.group(2),mm2.group(3))].add(z)
localtraits={k[0] for k in traitm}
def reach(start):
    seen=set();st=list(start)
    while True:
        while st:
            z=st.pop()
            if z in seen: continue
            seen.add(z)
            st.extend(adj.get(z,()))
            t=short(z)
            if not t: continue
            t=t[1]
            mm=pat_impl.match(t)
            if mm: st.extend(byname.get(mm.group(2),()))
            mm=re.match(r'^(.*)#[^#]+$',t)
            if mm and 'impl#' not in t:
                pass
            # struct -> fields
            if t.endswith('#'):
                pre=z
                st.extend(f for f in fieldsof.get(z,()))
        changed=False
        seentypes={re.match(r'^(.*/)?([A-Za-z0-9_]+)#$',short(z)[1]).group(2) for z in seen if short(z) and re.match(r'^(.*/)?([A-Za-z0-9_]+)#$',short(z)[1])}
        for z,T,tr,meth in impls:
            if z in seen or not tr or T not in seentypes: continue
            if tr in localtraits:
                if not any(a in seen for a in traitm.get((tr,meth),())): continue
            st.append(z);changed=True
        if not st: return seen
fieldsof=collections.defaultdict(list)
for z in defs:
    t=z
    mm=re.match(r'^(.*#)[A-Za-z0-9_]+\.$',t)
    if mm and not t.endswith('().'): fieldsof[mm.group(1)].append(z)
R={g:reach(v) for g,v in roots.items()}
# path through spec: client/server/tool roots reach spec/src symbols via adj, included.
pickle.dump(dict(R=R,sharedsyms=sharedsyms,roots=dict(roots),rootsites={g:{k:list(v) for k,v in d.items()} for g,d in rootsites.items()},adj=dict(adj),defs=defs),open('reach.pkl','wb'))
def kind(z):
    t=short(z)[1]
    if t.endswith('().'): return 'fn'
    if t.endswith('#'): return 'type'
    if t.endswith('.'):
        last=re.split(r'[#/\]]',t)[-1][:-1]
        return 'const' if last.isupper() or last.replace('_','').isupper() else 'field'
    return 'other'
tab=collections.defaultdict(lambda:collections.Counter())
cat={}
for z,(path,r) in sharedsyms.items():
    k=kind(z)
    if k not in('fn','const'): continue
    g='gpu' if z in R['gpu'] else None
    cl=z in R['client'];se=z in R['server'] or z in R['tool'];b=z in R['build']
    c=('G' if g else '')+('C' if cl else '')+('S' if se else '')+('B' if b else '')
    cat[z]=c or '-'
    lines=r[2]-r[0]+1
    mod=path.split('/src/')[1]
    tab[(path.split('/')[1],mod,k)][c or '-']+=lines
    tab[('ALL',path.split('/')[1],k)][c or '-']+=lines
    tab[('ALLN',path.split('/')[1],k)][c or '-']+=1
cats=sorted({c for t in tab.values() for c in t})
print('legend G=gpu C=client runtime S=server/tool B=build.rs; values=lines');print(cats)
for k in sorted(tab):
    print(k,dict(tab[k]))
pickle.dump(cat,open('cat.pkl','wb'))
