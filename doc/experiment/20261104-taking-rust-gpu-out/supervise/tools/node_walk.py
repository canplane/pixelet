import subprocess,glob,os,tempfile
SP='/private/tmp/claude-501/-Users-sanghoon-git-tmaxoffice-cx/eff52c1d-b098-47ac-a575-b58a0e234362/scratchpad'; d='/private/tmp/claude-501/-Users-sanghoon-git-tmaxoffice-cx/eff52c1d-b098-47ac-a575-b58a0e234362/scratchpad/target/debug/build/pxl-render-8cfddf659c56f728/out/shader'
src=open(d+'/walk_node.wgsl').read().split('\n')
b=max((f for f in glob.glob(SP+'/target/debug/deps/walk-*') if os.access(f,os.X_OK) and '.' not in os.path.basename(f)),key=os.path.getmtime)
pkg=SP+'/copy/rust/client/render'
done=set()
for l in open(SP+'/tools/lm4.out'):
    p=l.rstrip('\n').split('\t')
    if p[0]!='SURVIVED': continue
    was=p[4];new=p[2]
    idx=[i for i,x in enumerate(src) if x.strip()==was]
    if not idx: print('not node',p[1],new[:90]);continue
    ml=src[:];ml[idx[0]]=new
    f=tempfile.NamedTemporaryFile('w',suffix='.wgsl',delete=False);f.write('\n'.join(ml));f.close()
    try:
        r=subprocess.run([b,'--nocapture'],cwd=pkg,env={**os.environ,'MUT_NODE':f.name},capture_output=True,text=True,timeout=90)
        k='SURVIVED' if r.returncode==0 else 'killed'
    except subprocess.TimeoutExpired: k='hang'
    os.unlink(f.name)
    print(k,p[1],new[:100])
