#!/bin/sh
# Runs its arguments with the local databases' addresses (infra/local/.env)
# and the servers' own numbers (rust/server/common/.env) set, where each exists,
# and the newest installed NDK's compiler for an Android build unless one is set.
# The files are the checkout's, found from this script, so it runs from anywhere:
# the root's make runs cargo under it from rust/.
root=$(cd "$(dirname "$0")/.." && pwd)
for f in infra/local/.env rust/server/common/.env; do
  [ -f "$root/$f" ] && set -a && . "$root/$f" && set +a
done
for bin in "$HOME"/Library/Android/sdk/ndk/*/toolchains/llvm/prebuilt/darwin-x86_64/bin; do
  [ -d "$bin" ] && ndk=$bin
done
if [ -n "${ndk:-}" ]; then
  export CC_aarch64_linux_android="${CC_aarch64_linux_android:-$ndk/aarch64-linux-android29-clang}"
  export AR_aarch64_linux_android="${AR_aarch64_linux_android:-$ndk/llvm-ar}"
fi
exec "$@"
