# The audit after the move

**2026-09-14** — with the renderer, the shaders and the scheduler the
module's and the trees reorganised, three read-only audits went over what
the two days left: the Rust against the TypeScript it replaced (the
hand-ported parts, read against `archive/20260914-three-draw-paths`), the
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

- **A1** `bake/volume.rs` `set_voxel` / `fill_column` / `append_sub_voxel`
  do not check `cx, cy` against the band's span: a bad glue value writes
  into the next pool block silently. Early return. **Do.**
- **A2** Glue inputs that trap the module: pool and volume ids indexed
  unchecked, channel / type / topic ranges, `pxl_object_kind`'s id sizing a
  `resize` (a large id allocates gigabytes), decor `variant_count == 0`
  underflowing, `Builder::add`'s `assert!`. Bounds checks, return 0 or
  ignore. **Do.**
- **A3** `RunSet::carve` is `unsafe fn` but sound; `Builder::page_view` is
  a safe fn over glue addresses; two `band_view` calls without a SAFETY
  line. **Do.**
- **A4** The badge ladder and the occlusion tolerance (`module/world/lod.ts`)
  ignore the scheduler's pressure: under the byte budget badges gather on
  a different rung than the cubes and can be hidden by coarse boxes. Scale
  the glue's thresholds by the census's pressure. **Do.**
- **A5** The sky's time is tick time — clamped to 0.1 s a frame, ticked
  every 2 s at rest — so clouds and twinkle crawl at a twentieth speed
  while idle. Use the frame's clock. **Do.**
- **A6** The texture pack's sampler has no anisotropy; three's arrays had 4.
  **Do.**
- **A7** The beacon's specular and its point light, the zone's side-face
  border orientation, the gizmos' screen-sized points and the
  `anchor_center` marker. **Hold** — cosmetic, and reproducing three's
  material is not a goal.
- **A8** CUBICSPLINE channels collapsed, `matrix` joints at identity. No
  shipped asset has either. **Hold** until an asset does.
- **A9** `cargo test` and `cargo clippy --all-targets` fail at the
  workspace root: pxl-wasm's test binary cannot link natively, and a test
  constant approximates `FRAC_1_SQRT_2`. **Do.**

## B. Hot paths

- **B1** `companion.tsx` calls `invalidate()` every tick unconditionally,
  so the idle-frame path never happens (which is why pressure relieved on
  still frames). The module's draw bit says when an actor animates. **Do.**
- **B2** Climb mode runs `climb_polar` → BigInt page math → two bicubic
  samples every frame, moved or not. Memoise on (dist, polar, azimuth,
  map_center, tile revision). **Do.**
- **B3** `object/store.ts` re-encodes the whole store to `localStorage` on
  every placement. Debounce to the report phase until persistence moves.
  **Do.**
- **B4** `alloc_probe` is installed in DEV whether or not the panel shows:
  every typed-array construction through a Proxy, a stack capture past
  64 KB — it distorts the frames the ledger measures. Behind a toggle.
  **Do.**
- **B5** `Builder::tick_capped`'s `seen.contains` is O(n²) over the pending
  list; a `bands` Vec per scan step; seven Vecs a tick in the scheduler;
  `ask()`'s and `reclaim`'s Vecs; the raycast's Vec per chunk and
  `band.check()` per DDA cell. Scratch on the owners; holders first. **Do.**
- **B6** The mesher walks a run's column four times in `run_face_mask`
  (two suffice, same bits); the coarse tier is packed into `CoarseLanes`
  then copied whole into an `OwnedRunSet`; `emit_box` and
  `write_lone_box` write the same six faces. The byte-for-byte bake test
  guards it. **Do.**
- **B7** The renderer and the stage rebuild views over fixed module
  addresses every frame (env, draw row, census, stat); `census()` twice a
  frame, `stat()` ten times in the ledger; `take_census` objects retained
  by the ledger ring for the session. **Do.**
- **B8** Every tier event carves nine mesh lanes and fills the visibility
  lane; the renderer uploads the block by address and reads no lane. The
  node's cache becomes `{block, counts, rc}`; `visibility`, `role`,
  `carve_visibility`, `mesh_bounds` go; the 4 B/vertex visibility lane in
  the block goes the next time the mesher's layout is touched. **Do.**
- **B9** The POI index's `page_idx` / `abs_i` / `abs_f` lanes (~3 MB, three
  BigInt stores a record) are read by nothing. Delete. **Do.**
- **B10** Decor's build does a `store.cell()` lookup for each of 4096 cells
  per kind; `object.rs` inserts then gets; `build.rs`'s `advance` looks a
  chunk up three or four times. **Do.**

## C. Duplication, to one home

- **C1** The lattice word's pack/unpack four times (sched, decor, wasm,
  inline), the packed position's unpack three times → `space/ivec.rs`,
  the first commit of the PVF3 move. **Do.**
- **C2** `U_PER_CELL` five times, the 64-cell span seven ways, the page
  constants twice → one source each. **Do.**
- **C3** WGSL: the Y-up→Z-up `place` three times → `place.wgsl`; `PI`
  twice; chunk.wgsl's face-bit literals separate from mesher.rs's
  constants → a generated `const` preamble in `source()`. **Do.**
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
- **C7** pxl-wasm builds `World { .. }` three times and `volumes.expect`
  eight times; `mix64` and `Rng::next` are the same splitmix; the 2-D DDA
  twice in raycast.rs; the frame's point transform twice; "against the
  origin" six times; grow-and-upload three times (billboard, debug,
  marker). **Do.**
- **C8** The ledger's formatting helpers (MB rounding twelve times, n1/r1,
  the long-frame literal, the phase-key formatter). **Do.**

## D. Dead code

- **D1** `lod_build_stat.ts` whole; the ledger's `recheck_ms` (always 0)
  and `work.runs` (always 0) printed in four places; `landing_log`;
  `chunk_node`'s `shown` / `blend` / `layout.primary`; the node's own
  `CORE_BUILD.add` / `want` / `build_tick` path (tests only);
  `column_has_entries`; `_sample_ele_bilinear`; `clamp01`;
  `select_visible_billboards`; `stable_hash`; `in_range_chunks` and
  `scatter_decor_candidates` with the eligibility predicates (references
  only → `reference/`). **Do.**
- **D2** Store slots nothing reads: `selected_poi`, `reset_view`, the
  theme's `background` / `grid`, the pose store's `course` / `beta` /
  `gamma` / timestamps, `use_reverse_geocode`'s `loading`,
  `PoiPick.key` / `anchor_*` (a BigInt string per drawn badge),
  `cell_node.stats()`; `MountListener.on_unmount`, `VertexLane`,
  `CORE_MEMORY_BYTES`, an empty positioned div, commented-out JSX. **Do.**
- **D3** `enableMapSet()` from immer in main.tsx — nothing uses immer and
  it is not in the import map. **Do.**
- **D4** Dead `pub` in the crates (`ring::ARG_WORDS` / `at`,
  `buddy::region_bytes`, `sparse_tree::dep`, `object::BAND_STEPS`,
  `shader::Record` outside tests), six `too_many_arguments` allows on
  extern fns the lint does not fire on, `static_mut_refs` → an
  `UnsafeCell` newtype. **Do.**
- **D5** The orphan files and the scaffolding (`manage_foo`, `dev_route`,
  `use_app_init`, `app.store.ts`, `use_timer`, the pagination pair,
  `minimap_atlas`, `lib/env/mod.ts`). **Hold** — some are templates for
  modules to come; unrelated to the move.
- **D6** Exports nothing calls (`pxl_heap_used`, `pxl_scan_begin`,
  `pxl_model_remove`); `pxl_build_tick` and `OP_CHUNK_WANT` serve tests
  only. Delete the first, mark the second. **Do.**

## E. The binding's shape

- **E1–E3** Thirteen static-address exports that could be one table read
  at init; return types split between `*const u32` and `u32`; failure
  sentinels 0 / id+1 / −1 / `u32::MAX`; `pxl_bake_step` +
  `pxl_bake_mesh_words` stepping twice to read a size. **Hold** — the view
  and page moves rewrite the export set; settle the conventions then, once.
- **E4** Lines 193–802 of lib.rs (~35 exports) serve only `reference/` and
  the object store — mark the block. **Do.**

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
