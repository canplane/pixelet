---
name: convention
description: How the tree is written — names, where a thing goes, a file's header and citations, and the few rules the code holds everywhere. Use when naming, creating, moving or renaming a module, folder, file or type; when a file names a path out of its package; when a doc or comment cites code; and when writing rounding, float math, a map, a pointer from an offset, a script or an @expo/ui import.
---

# Convention

How the tree is written. What its parts are and where their boundaries run
is `ARCHITECTURE.md`'s, and the host/guest boundary's roles `AGENTS.md`'s.

## Names

- A structure is named by one word that maps to one path or component,
  never a loose phrase. A borrowed term names only what has its defining
  properties; else the project's own word.
- Folders and files are singular, for what each one is; plural only where
  the ecosystem names a collection of independent units (`skills/`,
  Expo's `modules/`) or the word has no singular in its sense (`minutes/`).
- Systemic modules take their conventional abbreviation (`sys`, `io`, `mem`,
  `abi`, `sched`); domain modules take the full word (`domain`, `scene`,
  `control`, `query`).

## Layout

- The tree is grouped by language, each a root of its own tool: `rust/`
  Cargo's, `ts/` Deno's workspace, `script/` the repository's own scripts,
  `xpute/` Make's. The root's `Makefile` only routes. Within a package,
  `src/` is its code and `script/` its own tools; a test sits beside what
  it tests.
- A path out of a package is never a counted `../`. Code names another
  package through the workspace's map (`@pxl/client/`, `@xpute/kit/`). A
  config no import map reaches takes the workspace or the repository from
  `ts/build/import-map.ts` (`WORKSPACE`, `REPO`), which finds them
  from the workspace's manifest. A config that must read another package's
  output has its build step put it inside first
  (`ts/app/native/script/build-native.py`, `ts/server/edge/script/stage.sh`).

## Files

- A source file opens with its own path as a comment: a crate's name and
  the path under its `src/` (`// pxl-kernel/domain/scene/marker/arrangement.rs`), a
  package's name and the path under its `src/` (`// @pxl/client/host/mem.ts`,
  `// @xpute/host/ipc/doorbell.ts`). A file that is moved takes its new
  path.
- Code is cited as its path and then its name in backticks
  (`domain/scene/marker/arrangement.rs` `tap`). A citation that is moved or
  renamed is fixed with it, in the live minutes too, where a claim the code
  no longer holds gets a note under it. `doc/minutes/archive/` and
  `doc/experiment/` name the files of their day and are not kept up.
- The tree's scripts are ES modules, TypeScript where a loader takes it:
  no `.cjs` or `.cts`. Node runs only the native toolchain's own scripts.

## Code

The same bits on every machine, in pixelet and xpute alike:

- One rounding, half to even. Rust's `.round()`, `Math.round`, `toFixed`
  and a floor of a half added round otherwise; the kit's scalar module
  names the other ways as options a library offers.
- pxl's Rust takes a transcendental from the pinned `libm`, never the
  platform's, writes no `powi`, whose precision is unspecified, and keys a
  map by `fnv`, never std's seeded hasher (`rust/clippy.toml`).

The core memory is addressed by offset:

- In the crates that do (xpute's guest and host, the kernel, the renderer,
  both brackets — pxl-tool `bin/check_rules.rs` `OFFSET_CRATES`), an offset
  becomes a pointer only through `xpute_guest::mem::base::ptr_at`, never
  `as *const` or `as *mut`.

One dependency has one place:

- `@expo/ui` is imported only by the sheet
  (`ts/client/src/shell/ui/panel.ios.tsx`): a control that takes a touch
  is UIKit's.
