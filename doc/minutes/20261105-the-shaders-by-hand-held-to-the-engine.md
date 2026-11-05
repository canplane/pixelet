# The shaders by hand, held to the engine, and one history

**2026-11-05** — recorded with the user, in the session that took rust-gpu out
on `small` and merged it (eff52c1d), while another session worked xpute on
`main` ([its minutes](./20261105-xpute-not-yet-for-other-programs.md)). Why
rust-gpu went is in [rust-gpu goes](./20261103-rust-gpu-goes.md); the work, as
it happened, is in
[the journal](../experiment/20261104-taking-rust-gpu-out/journal.md).

## What the user decided

**pxl-engine is the oracle; the WGSL is held to it word for word.** Every
compute pass is hand WGSL, assembled from parts (record.rs, part.rs) and
validated by naga in build.rs; a reference test runs it on the GPU against the
Rust and compares every word. Where the two disagree, the rule is settled on
both sides at once — symmetry: what one side does, the other does the same way.

- **A walk ends at `cast::STEPS_MOST` on both sides** (pxl-engine
  `walk_at_most`; beam_walk.wgsl, ask.wgsl, walk.wgsl), the engine's end the
  WGSL's: gone to its reach, `t_max`, its line kept. A test lowers the bound on
  both (tests/margin.rs, `set_const` in tests/gpu/mod.rs).
- **Code that holds nothing comes out of both**, not tested around:
  `Grid::sample_before`'s clamp, decor_pose's guard. A walk out of steps is
  "left to the walk that asks" (`ends` 4), as one that meets a node no slot
  keeps.
- **One definition for a layout and what binds to it.** A render pipeline's
  groups are checked both ways against what its shader reads, at build time
  (record.rs `lays_out`), as compute groups are (build.rs `BINDS`); a bind
  group is made off its layout's own group and stops on a mismatch (gpu/api.rs
  `Layout`, `Gpu::bind_group`). An entry no shader reads is removed.

**Rounding.** IEEE half to even in every language, as WGSL rounds `+ − ×`
(xpute ARCHITECTURE.md, after the golden records). WGSL's division may be 2.5
ulps off (Metal measured 13), square roots have bounds only, a product may fuse
into a sum, and Metal's default fast math may reorder a sum: no order written
in the source holds, and the compiler is the browser's or the driver's.

- **A rule the CPU and a GPU both decide decides in integers, or in floats the
  CPU rounded once and handed the GPU in a word**: a division is a product by
  a handed reciprocal, a square root the root, handed. What it still sums in
  floats takes `+ − ×`, and its test names the ulps it allows and counts what
  took them. (Example: the zoom read off `r2 * INV_SPLIT2`.)
- **A rule a GPU alone decides goes through one function**, and its test names
  its ulps (the eye's per-pixel ray, walk_node.wgsl `eye_aim`). Among them,
  each for its own end:
  - the light ray's start, in fixed point summed in i32, so that every
    pipeline that reads it (light_beam, light, light_miss, light_miss_hit,
    light_sample_box, sample_light) starts the ray alike, each compiling the
    one function with its own fusion;
  - the light's words `W_WIDTH`…`W_WIDTH_INVERSE` and the grid's
    `GRID_S_INVERSE`, `GRID_SLOPE`, reciprocals and roots the host rounds once
    and hands over: fewer divisions on the GPU, so cheaper.
- Why, what it costs and how a rule is written:
  [when the CPU and a GPU must give one answer](../tip/cpu-and-gpu-agree.md).
  Open with the user: the ground's top and the ways' road tops still round a
  float sum (13 and 96 cases allowed); made exact once their bits are counted.

**wgpu is followed as released, never forked.** It is not patched unless it has
a bug that bites; a real bug's fix is kept apart, documented, until a release
carries it ([naga on Metal](../tip/naga-on-metal.md)).

**Names.** pxl-engine `tally` → `count`, `TOOK` → `REUSED`, perf's
`eye_took`/`light_took` → `eye_reused`/`light_reused`; `light_take` →
`light_item`, `sample_taken` → `sample_item`, light.wgsl `took` →
`visible_entry`; the list's mark `list_raise` → `list_swap` (an exchange: a
place is listed where its mark held other than the cast's number). An
abbreviation such as `INV_SPLIT2` stays.

**History and records.**

- Work is integrated in a worktree of its own, not on a main checkout others
  edit, and merged when the user says.
- Commits are split by genre, not squashed into one: the passes by hand, the
  removal, the tests, the shared rules, the layouts, the names, ts, docs. A
  rewrite keeps the tree equal at each step's end and puts the originals on
  `original` (the deploy skill).
- The sessions' journals are merged into the history together, rewritten to
  name the rewritten commits; `doc/backlog.md` is never committed.
- **The backlog holds work, never verification.** Whether something holds, a
  run on a device, a path no test reaches: the agent that owes it carries it
  and does it (below, "Owed").

## What was done

On main, the genre commits from `7341cc537c` to `b3ce4024e4`, the records in
`f80c1996a9`; before the branch point the nineteen commits rewritten by genre
(`681bf362a0` … `3dbe469161`). At the tip: CPU 315 tests, pxl-render 62 on the
GPU, vitest 28, xpute 114, the page with no console error. The originals are on
`original` behind `chore: original/20261105-by-hand`.

## Open

In progress after the merge, in worktrees off main: the ways' divisions to the
host's reciprocals (way.rs, way.wgsl); render-pass shaders without nested
structs, `spacing_width` placed by who decides it, list offsets from pxl-engine
instead of arithmetic; the two test device harnesses as one; check_rules'
findings (`machine`, `unpack`, xpute's words); a reading of decor's
`fill_eighths` and the world query's surviving mutants.

**Owed** (verification, carried by whoever runs next):

- **The S23, once plugged in:** first set
  `adb shell settings put global stay_on_while_plugged_in 0` (left at 2).
  Then:
  - every setting runs;
  - its shadows compared with the Mac's pixel for pixel;
  - where the ~90 s first load goes;
  - `level_at`'s switch (build.rs), its cost;
  - `beam_walk`'s live words;
  - the native app (wgpu on Vulkan) against the page.
- **The iPhone itself:** Safari on the Mac stood in.
- **An Android driver:** stencil and early depth
  (`Device.depth32float_stencil8`).
- **The page through a resize and every pass:** bind groups made then are
  checked only as they are made.
- **The character preview screen.**
- **Performance and the wasm's size** against before the translation.
- **The commits before the branch point** do not build one by one (pxl-wasm
  against xpute's guest macro), as the originals did not; the tip does.
