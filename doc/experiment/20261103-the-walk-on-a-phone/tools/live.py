#!/usr/bin/env python3
# What of a generated entry's locals are live where: a structured backward liveness
# over naga's WGSL, a statement a line. python3 live.py <file.wgsl> [function]
import re,sys,collections
src=open(sys.argv[1]).read().split('\n')
fn=sys.argv[2] if len(sys.argv)>2 else None
ID=re.compile(r'\b[A-Za-z_][A-Za-z0-9_]*\b')
structs={}
i=0
while i<len(src):
    m=re.match(r'struct (\w+) \{',src[i])
    if m:
        f=[];i+=1
        while not src[i].startswith('}'):
            mm=re.match(r'\s*(\w+): (.+),$',src[i])
            if mm:f.append(mm.group(2))
            i+=1
        structs[m.group(1)]=f
    i+=1
def words(t):
    t=t.strip()
    m=re.match(r'array<(.+), (\d+)>$',t)
    if m:return words(m.group(1))*int(m.group(2))
    m=re.match(r'vec(\d)<',t)
    if m:return int(m.group(1))
    if t in('u32','i32','f32','bool'):return 1
    if t in structs:return sum(words(x) for x in structs[t])
    return 1
# the function's body
start=None
for k,l in enumerate(src):
    m=re.match(r'fn (\w+)\(',l)
    if m and ((fn and m.group(1)==fn) or (not fn and m.group(1)=='function_1')):start=k;break
depth=0;end=None
for k in range(start,len(src)):
    depth+=src[k].count('{')-src[k].count('}')
    if depth==0:end=k;break
body=[l.strip() for l in src[start+1:end]]
lines=list(range(start+2,end+1))
class N:pass
pos=0
def parse_block():
    global pos
    out=[]
    while pos<len(body):
        l=body[pos];ln=lines[pos]
        if not l:pos+=1;continue
        if l.startswith('}'):return out
        if l.startswith('if ') and l.endswith('{'):
            cond=l[3:-1];pos+=1;t=parse_block()
            e=[]
            if body[pos]=='} else {':
                pos+=1;e=parse_block()
            elif body[pos].startswith('} else if'):raise Exception('else if '+str(lines[pos]))
            assert body[pos]=='}',(lines[pos],body[pos]);pos+=1
            out.append(('if',cond,t,e,ln));continue
        if l=='loop {':
            pos+=1;b=parse_block();c=[];bi=None
            # continuing is parsed in parse_block as a statement
            assert body[pos]=='}',(lines[pos],body[pos]);pos+=1
            out.append(('loop',b,ln,lines[pos-1]));continue
        if l=='continuing {':
            pos+=1;c=parse_block();assert body[pos]=='}';pos+=1
            out.append(('continuing',c,ln));continue
        if l.startswith('switch ') and l.endswith('{'):
            cond=l[7:-1];pos+=1;cases=[]
            while body[pos]!='}':
                assert body[pos].endswith('{'),(lines[pos],body[pos]);pos+=1
                cases.append(parse_block());assert body[pos]=='}';pos+=1
            pos+=1;out.append(('switch',cond,cases,ln));continue
        if l.endswith('{'):
            pos+=1;b=parse_block();assert body[pos]=='}';pos+=1;out.append(('block',b,ln));continue
        out.append(('s',l,ln));pos+=1
    return out
tree=parse_block()
var={};decl=[]
def ids(e):return set(ID.findall(e))
vars_=set()
for l in body:
    m=re.match(r'var (\w+): (.+?)( = .*)?;$',l)
    if m:var[m.group(1)]=words(m.group(2));vars_.add(m.group(1))
def stmt(l):
    """the names a statement reads and those it kills"""
    if l.startswith('var '):
        m=re.match(r'var (\w+): .+? = (.*);$',l)
        if m:return ids(m.group(2)),{re.match(r'var (\w+)',l).group(1)}
        return set(),{re.match(r'var (\w+)',l).group(1)}  # zero init: a definition
    m=re.match(r'let (\w+)(?:: [^=]+)? = (.*);$',l)
    if m:return ids(m.group(2)),{m.group(1)}
    m=re.match(r'(\w+) = (.*);$',l)
    if m:return ids(m.group(2)),{m.group(1)}
    m=re.match(r'(\w+)((?:\.\w+|\[[^=]*\])+) = (.*);$',l)
    if m:return ids(m.group(3))|ids(m.group(2))|{m.group(1)},set()
    return ids(l),set()
loops=[]
peak=[0,0,0];hist=collections.Counter()
def weight(s):return sum(var.get(v,1) for v in s)
def note(s,ln):
    w=weight(s)
    if w>peak[0]:peak[:]=[w,len(s),ln]
def live(block,out,brk,cont,ret=frozenset()):
    cur=set(out)
    for st in reversed(block):
        k=st[0]
        if k=='s':
            l=st[1]
            if l=='break;':cur=set(brk);continue
            if l=='continue;':cur=set(cont);continue
            if l.startswith('return'):cur=set(ids(l[6:]));continue
            if l.startswith('break if'):cur=cur|set(brk)|ids(l[8:]);continue
            r,kill=stmt(l);cur=(cur-kill)|r;note(cur,st[2])
        elif k=='if':
            a=live(st[2],cur,brk,cont);b=live(st[3],cur,brk,cont);cur=a|b|ids(st[1]);note(cur,st[4])
        elif k=='block':cur=live(st[1],cur,brk,cont)
        elif k=='switch':
            s=set()
            for c in st[2]:s|=live(c,cur,cur,cont)
            cur=s|ids(st[1])
        elif k=='loop':
            after=set(cur);head=set()
            body_=[x for x in st[1] if x[0]!='continuing'];cn=[x for x in st[1] if x[0]=='continuing']
            for _ in range(30):
                cont_in=live(cn[0][1],head,after,head) if cn else head
                new=live(body_,cont_in,after,cont_in)
                if new==head:break
                head=new
            loops.append((st[2],st[3],set(head)))
            cur=head;note(cur,st[2])
        elif k=='continuing':pass
    return cur
live(tree,set(),set(),set())
locals_=vars_
def isvar(s):return {v for v in s if v in vars_}
print('function lines',lines[0],lines[-1],'vars',len(vars_),'phi',sum(v.startswith('phi') for v in vars_),'words',sum(var.values()),'lets',sum(1 for l in body if l.startswith('let ')))
print('peak live (vars and lets): words',peak[0],'names',peak[1],'at line',peak[2])
seen={}
for a,b,h in loops:seen[(a,b)]=h
out=sorted(seen.items(),key=lambda kv:-(kv[0][1]-kv[0][0]))[:int(sys.argv[3]) if len(sys.argv)>3 else 14]
print('loop lines | span | live at its head: vars, their words; lets')
for (a,b),h in out:
    v=isvar(h);print(f'{a:6}-{b:6} {b-a:6}  vars {len(v):5} words {sum(var[x] for x in v):5} lets {len(h)-len(v):4}  phi {sum(x.startswith("phi") for x in v)}')
