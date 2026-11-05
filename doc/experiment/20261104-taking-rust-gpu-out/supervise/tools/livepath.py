#!/usr/bin/env python3
# Words live along a call path through hand WGSL, as if every call were inlined.
# path(f) = max over f's statements: live after a call + path(callee); a loop head's own.
# High: sum as it stands. Low: an argument passed as a plain name that is also live after the call is counted once.
import re,sys
src=re.sub(r'//[^\n]*','',open(sys.argv[1]).read())
roots=sys.argv[2:]
structs={m.group(1):re.findall(r'\w+:\s*([\w<>]+)',m.group(2)) for m in re.finditer(r'struct (\w+) \{(.*?)\n?\}',src,re.S)}
def tw(t):
    t=(t or '').strip();m=re.match(r'vec(\d)',t)
    if m:return int(m.group(1))
    if t in structs:return sum(tw(x) for x in structs[t])
    return 1
CV={'TUNNELS_FIRST':4,'NO_CUT':4,'BOUNDS_EMPTY':4,'BOUNDS_OPEN':4}
fns={}
for m in re.finditer(r'\bfn (\w+)\(([^)]*)\)\s*(?:->\s*([\w<>]+)\s*)?\{',src):
    i=m.end()-1;d=0;k=i
    while True:
        if src[k]=='{':d+=1
        elif src[k]=='}':
            d-=1
            if d==0:break
        k+=1
    params=[(a.split(':')[0].strip().split()[-1],a.split(':')[1].strip()) for a in re.split(r',(?![^<]*>)',m.group(2)) if ':' in a]
    fns[m.group(1)]=dict(params=params,ret=m.group(3),body=src[i+1:k])
ID=re.compile(r'\b[A-Za-z_]\w*\b')
def ids(e):return set(ID.findall(re.sub(r'\.\w+','',e)))|set()
def split(body):
    st=[];cur='';par=0
    for ch in body:
        cur+=ch
        if ch in '([':par+=1
        elif ch in ')]':par-=1
        if par==0 and ch in ';{}':st.append(' '.join(cur.split()));cur=''
    return [s for s in st if s]
def parse(st):
    pos=[0]
    def block():
        out=[]
        while pos[0]<len(st):
            s=st[pos[0]]
            if s=='}':
                pos[0]+=1;return out
            m=re.match(r'for \((.*?);(.*?);(.*?)\) \{$',s)
            if m:
                pos[0]+=1;b=block()
                out.append(('s',m.group(1).strip()+';'))
                out.append(('loop',[('if',f'!({m.group(2).strip()})',[('s','break;')],[])]+b+[('s',m.group(3).strip()+';')]));continue
            m=re.match(r'while \((.*)\) \{$',s)
            if m:
                pos[0]+=1;b=block()
                out.append(('loop',[('if',f'!({m.group(1)})',[('s','break;')],[])]+b));continue
            m=re.match(r'if (.*) \{$',s)
            if m:
                pos[0]+=1;out.append(ifrest(m.group(1)));continue
            m=re.match(r'switch (.*) \{$',s)
            if m:
                pos[0]+=1;cases=[]
                while st[pos[0]]!='}':
                    pos[0]+=1;cases+=block()
                pos[0]+=1
                out.append(('if',m.group(1),cases,[]));continue
            if s.endswith('{'):
                pos[0]+=1;out.append(('block',block()));continue
            out.append(('s',s));pos[0]+=1
        return out
    def ifrest(c):
        t=block();e=[]
        if pos[0]<len(st):
            s=st[pos[0]]
            if s=='else {':
                pos[0]+=1;e=block()
            elif s.startswith('else if'):
                m=re.match(r'else if (.*) \{$',s);pos[0]+=1;e=[ifrest(m.group(1))]
        return ('if',c,t,e)
    return block()
memo={}
def analyse(name):
    if name in memo:return memo[name]
    memo[name]=(0,0,0,'')
    f=fns[name];W={p:tw(t) for p,t in f['params']}
    tree=parse(split(f['body']))
    def typ(n,rhs,decl):
        if decl:W[n]=tw(decl);return
        m=re.match(r'vec(\d)',rhs)
        if m:W[n]=int(m.group(1));return
        m=re.match(r'(\w+)\(',rhs)
        if m and m.group(1) in fns:W[n]=tw(fns[m.group(1)]['ret']);return
        if m and m.group(1) in structs:W[n]=tw(m.group(1));return
        if m and m.group(1) in('select','min','max','clamp','abs'):
            pass
        cand=[W.get(x,CV.get(x,1)) for x in ids(rhs) if W.get(x,CV.get(x,1))>1]
        W[n]=max(cand+[1]) if ('.' not in rhs and '[' not in rhs) else 1
    def stmt(l):
        m=re.match(r'var (\w+)(?:\s*:\s*([^=;]+))?(?: = (.*))?;$',l)
        if m:
            typ(m.group(1),m.group(3) or '',m.group(2));return ids(m.group(3) or ''),{m.group(1)}
        m=re.match(r'let (\w+)(?:\s*:\s*([^=]+))? = (.*);$',l)
        if m:
            typ(m.group(1),m.group(3),m.group(2));return ids(m.group(3)),{m.group(1)}
        m=re.match(r'(\w+) = (.*);$',l)
        if m:return ids(m.group(2)),{m.group(1)}
        m=re.match(r'(\w+)\s*([-+*/|&^]|<<|>>)= (.*);$',l)
        if m:return ids(m.group(3))|{m.group(1)},set()
        m=re.match(r'(\w+)((?:\.\w+|\[[^=]*\])+) = (.*);$',l)
        if m:return ids(m.group(3))|ids(m.group(2))|{m.group(1)},set()
        return ids(l),set()
    def scan(b):
        for x in b:
            if x[0]=='s':stmt(x[1])
            elif x[0]=='if':scan(x[2]);scan(x[3])
            else:scan(x[1])
    scan(tree)
    def w(s):return sum(W[x] for x in s if x in W)
    res=dict(peak=0,loop=0,lo=0,where='');
    def calls(text):
        out=[]
        for m in re.finditer(r'\b(\w+)\(',text):
            if m.group(1) in fns:
                j=m.end();d=1;args=[];cur=''
                while d>0:
                    ch=text[j]
                    if ch in '([':d+=1
                    if ch in ')]':d-=1
                    if d==1 and ch==',':args.append(cur.strip());cur=''
                    elif d>0:cur+=ch
                    j+=1
                if cur.strip():args.append(cur.strip())
                out.append((m.group(1),args))
        return out
    final=[False]
    def visit_call(text,after):
        if not final[0]:return
        for c,args in calls(text):
            pk,lp,lo,wh=analyse(c)
            base=w(after)
            shared=sum(W[a] for a in args if a in after and a in W)
            if base+lp>res['loop']:
                res['loop']=base+lp;res['lo']=base-shared+lo;res['where']=f"{c} +{base} / {wh}"
            res['peak']=max(res['peak'],base+pk)
    def live(b,out,brk,cont):
        cur=set(out)
        for x in reversed(b):
            if x[0]=='s':
                l=x[1]
                if l=='break;':cur=set(brk);continue
                if l=='continue;':cur=set(cont);continue
                if l.startswith('return'):
                    e=l[6:];visit_call(e,set());cur=ids(e)&set(W);continue
                r,k=stmt(l);visit_call(l,cur-k);cur=((cur-k)|r)&set(W)
                if final[0]:res['peak']=max(res['peak'],w(cur))
            elif x[0]=='if':
                visit_call(x[1],live(x[2],cur,brk,cont)|live(x[3],cur,brk,cont))
                cur=live(x[2],cur,brk,cont)|live(x[3],cur,brk,cont)|(ids(x[1])&set(W))
            elif x[0]=='block':cur=live(x[1],cur,brk,cont)
            elif x[0]=='loop':
                after=set(cur);h=set()
                saved=final[0];final[0]=False
                for _ in range(30):
                    n=live(x[1],h,after,h)
                    if n==h:break
                    h=n
                final[0]=saved
                live(x[1],h,after,h)
                if final[0] and w(h)>res['loop']:
                    res['loop']=w(h);res['lo']=w(h);res['where']=f"own loop {sorted(h)[:40]}"
                cur=h
        return cur
    final[0]=False;live(tree,set(),set(),set());final[0]=True;live(tree,set(),set(),set())
    memo[name]=(res['peak'],res['loop'],res['lo'],res['where'])
    return memo[name]
for r in roots:
    p,l,lo,wh=analyse(r)
    print(f"{r}: peak {p}, across a loop high {l} low {lo}")
    print('   ',wh)
for k in sys.argv[2:]: pass
print('--- by function: peak, loop high, low')
for k in sorted(memo,key=lambda k:-memo[k][1])[:24]:
    print(k,memo[k][:3])
