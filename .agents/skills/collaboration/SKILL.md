---
name: collaboration
description: How sessions running side by side share one checkout — on main's tip, no worktrees, no holds, each building into its own target. Use when another session works in the same tree, and before holding a path, waiting for a peer, or building Rust.
---

# Collaboration

Sessions run side by side so the work finishes sooner. Work that waits on
another session finishes no sooner than one session doing it all in turn, so
none of it waits:

- All work in the one checkout, on main's tip: no worktrees, nothing beside
  the repository.
- No holds. A path is never reserved, and another session's announced work is
  no reason to stop yours.
- When two collide, the one whose session id (`$CLAUDE_CODE_SESSION_ID`) sorts
  first resolves it.

Sharing one checkout must not make the sessions queue anywhere else, so each
builds into a target of its own, named by its session id. A shared target would
make cargo's lock on it line every session's build up behind the others, and
hand each the binary another built last. How is the cargo skill's.

The index is shared too, and a `git add` takes over whatever another session
staged in the same file. So a session commits through an index of its own:
`GIT_INDEX_FILE` in its target, read from `HEAD`, holding its own hunks alone.
What is generated from sources, the `.gen.wgsl` above all, is made from an
export of that index into the same target, never from the working tree, which
holds every session's unfinished work. The export is one directory a session
keeps, brought to the index's tree in place, as `git reset --hard` brings a
checkout:

```bash
t="$(git rev-parse --show-toplevel)/rust/target/session/$CLAUDE_CODE_SESSION_ID"
tree=$(GIT_INDEX_FILE="$t/commit.index" git write-tree)
mkdir -p "$t/export"
GIT_INDEX_FILE="$t/export.index" git --work-tree="$t/export" read-tree --reset -u "$tree"
```

A file the tree did not change keeps its time, so cargo there compiles what
changed and no more; an export made anew each time, every file new, compiles
every crate of it again.

Where its hunks stand on another's uncommitted change, that
one lands first.
