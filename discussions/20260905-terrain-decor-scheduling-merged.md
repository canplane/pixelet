# Terrain + decor merged into one build step

**2026-09-05** — same-day follow-up to
[20260905-tier1-geometry-baking-implemented.md](./20260905-tier1-geometry-baking-implemented.md),
finishing the other half of what
[20260903-terrain-decor-render-unification.md](./20260903-terrain-decor-render-unification.md)
originally named: _"one build step, one ready flag, no two states to
desync in the first place."_ Tier-1 baking unified _how_ terrain/objects
and decor each turn their own data into GPU geometry; it deliberately left
_whether they're the same build step_ alone. This closes that gap: a cross-
registry gate was drafted and rejected back on 2026-09-03 as premature
scaffolding relative to work not yet done — that work is now done, so this
is the real fix, not the gate.

## What changed

`chunk_stage.tsx`'s `Chunk` now builds and renders decor for every decor
kind alongside its own cube content, in the same `commit()`. Concretely:

- `ChunkStage` gains a `decor_bundles` prop. Since a hook can't be called a
  variable number of times per component (`useInstancedModelVariants` once
  per bundle in a `.map()` would violate React's rules of hooks), decor's own
  GLB assets load through a new `useAllDecorBundleAssets` (decor_layer.tsx):
  one hook call, flattening every bundle's own variants into one combined
  list, then re-slicing the single result back into per-bundle groups.
- `Chunk`'s `build_ref.current()` — already scanning/baking its own cube
  content — now also calls `build_decor_instances` for every bundle, using
  its own already-resolved `primary` LOD tier as decor's density key
  (decor never blends between tiers, so it only ever needed the one value,
  not the primary/secondary pair cube content uses). A new `decor_cache_ref`
  mirrors `bucket_cache_ref`'s own lifecycle exactly (populated in the same
  branch, for the same missing steps; cleared at the same points) so the two
  caches can never disagree about which steps are actually built.
- Rendering initially stayed two groups (see the addendum below for why
  that changed same-day): cube content's baked positions are chunk-local
  (need `Chunk`'s own offset translation), decor's were already
  page-absolute (`scatter_decor_candidates`/`build_decor_instances` return
  page-absolute coordinates by construction) — wrapping decor in that same
  offset would double-apply it. `Chunk`'s existing `group_ref` (already
  measured for its own `recheck()`) is reused for decor's distance/frustum
  test too; decor's own separate dummy measurement group is gone along with
  the component that owned it.
- `decor_layer.tsx`'s `ChunkDecor`/`PageDecor`/`DecorLayer` — each a
  separately-scheduled component with its own `ChunkLodControl`, own
  `tile_ref`/`objects_revision_ref`/`committed_*` bookkeeping duplicating
  what `Chunk` already tracked, own independently-computed distance/LOD
  guess — are deleted. The file keeps only pure build logic
  (`build_decor_instances`, `user_decor_pick`, `DECOR_DENSITY_BY_LOD_STEP`)
  and a rendering primitive (`DecorVariantMesh`), now called _from_ `Chunk`.
  This also retired the file's own duplicated `visible_pages`/page
  enumeration (its own doc comment had flagged this as "mirrors ChunkStage's
  own... duplicated rather than shared" — no longer duplicated once there's
  only one component enumerating pages/chunks at all).
- `world.tsx` composition collapses from `<ChunkStage>` + 4x `<DecorLayer
  bundle={...}>` siblings to one `<ChunkStage decor_bundles={DECOR_BUNDLES}
  />` (`DECOR_BUNDLES` a module-level array, not an inline literal —
  `useAllDecorBundleAssets` keys its own memoization off that reference
  staying stable).
- Removed a circular import as a side effect: `decor_layer.tsx` used to
  import `ChunkLodControl`/`use_lod_scheduler` from `chunk_stage.tsx` (for
  its own now-deleted `ChunkDecor`); `chunk_stage.tsx` now imports _from_
  `decor_layer.tsx` one-directionally.

## What this fixes, concretely

Before: up to 5 registrants per chunk position in `LodSchedulerRegistry`
(1 terrain `Chunk` + up to 4 `ChunkDecor`, one per decor kind), each with
its own `ready` flag, racing independently — decor (cheap, never deferred)
could visibly render before its own co-located terrain had. After: 1
registrant, 1 `ready` flag, decor and cube content committed together or
not at all. This is also a real scheduler-load reduction (fewer
`recheck()`/`commit()` calls per frame for the same visual content), not
just a correctness fix — a direct structural consequence of one control
instead of five, not a separate optimization pass.

## Verification

`deno task check`/`lint`/`test` all pass unchanged (138 tests — this
refactor is almost entirely React component/scheduling restructuring, with
little new pure-function surface, so live verification carried the real
weight here, as anticipated going in). Live, in a real populated scene
(server up with Docker/Postgres by this point in the day): terrain and
every decor kind (trees confirmed directly; boats/cars/trains share the
identical code path) render together correctly after a hard reload;
placing a tree via mode_bar.tsx's decor button, hovering it with the
eraser (ghost resolves via decor_hit.ts's real-mesh raycast, unaffected by
this refactor since `DecorVariantMesh` still registers itself the same way
regardless of which parent renders it), and erasing it all worked with zero
console errors — the same regression check run after the tier-1 baking
pass, confirming this deeper merge didn't disturb it.

## Addendum, same day: one group, not two

The "two groups" split above was this session's own design choice, not
something the user asked to preserve — decor's page-absolute coordinate
convention was itself just how an earlier session happened to build it.
Once asked directly, judged that confining decor to the chunk's own offset
group is the more correct structure (one coordinate convention for
everything a chunk renders, not a special case), and a safe path existed to
get there without touching the historically fragile part: `get_ele_at_local`
elevation sampling and `get_objects_in_cell` occupancy checks inside
`build_decor_instances` still operate on page-relative coordinates exactly
as before (unchanged); only the final `position` pushed into `by_variant` is
shifted by `-base_x * U_PER_CELL, -base_y * U_PER_CELL` at the last
possible moment, converting it to chunk-local right before it leaves the
function. `chunk_stage.tsx`'s `Chunk` then nests decor's own group inside
its existing offset `<group ref={group_ref}>` instead of rendering it as an
unoffset sibling.

This is a different mechanism from the double-offset bug a prior session's
own `lod_group_ref` comment described hitting (wrapping already-absolute
positions in an offset group on top of their own absolute value) — here,
exactly one thing applies the offset (the group), and the stored positions
were made relative to match it, not left absolute and wrapped anyway.

Verified the same way as the rest of this pass: `deno task check`/`lint`/
`test` all green, and live — terrain and decor still align correctly (no
positional drift), and place → hover → erase on a freshly-placed, spatially
isolated tree confirmed the raycast and removal still resolve correctly
under the new coordinate convention.

## Addendum, same day: the merge's own regression — a decor-only hover was
## rebuilding cube content too

Reported live: hovering the eraser over a placed tree (right after
hover_cursor.tsx's own real-decor-silhouette ghost, a separate same-day
change, made it obvious) showed the terrain/cube content for that spot
visibly flickering along with the decor ghost. Root cause was this merge's
own doing, not the silhouette ghost: `hover_cursor.tsx`'s
`set_virtual_hide(id)` bumps the object store's per-L2-column revision
(`objects/store.ts`) regardless of the target's kind, and `Chunk`'s single
`objects_revision`-keyed `useEffect` (added by this merge) reacts to *any*
bump by clearing `bucket_cache_ref`/`column_runs_ref` and fully rescanning/
rebaking cube content — even though `ObjectWorldLayer.paint()` never paints
a voxel for a decor entry in the first place (adapter.ts's own "decor never
paints a voxel of its own" comment), so a decor-only change can never
actually change what that rescan would produce. Before this merge, decor's
own `ChunkDecor` reacted to the same revision but only re-ran its own cheap
`build_decor_instances` call — the expensive terrain/cube rescan was a
separate component's concern entirely, so this exact bump never reached it.

Fixed by splitting the store's own per-L2-column revision into two
independent counters (`objects/store.ts`): `l2_column_revisions`
(cube-relevant — solid object place/remove/virtual-hide) and the new
`decor_l2_column_revisions` (decor place/remove/virtual-hide).
`set_virtual_hide` now looks up the target's own `kind` and bumps only the
matching counter (falling back to bumping both only if the object's
already gone from the store by the time it's unhidden — a rare edge case,
not a correctness gap). `chunk_stage.tsx`'s `Chunk` correspondingly splits
`build_ref` (cube content only now, returns `BuiltChunkBucket[] | null`)
from a new `decor_build_ref` (decor only, a plain `build_decor_instances`
call per bundle, no scan, no shared cache with cube content), and the
single structural `useEffect` splits into two: the original (still keyed
on the cube-only `objects_revision`, still clears+rebuilds *both* caches,
since a solid placement can occlude a procedural decor candidate) plus a
new decor-only effect keyed on `decor_revision` that touches only
`decor_cache_ref`/`layout.decor` — never `bucket_cache_ref`,
`column_runs_ref`, or `layout.buckets` — so `ChunkMesh` never remounts over
a decor-only change.

This doesn't undo the original fix this document describes: a solid
object's own change still rebuilds decor alongside cube content (occupancy
can shift what a procedural decor candidate resolves to), and decor is
still never committed without the cube content it sits on already having
been built for the same step — a decor-only change is the one case that
was always supposed to be cheap and wasn't.

Verified live via the dev-only `window.__PXL_RENDER_STATS__.mesh_mount`
counter (chunk_stage.tsx's own `ChunkMesh` mount-timing tracker): before the
fix, each eraser hover enter/exit on a placed decor entry incremented
`mesh_mount.count`; after, three full hover-enter/exit cycles on the same
tree left the count exactly unchanged, while the decor ghost itself still
rendered and updated correctly. `deno task check`/`lint`/`test` all green
(138 tests, unchanged).

## Addendum, same day: a second, unrelated flicker — the raycast boundary
## itself was unstable, independent of the rebuild bug above

After the fix above, live testing still showed a tree's hover ghost
alternating with its own normal (non-ghosted) render while the cursor sat
still — not the same symptom as the cube-rebuild flicker (that one showed
terrain content flickering alongside a *correctly* rendering ghost; this one
is the ghost/normal state of the tree itself flipping). Root cause is in
`decor_hit.ts`'s `raycast_decor`, unrelated to `objects/store.ts` or
`chunk_stage.tsx` entirely: the function rejects a decor hit with a strict
`hit.distance > max_dist`, where `hit.distance` is THREE's own
Möller-Trumbore triangle intersection against the baked decor mesh, and
`max_dist` is `chunk_raycast.ts`'s own, entirely independent closed-form
ray/AABB-slab intersection against a voxel-derived box (passed in via
`camera/input.tsx`'s `resolve_hit_target` as the already-found terrain/object
hit's own `t`). Near a decor object's own base — any tree's trunk touching
the ground it stands on, by construction — these two independently-computed
float64 distances land almost exactly together, and disagree only by pure
computation noise. A strict `>` comparison flips on essentially every frame
right at that boundary; a real mouse's own continuous sub-pixel sensor
jitter (present with no camera motion and no automation involved) walks the
disagreement across zero in either direction each frame, which reads as the
hover ghost itself flickering while the cursor appears stationary. This is
why the session's own browser-automation `hover` tool (a single discrete
mousemove) could never reproduce it directly — `update_hover()` only runs
from real `pointermove` events, and one event settles into whichever state
that one ray happened to land in, with nothing to perturb it again.

Fixed with a new tolerance constant, `SAME_SURFACE_EPS = 1e-3` world units
(`decor_hit.ts`), added to the comparison:
`hit.distance > max_dist + SAME_SURFACE_EPS`. 1e-3 is small enough that a
decor mesh genuinely occluded by terrain still loses (that gap is always at
least a real fraction of a cell, orders of magnitude bigger than this), and
comfortably larger than the float64 round-off between the two algorithms.
Two existing, purpose-built epsilons elsewhere in the codebase
(`objects/geometry.ts`'s `OVERLAP_EPS`, `camera/ray.ts`'s private
`SAME_EXTENT_EPS`) were considered and rejected as the wrong semantic fit
rather than reused blindly — this is a distinct concern (cross-algorithm
raycast-distance noise, not AABB-overlap noise or sub-cell quantization).

Verified with a new deterministic unit test
(`decor_hit.test.ts`, "tolerates float noise at the max_dist boundary
instead of flickering"): a synthetic box at a known analytic distance (9.5)
with `max_dist` set 0.0005 under that (inside the tolerance) still resolves
the hit; `max_dist` set 0.01 under that (outside the tolerance, a real
occlusion gap) still correctly resolves to `null` — the fix closes the
flickering boundary without swallowing genuine occlusion. Live, the
placement-preview ghost and the eraser-hover ghost on a freshly-placed tree
both still render as the real silhouette with no console errors.
`deno task check`/`lint`/`test` all green (139 tests — one new test added,
138 prior unchanged; lint's pre-existing 76-problem `xpute/core` baseline
unchanged).
