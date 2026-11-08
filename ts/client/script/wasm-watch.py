#!/usr/bin/env python3
# Builds the core's module again (wasm-build.py) on each change under rust/,
# outside any target/. One build runs at a time, and a change during a build
# queues one more. It says when public/core.wasm was replaced. Metro does not
# watch public/, so the page must be reloaded by hand. `make dev` runs this
# beside the dev server, which builds the module once at its start.
#
#   ./script/wasm-watch.py [--interval SECONDS]

import argparse
import os
import subprocess
import sys
import time
from pathlib import Path

APP = Path(__file__).resolve().parent.parent
BUILD = APP / "script/wasm-build.py"
OUT = APP / "public/core.wasm"
RUST = APP.parent.parent / "rust"


def tree():
    """Each file under rust/ outside a target/, by its path, with its time and
    size."""
    seen = {}
    stack = [RUST]
    while stack:
        try:
            entries = os.scandir(stack.pop())
        except OSError:
            continue
        with entries:
            for e in entries:
                try:
                    if e.is_dir(follow_symlinks=False):
                        if e.name != "target":
                            stack.append(e.path)
                    else:
                        st = e.stat(follow_symlinks=False)
                        seen[e.path] = (st.st_mtime_ns, st.st_size)
                except OSError:
                    continue
    return seen


def mtime(path):
    return path.stat().st_mtime_ns if path.is_file() else None


def main():
    parser = argparse.ArgumentParser(description="Rebuild the core's module on each change under rust/.")
    parser.add_argument("--interval", type=float, default=0.5, help="seconds between looks at the tree (0.5)")
    args = parser.parse_args()

    print(f"[wasm-watch] watching {RUST}", flush=True)
    last = tree()
    while True:
        time.sleep(args.interval)
        now = tree()
        if now == last:
            continue
        # The tree this build starts from. A change while the build runs
        # differs from it, and starts one more build after this one.
        last = now
        before = mtime(OUT)
        status = subprocess.run([sys.executable, str(BUILD)], cwd=APP).returncode
        if status != 0:
            print(f"[wasm-watch] build failed ({status}); the page keeps the module it has", file=sys.stderr, flush=True)
        elif mtime(OUT) != before:
            print("[wasm-watch] public/core.wasm rebuilt: reload the page", flush=True)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        sys.exit(130)
