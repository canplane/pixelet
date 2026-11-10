#!/usr/bin/env python3
# Builds the core's module (rust/client/core) to public/,
# where the page fetches it as /core.wasm
# (ts/client/src/service/platform/core.ts).
#
#   ./script/wasm-build.py [--published]

import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

APP = Path(__file__).resolve().parent.parent


def repo_of(start):
    """The repository that holds `start`: the nearest directory up that holds
    the TypeScript workspace's manifest, ts/pnpm-workspace.yaml. pxl-tool finds
    the repository the same way, by its own manifest. This works in a tree
    copied without .git."""
    for d in [start, *start.parents]:
        if (d / "ts" / "pnpm-workspace.yaml").is_file():
            return d
    sys.exit(f"no repository over {start}")


REPO = repo_of(APP)
RUST = REPO / "rust"
NUMBER_PATHS = APP / "script/number-paths.py"
TARGET = "wasm32-unknown-unknown"
# Cargo's own rule for its target directory: CARGO_TARGET_DIR, or else the
# workspace's target/.
TARGET_DIR = Path(os.environ["CARGO_TARGET_DIR"]) if os.environ.get("CARGO_TARGET_DIR") else RUST / "target"
OUT = APP / "public/core.wasm"
# The source path that each number in a published module's panics stands for.
# The deploy keeps this file, and it is never bundled.
PATHS = APP / "core.paths.json"

# `pnpm build:web` asks for the published module: no dev features, and
# numbered panic paths.
PUBLISHED = "--published" in sys.argv[1:]
# The published module is built with the release profile, and the dev
# server's with the watch profile, which a change rebuilds only in part
# (rust/Cargo.toml). Each profile builds into its own directory, so building
# one never starts the other over.
PROFILE = "release" if PUBLISHED else "watch"
BUILT = TARGET_DIR / TARGET / PROFILE / "pxl_core.wasm"

# The strips run here rather than in Cargo, so that the target features are
# still declared while wasm-opt runs.
STRIPS = [
    "--strip-debug",
    "--strip-producers",
    "--strip-target-features",
]
# -O3 inlines across the crate graph, which LTO does not, and gives the
# published module its speed; `preview` serves that module. The dev server's
# module only has to show a change working, and -O3 would take most of each of
# its rebuilds.
WASM_OPT = ["-O3", *STRIPS] if PUBLISHED else STRIPS

# embassy and critical-section export their `#[no_mangle]` symbols. Only
# these exports stay.
EXPORTS = ["run"]


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
        proc = subprocess.run(["wasm-opt", *WASM_OPT, str(src), "-o", str(out)])
    except FileNotFoundError:
        raise Failed(1, "[wasm-build] wasm-opt did not run. The module is finished by Binaryen: brew install binaryen (or your platform's package).")
    if proc.returncode != 0:
        raise Failed(proc.returncode or 1)


def finish(*args):
    """Rewrites the module with pxl-tool's wasm, which owns the module's
    binary layout."""
    proc = subprocess.run(["cargo", "run", "-q", "--package", "pxl-tool", "--bin", "wasm", "--", *map(str, args)], cwd=RUST, stdout=subprocess.PIPE, text=True)
    if proc.returncode != 0:
        raise Failed(proc.returncode or 1)
    return proc.stdout.strip()


def number_paths(module, table):
    """The step that numbers a device core's paths the same way."""
    proc = subprocess.run([sys.executable, str(NUMBER_PATHS), str(module), str(table), str(RUST)], stdout=subprocess.PIPE, text=True)
    if proc.returncode != 0:
        raise Failed(proc.returncode)
    return int(proc.stdout)


def mtime(path):
    return path.stat().st_mtime_ns if path.is_file() else None


def main():
    before = mtime(BUILT)
    try:
        status = subprocess.run(
            # The core declares several crate types. This asks cargo for the
            # module's alone, by name.
            ["cargo", "rustc", "--crate-type", "cdylib", "--profile", PROFILE, "--package", "pxl-core", "--target", TARGET, *(["--no-default-features"] if PUBLISHED else [])],
            cwd=RUST,
            env={**os.environ, "RUSTFLAGS": remapped()},
        )
    except FileNotFoundError as err:
        print(f"[wasm-build] cargo did not run ({err}). The core needs a Rust toolchain with the {TARGET} target: rustup target add {TARGET}", file=sys.stderr)
        return 1
    if status.returncode != 0:
        return status.returncode

    # An unchanged module is not finished again, so that its time says when it
    # last changed (wasm-watch.py). The finished module carries the time of the
    # build it was finished from, so it is this build's only when the two times
    # are equal. The dev server's build and the published one are separate
    # files, and either may have been finished last.
    if before is not None and mtime(BUILT) == before and mtime(OUT) == before:
        print("[wasm-build] up to date -> public/core.wasm")
        return 0

    # Each run works in its own directory under cargo's target, removed whole
    # at the end. A failed run leaves nothing beside the module, a killed one
    # leaves its directory where `cargo clean` removes it, and a dev server's
    # build and a check's never share a file. The directory is on the module's
    # volume, so its files can be renamed into place.
    work = Path(tempfile.mkdtemp(dir=TARGET_DIR, prefix="wasm-build-"))
    try:
        linked_file, opt_file = work / "linked.wasm", work / "done.wasm"
        shutil.copyfile(BUILT, linked_file)
        linked = linked_file.stat().st_size
        finish("exports", linked_file, *EXPORTS)
        optimized(linked_file, opt_file)
        numbered = work / "paths.json"
        paths = number_paths(opt_file, numbered) if PUBLISHED else 0
        # The module is moved into place last, because a reader keys on it.
        if PUBLISHED:
            os.replace(numbered, PATHS)
        built = mtime(BUILT)
        os.utime(opt_file, ns=(built, built))
        os.replace(opt_file, OUT)
        note = (", without the dev-only settings" if PUBLISHED else "") + (f", and {paths} source paths numbered" if paths > 0 else "")
        print(f"[wasm-build] {linked} -> {OUT.stat().st_size} bytes, stripped{note} -> public/core.wasm")
        return 0
    except Failed as err:
        if str(err):
            print(err, file=sys.stderr)
        return err.code
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
