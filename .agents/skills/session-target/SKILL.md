---
name: session-target
description: Build Rust into this session's own target, rust/target/session/$CLAUDE_CODE_SESSION_ID, so sessions sharing one checkout never hand each other a binary. Use before any cargo or make that compiles Rust.
---

# Session target

Several sessions build in one checkout, and a target shared between them
hands each the binary built last, from either. So each builds into its own,
named by the session id its environment carries:

```bash
export CARGO_TARGET_DIR="$(git rev-parse --show-toplevel)/rust/target/session/$CLAUDE_CODE_SESSION_ID"
```

Set it in the same command as the cargo or make it is for: a shell's
variables do not outlive the command.
