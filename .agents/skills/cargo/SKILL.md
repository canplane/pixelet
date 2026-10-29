---
name: cargo
description: How a session runs cargo — from rust/, where rustup reads the workspace's toolchain, into the session's own target, rust/target/session/$CLAUDE_CODE_SESSION_ID. Use before any cargo, or make that compiles Rust.
---

# Cargo

rustup picks the toolchain by the directory cargo runs in, and
`rust/rust-toolchain.toml` is read only under `rust/`. So cargo runs there, as
the Makefile's targets do (`cd rust && cargo <alias>`, the aliases in
`rust/.cargo/config.toml`); a cargo run elsewhere builds with whatever toolchain
is the machine's default.

It builds into the session's own target (why is the collaboration skill's):

```bash
export CARGO_TARGET_DIR="$(git rev-parse --show-toplevel)/rust/target/session/$CLAUDE_CODE_SESSION_ID"
```

Set it in the same command as the cargo or make it is for: a shell's variables
do not outlive the command.

What a change runs while working is the verify-change skill's
([verify-change](../verify-change/SKILL.md)).
