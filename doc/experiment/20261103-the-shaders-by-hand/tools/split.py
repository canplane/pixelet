"""Splits each function-local `phi_*` variable of an array or struct type into
one variable per scalar or vector leaf: a store of the whole value stores
each leaf, a load composes them.  python3 split.py in.wgsl out.wgsl"""
import re, sys
src = open(sys.argv[1]).read()
structs = {m.group(1): [(a, b.strip()) for a, b in re.findall(r"^\s*(\w+): (.+),$", m.group(2), re.M)]
           for m in re.finditer(r"struct (\w+) \{([^}]*)\}", src)}

def leaves(t, path):
    """[(access suffix, leaf type)] of type t."""
    t = t.strip()
    m = re.fullmatch(r"array<(.+), (\d+)>", t)
    if m:
        out = []
        for i in range(int(m.group(2))):
            out += leaves(m.group(1), f"{path}[{i}]")
        return out
    if t in structs:
        out = []
        for name, mt in structs[t]:
            out += leaves(mt, f"{path}.{name}")
        return out
    return [(path, t)]

def compose(t, names, it):
    t = t.strip()
    m = re.fullmatch(r"array<(.+), (\d+)>", t)
    if m:
        return f"{t}(" + ", ".join(compose(m.group(1), names, it) for _ in range(int(m.group(2)))) + ")"
    if t in structs:
        return f"{t}(" + ", ".join(compose(mt, names, it) for _, mt in structs[t]) + ")"
    return next(it)

def leafname(v, suffix):
    return v + re.sub(r"\[(\d+)\]", r"_i\1", re.sub(r"\.(\w+)", r"_f_\1", suffix))

tmp = [0]
lines = src.split("\n")
out = []
split = {}
for l in lines:
    m = re.match(r"^(\s*)var (phi_\w+): ([^;=]+);$", l)
    if m and (m.group(3).startswith("array<") or m.group(3).strip() in structs):
        ind, v, t = m.groups()
        ls = leaves(t, "")
        if 1 < len(ls) <= 64:
            split[v] = (t, ls)
            out += [f"{ind}var {leafname(v, s)}: {lt};" for s, lt in ls]
            continue
    m = re.match(r"^(\s*)(phi_\w+) = (.+);$", l)
    if m and m.group(2) in split:
        ind, v, e = m.groups()
        t, ls = split[v]
        tmp[0] += 1
        tv = f"split_{tmp[0]}"
        out.append(f"{ind}let {tv} = {e};")
        out += [f"{ind}{leafname(v, s)} = {tv}{s};" for s, _ in ls]
        continue
    m = re.match(r"^(\s*)let (\w+) = (phi_\w+);$", l)
    if m and m.group(3) in split:
        ind, n, v = m.groups()
        t, ls = split[v]
        out.append(f"{ind}let {n} = {compose(t, None, iter(leafname(v, s) for s, _ in ls))};")
        continue
    out.append(l)
text = "\n".join(out)
left = [v for v in split if re.search(rf"\b{v}\b", text)]
assert not left, f"used otherwise: {left[:5]}"
open(sys.argv[2], "w").write(text)
print(f"{sys.argv[1].split('/')[-1]}: {len(split)} split")
