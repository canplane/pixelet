---
name: cargo
description: How a session runs cargo — from rust/, into the session's own target, rust/target/session/$CLAUDE_CODE_SESSION_ID, with the NDK's compiler named for the Android lint. Use before any cargo, or make that compiles Rust (make check).
---

# Cargo

The session's own target, set in the same command:

```bash
export CARGO_TARGET_DIR="$(git rev-parse --show-toplevel)/rust/target/session/$CLAUDE_CODE_SESSION_ID"
```

`cargo lint-android` (in `make check`) needs the NDK's compiler at the app's
minSdk (29), or zstd-sys fails with
`failed to find tool "aarch64-linux-android-clang"`:

```bash
ndk=~/Library/Android/sdk/ndk/<version>/toolchains/llvm/prebuilt/darwin-x86_64/bin
export CC_aarch64_linux_android=$ndk/aarch64-linux-android29-clang \
  AR_aarch64_linux_android=$ndk/llvm-ar
```
