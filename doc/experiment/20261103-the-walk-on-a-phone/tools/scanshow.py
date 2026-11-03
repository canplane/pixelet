import json,statistics as st,sys
J=json.load(open(sys.argv[1]));R=J['rows'];K={0:'split',1:'cell',2:'lacks',7:'none'}
print('samples',len(R),'gap mean',round(st.mean(r['gap'] for r in R[1:]),2))
for p in('beam','sample','miss'):
    v=[r[p][0] for r in R];s=sorted(v);print(p,'mean %.0f p50 %d p90 %d p95 %d max %d, rays a node %.1f, frames over 1500: %d'%(st.mean(v),s[len(s)//2],s[int(len(s)*.9)],s[int(len(s)*.95)],s[-1],sum(v)/max(1,sum(r[p][1] for r in R)),sum(1 for x in v if x>1500)),'rules no_slot,narrow,lacks,reach,steps',[sum(r[p][i] for r in R) for i in range(2,7)])
b=[r['beam'][0] for r in R];
for p in('sample','miss'):
    x=[r[p][0] for r in R];print('corr beam rays vs',p,round(st.correlation(b,x),2))
big=sorted(J['big'],key=lambda r:-(r['beam'][0]+r['sample'][0]+r['miss'][0]))
print('big frames',len(big))
for r in big[:8]:
    print('== frame',r['frame'],'gap',r['gap'],'beam',r['beam'][:2],'sample',r['sample'][:2],'miss',r['miss'][:2],'beam rules',r['beam'][2:])
    for p in('beam','sample','miss'):
        for n in r['top'][p][:2]:
            print('   ',p,'z%d %s rays %d rule %d why %d asked %s held %s near %.1f far %.1f side %.1f'%(n['z'],K.get(n['kind'],n['kind']),n['rays'],n['rule'],n['why'],n['asked'],n['held'],n['near'],n['far'],n['side']))

root=lambda r:r['beam'][0]==6848
head=[r for r in R if 'forgets' in r]
import collections as C
c=C.Counter()
for r in head:
    k=(r['forgets'],'moved' if r['moved'] else '-','boxes' if r['holding'] else '-','writes' if r['writes'] else '-','WHOLE' if root(r) else ('part' if r['beam'][0]>1500 else 'few'))
    c[k]+=1
print('forgets,moved,boxes,writes,beam left: frames');[print('  ',k,v) for k,v in sorted(c.items())]
w=[r for r in head if root(r)];print('whole frames moved bits',[sum(bin(x[1]).count('1') for x in r['moved']) for r in w][:24]);print('words touched',C.Counter(x[0] for r in w for x in r['moved']).most_common(12));print('holding',C.Counter(r['holding'] for r in w).most_common(6),'writes',C.Counter(r['writes'] for r in w).most_common(6))
n=[r for r in head if not root(r)];print('not whole: frames with moved',sum(1 for r in n if r['moved']),'of',len(n),'bits',C.Counter(sum(bin(x[1]).count('1') for x in r['moved']) for r in n).most_common(6))
