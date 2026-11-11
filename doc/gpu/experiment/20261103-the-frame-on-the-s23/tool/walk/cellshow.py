import json,collections as C,sys
J=json.load(open(sys.argv[1]));R=J['rows'];F=J['frames']
print('frames',len(F),'with records',sum(1 for f in F if f[2]),'rows',len(R),'rays',sum(r['rays'] for r in R))
VAL=(1<<16)-1
def cls(kind,a0):
    if kind==0:return 'split'
    if kind==2:return 'lacks'
    v=a0&VAL
    return 'air' if v in(0,0x100) else 'filled'
K=[r for r in R if r['kind']==1]
print('held Cell rows',len(K),'rays',sum(r['rays'] for r in K),'held other kinds',C.Counter(r['kind'] for r in R).most_common())
tr=C.Counter();w=C.Counter();same=C.Counter();none=0;delta=C.Counter();wd=C.Counter();badex=[]
for r in K:
    b=cls(1,r['a0'])
    if not r['hit']:none+=1;tr[(b,'no re-ask kept')]+=1;w[(b,'no re-ask kept')]+=r['rays'];continue
    a=cls(r['hit']['kind'],r['hit']['a0']);tr[(b,a)]+=1;w[(b,a)]+=r['rays'];s=(r['a0'],r['a1'],1)==(r['hit']['a0'],r['hit']['a1'],r['hit']['kind']);same[s]+=1
    d=(r['hf']-r['af'],r['hn']-r['an']);delta[(d,b,a)]+=1
    if b=='air' and a!='air' and len(badex)<8:badex.append(r)
    if b!='air' and a!=b and len(badex)<16:badex.append(r)
print('held -> asked, nodes:',tr.most_common());print('held -> asked, rays:',w.most_common());print('same words',same.most_common(),'no entry at the asked zooms',none)
print('held minus asked (far,near), held, answer:');[print('  ',k,v) for k,v in delta.most_common(16)]
print('by zoom',sorted(C.Counter(r['z'] for r in K).items()))
print('changed samples:');[print('  ',{k:r[k] for k in('z','rays','af','an','hf','hn','near','far')},'held',hex(r['a0']),hex(r['a1']),'now',r['hit']) for r in badex[:12]]
