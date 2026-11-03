#!/usr/bin/env python3
# Live words of a hand-written entry: comments out, statements joined, `for` as a loop, then live.py's analysis,
# a name's words from its declared type, its constructor or the function that returns it, else one.
import re,sys
src=open(sys.argv[1]).read();entry=sys.argv[2]
src=re.sub(r'//[^\n]*','',src)
ret={m.group(1):m.group(2) for m in re.finditer(r'fn (\w+)\([^)]*\)\s*->\s*([\w<>]+)',src)}
structs={m.group(1):re.findall(r':\s*([\w<>, ]+?),?\n',m.group(2)+'\n') for m in re.finditer(r'struct (\w+) \{(.*?)\n\}',src,re.S)}
def words(t):
    t=(t or '').strip();m=re.match(r'vec(\d)<',t)
    if m:return int(m.group(1))
    if t in structs:return sum(words(x) for x in structs[t])
    return 1
i=src.index('fn '+entry);j=src.index('{',i);d=0;k=j
while True:
    if src[k]=='{':d+=1
    elif src[k]=='}':
        d-=1
        if d==0:break
    k+=1
body=src[j+1:k]
# tokens into statements
st=[];cur='';par=0
for ch in body:
    cur+=ch
    if ch in '([':par+=1
    elif ch in ')]':par-=1
    if par==0 and ch in ';{}':
        st.append(' '.join(cur.split()));cur=''
st=[s for s in st if s]
ID=re.compile(r'\b[A-Za-z_]\w*\b')
pos=0
def ids(e):return set(ID.findall(e))
W={}
def typ(name,rhs,decl):
    if decl:W[name]=words(decl);return
    m=re.match(r'(vec\d)<',rhs) or re.match(r'(vec\d)',rhs)
    if m:W[name]=int(m.group(1)[3]);return
    m=re.match(r'(\w+)\(',rhs)
    if m and m.group(1) in ret:W[name]=words(ret[m.group(1)]);return
    if m and m.group(1) in structs:W[name]=words(m.group(1));return
    W[name]=max([W.get(x,1) for x in ID.findall(rhs) if x in W and W[x]>1 and '.' not in rhs and '[' not in rhs]+[1])
def block():
    global pos;out=[]
    while pos<len(st):
        s=st[pos]
        if s=='}':pos+=1;return out
        m=re.match(r'for \((.*?);(.*?);(.*?)\) \{$',s)
        if s.startswith('} else {') or s=='else {':pos+=1;continue
        if m:
            pos+=1;b=block();init=m.group(1).strip()+';';out+=[('s',init)]
            cond=m.group(2).strip();upd=m.group(3).strip()+';'
            out.append(('loop',[('if',f'!({cond})',[('s','break;')],[])]+b+[('s',upd)]));continue
        m=re.match(r'if (.*) \{$',s)
        if m:
            pos+=1;t=block();e=[]
            if pos<len(st) and st[pos] in ('else {',):pos+=1;e=block()
            elif pos<len(st) and st[pos].startswith('else if'):
                st[pos]=st[pos][5:];e=block_one()
            out.append(('if',m.group(1),t,e));continue
        if s=='loop {':pos+=1;out.append(('loop',block()));continue
        if s.endswith('{'):pos+=1;out.append(('block',block()));continue
        out.append(('s',s));pos+=1
    return out
def block_one():
    global pos
    s=st[pos];m=re.match(r'if (.*) \{$',s);pos+=1;t=block();e=[]
    if pos<len(st) and st[pos]=='else {':pos+=1;e=block()
    elif pos<len(st) and st[pos].startswith('else if'):st[pos]=st[pos][5:];e=block_one()
    return [('if',m.group(1),t,e)]
tree=block()
def stmt(l):
    m=re.match(r'var (\w+)(?:\s*:\s*([^=;]+))?(?: = (.*))?;$',l)
    if m:
        typ(m.group(1),m.group(3) or '',m.group(2));return ids(m.group(3) or ''),{m.group(1)}
    m=re.match(r'let (\w+)(?:\s*:\s*([^=]+))? = (.*);$',l)
    if m:typ(m.group(1),m.group(3),m.group(2));return ids(m.group(3)),{m.group(1)}
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
        elif x[0] in('loop','block'):scan(x[1])
scan(tree)
names=set(W)
peak=[0,0,''];loops=[]
def w(s):return sum(W[x] for x in s if x in names)
def note(c,where):
    if w(c)>peak[0]:peak[:]=[w(c),len(c&names),where]
def live(b,out,brk,cont):
    cur=set(out)
    for x in reversed(b):
        if x[0]=='s':
            l=x[1]
            if l=='break;':cur=set(brk);continue
            if l=='continue;':cur=set(cont);continue
            if l.startswith('return'):cur=ids(l[6:]);continue
            r,k=stmt(l);cur=(cur-k)|r;note(cur,l[:50])
        elif x[0]=='if':
            cur=live(x[2],cur,brk,cont)|live(x[3],cur,brk,cont)|ids(x[1]);note(cur,'if '+x[1][:40])
        elif x[0]=='block':cur=live(x[1],cur,brk,cont)
        elif x[0]=='loop':
            after=set(cur);h=set()
            for _ in range(30):
                n=live(x[1],h,after,h)
                if n==h:break
                h=n
            loops.append(w(h));cur=h
    return cur
live(tree,set(),set(),set())
print('entry',entry,'names',len(names),'peak words',peak[0],'at',peak[2],'| loop heads, words:',sorted(set(loops),reverse=True)[:6])
