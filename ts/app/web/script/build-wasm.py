#!/usr/bin/env python3
# Builds the guest's module (rust/client-bracket/wasm) to where
# src/guest.ts loads it.
#
#   ./script/build-wasm.py [--published]

import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

WEB = Path(__file__).resolve().parent.parent


def repo_of(start):
    """The repository over `start`: the nearest directory up holding the
    TypeScript workspace's manifest, ts/pnpm-workspace.yaml, as pxl-tool finds it by
    its own manifest. It works in a tree copied without .git."""
    for d in [start, *start.parents]:
        if (d / "ts" / "pnpm-workspace.yaml").is_file():
            return d
    sys.exit(f"no repository over {start}")


REPO = repo_of(WEB)
RUST = REPO / "rust"
TARGET = "wasm32-unknown-unknown"
# Cargo's own rule: CARGO_TARGET_DIR, else the workspace's target/.
TARGET_DIR = Path(os.environ["CARGO_TARGET_DIR"]) if os.environ.get("CARGO_TARGET_DIR") else RUST / "target"
OUT = WEB / "src/guest.wasm"
# The source path each number in a published module's panics stands for; kept by the deploy, never bundled.
PATHS = WEB / "src/guest.paths.json"

# What `pnpm build` bundles: no dev features, numbered panic paths.
PUBLISHED = "--published" in sys.argv[1:]
# The published module is release's bytes; the dev server's, watch's, which a
# change rebuilds in part (rust/Cargo.toml). Each in its own directory, so
# building one never starts the other over.
PROFILE = "release" if PUBLISHED else "watch"
BUILT = TARGET_DIR / TARGET / PROFILE / "pxl_wasm.wasm"

# Strips run here, not via Cargo, so target features are still declared while wasm-opt runs.
STRIPS = [
    "--strip-debug",
    "--strip-producers",
    "--strip-target-features",
]
# Inlines across the crate graph, which LTO does not: the published module's
# speed, which `preview` serves. The dev server's module only has to show a
# change working, and -O3 would be most of each of its rebuilds.
WASM_OPT = ["-O3", *STRIPS] if PUBLISHED else STRIPS

# embassy and critical-section export their `#[no_mangle]` symbols; only these stay.
EXPORTS = ["intr"]


class Failed(Exception):
    """A step that failed, and the code the build exits with."""

    def __init__(self, code, message=""):
        super().__init__(message)
        self.code = code


def remapped():
    """Panic locations survive `strip` and carry the home directory, so the
    prefixes are remapped. The repo goes first since it lies under home."""
    prefixes = [p for p in (str(REPO), os.environ.get("HOME")) if p]
    flags = [f"--remap-path-prefix={p}=" for p in prefixes]
    return " ".join(f for f in [os.environ.get("RUSTFLAGS"), *flags] if f)


def optimized(src, out):
    try:
        done = subprocess.run(["wasm-opt", *WASM_OPT, str(src), "-o", str(out)])
    except FileNotFoundError:
        raise Failed(1, "[build-wasm] wasm-opt did not run. The module is finished by Binaryen: brew install binaryen (or your platform's package).")
    if done.returncode != 0:
        raise Failed(done.returncode or 1)


def finish(*args):
    """The module is rewritten by pxl-tool's finish_wasm, which owns the binary."""
    done = subprocess.run(["cargo", "run", "-q", "--package", "pxl-tool", "--bin", "finish_wasm", "--", *map(str, args)], cwd=RUST, stdout=subprocess.PIPE, text=True)
    if done.returncode != 0:
        raise Failed(done.returncode or 1)
    return done.stdout.strip()


def mtime(path):
    return path.stat().st_mtime_ns if path.is_file() else None


def main():
    before = mtime(BUILT)
    try:
        status = subprocess.run(
            ["cargo", "build", "--profile", PROFILE, "--package", "pxl-wasm", "--target", TARGET, *(["--no-default-features"] if PUBLISHED else [])],
            cwd=RUST,
            env={**os.environ, "RUSTFLAGS": remapped()},
        )
    except FileNotFoundError as err:
        print(f"[build-wasm] cargo did not run ({err}). The guest needs a Rust toolchain with the {TARGET} target: rustup target add {TARGET}", file=sys.stderr)
        return 1
    if status.returncode != 0:
        return status.returncode

    # Finishing an unchanged module again would reload every open dev page.
    # The module carries the time of what it was finished from, so it is this
    # build's only when the two are equal: the dev server's and the published
    # are each their own file, and either may have been finished last.
    if before is not None and mtime(BUILT) == before and mtime(OUT) == before:
        print("[build-wasm] up to date -> src/guest.wasm")
        return 0

    # One directory a run under cargo's target, removed whole: a failed run
    # leaves nothing beside the module, a killed one leaves it where cargo
    # clean takes it, and a dev server's build and a check's never share a
    # file. On the module's volume, so what it holds is renamed into place.
    work = Path(tempfile.mkdtemp(dir=TARGET_DIR, prefix="pxl-build-wasm-"))
    try:
        linked_file, done = work / "linked.wasm", work / "done.wasm"
        shutil.copyfile(BUILT, linked_file)
        linked = linked_file.stat().st_size
        finish("exports", linked_file, *EXPORTS)
        optimized(linked_file, done)
        numbered = work / "paths.json"
        paths = int(finish("number", done, numbered)) if PUBLISHED else 0
        # The module last: it is what a reader keys on.
        if PUBLISHED:
            os.replace(numbered, PATHS)
        built = mtime(BUILT)
        os.utime(done, ns=(built, built))
        os.replace(done, OUT)
        said = (", without the dev-only settings" if PUBLISHED else "") + (f", and {paths} source paths numbered" if paths > 0 else "")
        print(f"[build-wasm] {linked} -> {OUT.stat().st_size} bytes, stripped{said} -> src/guest.wasm")
        return 0
    except Failed as err:
        if str(err):
            print(err, file=sys.stderr)
        return err.code
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
