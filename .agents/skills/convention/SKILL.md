---
name: convention
description: How the tree is written — names (singular, short for systemic modules and whole for the domain's, one word a structure), the layout (a root a language, src/ and script/ in a package, no counted ../ out of one, cargo from rust/), a source file's path header, how code is cited, and the code's fixed rules (rounding half to even, libm and fnv in Rust, offsets through ptr_at, ES modules, @expo/ui only in the sheet). Use when naming, creating, moving or renaming a module, folder, file or type; when a file names a path out of its package or runs cargo; when a doc or comment names a structure or cites code; and when writing rounding, float math, a map, a pointer from an offset, a script or an @expo/ui import.
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

## Layout

- The tree is grouped by language, each a root of its own tool: `rust/`
  Cargo's, `ts/` Deno's workspace, `script/` the repository's own scripts,
  `xpute/` Make's. The root only routes (`deno.jsonc` tasks). Within a
  package, `src/` is its code and `script/` its own tools; a test sits
  beside what it tests.
- A path out of a package is never a counted `../`. Code names another
  package through the workspace's map (`@pxl/client/`, `@xpute/kit/`). A
  config no import map reaches takes the workspace or the repository from
  `ts/client/script/import-map.ts` (`WORKSPACE`, `REPO`), which finds them
  from the workspace's manifest. A config that must read another package's
  output has its build step put it inside first
  (`ts/app/native/script/build-native.ts`, `ts/server/edge/script/stage.ts`).
- cargo runs from `rust/`, where rustup reads `rust-toolchain.toml`: the
  root's tasks as `cd rust && cargo <alias>` (`rust/.cargo/config.toml`).

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
  (`rust/clippy.toml`).
- In the crates that address the core memory by offset (xpute's guest and
  host, the kernel, the renderer, both brackets — `script/check-rules.ts`
  `OFFSET_CRATES`), an offset becomes a pointer only
  through `xpute_guest::mem::base::ptr_at`, never `as *const` or `as *mut`.
- The tree's scripts are ES modules, TypeScript where a loader takes it:
  no `.cjs` or `.cts`. Node runs only the native toolchain's own scripts.
- `@expo/ui` is imported only by the sheet
  (`ts/client/src/shell/ui/panel.ios.tsx`): a control that takes a touch
  is UIKit's.
