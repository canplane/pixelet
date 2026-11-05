import re,sys
src=open(sys.argv[1]).read().split('\n')
out=[]
rules=[(r'(?<![<>=!])<(?![<=])','<='),(r'<=','<'),(r'>=','>'),(r'(?<![<>=\-])>(?![>=])','>='),(r'==','!='),(r'!=','=='),(r'&&','||'),(r'\|\|','&&'),
 (r'\bmin\(','max('),(r'\bmax\(','min('),(r'\+ 1u','+ 2u'),(r'- 1u','- 2u'),(r'\+ 1i','+ 2i'),(r'- 1i','- 2i'),(r'>> ','<< '),(r'\bselect\(([^,()]+), ([^,()]+),',r'select(\2, \1,')]
for n,l in enumerate(src,1):
    s=l.strip()
    if not s or s.startswith('//') or s.startswith('@') or s.startswith('struct ') or re.match(r'fn \w+\(',s) or s.startswith('const '): continue
    if 'vec2<u32>' in s and '@' in s: continue
    if src.count(l)>1: continue
    for pat,rep in rules:
        for m in re.finditer(pat,l):
            new=l[:m.start()]+m.expand(rep) +l[m.end():] if '\\' in rep else l[:m.start()]+rep+l[m.end():]
            if new!=l and ' ||| ' not in l:
                out.append(f"L{n}:{m.start()} {pat[:6]} ||| {l} ||| {new}")
open(sys.argv[2],'w').write('\n'.join(out)+'\n');print(len(out))
