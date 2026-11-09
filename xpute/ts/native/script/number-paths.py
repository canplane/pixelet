#!/usr/bin/env python3
# Numbers the source paths a published core holds, in the built file.
#
#   number-paths.py <core> <paths.json> <cargo workspace>
#
# Rust embeds a caller's path for a `#[track_caller]` panic, with no stable way
# out. Each is overwritten in place, same length, as `...#n`: a `&'static str`
# keeps its length apart, so the core runs the same. paths.json says what each
# number stands for and is never shipped. It prints how many were numbered.
#
# A `.rs` under no known source root stops the build, since it could be a format
# string. So does the home directory left anywhere in the file.
#
# Xcode names no Python of its own, so this keeps to what macOS's 3.9 reads.

import json
import os
import re
import subprocess
import sys
from pathlib import Path

PATH = re.compile(rb"[A-Za-z0-9_/.+-]*\.rs\b")
# `src/` is a crate's own path, as the standard library's line tables hold it in
# a static library.
SOURCE_ROOT = re.compile(rb"(?:^|[^A-Za-z0-9_])xpute/|/library/|/registry/|^/rustc/|^/rust/deps/|\.rustup/|^src/")


def members(workspace):
    """Cargo hands rustc a member's paths relative to the workspace, so no
    prefix remap reaches them."""
    out = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=workspace,
        check=True,
        stdout=subprocess.PIPE,
    ).stdout
    meta = json.loads(out)
    root = Path(meta["workspace_root"])
    found = []
    for package in meta["packages"]:
        at = Path(package["manifest_path"]).parent
        if root in at.parents:
            found.append(f"{at.relative_to(root)}/src/".encode())
    return tuple(found)


def main():
    core, table, workspace = sys.argv[1:]
    bytes_of = bytearray(Path(core).read_bytes())
    roots = members(workspace)
    numbers = []
    for match in PATH.finditer(bytes(bytes_of)):
        text = match.group()
        if b"/" not in text:
            continue
        if not SOURCE_ROOT.search(text) and not text.startswith(roots):
            sys.exit(f"[number-paths] {text.decode()} ends in .rs but is under no source root")
        if text not in numbers:
            numbers.append(text)
        # The number goes last: the linker merges a string that is another's tail.
        tag = f"#{numbers.index(text)}".encode()
        start, end = match.span()
        at = max(start, end - len(tag))
        bytes_of[start:at] = b"." * (at - start)
        bytes_of[at:end] = tag[len(tag) - (end - at) :]
    home = os.environ["HOME"].encode()
    if home in bytes_of:
        sys.exit(f"[number-paths] {core} still names the home directory")
    if numbers:
        Path(core).write_bytes(bytes_of)
    rows = {f"#{k}": path.decode() for k, path in enumerate(numbers)}
    Path(table).write_text(json.dumps(rows, indent=2) + "\n")
    print(len(numbers))


if __name__ == "__main__":
    main()
