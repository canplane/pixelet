import sys
L=open(sys.argv[1]).read().split("\n"); out=[]; n=0
for l in L:
    out.append(l)
    if l.strip()=="loop {":
        ind=l[:len(l)-len(l.lstrip())]+"    "
        out+=[f"{ind}guard_{n} = guard_{n} + 1u;",f"{ind}if (guard_{n} > 1000u) {{",f"{ind}    break;",f"{ind}}}"]; n+=1
i=next(k for k,x in enumerate(out) if x.startswith("fn "))
out=out[:i]+[f"var<private> guard_{k}: u32;" for k in range(n)]+out[i:]
open(sys.argv[2],"w").write("\n".join(out))
