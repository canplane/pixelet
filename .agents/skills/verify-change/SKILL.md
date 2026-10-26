---
name: verify-change
description: What a change runs while working, Rust or TypeScript — only what it reaches, once it is ready to run; lint, format and the whole check are the deploy's, once. Use before running any build, check or test while working.
---

# Verifying a change

A change runs what it reaches, and only when there is something to learn from
it. How cargo runs is the cargo skill's ([cargo](../cargo/SKILL.md)).

- **Docs, comments, skills:** nothing.
- **A crate's code:** `cargo check` of that crate, for the target it runs on
  (`--target wasm32-unknown-unknown` for the client's). Not the crates beside
  it, not the other targets.
- **pxl-spec-core code a shader uses, or pxl-shader:** `cargo shader-wgsl`,
  then Tint alone (`deno test -A src/shader.test.ts` in `ts/app/web`): a
  shader Tint refuses fails the frame without a word. A comment alone in
  pxl-spec-core needs neither: the shaders' digest goes stale, the deploy's
  check says so, and one `cargo shader-wgsl` then mends it.
- **`spec/` JSON:** `cargo spec --write`.
- **A TypeScript module:** `deno check` of that file, from `ts/`; what it
  imports comes with it. Another member's check (`app/web`, `app/native`,
  `server/edge`) only when that member's own files changed.
- **The kernel or the client, seen working:** the dev server already rebuilds
  the guest and the page on a change; one look in the browser.
- **Logic a test covers:** that test file alone (`cargo test -p <crate>
  <module>`, `deno test -A <file>.test.ts`), once the change is ready to run,
  not after every edit.

No lint, no format, no `make check` or `make test`, no `deno task check` or
`test`, no `check-diff.py` by reflex: they hold the code's look and the whole
tree, which is the deploy's one check before it sends
([deploy](../deploy/SKILL.md)), not a change's.
