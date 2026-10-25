---
name: convention
description: How the tree is written — names (singular, short for systemic modules and whole for the domain's, one word a structure), a source file's path header, how code is cited, and the code's fixed rules (rounding half to even, libm and fnv in Rust, offsets through ptr_at, ES modules, @expo/ui only in the sheet). Use when naming, creating, moving or renaming a module, folder, file or type; when a doc or comment names a structure or cites code; and when writing rounding, float math, a map, a pointer from an offset, a script or an @expo/ui import.
---

# Convention

How the tree is written. `AGENTS.md` is how a session works in it, and the
host/guest boundary's words; `ARCHITECTURE.md` is what the system is.

## Names

- A structure is named by one word that maps to one path or component,
  never a loose phrase. Borrow a known term only where the code has its
  defining properties, else use the project's own; an outside term is a
  borrowed structure, said once, never an identity.
- Folders and files are singular, for what each one is; plural only where
  the ecosystem names a collection of independent units (`crates/`, `apps/`,
  `packages/`) or the word has no singular in its sense (`minutes/`).
- Systemic modules take their conventional abbreviation (`sys`, `io`, `mem`,
  `abi`, `sched`); domain modules take the full word (`domain`, `scene`,
  `control`, `query`).

## Files

- A source file opens with its own path as a comment: a crate's name and
  the path under its `src/` (`// pxl-kernel/domain/scene/marker/arrangement.rs`), a
  package's name and the path under its `src/` (`// @pxl/client/host/sys.ts`,
  `// @xpute/host/ipc/doorbell.ts`). A file that is moved takes its new
  path.
- Code is cited as its path and then its name in backticks
  (`domain/scene/marker/arrangement.rs` `tap`). A citation that is moved or renamed is
  fixed with it. `doc/minutes/` and `doc/experiment/` name the files of
  their day and are not kept up.

## Code

- One rounding: half to even, in pixelet and xpute alike. Rust's `.round()`,
  `Math.round`, `toFixed` and a floor of a half added round otherwise; the
  kit's scalar module names the other ways as options a library offers.
- The same bits on every machine: pxl's Rust takes a transcendental from the
  pinned `libm`, never the platform's, writes no `powi`, whose precision is
  unspecified, and keys a map by `fnv`, never std's seeded hasher
  (`crates/clippy.toml`).
- In the crates that address the core memory by offset (xpute's guest, the
  kernel, the renderer, both brackets), an offset becomes a pointer only
  through `xpute_guest::mem::base::ptr_at`, never `as *const` or `as *mut`.
- The tree's scripts are ES modules, TypeScript where a loader takes it:
  no `.cjs` or `.cts`. Node runs only the native toolchain's own scripts.
- `@expo/ui` is imported only by the sheet
  (`apps/client/src/shell/ui/panel.ios.tsx`): a control that takes a touch
  is UIKit's.
