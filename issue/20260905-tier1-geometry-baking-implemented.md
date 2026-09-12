# Tier-1 Geometry Baking: Implemented

**2026-09-05** — implements the direction
[20260903-terrain-decor-render-unification.md](./20260903-terrain-decor-render-unification.md)
decided but deliberately left unscheduled: replacing `chunk_stage.tsx`'s
box-instancing and `decor_layer.tsx`'s per-variant GLB-instancing with real
per-chunk baked geometry, Minecraft's own block-mesh model as the literal
reference. That document is the fuller record of _why_ baking beats
instancing here (the draw-call-vs-vertex-memory tradeoff, the concrete bugs
that motivated caring about this) — this one records what changed while
actually building it, including decisions the original document didn't (and
couldn't) settle in advance.

## What actually landed

- `chunk_mesher.ts` now bakes real per-vertex box geometry directly (a
  literal 6-face — 5 for an open-bottom box — corner table, transformed by
  each surviving run's own min/size) instead of writing GPU instance
  matrices for a shared `TERRAIN_BOX`/`OBJECT_BOX`. Both the fine (per-run)
  and coarse (box-averaged) paths bake the same way.
- **Terrain and solid objects now bake into ONE merged mesh per chunk**, not
  two. The original discussion didn't mandate this — it only established
  that geometry should be baked, not instanced. Once baking made the
  open-bottom-vs-full-box distinction a per-run decision (`material_class
  === OBJECT ? 6 faces : 5`) instead of a per-mesh one, keeping two separate
  meshes had no remaining reason to exist: they already compiled the same
  shader (`inject_chunk_shader`), and merging them is more faithful to "one
  shared per-chunk mesh" than preserving the split.
- `chunk_stage.tsx`'s shader lost its `instanceMatrix` decomposition
  entirely. A baked vertex's `position`/`normal` are already real — the only
  per-vertex inputs left to bake are `a_local_pos` (the box's own canonical
  `[-0.5, 0.5]` corner) and `a_box_scale` (that box's own real per-axis
  size), which the UV-tiling and bevel formulas already needed and now read
  directly instead of reconstructing from `instanceMatrix`'s diagonal. This
  made the shader _simpler_, not more complex — worth noting since "bake
  real geometry" sounds like it should cost shader complexity, not remove
  it.
- Visibility/dither (the LOD blend mechanism) ports to a per-vertex
  attribute flip, same `discard`-based approach, same "flip a typed array
  and set `needsUpdate`" cost shape as the per-instance version it replaced
  — this was the one piece of the original discussion's risk list
  (`render/lod.ts`'s blend system) that turned out to need no redesign at
  all, just a wider array.
- `decor_layer.tsx`'s `DecorVariantMesh` now bakes every placement's own
  real, separately-transformed copy of its variant's GLB geometry into one
  merged mesh (`instanced_model.ts`'s new `bake_decor_instances`, using
  three.js's own `BufferGeometry.applyMatrix4` — needed here, unlike the
  cube baker's hand-rolled position math, because an arbitrary GLB's normals
  have no guarantee of staying axis-aligned under an instance's rotation the
  way a box's always does under scale alone).
- Cube content and decor still bake into **separate** meshes, on purpose:
  they compile genuinely different materials (procedural texture-array-tap
  blending vs. a real UV-mapped skin texture), so one merged draw call was
  never on the table regardless of baking. "One shared per-chunk mesh, no
  instancing" was never a claim that literally everything in a chunk becomes
  one GPU object — it's the claim that each own content class stops sharing
  one primitive across many instances.

## Fast-follow, same day: real decor raycasting

`camera/ray.ts`'s `resolve_hit_decor` — the exact-cell lookup added earlier
this session alongside user-placeable decor, back when decor had no real
geometry to hit-test against — is gone. Decor is now hit-tested with a real
`THREE.Raycaster` pass against its own baked mesh:

- `decor_layer.tsx`'s own `instanced_model.ts` type (`ModelInstanceTransform`)
  gained an `id: string | null` field — the placement's own objects/store.ts
  leaf id for a user-placed instance, `null` for a procedural
  (scatter.ts-derived, never-stored) one.
- New `decor_hit.ts`: a module-level registry of currently-mounted decor
  meshes (`register_decor_mesh`, called from `DecorVariantMesh` on every
  mount/rebuild — camera/input.tsx needs to query "every mounted decor mesh"
  from outside the render tree, the same reasoning objects/store.ts's own
  module-level Maps use instead of a React context) and `raycast_decor`,
  which runs `THREE.Raycaster.intersectObjects` against them and resolves a
  hit's own `faceIndex` back to a source instance (every instance in one
  baked mesh shares one source geometry, so the mesh's own total face count
  divided evenly by its own instance count gives a constant
  faces-per-instance stride) — and, through that, to a real id or `null`.
  A `null` (procedural) resolution is treated as no hit at all, so
  procedural decor stays exactly as transparent to hit-testing as it always
  was.
- `camera/input.tsx`'s new `resolve_hit_target` tries the real decor
  raycast first (bounded by the already-found terrain/object hit's own
  distance — decor can only be the right answer when it's genuinely closer
  along the ray), then falls through to `ray.ts`'s own (now decor-free)
  `resolve_hit_object` for solid objects. Split out of `ray.ts` itself
  because only `input.tsx` holds the live `THREE.Raycaster` a real mesh
  intersection needs — `ray.ts`'s own raycast stays deliberately THREE-free.

This was scoped as a deferred fast-follow earlier the same day, then done
anyway once discussed — not because the estimate was wrong, but because the
whole point of today was picking up exactly the things a prior session
punted on "귀찮아서" (too much hassle). Verified with a new
`decor_hit.test.ts` (registry + faceIndex resolution + procedural
transparency + max_dist bounding + unregister) and live in a real populated
scene: placing a tree, hovering it with the eraser (ghost appears exactly
on the model — cell-exact used to line up too for today's always-one-cell
decor, so this is a precision win for non-full-cell decor shapes later, not
a bug fix today), and erasing it, all resolved correctly with zero console
errors.

## What this does NOT change

- **Raycasting for cube content.** `chunk_raycast.ts` already derived hit
  boxes straight from `MeshRun`/`run_to_box`, independent of how they're
  drawn — untouched.
- **The terrain/decor ready-state desync** — at the time this section was
  first written, baking hadn't merged terrain and decor into one physical
  build step, and that was believed to be a separate, bigger, unplanned
  change. It happened anyway, the same day:
  [20260905-terrain-decor-scheduling-merged.md](./20260905-terrain-decor-scheduling-merged.md).

## Verification

No live device profiling was done — the tradeoff analysis in the original
discussion (instancing's per-instance-transform cost vs. baking's
duplicated-vertex cost, expected to net a win from fewer draw calls) remains
a projection, not a measurement, same as it was before this landed. What
_was_ verified: the full typecheck/lint/test suite (rewritten
`chunk_mesher.test.ts`/`chunk_bucket.test.ts`/`pipeline_integration.test.ts`
assert the baked vertex/attribute output directly instead of decomposing
instance matrices; new `instanced_model.test.ts`/`decor_hit.test.ts` exercise
the decor baking merge and its own real-mesh raycast), plus — once the
user's own local Postgres/API came up later the same day — a real populated
scene in a live browser: terrain, texture blending, bevel edges, trees, and
a placed-then-erased user decor tree all confirmed working with zero
console errors. The initial pass (before that infra was available) relied
on a synthetic WebGL compile-and-render smoke test instead (a synthetic
chunk baked, materialized through the real
`create_material()`/shader-injection path, rendered to an offscreen canvas,
confirming the shader compiled with zero GL errors) — superseded by the
real scene check once possible, not left as the final word.
