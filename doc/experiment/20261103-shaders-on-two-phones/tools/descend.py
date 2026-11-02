"""Finds the block of a generated shader the phone's driver refuses: empties
one block at a time (an if's or else's body; a loop removed whole), compiles
each variant on the phone (cdp.mjs), and descends into the first block whose
emptying makes it compile.  python3 descend.py <name>"""
import json, os, re, subprocess, sys

NAME = sys.argv[1]
BASE = sys.argv[2] if len(sys.argv) > 2 else None
SRC = BASE or f"../../../../rust/client/render/src/frame/shader/{NAME}.gen.wgsl"
OUT = "variants"
lines = open(SRC).read().split("\n")

# Blocks: each line ending in "{" opens one closed by the matching "}" line.
stack, blocks = [], []
for i, l in enumerate(lines):
    t = l.strip()
    if t.startswith("}") and stack:
        o = stack.pop()
        blocks.append((o, i))
    if t.endswith("{"):
        stack.append(i)
parent = {}
for b in blocks:
    inside = [c for c in blocks if c[0] < b[0] and b[1] < c[1]]
    parent[b] = max(inside, key=lambda c: c[0]) if inside else None

def kind(b):
    t = lines[b[0]].strip()
    return "continuing" if t.startswith("continuing") else "loop" if t.startswith("loop") else "fn" if t.startswith("fn ") else "if"

def variant(b):
    o, c = b
    if kind(b) == "loop":
        keep = lines[:o] + lines[c + 1:]
    else:
        keep = lines[:o + 1] + lines[c:]
    return "\n".join(keep)

def compile_all(cands):
    os.makedirs(OUT, exist_ok=True)
    files = []
    for b in cands:
        f = f"{OUT}/{NAME}-{b[0]+1}-{b[1]+1}.wgsl"
        open(f, "w").write(variant(b))
        files.append(f)
    out = subprocess.run(["node", "cdp.mjs", *files], capture_output=True, text=True).stdout
    res = {}
    for line in out.strip().split("\n"):
        f, _, said = line.partition(": ")
        res[f] = said
    return [(b, res.get(f.split("/")[-1], "?")) for b, f in zip(cands, files)]

fns = [b for b in blocks if kind(b) == "fn"]
here = max(fns, key=lambda b: b[1] - b[0])
path = []
while True:
    kids = [b for b in blocks if parent[b] == here and kind(b) != "continuing"]
    if not kids:
        break
    results = compile_all(kids)
    for b, said in results:
        print(f"  {kind(b)} {b[0]+1}-{b[1]+1} ({b[1]-b[0]} lines): {said}", flush=True)
    fixed = [b for b, said in results if said.startswith("ok")]
    if not fixed:
        print(f"no single child of {here[0]+1}-{here[1]+1} fixes it", flush=True)
        break
    here = min(fixed, key=lambda b: b[1] - b[0])
    path.append(here)
    print(f"-> into {kind(here)} {here[0]+1}-{here[1]+1}", flush=True)
print("path:", " > ".join(f"{b[0]+1}-{b[1]+1}" for b in path))
