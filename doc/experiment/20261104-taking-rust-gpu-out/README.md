# Taking rust-gpu out

**2026-11-04.** What taking rust-gpu out has shown so far, on a Galaxy S23 at the
commits named: a query by hand costs what the generated passes cost, so a pass has
to answer from the observed pyramid and ask the world only for what is left; and
what is left is often a choice made in the pyramid, not the shader's size. Each S23
figure is one load, and a pass moves up to twofold between loads, so only large
differences here say anything. Why rust-gpu goes is in
[rust-gpu goes](../../minutes/20261103-rust-gpu-goes.md). The phone's own costs are in
[the walk on a phone](../20261103-the-walk-on-a-phone/README.md).

Closed: rust-gpu went whole on 2026-11-05, every pass WGSL by hand, each held to
pxl-engine ([the minutes](../../minutes/20261105-the-shaders-by-hand-held-to-the-engine.md)).
How the round went, pass by pass, is [the journal](./journal.md); `decor/` holds the
decor passes' plan, rulings and records, `supervise/` the sweeps of `world.wgsl` and
their tools. `f9f66682e` and `e8d616ba4` are on main in `47d238aab4`, `e8d616ba4`'s
tree.

## The query on the phone

`world.wgsl` is `world::node` by hand, f9f66682e, read-only, no atomics, no arrays.
The S23 makes a pipeline from it, 573 ms called once a thread and 766 ms called 14
times along a loop with about 28 words live across each turn, so it stands under
the refusal.

| Along a loop of 14 | Time a visit |
|---|---|
| The query, tables all zero | 108.6 ns |
| The query, tables pseudo-random | 104.9 ns |
| A trivial body | 0.09 ns |
| `sample_hit`, by hand, over the pyramid (earlier run) | 2.4 ns |
| `sample`, `beam`, generated (earlier run) | 111–138 ns |

- The tables are made up, and zero and random agree, so this does not show the worst
  path was taken, or split the query's own work from a cost of registers. It is a
  shape.
- The trivial body is a floor, not a walk's own cost: the driver likely reduces it to
  almost nothing.
- **So asking the world on every visit costs what the generated passes cost.** A pass
  walks the pyramid by hand and asks only of what it cannot answer.

## Why the light's slots were left

`light_miss_hit_cs` by hand answers a missed slot from the pyramid, and left what it
could not to `light_miss_cs`. The hand pass was cheap and took nothing off the frame.

| Shadows on, 25 s circle | f9f66682e | e8d616ba4 |
|---|---|---|
| `light_hit` p50 / p95 | 7.5 / 15.3 ms | 6.1 / 14.9 ms |
| `light_walk` p50 / p95 | 403 / 875 ms | 42.1 / 131.5 ms |
| `miss` nodes a cast | 4,291,609 | 140,074 |
| Slots walked | 5.13%, 20.0 nodes each | 3.42%, 1.0 nodes each |
| Total GPU p50 / p95 | 345 / 1,155 ms | 171 / 450 ms |
| fps | 2.3 | 5.0 |

- **The cause, by reading, then by the change:** the light's walks wrote no filled cell
  (`Observer::assets` false), which kept the pyramid's slots for the eye. A ray that
  stops on a filled cell found no slot there, so the hand pass left it, the generated
  pass walked it whole from the root, and did not keep the cell for the next cast.
  e8d616ba4 is that one word. The `miss` and slots rows are from 3 casts and 8; the spans from 30 frames and 67–71.
- **The eye does not pay in this view.** Alternating in one sitting, shadows on, `f9f66682e`
  then `e8d616ba4` twice: 2.6, 7.0, 2.2, 6.6 fps; the eye walked 0.64%, 0.49%, 0.60%, 0.42%.
  At rest the two commits differ by a mean of 0.56, each from itself by 0.50 and 0.59. A long
  drive, where the pyramid's slots run out, is not run.
- **A sitting drifts.** Shadows never on, where the light's walks do not run: `06a46035c`
  24.1 fps, `6e9a7cb83` 22.1, `e8d616ba4` 19.8, `06a46035c` again 19.4. Every pass rises
  together, thermal status 0 throughout. A figure from a warm phone is not set against one
  from a cold one.
- `sample`, `light_sample`, `beam`, `light_beam` and `miss` now each hold 50–90 ms at
  p95: no one wall.

## What a test at 0 differ does not say

The first test of `world.wgsl` compared 1,849,742 asks with the Rust at 0 differ, and
35 of 93 hand mutants survived it. A mutant is one small change to the code, an
operator or a constant: a test that passes with it does not hold that line.

- Closing them added what real callers vary: a zoom per node, not one for a descent;
  a clip level not laid; a concave service area; rows past the poles; heights past
  the world.
- Widening the test's world later stopped it reaching one value, and an old mutant
  came back. A sweep runs again after the world changes.
- At f9f66682e, on 2,700,150 asks: of 443 mutants, 388 fail, 7 hang, 48 survive.
- **A survivor is one of four, and a report gives them apart:** reachable, and the test
  misses it (3); not reached, at an i16's very end (4); held only by what callers or
  geometry give (about 6); or equal outright (the rest, read and not each proved).
- An outside sample caught a call the review had made. 14 mutants at random: 13 fail,
  and `world.wgsl` line 457 survives, which the review had as equal outright. A flip
  of one side changes an answer at an i16's end, which the test world has not. So the
  one who writes, the one who reviews and the one who samples the review are three.

## Faults seen, which later passes are held to

- No `var` of a struct type, no nested struct, no array carried through branches.
- Live words counted along the whole call path, the entry's included. `world_node`'s
  worst path is 76–94 and `walk` at its `slot_of` call 61, by source count.
- Constants only in the passes that read them, checked in the assembled output.
- A layout read from a `pub`, not inferred by arithmetic.
- naga, Tint and the S23 are three gates. Tint now reads every assembled module
  (`ts/tool/shader/shader.test.ts`).

## Open

- **The browser is not what makes the phone slow.** The Android app at e8d616ba4, wgpu
  on Vulkan, Release, same place and motion: 14.8 fps shadows off, 4.3 on, 1.7 casts a
  second on the drive, against the page's 16.4, 5.0 and 2.5. It draws 2.5 Mpx against
  2.1 and paced at about 25 ms, so this does not place naga against Tint.
- The drive with shadows on casts on few frames now (61 of 707 in 24.8 s), 2.5 casts a
  second against the 2.7 fps of a drive that cast throughout: no gain there yet.
- Trees' shadows at dawn. At noon they cast, on the Mac with a no-decor control and on
  the S23 at e8d616ba4 with shadows off, on, off. At dawn on the Mac they cast too
  ([the journal](./journal.md), "Tree shadow, decor").
- `world.wgsl` has not read a real world on the S23: no pass uses it yet. Every asking
  pass reads it since the passes went by hand, on the S23 from 2026-11-05
  ([the frame on the S23](../20261105-the-frame-on-the-s23/journal.md)).
