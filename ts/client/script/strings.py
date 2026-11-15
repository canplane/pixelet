#!/usr/bin/env python3
# Refuses a published core that holds a string of pixelet's or xpute's sources
# outside what it may hold.
#
#   strings.py <core> <cargo's build of it> <target> <cargo workspace>
#
# A string of ours is a piece of a string literal, cut at its format arguments
# and at what is not text, in a package of this repository the core is built
# from for `target`. The whole file is read, as number-paths.py reads it, and
# the shaders this build includes are taken out first. The file may hold a
# piece where its source puts it, as a literal `ALLOWED` names or a shader's
# entry point's name. Anywhere else, texts the file may hold must cover the
# piece, each reaching before it or past it: the shaders' entry points' names,
# the standard library's and the other crates' texts and names, the numbered
# panic paths and the literals `ALLOWED` names. A piece the same as one of
# those texts passes wherever we write it. Each other piece is printed, and
# the build stops.

import json
import re
import subprocess
import sys
from bisect import bisect_left
from itertools import accumulate
from pathlib import Path

# A shorter piece is a word any text holds.
LEAST_SZ = 4
# A Rust file's comments, its string literals, raw or not, and its char
# literals, which may hold a quote: what of it is not code.
NOT_CODE = re.compile(rb'//[^\n]*|/\*.*?\*/|b?r(#*)"(.*?)"\1|b?"((?:[^"\\]|\\.)*)"|\'(?:[^\'\\\n]|\\[^\n]{1,10}?)\'', re.S)
ESCAPE = re.compile(rb"\\(?:x([0-9a-fA-F]{2})|u\{([0-9a-fA-F]+)\}|\n\s*|(.))", re.S)
SIMPLE = {b"n": b"\n", b"t": b"\t", b"r": b"\r", b"0": b"\0", b"\\": b"\\", b'"': b'"', b"'": b"'"}
ARGUMENT = re.compile(rb"\{[^{}]*\}")
NAME = re.compile(rb"[A-Za-z_][A-Za-z0-9_]*")
# A text the core's file holds: printable ASCII and UTF-8's multibyte
# characters.
TEXT = re.compile(rb"(?:[\x20-\x7e\t\n\r]|[\xc2-\xf4][\x80-\xbf]+)+")
# A source path number-paths.py numbered.
NUMBERED = re.compile(rb"\.+#[0-9]+")
# A shader's entry point, after its stage's attribute.
ENTRY = re.compile(rb"@(?:compute|vertex|fragment)\b[^{]*?\bfn (\w+)", re.S)
# A package's directories that are not built into its library.
OUTSIDE_LIBRARY = {"target", "tests", "examples", "benches", "build"}
# How far before a literal its declaration is read.
DECL_SZ = 200
# A `&str` constant's declaration, up to its value.
STR_CONST = re.compile(rb"const \w+: &(?:'static )?str =\s*$")
# A URI constant's declaration, up to one of its words.
URI_CONST = re.compile(rb'const \w+_URI: &\[&str\] = &\[(?:"[^"]*",\s*)*$')

# The literals of our sources the core may hold, by their file: of each, the
# test of a literal's text, its source, its offset there and whether it is a
# byte string.
ALLOWED = {
    # glTF's keys and names.
    "rust/client/render/src/asset.rs": lambda text, src, off, byte: NAME.fullmatch(text),
    # JSON's words.
    "xpute/kit/rust/src/codec/json.rs": lambda text, src, off, byte: NAME.fullmatch(text),
    # The images' magic numbers.
    "rust/spec/src/asset.rs": lambda text, src, off, byte: byte,
    # The line-break sets.
    "rust/client/core/src/sys/io/text.rs": lambda text, src, off, byte: STR_CONST.search(src, max(0, off - DECL_SZ), off),
    # The URIs the guest opens its GPU device by and serves its surface under.
    "rust/gpu/link/src/api.spec.rs": lambda text, src, off, byte: URI_CONST.search(src, max(0, off - DECL_SZ), off),
}


def unescape(text):
    def one(m):
        if m.group(1):
            return bytes([int(m.group(1), 16)])
        if m.group(2):
            return chr(int(m.group(2), 16)).encode()
        if m.group(3) is None:
            return b""
        return SIMPLE.get(m.group(3), m.group(0))

    return ESCAPE.sub(one, text)


def literals(src):
    """Each string literal of a Rust source, as its bytes, with its offset and
    whether it is a byte string."""
    for m in NOT_CODE.finditer(src):
        if m.group(2) is not None:
            yield m.group(2), m.start(), m.group().startswith(b"b")
        elif m.group(3) is not None:
            yield unescape(m.group(3)), m.start(), m.group().startswith(b"b")


def pieces(text):
    """A literal's pieces of `LEAST_SZ` bytes or more, cut at its format
    arguments and at what is not text, as the core's file holds them."""
    for piece in ARGUMENT.split(text):
        piece = piece.replace(b"{{", b"{").replace(b"}}", b"}")
        for m in TEXT.finditer(piece):
            if len(m.group()) >= LEAST_SZ:
                yield m.group()


def texts(files):
    """The pieces of the literals of `files`."""
    return {p for f in files for text, _, _ in literals(f.read_bytes()) for p in pieces(text)}


def names(files):
    """The names of the code of `files`, out of its comments and literals."""
    return {m.group() for f in files for m in NAME.finditer(NOT_CODE.sub(b" ", f.read_bytes())) if len(m.group()) >= LEAST_SZ}


def packages(workspace, repo, target):
    """The directories of the packages the core is built from for `target`:
    those in `repo`, then the others'."""
    meta = json.loads(subprocess.run(["cargo", "metadata", "--format-version", "1", "--filter-platform", target], cwd=workspace, check=True, stdout=subprocess.PIPE).stdout)
    by_id = {p["id"]: p for p in meta["packages"]}
    deps = {n["id"]: [d["pkg"] for d in n["deps"] if any(k["kind"] is None for k in d["dep_kinds"])] for n in meta["resolve"]["nodes"]}
    seen, todo = set(), [next(i for i, p in by_id.items() if p["name"] == "pxl-core")]
    while todo:
        i = todo.pop()
        if i not in seen:
            seen.add(i)
            todo += deps[i]
    dirs = sorted(Path(by_id[i]["manifest_path"]).parent for i in seen)
    return [d for d in dirs if repo in d.parents], [d for d in dirs if repo not in d.parents]


def sources(dirs):
    """The Rust files built into the libraries under `dirs`."""
    for d in dirs:
        for f in sorted(d.rglob("*.rs")):
            parts = f.relative_to(d).parts
            if OUTSIDE_LIBRARY.isdisjoint(parts) and parts != ("build.rs",) and not f.name.endswith(".test.rs"):
                yield f


def library(workspace):
    """The standard library's sources: core, alloc and std."""
    sysroot = subprocess.run(["rustc", "--print", "sysroot"], cwd=workspace, check=True, stdout=subprocess.PIPE, text=True).stdout.strip()
    dirs = [Path(sysroot, "lib/rustlib/src/rust/library", c) for c in ("core", "alloc", "std")]
    # Without them every text the standard library puts in the core would read
    # as ours: the toolchain lacks its rust-src component.
    missing = [d for d in dirs if not d.is_dir()]
    if missing:
        sys.exit(f"[strings] no standard library sources at {missing[0]}: add the toolchain's rust-src component")
    return dirs


def shaders(built):
    """The shaders this build of the core includes: the WGSL files its
    dep-info names (pxl-render build.rs `assemble`)."""
    _, deps = Path(built).with_suffix(".d").read_text().split(": ", 1)
    return [Path(p).read_bytes() for p in deps.split() if p.endswith(".wgsl")]


def entries(shaders):
    """The names of the shaders' entry points."""
    return {name for text in shaders for name in ENTRY.findall(text)}


def found_at(module, text):
    """Where `module` holds `text`."""
    at = module.find(text)
    while at >= 0:
        yield at
        at = module.find(text, at + 1)


def holding(texts, pieces):
    """The texts that hold one of `pieces`."""
    by_head = {}
    for p in pieces:
        by_head.setdefault(p[:LEAST_SZ], []).append(p)
    return {t for t in texts if any(t.startswith(p, i) for i in range(len(t) - LEAST_SZ + 1) for p in by_head.get(t[i : i + LEAST_SZ], ()))}


def main():
    core, built, target, workspace = sys.argv[1:]
    repo = Path(workspace).resolve().parent
    # A shader may lie inside another, so each is found in the file as built.
    file = Path(core).read_bytes()
    module = bytearray(file)
    wgsl = shaders(built)
    for text in wgsl:
        for at in found_at(file, text):
            module[at : at + len(text)] = bytes(len(text))
    module = b"\0".join(m.group() for m in TEXT.finditer(module) if len(m.group()) >= LEAST_SZ)
    entry = entries(wgsl)
    ours, others = packages(workspace, repo, target)
    theirs = [*sources(library(workspace)), *sources(others)]
    # Each piece of ours, with the first place its sources hold it, and the
    # texts the module may hold; a piece that is one of those is not checked.
    checked, held = {}, texts(theirs) | names(theirs) | entry
    for f in sources(ours):
        src = f.read_bytes()
        path = f.relative_to(repo).as_posix()
        allowed = ALLOWED.get(path, lambda *_: False)
        for text, off, byte in literals(src):
            for p in pieces(text):
                if allowed(text, src, off, byte):
                    held.add(p)
                else:
                    checked.setdefault(p, (path, src.count(b"\n", 0, off) + 1))
    found = {p: list(found_at(module, p)) for p in checked if p not in held}
    found = {p: ats for p, ats in found.items() if ats}
    # The places of the module that may be held (the numbered paths and the
    # texts it may hold that hold a piece found), in the order they start,
    # and how far those up to each reach; and the texts it may hold by their
    # first and their last bytes, for a place that holds a piece's end or its
    # start.
    spans = [m.span() for m in NUMBERED.finditer(module)]
    spans += [(at, at + len(t)) for t in holding(held, found) for at in found_at(module, t)]
    spans.sort()
    starts = [a for a, _ in spans]
    reach = list(accumulate((b for _, b in spans), max))
    heads, tails = {}, {}
    for t in held:
        heads.setdefault(t[:LEAST_SZ], []).append(t)
        tails.setdefault(t[-LEAST_SZ:], []).append(t)

    def covered(a, b):
        """Whether places that may be held cover `a` to `b`, each reaching
        before `a` or past `b`."""
        k = bisect_left(starts, a)
        end = max(reach[k - 1], a) if k else a
        for e in range(a + 1, b):
            for t in tails.get(module[e - LEAST_SZ : e], ()):
                if len(t) > e - a and module.startswith(t, e - len(t)):
                    end = max(end, e)
        firsts = [s for s, e in spans[k : bisect_left(starts, b)] if e > b]
        firsts += [s for s in range(a + 1, b) for t in heads.get(module[s : s + LEAST_SZ], ()) if s + len(t) > b and module.startswith(t, s)]
        return end >= b or any(s <= end for s in firsts)

    bad = [p for p, ats in sorted(found.items()) if not all(covered(at, at + len(p)) for at in ats)]
    for p in bad:
        path, line = checked[p]
        print(f"[strings] {core} holds {p.decode(errors='replace')!r} ({path}:{line})", file=sys.stderr)
    if bad:
        sys.exit(1)


if __name__ == "__main__":
    main()
