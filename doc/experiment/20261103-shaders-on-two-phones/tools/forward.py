"""Forwards composite lets: `let X = T(a, b, …);` with T an array or struct type,
each `X.f` / `X[i]` (constant) replaced by the component, nested; X kept only
where still used whole.  python3 forward.py in.wgsl out.wgsl"""
import re, sys
src = open(sys.argv[1]).read()
structs = {m.group(1): [a for a in re.findall(r"^\s*(\w+): .+,$", m.group(2), re.M)]
           for m in re.finditer(r"struct (\w+) \{([^}]*)\}", src)}

def split_args(s):
    out, depth, cur = [], 0, ""
    for i, ch in enumerate(s):
        prev = s[i - 1] if i else " "
        if ch in "([" or (ch == "<" and prev != " " and prev != "<"): depth += 1
        elif ch in ")]" or (ch == ">" and prev != " " and prev != ">" and prev != "-"): depth -= 1
        if ch == "," and depth == 0:
            out.append(cur.strip()); cur = ""
        else:
            cur += ch
    if cur.strip(): out.append(cur.strip())
    return out

def ctor(e):
    """(type, [args]) where e is T(args) with T an array or struct type, else None."""
    m = re.match(r"^(array<.+>|type_\d+)\((.*)\)$", e.strip())
    if not m: return None
    t, inner = m.group(1), m.group(2)
    depth = 0
    for i, ch in enumerate(e.strip()[len(t):]):
        if ch == "(": depth += 1
        elif ch == ")":
            depth -= 1
            if depth == 0 and i != len(e.strip()) - len(t) - 1: return None
    return t, split_args(inner)

def access(t, args, path):
    """The component expression for a path like .member[1] of T(args)."""
    m = re.match(r"^(?:\.(\w+)|\[(\d+)\])(.*)$", path)
    if not m: return None
    if not args: return None
    if m.group(1):
        if t not in structs or m.group(1) not in structs[t]: return None
        a = args[structs[t].index(m.group(1))]
    else:
        a = args[int(m.group(2))]
    rest = m.group(3)
    if not rest: return a
    c = ctor(a)
    if c and arity_ok(*c): return access(c[0], c[1], rest)
    if c: return None
    return a + rest

def arity_ok(t, args):
    m = re.fullmatch(r"array<(.+), (\d+)>", t)
    if m: return len(args) == int(m.group(2))
    return t in structs and len(args) == len(structs[t])
skipped = []
forwarded = [0]

def forward_fn(body):
    defs = {}
    for l in body.split("\n"):
        m = re.match(r"^\s*let (_e\d+|split_\d+) = (.+);$", l)
        if m:
            c = ctor(m.group(2))
            if c and arity_ok(*c): defs[m.group(1)] = c
            elif c and c[1]: skipped.append(l.strip()[:120])
    for x, (t, args) in defs.items():
        pat = re.compile(rf"\b{x}((?:\.\w+|\[\d+\])+)")
        def rep(m):
            a = access(t, args, m.group(1))
            if a is None: return m.group(0)
            forwarded[0] += 1
            return f"({a})"
        body = pat.sub(rep, body)
    return body

parts = re.split(r"(?m)^(?=fn )", src)
text = "".join(forward_fn(p) if p.startswith("fn ") else p for p in parts)
open(sys.argv[2], "w").write(text)
print(f"{sys.argv[1].split('/')[-1]}: {forwarded[0]} accesses forwarded, {len(skipped)} skipped", *skipped[:2], sep="\n  ")
