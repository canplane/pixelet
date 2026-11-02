"""Writes one variant per function of in.wgsl, its body a return of its type's
zero value: python3 fnbisect.py in.wgsl outdir"""
import re, sys, os
src = open(sys.argv[1]).read()
os.makedirs(sys.argv[2], exist_ok=True)
lines = src.split("\n")
for i, l in enumerate(lines):
    m = re.match(r"^fn (\w+)\(.*\)(?: -> (.+))? \{$", l)
    if not m: continue
    j = next(k for k in range(i + 1, len(lines)) if lines[k] == "}")
    if j - i < 8: continue
    body = [f"    return {m.group(2)}();"] if m.group(2) else ["    return;"]
    v = lines[:i + 1] + body + lines[j:]
    open(f"{sys.argv[2]}/{i+1:05d}-{m.group(1)}-{j-i}.wgsl", "w").write("\n".join(v))
