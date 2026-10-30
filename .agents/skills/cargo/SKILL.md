---
name: cargo
description: How a session runs cargo — from rust/, into the session's own target, rust/target/session/<the session's id>, with the NDK's compiler named for the Android lint. Use before any cargo, or make that compiles Rust (make check).
---

# Cargo

The session's own target, `rust/target/session/<id>`, set in the same command
as cargo. `<id>` is the session's, `$CLAUDE_CODE_SESSION_ID` where it is set;
where none is, choose one unique id once (`date +%s`-$RANDOM) and write it
into every later command as it is: a shell variable does not outlive the
call.

```bash
export CARGO_TARGET_DIR="$(git rev-parse --show-toplevel)/rust/target/session/<id>"
```

`cargo lint-android` (in `make check`) needs the NDK's compiler at the app's
minSdk (29), or zstd-sys fails with
`failed to find tool "aarch64-linux-android-clang"`:

```bash
ndk=~/Library/Android/sdk/ndk/<version>/toolchains/llvm/prebuilt/darwin-x86_64/bin
export CC_aarch64_linux_android=$ndk/aarch64-linux-android29-clang \
  AR_aarch64_linux_android=$ndk/llvm-ar
```
