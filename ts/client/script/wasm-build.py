#!/usr/bin/env python3
# Builds the core's module (rust/client/core) to public/,
# where the page fetches it as /core.wasm
# (ts/client/src/runtime/boot.web.ts).
#
#   ./script/wasm-build.py [--published | --check]
#
# `--check` builds the published module beside cargo's build of it, where the
# dev server does not read it, and puts nothing in public/.

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

APP = Path(__file__).resolve().parent.parent
# The guest, as the device's build reads it too (core-build.py): its cargo
# workspace, its crate, and cargo's arguments for a debug and a release build.
GUEST = json.loads((APP / "package.json").read_text()).get("xpute", {}).get("guest") or sys.exit(f"[wasm-build] {APP}/package.json names no xpute.guest")
RUST = (APP / GUEST["workspace"]).resolve()
PACKAGE = GUEST["package"]
NUMBER_PATHS = APP / "script/number-paths.py"
STRINGS = APP / "script/strings.py"
TARGET = "wasm32-unknown-unknown"
# Cargo's own rule for its target directory: CARGO_TARGET_DIR, or else the
# workspace's target/.
TARGET_DIR = Path(os.environ["CARGO_TARGET_DIR"]) if os.environ.get("CARGO_TARGET_DIR") else RUST / "target"

# `pnpm build:web` asks for the published module: a release build, with
# numbered panic paths.
CHECK = "--check" in sys.argv[1:]
PUBLISHED = CHECK or "--published" in sys.argv[1:]
# The published module is built with the release profile, and the dev
# server's with the watch profile, which a change rebuilds only in part
# (rust/Cargo.toml). Each profile builds into its own directory, so building
# one never starts the other over.
PROFILE = "release" if PUBLISHED else "watch"
EXTRA = GUEST.get("args", {}).get("release" if PUBLISHED else "debug", [])
BUILT = TARGET_DIR / TARGET / PROFILE / (PACKAGE.replace("-", "_") + ".wasm")
OUT = BUILT.with_name("core.wasm") if CHECK else APP / "public/core.wasm"
# The source path that each number in a published module's panics stands for.
# The deploy keeps this file, and it is never bundled.
PATHS = BUILT.with_name("core.paths.json") if CHECK else APP / "core.paths.json"
# What finishes and checks a published module.
SCRIPTS = [Path(__file__).resolve(), NUMBER_PATHS, STRINGS]

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
# these exports stay: the module's memory and the link's three exports
# (@xpute/link link.ts `Guest`).
EXPORTS = ["memory", "link_init", "link_boot", "link_in"]


class Failed(Exception):
    """A step that failed, and the code the build exits with."""

    def __init__(self, code, message=""):
        super().__init__(message)
        self.code = code


def remapped():
    """Panic locations survive `strip` and carry the home directory, so the
    prefixes are remapped (number-paths.py `prefixes`)."""
    flags = subprocess.run([sys.executable, str(NUMBER_PATHS), "--remaps", str(RUST)], check=True, stdout=subprocess.PIPE, text=True).stdout.strip()
    return " ".join(f for f in [os.environ.get("RUSTFLAGS"), flags] if f)


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


def strings(module):
    """The step that refuses a string of our sources the published module may
    not hold."""
    proc = subprocess.run([sys.executable, str(STRINGS), str(module), str(BUILT), TARGET, str(RUST)])
    if proc.returncode != 0:
        raise Failed(proc.returncode)


def mtime(path):
    return path.stat().st_mtime_ns if path.is_file() else None


def main():
    before = mtime(BUILT)
    try:
        status = subprocess.run(
            # The core declares several crate types. This asks cargo for the
            # module's alone, by name.
            ["cargo", "rustc", "--crate-type", "cdylib", "--profile", PROFILE, "--package", PACKAGE, "--target", TARGET, *EXTRA],
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
    # A published module is also finished again where a script that finishes
    # or checks it changed after it was last finished, when its paths' table
    # was written.
    checked = not PUBLISHED or max(map(mtime, SCRIPTS)) < (mtime(PATHS) or 0)
    if before is not None and mtime(BUILT) == before and mtime(OUT) == before and checked:
        print(f"[wasm-build] up to date -> {OUT}")
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
            strings(opt_file)
            os.replace(numbered, PATHS)
        built = mtime(BUILT)
        os.utime(opt_file, ns=(built, built))
        os.replace(opt_file, OUT)
        note = "".join(f", {a}" for a in EXTRA) + (f", and {paths} source paths numbered" if paths > 0 else "")
        print(f"[wasm-build] {linked} -> {OUT.stat().st_size} bytes, stripped{note} -> {OUT}")
        return 0
    except Failed as err:
        if str(err):
            print(err, file=sys.stderr)
        return err.code
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
