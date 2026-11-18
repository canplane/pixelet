import re,glob,os,subprocess
D=subprocess.check_output(["git", "-C", os.path.dirname(os.path.abspath(__file__)), "rev-parse", "--show-toplevel"], text=True).strip()+'/tmp/cmp/s23/'
def rep(p):
    t=open(p,errors='replace').read()
    n=re.search(r'frames n=(\d+)\s+over ([\d.]+)s',t); k=re.search(r'kinds\s+(.*?)\s+wakes',t)
    kinds=dict((a,int(b)) for a,b in re.findall(r'(\w+) (\d+)',k.group(1))) if k else {}
    iv=re.search(r'interval p50 ([\d.]+)\s+p95 ([\d.]+)',t); g=re.search(r'gpu\s+p50 ([\d.]+)\s+p95 ([\d.]+)',t)
    drawn=sum(v for a,v in kinds.items() if a not in('held','stood','unready','dropped'))
    return n and int(n.group(1)), drawn/float(n.group(2)) if n else 0, kinds, iv.groups() if iv else None, g.groups() if g else None
for tag in ['warm','main-1','base-1','base-2','main-2']:
    p=D+tag+'-report.txt'
    if not os.path.exists(p): continue
    clk=[int(m)//1000000 for m in re.findall(r'^t \d+ (\d+) ',open(D+tag+'.log').read(),re.M)]
    from collections import Counter
    c=Counter(clk)
    n,dps,kinds,iv,g=rep(p)
    r=D+tag+'-rest-report.txt'
    rest=rep(r) if os.path.exists(r) else None
    print(f"{tag:7} clock {dict(sorted(c.items()))} | drive n={n} drawn/s={dps:.1f} iv={iv} gpu={g} | rest " + (f"n={rest[0]} kinds={rest[2]} gpu={rest[4]}" if rest else "-"))
