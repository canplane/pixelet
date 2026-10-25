#!/usr/bin/env python3
# `make check` and `make test`, narrowed to the working tree's changes: a
# crate when it or a dependency changed, TypeScript by what imports what. One
# selection drives both.
#
#   ./script/check-diff.py

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
changed = sorted(set(git("diff", "--name-only", "--no-renames", "HEAD")) | set(git("ls-files", "--others", "--exclude-standard")))
present = [path for path in changed if (ROOT / path).is_file()]


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
# xpute's crates are no member here, but its path dependencies are.
for p in packages:
    for d in p["dependencies"]:
        if d.get("path") and any(path.startswith(relative(d["path"]) + "/") for path in changed):
            touched.add(d["name"])
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


def rust(verb: str) -> list[list[str]]:
    """The cargo commands of the aliases `verb` begins (`lint-`, `test-`),
    each spelled out and its packages narrowed to the touched ones; none that
    names no touched one."""
    out = []
    for name, alias in aliases.items():
        if not name.startswith(verb):
            continue
        args = ["cargo", *alias]
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


def cargo(verb: str) -> None:
    for argv in rust(verb):
        runs.append((" ".join(argv[:2]), argv, RUST))


fmt_rust = [arg for name in sorted(touched & members) for arg in ("-p", name)]
# ts/client imports xpute's TypeScript, and every app and the Worker the client.
xpute = under("xpute/")
ts = under("ts/") or under("xpute/ts/")

if ts:
    task("check-ts")
if fmt_rust:
    runs.append(("cargo fmt", ["cargo", "fmt", "--check", *fmt_rust], RUST))
cargo("lint-")
if under("spec/") or any(".spec." in path for path in changed) or "pxl-tool" in touched:
    task("check-spec")
if under("ts/client/") or under("ts/app/web/") or under("ts/server/edge/") or under("xpute/ts/") or "pxl-wasm" in touched:
    task("check-build")
# Each formatted file under ts/, by its config; what lies outside the trees is not formatted.
formatted = [path for path in present if re.search(r"\.(ts|tsx|js|json|jsonc|md)$", path)]
for tree in ["ts/"]:
    own = [path[len(tree):] for path in formatted if path.startswith(tree)]
    if own:
        runs.append((f"fmt {tree}", ["deno", "fmt", "--check", *own], ROOT / tree))
task("check-rules")
if xpute:
    task("check-xpute")

# The tests of what the same selection reached.
# The web app's test compiles the generated shaders with Tint.
if under("ts/client/") or under("ts/app/web/") or under("xpute/ts/") or under("rust/client/render/src/frame/shader/"):
    task("test-ts")
if xpute:
    task("test-xpute")
cargo("test-")

print(f"check-diff: {len(changed)} changed file(s); crates {', '.join(sorted(touched)) or 'none'}")
for label, argv, cwd in runs:
    print(f"check-diff: {label}", flush=True)
    status = subprocess.run(argv, cwd=cwd).returncode
    if status != 0:
        print(f"check-diff: {label} failed", file=sys.stderr)
        sys.exit(status or 1)
