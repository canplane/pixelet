# Decor passes in hand WGSL: the implementer's plan (read-only stage, nothing measured)

`decor.rs` = rust/client/shader/src/decor.rs.

The shader crate went with rust-gpu. The decor passes landed by hand in `e85332699f`, on main `9e9b30b924`
([the journal](../journal.md)), now `rust/client/render/src/frame/shader/decor_*.wgsl`,
footing and pose one pass, `decor_cs` in decor_pose.wgsl.

## Headline claims
- The GPU's `Line` holds nothing: `Ground::root` is `UNPLACED` (decor.rs:51), `child` on it stays `UNPLACED` (world.rs:88), so the 33-entry stack `decor_cs` rebuilds (decor.rs:470) is constant.
- An unedited ask is one `world_cell(UNPLACED, …)`; an edited ask is one `world_root` plus `world_cell`.
- `footing` needs no walk: read off the column, `bound_over` + `ground_cell` (world.wgsl:701,705).

## Numbers derived from the catalog
LEAF_CELL 24, FIRST_CELL 13, ACROSS 8, EYE_ENTRIES 768, BRICK_CELLS 4096, BOX_CAP 21. Roots: tree 23, boat 22, car 23, train 23. Caps 864, 176, 144, 80. BRICK_RECORDS 1264, RUNS 79, KIND_RECORDS 864 (x = 14 workgroups). A whole load at most 576 pairs, about 43k records.

## Entries
- decor_mark_cs :213. No world. WG 64 over 768. 0 visits. rows read only for its length, goes.
- decor_thin_cs :300. No world, hashes only. WG 64, indirect. Neighbour candidates: tree 0, car/train 24, boat up to 48.
- decor_footing_cs :368. `footing` generation.rs:469, `finest` decor.rs:140, `support::column_surface` support.rs:79. Per record 1-2 walks, each at least 2·cell−7 visits, up to ~3.5M visits a whole load (unmeasured). Direct column read proposed: 1 visit equivalent, scanning cells only where the column's way bound is non-empty.
- decor_cs :431. `pose`, `asks`, `overlaps_placed` generation.rs:507,532,752, `PlacedCursor::next` world.rs:368, `placed_pose`, `row_of`. Asks per record at s=0: tree 7, car 21, train 27, boat up to 88; failing first box cell returns early. Plus at most 8 zoom-21 tree descents. Pass rate unmeasured.
- decor_cnt_cs :584. No world queries; one `step` per laid record.
- decor_args_cs :689. WG 1. No world.
- decor_compact_cs :725. No world.

Footing cases from `ground::node` and `world_cell`: is_past → hit at z=-1; not held → lacks; idx not ground and not unknown → miss; else no ways → hit at top-1, rise from eighths(part); way bound non-empty → scan down through the `world_cell` site from max(way top, ground top); under a bore the scan can read `hollow`, stopping at rock at the latest.

Pose fast path: edited ask is `cut_holds(carve)` or `tree_val(tree_down…)`; nothing there → false with no `world_cell`; otherwise the same `world_cell` site. Asks at s=0 become tree 4, car 18, train 21, boat 48.

## To write in WGSL
Generation words: head_at, cnt_at, record_at, entry_record, part_at, record_word, record_of. reaches, meets, asks, footing's result, overlaps_placed, placed_pose. distance2, drawn_at, held_at, tier. Scatter: count, candidate_at, before, meet, variant, side_of, retained, stands_at, cells, allows, mul_hi. share::quarter, 2-D node_seed, SplitMix32. The placed walk with no cursor array. From the shader crate: list_entry, is_changed, shown_at, run_of, is_held, about.
Generated, not typed: row facts as `switch` functions from pub Rust (decor_cap, decor_root, decor_widest, decor_first_record, decor_first_group); row layout from offset_of!(Decor, …). Rows stay a buffer.

## Live words (hand estimates)
decor_cs merged: ~24 across the ask loop plus world_cell 76-94 = 100-118. thin 40-45, cnt 35, mark 20, compact 20, args 10. Risk: hollow → holds → hand_down under the ask loop. One `world_cell` call site in a state machine: scan, vote, box.

## Modules
`decor_word()` in build.rs; read_consts trims each pass. New parts: seed, clip (moved out of world.wgsl), decor_record, decor_scatter, decor_mark, decor_thin, decor_pose, decor_cnt, decor_args, decor_compact. World passes take world.wgsl's binding numbers. Group constants in generator.rs. No new span: GpuSpanName::generate covers it.

## Tests
Word for word against the Rust on real buffers; records and instances compared as sets per pair and per region (atomics make order free).
Varied: mark: LIST_EVERY, head stale/matching, empty entry, 0-21 boxes, EVERY and by zoom, X round the world, shown bits. thin: cells 22-24 and coarser, seam, south edge, every decor, room overflow. footing: finest ≠ cell, lacks at k=0 and 1, past, wall, void idx, way bridge and bore with a cave under, each part eighth, poles. pose: 4 decors, vote both ways, flip, coarse s, a box across columns, edit of air/object/fold, carve, hide, placed overlap across X, records in part. cnt: eye moved, range, hide, tier edges, held_at at seam and gap, placed tail, laid over cap. args: mesh 0. compact: X wrap, slot over cap.
Named tallies prove each outcome reached; a sweep over each new module and world.wgsl's moved parts, every survivor classed; Tint through the assembled example and shader.test.ts; S23: pipeline made, `generate` span, first load.
First-load limit: face_cs, light_*, miss sit at 12-16 s side by side, so taking decor out may not move the 15 s wall.

## Order
args, decor_cs with footing, thin, mark, cnt, compact. Each swaps alone in part.rs and each gets an S23 load.

## Reductions
decor_footing_cs goes (one pipeline, one dispatch; footing words and FOOTING_MARK unused on the GPU). Line, AssetLine and the stack go. decor_cs writes only out[r] (r+1..r+4 already hold what decor.rs:495-498 rewrites). held=false becomes UNGENERATED at once. Second zeroing in mark goes (decor.rs:257). Dead guards go: s>=EYE_ENTRIES :223, i>=BRICK_CELLS :310, r+4>=len :388,:450, i+3 :628, i+4 :767. Mark's rows binding goes. `Instances` (:835) stays for light_mark_draws_cs.

## To rule on
- R1 Merge footing into decor_cs. Changes on purpose the reason at decor.rs:365. Recommends merge; split back if the S23 refuses.
- R2 Column read in place of support::column_surface: a second definition held by the test. Recommended.
- R3 Start point: needs main's coming commit (real_world/, walk_node, read_consts, examples/assembled.rs, shader.test.ts, renames, no-palette form). Recommends rebasing once it lands.
- R4 Reference: committed test uses the Rust with the shader crate's glue ported into the test (render cannot link spirv-std); a one-time scratch differential runs the .gen.wgsl modules pass by pass on the same device, to hold that port and the floats; the 7 gen files deleted at the bundle's end.
- R5 Move seed_mix, node_seed, rotl, halve, wrap_idx and the clip-level functions out of world.wgsl into seed.wgsl and clip.wgsl, so thin, mark and cnt don't include the world. Moves only, after the renames.
- R6 Keep `hollow` on the ask path. Recommended: exact.
- R7 Column shortcut for upper box cells only if the S23's `generate` span says it matters.
