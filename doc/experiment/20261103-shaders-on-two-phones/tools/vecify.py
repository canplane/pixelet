"""array<S, N> (S a 32-bit scalar, N 2..4) as vecN<S>, except in the structs a
uniform or storage binding reaches.  python3 vecify.py in.wgsl out.wgsl"""
import re, sys
src = open(sys.argv[1]).read()
# Structs: name -> body
structs = {m.group(1): m.group(2) for m in re.finditer(r"struct (\w+) \{([^}]*)\}", src)}
roots = set(re.findall(r"var<(?:uniform|storage)[^>]*> \w+: (\w+);", src))
held = set()
def reach(t):
    if t in held or t not in structs: return
    held.add(t)
    for u in re.findall(r"\b(type_\d+)\b", structs[t]): reach(u)
for r in roots: reach(r)
SMALL = re.compile(r"array<(u32|i32|f32), ([234])>")
def swap(text): return SMALL.sub(lambda m: f"vec{m.group(2)}<{m.group(1)}>", text)
out, last = [], 0
for m in re.finditer(r"struct (\w+) \{[^}]*\}", src):
    out.append(swap(src[last:m.start()]))
    out.append(m.group(0) if m.group(1) in held else swap(m.group(0)))
    last = m.end()
out.append(swap(src[last:]))
open(sys.argv[2], "w").write("".join(out))
