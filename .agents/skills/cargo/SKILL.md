---
name: cargo
description: How a session runs cargo — from rust/, into the session's own target, rust/target/session/$CLAUDE_CODE_SESSION_ID. Use before any cargo, or make that compiles Rust.
---

# Cargo

cargo runs in `rust/`, where `rust/rust-toolchain.toml` picks the toolchain.
It builds into the session's own target, set in the same command:

```bash
export CARGO_TARGET_DIR="$(git rev-parse --show-toplevel)/rust/target/session/$CLAUDE_CODE_SESSION_ID"
```
