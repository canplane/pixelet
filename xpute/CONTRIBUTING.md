# Contributing to xpute

This is how to build xpute and check a change. How it works is in
[ARCHITECTURE.md](./ARCHITECTURE.md).

xpute builds with Node at the version `.tool-versions` names, with pnpm; rustup,
which installs the toolchain `rust-toolchain.toml` names; a C++20 compiler, as
`c++` or `$CXX`; GNU Make; and the
[WASI SDK](https://github.com/WebAssembly/wasi-sdk) at `/opt/wasi-sdk` or
`$WASI_SDK_PATH`, with which the C++ tests compile the library for wasm32.
The device's crates are linted for Android too, which takes Rust's
`aarch64-linux-android` target, which `rust-toolchain.toml` names, and the
Android NDK, its clang and `llvm-ar` named by
`CC_aarch64_linux_android` and `AR_aarch64_linux_android`. Each command runs
from xpute's root.

```sh
pnpm install                        # the TypeScript packages
cargo spec --write                  # the generated files, written
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo lint-android                  # the device's crates, for Android
cargo test --workspace
pnpm check && pnpm test
make -C cpp -j8 check test          # the C++ compiled, its tests, wasm32
```
