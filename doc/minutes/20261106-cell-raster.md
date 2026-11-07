# Cell raster: list every cell at its zoom, raster it, eye and light alike

**2026-11-06** — an agent's proposal, written for
[the frame is what it must draw](./20261106-the-frame-is-what-it-must-draw.md); no
meeting's record, and no decision: the decision is the user's. Code read at
`af0cd3b3d0`. WGSL read by text search (no LSP server for it); Rust symbols by LSP where
it answered.

## 1. The listing rule

A walk asks a node at a pair of zooms, `far` at its farthest corner and `near` at its nearest point, both on the face the ray enters across (`cast.rs:648-677`, `walk.wgsl:121-127`). Each zoom is `cell_zoom_on(d² / 48², slant)` (`grid.rs:94-98`, `zoom.wgsl:21-26`), held to the eye's range. The world answers a cell where `zoom >= near` (`world.rs:152`). Coarser than that, it answers a split with the box its content lies in, air, or a lack (`world.rs:104-135`, `answer.rs:20-31`). A cell spans 1–2 view cells (1/96 rad, `grid.rs:17,26-29`) where it starts being drawn.

- **List** (eye): start at the roots and keep a node that meets the frustum (the screen's tangents, `FAR`; only the claimed tiles' box in a frame not drawn whole, `claimed_box.wgsl`). Ask it at every (far, near) pair its rays may ask (§2). The answer decides the node:
  - a cell that is not transparent (`answer.rs:157`) is **listed** as its box, or as its fill box where it fills less than its cube (`cast.rs:464`);
  - a lack is listed as a lack box;
  - air is **pruned**;
  - a split's 8 children are listed only where they meet its content box (`cast.rs:906-939`);
  - the cell holding the eye is never listed (`cast.rs:846`).
- **List** (light): the same descent, asked at the tile's band alone, `[band, band]` (`light.rs:247-255`, `walk.wgsl:123`). It runs inside the claimed tiles' columns, with the swept-hexagon test `Light::reaches` (`light.rs:299-332`) as the column test, one list per distinct band.
- **Answers:** off the node cache at the pair's key, else the world is asked and the answer kept (`ask.wgsl:142` `keep`). A split held at other zooms with `zoom < near` is descended without its box. That is exact (`walk.wgsl:20-23`) but prunes less.
- **CPU twin:** `pxl-engine/src/list.rs`, beside `cast.rs`, over the same `Query` the walk uses (`cast.rs:687`). It has `list::eye(q, o, frustum, detail)` and `list::light(q, light, tiles)`, and returns cells with their boxes and asked pairs. Its test: for every ray, the nearest box the ray enters and accepts (§2) equals `cast::first(.., Until::Opaque)`: t, axis, node and asset.

## 2. Exactness

**Per node.** A ray's walk visits a partition of its interval: every node it enters is a split it goes into, or a cell it stops at or passes as air (`cast.rs:836-990`). That partition's leaves are exactly the listed cells the ray enters and accepts. So the nearest entered box is the first cell it does not pass, which is the walk's hit. `raster_enters` (`raster.wgsl:182`) is the walk's slab test per pixel, so a touched-only box is not taken, as the walk does not take it (`cast.rs:836`). `cast::entry_with` then works out t exactly from the visible cell, as today (`cast.wgsl:3-8`).

**Per light tile.** One zoom is asked, so whether a node is a cell does not depend on the ray. The list at band b, rastered only into tiles of band b, is exact.

**Where it fails or needs care:**

- **Slant, the eye's one real gap.** `node_slant` reads the ray's own direction (`cast.rs:675-677`, called only at `cast.rs:838` per LSP). Rays through one face therefore cross slant octaves and ask different pairs. The comment at `cast.rs:663-666` says otherwise, and the code contradicts it.
  - Differs where a node lies on an octave boundary.
  - What keeps it exact:
    - list the node under every pair its faces' cosine range gives. Each face's extremes are at its nearest point and its farthest corner;
    - in the fragment, accept a cell only where the pixel's own ray asks it as a cell and asks each ancestor as a split meeting that ancestor's box. `zoom_on` is bit arithmetic, so the check costs ALU only;
    - mark ambiguous chains when listing, so unambiguous cells skip the check.
- **Overhangs and caves:** equal, because a filled coarse cell stops the walk without descending, and the list does not descend into it either.
  - It differs in cost, not picture. Every surface in the frustum is listed, hidden or not.
  - The world gives no "solid" answer. A node under the surface is a split down to `near` (`ground.rs:288-313`: the slab runs from the world's bottom), so the list goes volumetric (§4).
  - What keeps it: a new world answer, a node with nothing transparent under it at the asked zooms, listed as one box. The pixel's hit cell inside it is then resolved by a short descent along the pixel's ray. That descent is the walk's own hit, bounded by `near − zoom` levels with every child filled.
- **Splits held at other zooms and the cache's zoom keys:** equal, because the list asks every node at its own pair. The approximation `slot_of` makes, taking air held at other zooms as air "on nearly every node" with `revisits` correcting it (`walk.wgsl:20-23,66-70,132`), goes. The key's zoom pair stays.
- **Blind walks:** go. The list never takes a split without its box, except the exact case above, and lacks under it are listed (`walk.wgsl:198-201`).
- **Narrow splits a beam ray stops at:** go with the beams. Only `Until::Beam` applies `narrow` (`cast.rs:945-960`, `walk.wgsl:145-176`), and no eye or texel ray does.
- **Fill eighths:** equal, because the listed box is `fill_bounds` and the walk stops at that box or passes the cell as air (`cast.rs:902-920`, `walk.wgsl:185-196`).
- **Depth ties:** two entries within a depth ulp keep either. Differs only there.
- **Today's picture:** a pixel takes its visible *sample's* box (`cast.wgsl:3-8`). A cell that no sample meets is not drawn today and is drawn by the list. The reference becomes `cast::first` per pixel, not today's frame. This is a contract change to name.
- **Decor stays:** its own instanced passes against the world's depth (`decor_*`, `light_mark_draws.wgsl`), and its placement off the CPU walk (`support.rs:501`).

## 3. What owns what

- **Goes, eye:**
  - beam: `beam.wgsl`, `beam.eye.wgsl`, `beam_hit.wgsl`, `beam_walk.wgsl`;
  - cells and samples: `cell.wgsl`, `resume.wgsl`, `sample*.wgsl`, `sample_box.wgsl`, `sample_put.wgsl`;
  - misses and hits: `miss.wgsl`, `miss_hit.wgsl`, `miss_word.wgsl`, `list.wgsl`, `hit_lead.wgsl`, `hit_faces.wgsl`;
  - the walks: `walk.wgsl`, and in `walk_node.wgsl` the walk's own `Start`, `Walked` and `spacing_width`/`narrow`. Its node helpers stay.
- **Goes, light:**
  - `beam.light.wgsl`, `light_cell.wgsl`, `light_resume.wgsl`, `light_sample_box.wgsl`, `light_miss_hit.wgsl`, `light_miss.wgsl`;
  - the lattice half of `light_list.wgsl` and `light_aim.wgsl`'s cell start;
  - `light::BEAM`, `BEAM_WINDOW`, `LINE_WORDS` (`light.rs:720-759`).
- **Goes, engine:** `beam.rs` lines and samples, `Until::Beam`, `Spacing`, `narrow`, and the hand-on margin (`light::margin`).
- **Changes:**
  - `raster.wgsl` and `gpu/ray/raster.rs` draw listed boxes (`BOX_ID` becomes the list index, `raster.rs:46-50`) and gain the chain check.
  - `cast.wgsl` takes the visible cell or the sky, with no `ray::MISS` mark.
  - `light.wgsl` makes the same change for slots.
  - `face.wgsl` asks faces off the listed cell's parent entry (its Inheritance) instead of a beam line.
  - `boxes.wgsl` and `box_args.wgsl` are written by the list.
  - `ask.wgsl` becomes the list's one-thread-a-node ask-and-keep.
- **Node cache (`node_cache.rs`) changes:**
  - pass tags go from 6 to 2 (eye list, light list; `node_cache.rs:193-199`);
  - `MARKS_AT` and `HITS_ARGS`/`HITS_CNT` go (`:90-99,165-170`);
  - the key's zoom pair, Inheritance, buckets and `forget.wgsl` stay.
- **Stays:**
  - zoom rule: `zoom.wgsl`, `grid.rs`;
  - world query: `world.wgsl`;
  - light claims: `light_slide`, `light_mark`, `light_mark_draws`, `light_propagate` (`CLAIM_BUDGET`, `within_budget` `:113`), `light_stand`, `light_lay`, `light_gate`, `light_map`, `light_box`;
  - screen tiles: `screen_*`, `cast_tiles`, `raster_gate`;
  - decor, AO, atmosphere.
- **Tests:**
  - replaced: `sample.rs`, `cast.rs` (its `cast_cs` half), `light.rs` (its `light_cs` half), `light_miss.rs`, `margin.rs`, `walk.rs`'s narrow cases, and `beam.test.rs` in the engine;
  - new: `list.rs`, for the GPU list against `pxl-engine` `list` and the picture against `cast::first`;
  - kept: `world.rs`, `light_reads.rs`, `fill.rs`, `fold.rs`, `way.rs`, `decor*.rs`.

## 4. Size bound, worst drive frame

**Eye.** The field is 53.13° vertically (`HALF_TAN` 0.5, `pxl-spec camera/grid.rs:17`). At the S23's 2340×1080 the screen is Ω = 1.34 sr in landscape (0.40 in portrait; the drive's orientation is not in the code). That is Ω·96² = **12.3k view cells**. Slant coarsens a cell by about its foreshortening, so visible cells ≈ 12k.

| Count, worst frame | How it is reached | Estimate |
|---|---|---|
| Listed cells | 12k visible × hidden-surface depth D (planning 4, unmeasured) × surface shell (~2 cells a column on slopes) | **~100k** |
| Nodes visited | Splits ≈ cells/3, air siblings ≈ cells | ~230k |
| Without a solid answer | Frustum below the horizon (~0.67 sr) × 68³ cells per steradian per e-fold of distance × ln(far/near) ≈ 8 | **~1.7M** |

The 68 in the last row is the middle of 48–96 cells a radian.

**Light.** Map 2048² (`shadow.rs:46`), tiles 8² texels, at most 4096 tiles a cast (`light.rs:687-694`), which is 262k texels. The claimed disk is nearest the middle, radius √(4096/π)·8 ≈ 289 texels. A band cell at D texels from the middle is D/96 to D/48 texels wide (`light.rs:247-262,408-415`).

- Cells at most one a texel: π·68² + 2π·68²·ln(289/68) ≈ **57k**. Uncapped: ≈ 165k.
- At 07:00 the sun grazes the ground. At an elevation α of 10–15°, an assumption the code does not state, each texel column covers 1/sin α ≈ 4–6 cells of ground, and all of them are listed though rays miss most of them. That gives **0.25–1M**.

**Memory.**

| Item | Size | Eye (100k cells) | Light (1M cells) |
|---|---|---|---|
| List entry: node 4, asset 2, pair/flags 1, parent slot 1 | 32 B | | |
| Raster box (`raster.rs:46-50`) | 32 B | | |
| Total | | 6.4 MB | 64 MB |

- Each level's frontier is at most 16 B a visited node.
- The node cache (2¹⁹ entries) must hold eye plus light visits, ~1.2M worst. That is over its size, so cache churn is a risk.
- What a list does past its room is an open decision. The commit rule requires every count to be bounded (`af0cd3b3d0`).

## 5. Order of the change

Light first, each step with its own tests, the picture equal to `cast::first`, and one path a job:

1. **The world's solid answer** in `world.rs`/`ground.rs` and `world.wgsl`. Tested by `world.test.rs` and `tests/world.rs`, word for word.
2. **`pxl-engine` `list`**, eye and light, with its exactness test against `cast::first` over the test worlds, slant boundaries and fill eighths included.
3. **The light's list on the GPU.** It replaces the light's beam, cells, samples, sample boxes and misses, and `light.wgsl` takes the visible cell. In the same step:
   - `light.rs` is rewritten;
   - `light_miss.rs` and `margin.rs` are deleted;
   - the light's node-cache tags drop out.
4. **The eye's list on the GPU, with the raster's chain check.** It replaces the beam, cells, samples, misses and hit faces; `cast.wgsl` and `face.wgsl` are switched. In the same step:
   - `sample.rs` and `cast.rs` are rewritten;
   - `MARKS`/`HITS` are removed.
5. **Removal of what nothing calls:** `walk.wgsl`, `resume.wgsl`, `list.wgsl`, `beam.rs` lines, `Until::Beam`, `Spacing`, `narrow`, the `revisits` rule and `walk.rs`'s narrow cases. The cast's own `walk` stays as the engine's reference and the CPU readers' path (`support.rs`).

**Open beside the order:**

- A level-by-level descent with indirect args makes `gpu/split.rs` cut a command buffer per level, about 20 a list. A persistent-thread queue (atomicAdd only, `ask.wgsl:5-8`) avoids that.
- The record's "cost by the screen, not the world" (`doc/minutes/20261031-why-rays-and-what-makes-them-fast.md:42`) becomes "cost by the cells the zoom rule draws in the frustum". This is a contract change to name.
