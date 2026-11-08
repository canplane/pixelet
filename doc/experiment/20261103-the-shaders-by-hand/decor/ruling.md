# The supervisor's rulings on the decor plan (read-only round, nothing built)

Paths: `decor.rs` = rust/client/shader/src/decor.rs. Every live-word figure is a source count; no pass has met naga, Tint or the S23.

The shader crate went with rust-gpu. What the rulings led to landed in `e85332699f`, on main `9e9b30b924` ([the journal](../README.md)), now `rust/client/render/src/frame/shader/decor_*.wgsl`.

## Finding not in the plan: 9 storage buffers
`world.wgsl` reads `palette_indices` (binding 4; `fold = palette_indices[...]` in `ground_cell`), so `decor_cs` on it binds clipmap, ground, palette, shards, overwrites, pool, records, rows, list: 9. `floor.rs` allows 8, and main's `shader.test.ts` makes each pipeline on a device of default limits. Today's `decor_cs` holds 8 through `NoPalette` (decor.rs:460). The "no-palette form" is NOT coming. `asset[0]` never reads `fold`, so decor can go without it.

## Verdicts
- Headline: holds. But each ask is `shard_at` + `world_cell`, as `world_node` does. With `SHARD_UNKNOWN` passed raw `is_wall` reads no served bits, so every leaf is a wall.
- Footing as a column read holds except:
  - Column idx 0 (void) with a way bound at the leaves: Rust hits (`world.rs:256`, `!fills(val)` takes the way). Test the way bound BEFORE idx. `tests/real_world` has no void idx column (mod.rs:258-264): add one.
  - "Stopping at rock" is untrue for a void idx: `by_depth` is not reached, so a scan runs void to z=-32768. Below the way bound's `lo` with a void idx it must end in a miss.
  - Scan start: `max((hi-1) >> (24-zoom), top-1)`.
  - Top cell of the world: the walk's `inside` (cast.rs:845) passes it, the column read hits; unreached, at an i16's end.
- Pose fast path: holds for every edit kind. `tree_val != VOID` does not mean placed: fold of matter under an eighth is `VOID`, not placed; fold whose kind is air is placed unless the ground lacks. So the result is `is_placed` of `world_cell`.
- CPU: the kernel keeps `generation::placement` at cell 24. Equal must stay: hash and thinning, `at` and `sink`, variant and turn, the box rule, `overlaps_placed`. Held by transitivity; the risk is the ported glue: keep it `support::*` over `real_world::World` (mod.rs:160-233), no logic of its own, and include cell 24 cases.
- Reductions:
  - Merged `decor_cs` writes `r`, `r+3`, `r+4`, drops `r+1`, `r+2`. Thin leaves the seed at `r+3` (decor.rs:349); unwritten it would read as z.
  - `FOOTING_MARK`, `footing_word`, `is_footing`, `footing_of` go (readers: decor.rs, generation.test.rs:68-81). Keep and reword the assert at generation.rs:100.
  - Second zeroing :257 dead. Mark's rows binding goes. Guards :310 :388 :450 :628 :767 dead; :223 dead only by 768/64: add a const assert.
  - `light_mark_draws_cs`, kernel, indirect args unaffected; cnt → args → compact order stays. `list` stays read-only in thin and `decor_cs` (it is their indirect buffer).
  - More dead: thin's `r+3` seed (decor.rs:489 recomputes). `RETAINED`: in a whole pair every record is asked, thin's `r` write goes. Thin's `held.z==0` (:314) and its clipmap binding: q is in the head mark wrote. `stands_at` in `pose` on the GPU: same seed, variant, cell as thin. `boxed` always true. `ColumnSurface::of_line`: only decor.rs:490 and its test.
- Live words: `decor_cs` 101-121, peak 123. `world_cell` peak 94, across a loop 76-92. Ask loop 27 merged; writing z and sink out at once and deriving `lo` gives 25 or less.
- First load: decor_cs and decor_footing_cs finish last in the load records; without them the wall ends at face_cs, 0.9-2.3 s earlier; without face_cs too, 12.0-14.0 s. decor_args_cs (1.5 KB) ends at 11-13 s, so an end time is a queue position. Gate: `load.js` before and after, wall and last three names.

## Test plan additions
- Frame sequences through all passes: whole, then part, then an edit removed so an `UNGENERATED` record returns. Whole and part pairs in one dispatch. Records not reached keep their region bits.
- Footing: void idx + way; void idx with no way.
- Pose: a decor leaf in the box. A hidden placed decor still claims in `overlaps_placed`. A non-decor placed leaf in reach; a high handle in the data word. Folds at coarse s: both sides of an eighth, kind air, ground lacking.
- Cnt, compact: `is_held` false; first and last run of each decor; placed count across `RUNS`; eye exactly on a record (`d` floor 1e-3).
- Args: on fresh uniforms, as the frame writes them.
- Hand compact read by the generated `light_mark_draws_cs`.
- Negative X, Y, z through `>>`, `(base+1)*(1<<s)`, `top(-1)`.
- Comparison: skip `r+3`, `r+4` of `UNGENERATED` records. With overflow, check the count and that records are a subset.
- Real path: page at one place, gen against hand, `regions` words equal.
- Floats: word-for-word on `d` will flake (WGSL sqrt and / are not bit-stable; contraction moves `distance2`). Lanes x, y, z exact: by bits. Spin: table of generated bit patterns. Sink: from generated f32 bits of `1 - rise` per k, then ×2^n + 0.5, exact. Tier and `zoom_at`: away from an edge, with cases 8 ULP either side. The gen differential holds bit-level on one device: run before deleting, keep its output.

## Rulings
- R1 merge: yes. Name the changed reason at decor.rs:365. Writes `r`, `r+3`, `r+4`.
- R2 column read: yes, with the way bound before idx and the scan's end for void idx. Owner is `support::column_surface`; the test's reference `generation::footing` through the real walk.
- R3: see the auditor's override in the message you were sent.
- R4: yes. Glue as `support::*` over `real_world::World`. Differential covers footing + pose against merged, on every swap, kept in the record.
- R5: yes, as its own commit, moves only, after the renames land. `seed` and `clip` also join `world_asked` (main's build.rs) and `SHADER_LIGHT_MISS`. Scatter needs a 2-D `node_seed`. Give `clipmap` a one-line part, so mark does not take all of `clip`; thin loses the binding. `decor_word` holds only decor's facts: a repeat of a `walk_word` or `world_word` name is a parse error, which enforces one owner.
- R6 keep `hollow`: yes.
- R7 upper-cell shortcut: no, not now.
- R8 new: decor binds no palette. Move the one read in `ground_cell` behind a function in a small part, with a binding form (light_miss, `world_asked`) and a form returning 0 (decor). Per pass, as `PaletteWords`/`NoPalette` is. `decor_cs` then holds 8. Rows stay a buffer.

## Facts
Retyped now: `seed_mix`'s multipliers and `0x9e3779b9` (private in xpute/rust/kit/src/math/rng.32.rs); `SplitMix32` would type them again: make them `pub` in the kit and generate. `side()` in main's `walk_node.wgsl`: generate from `answer::side`.
Must become `pub`, not typed: `PLACED_TAG`, `COUNT`, `SPLIT`, `TURN_SHIFT`, `CELL_SHIFT`, `CNT_AT`, `SLOTS_AT`, `PLACED_HEAD_WORDS`, `READ_REACH`, `CLAIM_EXTENT`, `BRICK_RECORDS`, `ROOTS`.
Workgroup sides move from the shader crate (`workgroup.rs`) to `generation.rs`; `compute::*_GROUP`, `*_WORKGROUP` exist only for `.gen.wgsl`. Each hand group in `generator.rs` gets `floor::holds`, as main's `cast.rs` does for `LIGHT_MISS_GROUP`. `regroup` binds by position (generator.rs:416): it needs binding numbers.

## The implementer does differently
1. R8 first, with `floor::holds` on every hand group.
2. Way bound before idx; scan's end for void idx; both in the test world.
3. Merged writes `r`, `r+3`, `r+4`.
4. `shard_at` before each `world_cell`.
5. Remove the further dead code above; const assert for :223.
6. `list` read-only; bind by number.
7. Move workgroup sides; private constants `pub`; generate the hash constants, `side` and sink.
8. `clipmap` its own part. Thin reads q from the head.
9. The listed cases; floats as above; sweep after each test-world change.
10. First-load gate on the load figures.
