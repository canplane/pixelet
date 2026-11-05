#!/usr/bin/env python3
import os,subprocess,sys,glob,tempfile,concurrent.futures as cf
sp,mut,out,procs=sys.argv[1:5]
pkg=f'{sp}/copy/rust/client/render'
bin=max((f for f in glob.glob(f'{sp}/target/debug/deps/world-*') if os.access(f,os.X_OK) and '.' not in os.path.basename(f)),key=os.path.getmtime)
lines=[l for l in open(mut).read().split('\n') if l.strip()]
o=open(out,'w')
def one(l):
    t=tempfile.NamedTemporaryFile('w',suffix='.txt',delete=False);t.write(l+'\n');t.close()
    env=dict(os.environ,MUTANTS=t.name,WORLD_WGSL=f'{pkg}/src/frame/shader/world.wgsl')
    try:
        r=subprocess.run([bin,'--nocapture'],cwd=pkg,env=env,capture_output=True,text=True,timeout=60)
        res=[x for x in r.stdout.split('\n') if x.startswith('MUT')]
        s=res[0] if res else f'MUT ?? {l.split(" ||| ")[0]}: rc {r.returncode}'
    except subprocess.TimeoutExpired:
        s=f'MUT hang {l.split(" ||| ")[0]}: timed out'
    os.unlink(t.name);o.write(s+'\n');o.flush()
with cf.ThreadPoolExecutor(int(procs)) as ex:list(ex.map(one,lines))
