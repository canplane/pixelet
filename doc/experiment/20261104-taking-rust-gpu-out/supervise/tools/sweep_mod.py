#!/usr/bin/env python3
# Mutants of an assembled module, each run through a test binary built to read MUT_WGSL.
# sweep_mod.py MODULE TEST_PREFIX OUT JOBS LIMIT [EXCLUDE_MODULE]
import re,subprocess,sys,os,glob,tempfile,concurrent.futures as cf,time
mod,prefix,out,jobs,limit=sys.argv[1],sys.argv[2],sys.argv[3],int(sys.argv[4]),int(sys.argv[5])
ex=set(open(sys.argv[6]).read().split('\n')) if len(sys.argv)>6 else set()
SP=os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
pkg=f'{SP}/copy/rust/client/render'
b=max((f for f in glob.glob(f'{SP}/target/debug/deps/{prefix}-*') if os.access(f,os.X_OK) and '.' not in os.path.basename(f)),key=os.path.getmtime)
src=open(mod).read().split('\n')
rules=[(r'(?<![<>=!-])<(?![<=])','<='),(r'<=','<'),(r'>=','>'),(r'(?<![<>=\-])>(?![>=])','>='),(r'==','!='),(r'!=','=='),(r'&&','||'),(r'\|\|','&&'),
 (r'\bmin\(','max('),(r'\bmax\(','min('),(r'\+ 1u','+ 2u'),(r'- 1u','- 2u'),(r'\+ 1i','+ 2i'),(r'- 1i','- 2i'),(r'>> ','<< ')]
muts=[]
for i,l in enumerate(src):
    t=l.strip()
    if not t or l in ex or t.startswith(('@','struct ','const ','fn ','var<','//')): continue
    if re.match(r'^\w+: ',t): continue
    for pat,to in rules:
        for m in re.finditer(pat,l):
            new=l[:m.start()]+to+l[m.end():]
            if re.search(r'vec\d<=|vec\d<[iuf]32>=|<=[iuf]32|array<=|atomic<=',new): continue
            muts.append((i,new))
print(len(muts),'mutants',flush=True)
o=open(out,'w')
cnt={}
def run(k):
    i,new=muts[k]; ml=src[:]; ml[i]=new
    f=tempfile.NamedTemporaryFile('w',suffix='.wgsl',delete=False); f.write('\n'.join(ml)); f.close()
    try:
        r=subprocess.run([b,'--nocapture'],cwd=pkg,env={**os.environ,'MUT_WGSL':f.name},capture_output=True,text=True,timeout=limit)
        s=r.stdout+r.stderr
        kind='SURVIVED' if r.returncode==0 else ('refused' if re.search(r'Shader.*(error|parsing)|Validation Error|refused',s) and 'differ' not in s[-400:] else 'killed')
    except subprocess.TimeoutExpired:
        kind='hang'
    os.unlink(f.name)
    return k,kind
with cf.ThreadPoolExecutor(jobs) as ex2:
    for k,kind in map(lambda f:f.result(),[ex2.submit(run,k) for k in range(len(muts))]):
        cnt[kind]=cnt.get(kind,0)+1
        i,new=muts[k]
        o.write(f'{kind}\t{i+1}\t{new.strip()}\t|was\t{src[i].strip()}\n');o.flush()
print(cnt,flush=True)
