# The audit after the move

**2026-09-14** — with the renderer, the shaders and the scheduler the
module's and the trees reorganised, three read-only audits went over what
the two days left: the Rust against the TypeScript it replaced (the
hand-ported parts, read against `b9e31004`), the
crates for duplication, hot-path waste, stale comments and unsafe surfaces,
and the client's live code for the same. This is the consolidated list,
each item with a decision, and it is worked in the order at the end; an
item is struck when its commit lands. The four moves that follow it are in
[20260914-what-stays-in-javascript.md](./20260914-what-stays-in-javascript.md).

## What the port got wrong

Put right in `b9fb023` (deployed as b9): the grade's brightness and
contrast ran in linear light, whose zero at linear 0.05 crushed every night
colour to black (postprocessing's BrightnessContrastEffect takes sRGB); the
vignette was a full-screen pass and the sky and the badges had none; the
fog was a ramp over the ray's length where three's is a smoothstep over
view depth; the terrain took the brighter of the sun and the moon, and the
light's direction snapped where their intensities crossed.

Confirmed matching, so not to be re-derived: the ephemeris line for line,
the frame composition and projection, Lambert over pi with the hemisphere
on n.z, every rim/edge/dither/UV constant, the billboards' fit, fade and
occlusion numbers, the puck's ease and spin, the ghost's biases, no tone
mapping (the composer never tone-mapped), reversed-Z.

## A. Safety and correctness — first

- ~~**A1**~~ `bake/volume.rs` `set_voxel` / `fill_column` / `append_sub_voxel`
  did not check `cx, cy` against the band's span: a bad glue value wrote
  into the next pool block silently. They refuse it now.
- ~~**A2**~~ Two glue inputs that could not be a bug of the module's are
  bounded: an object state past a leaf's width is not taught (it sized a
  `resize` to the id — gigabytes for a large one), and a decor kind with
  no variants contributes no group instead of underflowing. The rest —
  pool and volume ids, channel / type / topic indices, `Builder::add`'s
  assert — are indices the glue derives from constants; a wrong one is a
  bug, and the module traps on bugs by design (fixed_heap.rs says the
  same of its arena). Left as traps.
- ~~**A3**~~ `RunSet::carve` is a safe fn (it checks both preconditions);
  `Builder::page_view` is `unsafe fn` with its contract, its three callers
  say why they hold it; the two `band_view` calls carry their SAFETY.
- ~~**A4**~~ The glue's ladder takes the module's pressure through the
  census: badges climb the rungs the cubes are on, and the occlusion
  tolerance assumes the cell actually drawn.
- ~~**A5**~~ The sky's clock is wall time since the first observation
  (`observe` is told the clock every tick), not the tick's clamped delta.
- ~~**A6**~~ The texture pack's sampler is anisotropic at 4.
- **A7** The beacon's specular and its point light, the zone's side-face
  border orientation, the gizmos' screen-sized points and the
  `anchor_center` marker. **Hold** — cosmetic, and reproducing three's
  material is not a goal.
- **A8** CUBICSPLINE channels collapsed, `matrix` joints at identity. No
  shipped asset has either. **Hold** until an asset does.
- ~~**A9**~~ `cargo test` and `cargo clippy --all-targets` pass at the
  workspace root: pxl-wasm has no test target, and the test constant is
  one clippy does not read as a named one.

## B. Hot paths

- ~~**B1**~~ The module says when an actor animates (`Models::tick`
  returns it, folded into the tick's draw bit); the companion no longer
  asks for frames itself. An actor playing a clip still means a frame a
  tick — that is what an animated companion is — but the module decides.
- ~~**B2**~~ `climb_polar` is memoised on the view, the map centre and the
  page table's revision.
- ~~**B3**~~ The store's save is put off 300 ms and flushed when the page
  hides; a burst of writes is one save. Immediate off the page (the tests).
  Lost in the move to the kernel (2026-09-15): nothing calls
  `flush_save_if_due` and nothing flushes on hide, so nothing is saved.
- ~~**B4**~~ The allocation probe is installed only from the console
  (`__pxl_alloc_probe()`), never by default. Not carried into the kernel;
  it is in apps/web1 only.
- ~~**B5**~~ `tick_capped` compares against the two holders instead of a
  seen list; the scan job's and the raycast's bands are kept on their
  owners (`Volumes::band_views` fills them); the scheduler's tick keeps
  its seven lists as `Scratch`; `ask` counts into its pair, `reclaim`
  collects the evicted steps as a bitmask. `band.check()` is per walk of a
  column, not per cell, and stays.
- ~~**B6**~~ `run_face_mask` walks its own column twice (`column_covers`
  gives "anything there" and "terrain there" in one walk); the coarse
  tier is packed straight into the `OwnedRunSet`; `write_box` is the one
  face writer. The bake test held.
- ~~**B7**~~ The env, the draw row, the census and the build stat are one
  view each, and `census()`/`stat()` fill one object each. `take_census`
  keeps a fresh object a frame on purpose: the ledger's ring holds it.
- ~~**B8**~~ A tier arrives as `MeshBlock {block, counts}` and the node
  holds it until the module evicts the step — no lanes carved, no
  visibility fill, no count between a cache and a layout the node no
  longer keeps (the module draws from its own lists). `chunk_bucket.ts`
  is gone; `carve_baked_mesh`/`carve_visibility` serve the reference tests
  only. The visibility lane in the block goes with the mesher's layout.
- ~~**B9**~~ The POI index's `page_idx` / `abs_i` / `abs_f` lanes are gone.
- ~~**B10**~~ Decor looks a cell up only in a chunk with entries;
  `index_cells` sums each cell's record and writes it once. `build.rs`'s
  two or three `BTreeMap` lookups per advance sit beside a multi-millisecond
  scan or bake step and are left.

## C. Duplication, to one home

Not in scope: `packages/xpute/core` and `crates/xpute-core` are the same
kit twice on purpose, one for each runtime, and stay symmetric; so is
`@pxl/shared`'s space and `crates/pxl-shared`, and any piece both
runtimes need at all. A piece that is in both is not a duplicate to move
to one side. The TypeScript side is read-only in this: nothing there is
deleted for the Rust's sake, and a Rust module that uses one thing of a
TypeScript file is that whole file, at the same path (`script/pair.ts`).

- ~~**C1**~~ `space/ivec.rs`: `lanes` / `gather` and their XY halves;
  `object::unpack_pos` for the packed position.
- ~~**C2**~~ `space/constant.rs`: the cell, chunk and page constants once.
- ~~**C3**~~ `place.wgsl` (`place_up`, `spin_z`) for decor, the actors and
  the beacon; `PI` once in grade.wgsl; `faces.wgsl` names the face bits
  and a naga test reads its constants back against the mesher's.
- **C4** TS: `DRAW_WORDS` / `UNIFORM_FLOATS` / pipeline ids restated in
  seven passes, the same draw-list walk twelve times, lane binding three
  times, the dynamic uniform buffer's growth twice → a layout table read
  from the module, the list bucketed by pipeline once a frame, `bind_lanes`,
  one uniform ring. **Do.**
- **C5** "Read a row of the module's memory" thirteen times, `take → fill
  → pxl_* → give` five times → `row()` and `with_lane()` in `wasm/pxl.ts`.
  **Do.**
- **C6** FNV twice (the kit has it), hex colour three times, pixels per
  unit four times (the scale bar's against `innerHeight`, which can
  disagree), degrees/radians at seven sites, the Seoul fallback three
  times, the DPR clamp three times, canonical→local three times, the
  badges' `RENDER_SCALE` rationale twice. **Do.**
- ~~**C7**~~ pxl-wasm splits the state once (`parts`) and opens the
  volumes once (`opened`); `mix64` is the one finaliser; `GridWalk` is
  the one DDA; `Frame::world_of` the one transform;
  `DrawList::translation` the one "against the origin"; `Held` the one
  grow-and-upload, for the badges and the gizmos — the puck's block is
  replaced at its exact size and keeps its own path.
- **C8** The ledger's formatting helpers (MB rounding twelve times, n1/r1,
  the long-frame literal, the phase-key formatter). **Do.**

## D. Dead code

- ~~**D1**~~ Gone: `lod_build_stat.ts`, `recheck_ms` and `work.runs`
  (the ledger's `cpu` is `tick_ms`, the module's tick; `mounts.rendered`
  is `layouts`), `landing_log`, the node's `shown` / `blend` / layout and
  its own-add path (a node takes the module's id), `clamp01`;
  `in_range_chunks` is `reference/chunk_range.ts`. Still to move:
  `column_has_entries`, `_sample_ele_bilinear`, `select_visible_billboards`,
  `stable_hash`, the scatter with its predicates — with step 5's
  duplication pass, which touches the same files. `CoreBuild.add` /
  `want` / `build_tick` stay as the reference tests' door.
- ~~**D2**~~ Gone: `selected_poi`, `reset_view`, the theme's `background`
  / `grid`, `PoiPick.key` / `anchor_*`, `VertexLane`, `CORE_MEMORY_BYTES`,
  the empty div and the commented-out JSX. Kept: the pose store's
  `beta` / `gamma` (they gate a store write), `MountListener.on_unmount`
  (the reference tests observe it), `use_reverse_geocode`'s `loading` and
  `cell_node.stats()` (small, left for the POI move).
- ~~**D3**~~ immer's `enableMapSet()` is out of main.tsx.
- ~~**D4**~~ `BAND_STEPS` gone, `Record` / `Buffer` test-only, the six
  dead allows gone, the state in an `UnsafeCell` cell. `ring::ARG_WORDS`
  / `Ring::at`, `region_bytes` and `Tree::dep` stay: each is the pair of
  a public thing on the TypeScript side (`RING_ARG_WORDS`, the ring over
  given words, `BuddyAllocator.region_bytes`, the tree's layout).
- **D5** The orphan files and the scaffolding (`manage_foo`, `dev_route`,
  `use_app_init`, `app.store.ts`, `use_timer`, the pagination pair,
  `minimap_atlas`, `lib/env/mod.ts`). **Hold** — some are templates for
  modules to come; unrelated to the move.
- ~~**D6**~~ The three uncalled exports are gone from the crate and from
  `pxl.ts`; `pxl_build_tick` and `OP_CHUNK_WANT` say they are the tests'.

## E. The binding's shape

- **E1–E3** Thirteen static-address exports that could be one table read
  at init; return types split between `*const u32` and `u32`; failure
  sentinels 0 / id+1 / −1 / `u32::MAX`; `pxl_bake_step` +
  `pxl_bake_mesh_words` stepping twice to read a size. **Superseded** —
  the export table is bindgen by hand and folds into the rings
  ([what-stays-in-javascript, restated](./20260914-what-stays-in-javascript.md));
  nothing in it is tidied on the way out.
- ~~**E4**~~ lib.rs's sections say which exports are reference only (the
  bake and scan steps, the hand-driven volumes and terrain,
  `pxl_object_paint`, the raycast's column) — the live ones are
  interleaved, so the mark is per section, not one block.

## F. Comments and names the word sweep missed

- **F1** Crate comments naming TypeScript files that no longer exist
  (`chunk_stage.tsx`, `chunk_table.ts`, `celestial_position.ts`,
  `debug_layer.tsx`, `decor_layer.ts`, `arena.rs`, …), nine places. **Do.**
- **F2** Comments describing behaviour the code no longer has: `sched_ms`
  "does not include the render" (the WebGPU submit is inside the visible
  phase now), `mounts` as React mounts (they are node constructions),
  hover_cursor's "which polygon offset wins" (it is the coplanar pipeline),
  tick.ts's header narrating three's loop, poi_layer's sprite era, "the
  three path" / "three's shadow camera" / "where the dome stood" in ten
  places, `frame_wgsl.ts` in shadow.ts, `SKY_DOME_SCALE` → `SKY_RADIUS`,
  elevation.ts's header naming files that do not exist, terrain/store.ts's
  ownership list. **Do.**
- **F3** Twelve comments naming a `reference/` file as the live mechanism
  (`terrain_layer.ts`, `chunk_mesher.ts`, `object_layer.ts`,
  `voxel_scan.ts`) → the crate. **Do.**
- **F4** `poi_layer.tsx` / `PoiLayer`, `ChunkMountsProps = Record<string,
  never>`. **Hold** — renamed with the POI move.

## G. Tests

- **G1** `fdlibm.rs` and `srgb.rs` claim agreement with V8 and pin nothing;
  decor's RNG test never calls `Rng::next`. Pin values from the kit. **Do.**
- **G2** The mesher, scan, volume, build and terrain are held only by
  `reference/*.test.ts`; Rust fixtures are needed before `reference/`
  retires. **Hold** — that is the condition reference/README.md states.
- **G3** `elevation_sample.test.ts`, `scatter.test.ts`,
  `store_reference.test.ts` are reference tests in the live tree → move.
  **Do.**
- **G4** Live logic with no test: chunk_node's ownership rules (three bugs
  on record), poi_index's job lifecycle (the 2026-09-12 bug),
  pipeline.ts's record decoder, `view_compose`, `atlas.pack`. **Do** the
  first two; the rest move soon.

## H. Boundary leaks — the input to the moves, not fixed here

The view: `lead_point`, `view_compose`, the camera clearance's TypeScript
bicubic, the theme key derived from `pxl_env_daylight`. Pages:
`pages_to_hold` ranking pages from the anchor the module already mounts
by; the demand table sorted by TypeScript. POI: a frustum test per
candidate in TypeScript, the fade curve in both TypeScript and WGSL,
`lod.ts`'s copy of the ladder, atlas UVs packed into the module's lane by
TypeScript. Objects: the whole store saved per write, the entry's name
searched again after the module's hit. All four are the moves in
[20260914-what-stays-in-javascript.md](./20260914-what-stays-in-javascript.md).

## Order

1. A1–A3, A9, A4–A6 — safety and correctness, small.
2. B1–B4, B7, B9 — the frame's and the idle view's cost, the save, the
   probe.
3. B8, D1–D3 — the mesh lanes nobody reads and the dead code beside them.
4. C1–C3, B5, B6, B10, C7, D4, D6, E4 — the crates' duplication and hot
   paths.
5. C4–C6, C8, D2 — the client's duplication.
6. F1–F3, G1, G3, G4 — comments and tests.

Each step is one commit with the gate green; a ledger capture after 2 and
3 says what the frame gained.

Steps 5 and 6 were not taken up before the client moved into the kernel
(2026-09-15); most of the TypeScript their items name is gone.
