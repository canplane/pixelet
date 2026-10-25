#!/usr/bin/env python3
# Copies what git tracks under xpute/ into xpute's own repository byte for
# byte, its root being this directory, and beside it the C++ library in two
# files, which cpp/Makefile's `amalgamation` makes. Prints a commit message
# naming the commit it came from, as <repository>@<sha>, for `git commit -F -`
# there.
#
#   ./script/export.py --to ../xpute [--check]

import os
import re
import subprocess
import sys
from pathlib import Path

XPUTE = Path(__file__).resolve().parent.parent

# Neither written nor compared: build output and lock files, a lock or
# node_modules/ in any tree, as .gitignore has them.
UNMANAGED = ["rust/target/", "target/", "rust/Cargo.lock"]
UNMANAGED_ANYWHERE = ["deno.lock", "node_modules"]

# Made by the step the C++ tree runs itself, from what git tracks there, and
# shipped at cpp/amalgamation/ as SQLite ships its two files; here they are
# build output, under target/.
AMALGAMATION = ["xpute.hpp", "xpute.cpp"]


def unmanaged(path: str) -> bool:
    return any(path.startswith(u) if u.endswith("/") else path == u for u in UNMANAGED) or any(
        name in UNMANAGED_ANYWHERE for name in path.split("/")
    )


def note(message: str) -> None:
    """stderr, so stdout carries only the commit message."""
    print(f"[export] {message}", file=sys.stderr)


def die(message: str):
    note(message)
    sys.exit(1)


def run(args: list[str], cwd: Path = XPUTE) -> subprocess.CompletedProcess:
    return subprocess.run(args, cwd=cwd, capture_output=True)


def git(args: list[str], cwd: Path = XPUTE) -> str:
    out = run(["git", *args], cwd)
    if out.returncode != 0:
        die(f"git {' '.join(args)}: {out.stderr.decode().strip()}")
    return out.stdout.decode().rstrip()


def stamp() -> str:
    """The repository this tree is developed in, by its checkout's name, and its commit."""
    source = Path(git(["rev-parse", "--show-toplevel"])).name
    dirty = "-dirty" if git(["status", "--porcelain", "--", "."]) else ""
    return f"{source}@{git(['rev-parse', '--short', 'HEAD'])}{dirty}"


def draft(to: Path, at: str) -> str:
    """The stamp, then the subjects since the one the repository's last commit names, when that still resolves."""
    last = run(["git", "log", "-1", "--format=%B"], to)
    # The stamp is the message's first word; a subject below may hold an `@` of its own.
    found = re.match(r"\S+@([0-9a-f]+)", last.stdout.decode()) if last.returncode == 0 else None
    previous = found and found.group(1)
    resolves = previous is not None and run(["git", "cat-file", "-e", f"{previous}^{{commit}}"]).returncode == 0
    subjects = [s for s in git(["log", "--format=%s", f"{previous}..HEAD", "--", "."]).split("\n") if s] if resolves else []
    return f"{at}\n" if not subjects else f"{at}\n\n" + "\n".join(f"  {s}" for s in subjects) + "\n"


# `executable` keeps the mode of scripts the tasks call by path.
File = tuple[str, bytes, bool]


def tracked() -> list[File]:
    # `mode sha stage<TAB>path`, one to a NUL, relative to xpute/.
    staged = [line for line in git(["ls-files", "-s", "-z", "--", "."]).split("\0") if line]
    if not staged:
        die("git listed nothing to export")
    files = []
    for line in staged:
        path = line[line.index("\t") + 1 :]
        files.append((path, (XPUTE / path).read_bytes(), line.startswith("100755 ")))
    return files


def amalgamation() -> list[File]:
    # Its own output captured, so stdout carries only the commit message.
    out = run(["make", "-C", "cpp", "amalgamation"])
    if out.returncode != 0:
        die(f"make -C cpp amalgamation: {out.stderr.decode().strip()}")
    return [(f"cpp/amalgamation/{name}", (XPUTE / "target/cpp/amalgamation" / name).read_bytes(), False) for name in AMALGAMATION]


def present(to: Path) -> list[str]:
    """What the repository holds besides `.git` and what is unmanaged."""
    if not to.is_dir():
        die(f"{to} is not a directory; clone the repository there first")
    out = []
    for dir, dirs, names in os.walk(to):
        at = Path(dir).relative_to(to)
        dirs[:] = sorted(d for d in dirs if d != ".git" and not unmanaged(f"{(at / d).as_posix()}/"))
        out += [p for p in ((at / n).as_posix() for n in sorted(names)) if not unmanaged(p)]
    return out


def compare(files: list[File], to: Path) -> tuple[list[str], list[str]]:
    differs = []
    for path, data, executable in files:
        full = to / path
        if not full.is_file():
            differs.append(path)
        elif full.read_bytes() != data or bool(full.stat().st_mode & 0o111) != executable:
            differs.append(path)
    kept = {f[0] for f in files}
    return differs, [p for p in present(to) if p not in kept]


def write(files: list[File], to: Path) -> None:
    kept = {f[0] for f in files}
    for path in present(to):
        if path not in kept:
            (to / path).unlink()
    for path, data, executable in files:
        full = to / path
        full.parent.mkdir(parents=True, exist_ok=True)
        full.write_bytes(data)
        full.chmod(0o755 if executable else 0o644)


def main() -> None:
    args = sys.argv[1:]
    if "--to" not in args or args.index("--to") + 1 >= len(args):
        die("--to <dir> is required: the xpute repository to export into")
    to = Path(args[args.index("--to") + 1])
    files = tracked() + amalgamation()

    if "--check" in args:
        differs, extra = compare(files, to)
        if not differs and not extra:
            note(f"{to} is what this tree holds ({len(files)} files)")
            return
        for path in differs:
            note(f"  differs  {path}")
        for path in extra:
            note(f"  extra    {path}")
        die(f"{to} is not what this tree holds")

    at = stamp()
    message = draft(to, at)
    write(files, to)
    note(f"{len(files)} files into {to}, from {at}")
    print(message)


if __name__ == "__main__":
    main()
