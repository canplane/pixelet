#!/bin/sh
# Runs rustc for every build of the workspace (config.toml's
# `build.rustc-wrapper`), through sccache where the machine has it, so each
# session's target and each worktree compile a dependency once between them.
# sccache keys a compile by every CARGO_* variable, and the target's is not
# one rustc reads: cargo has already passed it as --out-dir. Left in, it would
# give each session's target a cache of its own. Without sccache, as in the
# server images and CI, this is rustc alone.
unset CARGO_TARGET_DIR CARGO_BUILD_TARGET_DIR
# macOS drops DYLD_* on the way into this shell, and rustup's names the
# toolchain's lib/, where rust-lld finds libLLVM: given back from the
# compiler's own path, <toolchain>/bin/<rustc or clippy-driver>. Set, the
# variable replaces dyld's own fallback, so that is kept after it.
toolchain=${1%/*}
toolchain=${toolchain%/bin}
if [ -d "$toolchain/lib" ]; then
  DYLD_FALLBACK_LIBRARY_PATH="$toolchain/lib:${DYLD_FALLBACK_LIBRARY_PATH:-$HOME/lib:/usr/local/lib:/usr/lib}"
  export DYLD_FALLBACK_LIBRARY_PATH
fi
if command -v sccache >/dev/null 2>&1; then
  # A server that cannot start or answer compiles here instead of failing it.
  SCCACHE_IGNORE_SERVER_IO_ERROR=1 exec sccache "$@"
fi
exec "$@"
