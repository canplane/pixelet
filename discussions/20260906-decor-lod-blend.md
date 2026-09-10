# Decor gets the same LOD threshold blend cube content already had

**2026-09-06** — closes the last item on `decor_layer.tsx`'s own
"CURRENT LIMITATIONS" list (see [20260906-decor-lit-shading.md](./20260906-decor-lit-shading.md)
for the other item that list used to carry, the unlit-material gap, closed
earlier the same day). Decor's density thinning with distance
(`DECOR_DENSITY_BY_LOD_STEP`) used to pop synchronized across every chunk
that crossed a threshold in the same frame — a whole ring of forest visibly
halving in density at once. Cube content already solved the equivalent
problem for its own tier pop (`render/lod.ts`'s `get_lod_pair_visibility`);
decor now reuses the identical mechanism.

## What this is NOT

This does not make an individual tree fade, shrink, or otherwise change its
own representation with distance — decor still has no notion of a "coarser"
model the way cube content's box-merging LOD does (see this session's own
prior discussion: a surviving tree at any density level is always the exact
same full-detail model). What changes is **where** the population-level
thinning happens: instead of every chunk at a given distance switching
density in the same frame (a hard-edged ring), the switch now staggers
across a range of distances (`render/lod.ts`'s existing `BLEND_BAND_FRACTION`
band), the same way cube content's own tier pop got smeared rather than
fixed.

## What changed

- **`chunk_stage.tsx`'s `Chunk` now builds decor for both `primary` and
  `secondary`** (its existing per-tier LOD pair) whenever a chunk sits
  inside a blend band, not just `primary`. `decor_cache_ref` was already
  keyed by `LodStep` alone (one entry per density level, not per role), so
  asking for both is two lookups against the one cache, not a second cache
  or a second build path — a new `build_decor_pair(primary, secondary)`
  helper is the only new code at the build-data level.
- **`LodLayout.decor`** changes shape from `ModelInstanceTransformList[][]`
  (one built set) to a new `DecorLayoutPair` — `{ primary, secondary }`,
  `secondary` null outside a blend band, mirroring `LodLayout.secondary`
  itself exactly.
- **New shared `render/lod_visibility.ts`**, holding `apply_visibility` and
  its registry types — factored out of `chunk_stage.tsx` so decor_layer.tsx
  can register into the _same_ registry `ChunkMesh` (cube content) already
  uses, without decor_layer.tsx importing anything from chunk_stage.tsx
  itself (that reverse direction was removed as a circular import back on
  2026-09-05 — see decor_layer.tsx's own top doc comment). One registry, one
  shared `apply_visibility` call per `recheck()`, now drives both cube
  content and decor's own primary/secondary visibility together — a chunk's
  terrain and its trees switch at the exact same moment, not two
  independently-timed transitions.
- **Decor toggles via a plain `THREE.Object3D.visible` flip**, not cube
  content's per-vertex `a_visible` attribute + fragment-shader discard.
  Cube's own mechanism exists because one built mesh gets _reused_ across
  LOD transitions and needs a way to hide/show without touching geometry;
  decor's per-step build cache already hands back a distinct, already-baked
  mesh per density level, so there's nothing to reuse — an entire mesh is
  either the one currently drawing or it isn't, which `.visible` expresses
  directly, no new per-vertex infrastructure needed.
- **`decor_hit.ts`'s `raycast_decor` now filters out invisible meshes**
  before raycasting. This was a real, easy-to-miss gap: `THREE.Raycaster`
  does not check `Object3D.visible` on its own (that flag only affects the
  render pass) — without this filter, hovering during a blend band could
  resolve a hit against whichever of the two blend-pair meshes happened to
  be geometrically nearer along the ray, regardless of which one was
  actually on screen.

## Verification

`deno task check`/`lint`/`test` all green (139 tests, lint's 76-problem
`xpute/core` baseline unchanged — one transient regression to 77 while
`ChunkVisibilityDecision` sat imported-but-unused mid-edit, caught and fixed
before this landed). Live: walked the character across many chunk
boundaries (well past `RENDER_RADIUS`, forcing sustained mount/unmount
churn on the shared registry) with zero new console errors beyond the
pre-existing, unrelated `THREE.GLTFLoader: Couldn't load texture
Textures/colormap.png` noise. Did **not** visually confirm the actual
staggered-pop effect itself this pass — reproducing a real threshold
crossing (192/384/768 world units) needs either a sustained zoom-out this
session's browser-automation tooling couldn't drive (scroll events reached
the page but produced no camera zoom — this map's camera may not expose
wheel-zoom at all) or a much longer walk than was practical here. The
mechanism is a direct, one-to-one reuse of cube content's own already-live
blend path (same `get_lod_pair_visibility`, same `stable_hash`, same
`recheck()`/`apply_visibility` call sites) through the same shared registry,
so the main net-new risk surface is the registry generalization and the
raycast-visibility filter above, not the blend math itself — but the
staggered-pop visual is still worth a direct look before calling this fully
closed.
