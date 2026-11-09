#!/usr/bin/env python3
# `make check` and `make test`, narrowed to what changed since `base` (HEAD
# by default, the working tree's changes; a commit or branch to take what was
# committed since it too): a crate when it, a file it reads or a dependency
# changed, TypeScript by its tree. One selection drives both. `--list` prints
# the selection and runs nothing.
#
#   ./script/diff-check.py [--list] [base]

import itertools
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RUST = ROOT / "rust"


def git(*args: str) -> list[str]:
    out = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True, check=True)
    return [line for line in out.stdout.split("\n") if line]


# --no-renames: a file moved out of a crate touches the crate it left.
args = sys.argv[1:]
listing = "--list" in args
args = [a for a in args if a != "--list"]
base = args[0] if args else "HEAD"
changed = sorted(set(git("diff", "--name-only", "--no-renames", base)) | set(git("ls-files", "--others", "--exclude-standard")))


def under(prefix: str) -> bool:
    return any(path.startswith(prefix) for path in changed)


metadata = subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=RUST, capture_output=True, text=True, check=True)
packages = json.loads(metadata.stdout)["packages"]


def relative(path: str) -> str:
    return str(Path(path).relative_to(ROOT))


# The deepest crate a file is under is its own.
crate_dirs = sorted(((p["name"], relative(str(Path(p["manifest_path"]).parent)) + "/") for p in packages), key=lambda c: -len(c[1]))
touched: set[str] = set()
for path in changed:
    own = next((name for name, d in crate_dirs if path.startswith(d)), None)
    if own:
        touched.add(own)
    # What under rust/ is no crate's (the lock, the aliases, the workspace) builds them all.
    elif path.startswith("rust/"):
        touched |= {name for name, _ in crate_dirs}
# A crate reads what its sources name: a module by `#[path]`, and a file of the
# repository by its path from the root (`PXL_REPO`, pxl-tool `root`).
tops = {path.split("/")[0] for path in git("ls-files")} - {"rust"}
under_root = "(?:" + "|".join(map(re.escape, sorted(tops))) + ")/"
named = re.compile(r'(?:"(?:\.\./)*|PXL_REPO"\),\s*"/)(' + under_root + r'[^"\s]*)"')
module = re.compile(r'#\[path = "([^"]+)"\]')
for name, d in crate_dirs:
    for source in (ROOT / d).rglob("*.rs"):
        text = source.read_text(errors="replace")
        reads = [m.rstrip("/") for m in named.findall(text)]
        reads += [relative(str((source.parent / m).resolve())) for m in module.findall(text)]
        if any(path == r or path.startswith(r + "/") for r in reads for path in changed):
            touched.add(name)
# xpute's crates are no member here, but its path dependencies are.
for p in packages:
    for d in p["dependencies"]:
        if d.get("path") and any(path.startswith(relative(d["path"]) + "/") for path in changed):
            touched.add(d["name"])
# What the workspace patches (the vendored wgpu) every crate above it builds on.
with open(RUST / "Cargo.toml", "rb") as f:
    patched = [relative(str((RUST / crate["path"]).resolve())) + "/" for source in tomllib.load(f).get("patch", {}).values() for crate in source.values() if "path" in crate]
if any(map(under, patched)):
    touched |= {name for name, _ in crate_dirs}
grew = True
while grew:
    grew = False
    for p in packages:
        if p["name"] not in touched and any(d.get("path") and d["name"] in touched for d in p["dependencies"]):
            touched.add(p["name"])
            grew = True

members = {p["name"] for p in packages}

# rust/.cargo/config.toml's aliases, each the arguments it stands for.
with open(RUST / ".cargo/config.toml", "rb") as f:
    aliases: dict[str, list[str]] = tomllib.load(f).get("alias", {})


def recipe(target: str) -> str:
    """The Makefile's recipe for `target`."""
    lines = (ROOT / "Makefile").read_text().split("\n")
    at = next(k for k, line in enumerate(lines) if line.startswith(target + ":")) + 1
    return "\n".join(itertools.takewhile(lambda line: line.startswith("\t"), lines[at:]))


def rust(target: str) -> list[list[str]]:
    """The cargo commands of the aliases `target`'s recipe runs, each spelled
    out and its packages narrowed to the touched ones; none that names no
    touched one."""
    out = []
    for name in re.findall(r"cargo ([\w-]+)", recipe(target)):
        if name not in aliases:
            continue
        args = ["cargo", *aliases[name]]
        kept: list[str] = []
        any_kept = False
        i = 0
        while i < len(args):
            if args[i] == "-p":
                if args[i + 1] in touched:
                    kept += ["-p", args[i + 1]]
                    any_kept = True
                i += 1
            elif args[i] == "--features" and i + 1 < len(args) and "/" in args[i + 1]:
                # A selected package's feature, which goes when the package does.
                if args[i + 1].split("/")[0] in touched:
                    kept += [args[i], args[i + 1]]
                i += 1
            elif args[i] == "--all":
                for member in sorted(touched & members):
                    kept += ["-p", member]
                any_kept = "-p" in kept
            else:
                kept.append(args[i])
            i += 1
        if any_kept:
            out.append(kept)
    return out


runs: list[tuple[str, list[str], Path]] = []


def task(name: str) -> None:
    runs.append((name, ["make", name], ROOT))


def cargo(target: str) -> None:
    for argv in rust(target):
        # The Android lint builds C with the NDK's compiler (script/env.sh).
        run = [str(ROOT / "script/env.sh"), *argv] if "aarch64-linux-android" in argv else argv
        runs.append((" ".join(argv[:2]), run, RUST))


fmt_rust = [arg for name in sorted(touched & members) for arg in ("-p", name)]
# ts/client imports xpute's TypeScript, and every app and the Worker the client.
# What `make check` and `make test` leave to a deploy (the bundle, xpute's own
# checks and tests) is not selected here either.
ts = under("ts/") or under("xpute/ts/")

if ts:
    task("check-ts")
if fmt_rust:
    runs.append(("cargo fmt", ["cargo", "fmt", "--check", *fmt_rust], RUST))
cargo("check-rust")
if under("spec/") or any(".spec." in path for path in changed) or "pxl-tool" in touched:
    task("check-spec")
task("check-rules")

# What the assembled shaders are made from: their sources, the words the build writes and
# the crates those words and the Rust they translate come from.
translated = [d for name, d in crate_dirs if name in ("pxl-engine", "pxl-content")]
shaders = [
    *translated,
    "rust/client/render/src/frame/shader/",
    "rust/client/render/build.rs",
    "rust/client/render/src/param.rs",
    "rust/client/render/src/look/",
]

# The tests of what the same selection reached.
# The shader test compiles the shaders with Tint.
if ts or any(map(under, shaders)):
    task("test-ts")
cargo("test-rust")
# The GPU's tests hold the shaders written by hand to the Rust they translate,
# so a change to either side is checked against the other.
if any(map(under, [*shaders, *patched, "rust/client/render/tests/"])):
    task("test-gpu")

print(f"diff-check: {len(changed)} changed file(s); crates {', '.join(sorted(touched)) or 'none'}")
for label, argv, cwd in runs:
    if listing:
        print("  " + " ".join(argv))
        continue
    print(f"diff-check: {label}", flush=True)
    status = subprocess.run(argv, cwd=cwd).returncode
    if status != 0:
        print(f"diff-check: {label} failed", file=sys.stderr)
        sys.exit(status or 1)

# What a browser's compiler and a phone's driver refuse, no test here can see.
if not listing and any(map(under, shaders)):
    print("diff-check: what a shader is made from changed; the probe in Safari is left to run (.agents/skills/shader)")
