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
