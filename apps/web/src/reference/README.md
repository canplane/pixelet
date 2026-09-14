# reference

The TypeScript the Rust core is held to. Nothing here is run by the app:
every module in this directory is a reference implementation the tests
compare the module's output against — the bake byte for byte
(`wasm_bake.test.ts`, `core_build.test.ts`), the scan (`wasm_scan.test.ts`),
the terrain paint cell for cell (`terrain.test.ts`), decor instance for
instance (`core_decor.test.ts`), the raycast box for box
(`core_raycast.test.ts`), the scheduler's range and order
(`core_sched.test.ts`) — and the account, in their comments, of why each
rule is what it is. `boundary.test.ts` holds the line: no live module
imports from here.

When a reference has been held to for long enough that the crate's own
tests carry the rule, it can go; until then a change to a rule is made in
the crate and the reference together, and the test says whether they still
agree.
