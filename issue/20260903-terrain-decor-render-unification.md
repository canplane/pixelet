# Terrain/Decor Render Pipeline: What's Unified, What Isn't, and the Decided Direction

**2026-09-03** — investigation into visible LOD/scheduling glitches (chunks
stuck permanently empty, decor rendering before its co-located terrain chunk,
"far" content resolving before "near" content during camera movement). Several
real bugs were found and fixed; the underlying architectural question — should
terrain and decor be one rendering pipeline — was resolved as a decision, not
implemented this session.

## What was actually broken, and fixed

All four were found by reading code, not by guessing from symptoms, and are
now covered by tests/typecheck/lint passing plus doc comments at their own
call sites:

- **`chunk_stage.tsx`'s recheck-priority sort had no real distance to sort
  by.** A `ChunkLodControl`'s `distance2` used to start at `Infinity`
  (a true sentinel), so a whole batch of chunks mounted in the same frame (a
  big pan/zoom) ranked by Set-registration order — which tracks nothing
  about camera position — until each one got its own first real `recheck()`.
  Fixed by seeding `distance2`/`in_frustum` from a new
  `estimate_initial_chunk_view` (`render/chunk_lod.ts`) at construction time,
  for both `chunk_stage.tsx`'s `Chunk` and `decor_kinds.tsx`'s `ChunkDecor`.
- **An "urgent tier" in the recheck sort backfired under sustained camera
  motion.** An earlier fix (since reverted) gave `blend_active`/`settling`
  controls unconditional priority over merely-`full_recheck`-eligible ones —
  correct for a single frame, but a continuously moving camera keeps minting
  new `blend_active` chunks at its leading edge, an unbounded stream that can
  occupy that tier forever and starve an older, still-pending, genuinely
  closer chunk. Fixed by sorting purely by `(in_frustum desc, distance2 asc)`
  — safe now that every control seeds a real distance instead of `Infinity`.
- **The real cause of chunks that never recovered (60+ seconds, not a
  scheduling delay):** `chunk_stage.tsx`'s `build_ref.current()` calls
  `with_temporary_composite`, which returns `null` when
  `composite_chunk_layers` finds nothing painted by any layer for that exact
  chunk key — a real, if rare, transient case (a batch of chunks racing their
  first build around the same moment can momentarily read a layer as empty).
  The code didn't check for this distinctly from "scan deferred"; it fell
  through and returned an **empty bucket array**, which `commit()` treats as
  a fully successful build — `ready: true`, `pending: false`, permanently,
  with nothing to show and no retry path. A whole neighboring batch hitting
  this at once produces a contiguous hole or ring, not one stray chunk —
  matching what was reported. Fixed by returning `null` (same as a deferred
  scan) instead of falling through, so the scheduler retries it.
- **`decor_kinds.tsx`'s `ChunkDecor` kept accumulated LOD state
  (`committed_lod_step`, `committed_tile`, `committed_objects_revision`,
  `has_measured`) as closure-local `let`s inside the `control` `useMemo`
  factory**, unlike `chunk_stage.tsx`'s own `Chunk` (which keeps its
  equivalent state — `bucket_cache_ref`, `column_runs_ref`, etc. — in
  `useRef`s outside the memo). The control legitimately gets rebuilt when
  `initial_view.in_frustum` flips (e.g. across an anchor rebase); every
  rebuild silently reset the closure locals to their initial values, replaying
  the "first measurement, no easing" path and discarding any real settled
  state — visually a pop, then a snap back to empty. Fixed by moving that
  state into refs, mirroring `chunk_stage.tsx`'s own pattern exactly.

## What's actually unified today, and what isn't

Only the **scheduling/LOD layer** is shared. The **geometry/build pipeline**
and the **ready state** are not, and that's the direct cause of the remaining
known issue below.

Unified:

- One `LodSchedulerRegistry` / `LodSchedulerProvider` recheck-commit loop,
  one shared per-frame work budget (`LOD_WORK_BUDGET_MS`,
  `LOD_MAX_CHUNKS_PER_FRAME`), one priority order — `chunk_stage.tsx`'s
  `Chunk` and `decor_kinds.tsx`'s `ChunkDecor` are equal registrants.
- The distance/frustum primitives (`chunk_distance_to_camera`,
  `update_frustum`, `estimate_initial_chunk_view`, `APPROXIMATE_CHUNK_RADIUS`
  — all in `render/chunk_lod.ts`).
- Page/chunk enumeration (`visible_pages`'s filter logic, `in_range_chunks`)
  — same inputs, same result set for terrain and decor, though
  `visible_pages` itself is duplicated inline in both `ChunkStage` and
  `DecorLayer` rather than extracted (documented in `decor_kinds.tsx`'s own
  doc comment as a deliberate choice, not an oversight).
- The LOD tier vocabulary (`module/world/lod.ts`'s `LOD_STEP_BY_LEVEL`,
  `get_lod_guide`) — decor's density falloff is keyed to the same tiers
  terrain hands off at, just without the blend/dither machinery (decor
  never blends between tiers).

Not unified:

- **Geometry/render technique.** `chunk_stage.tsx` instances one shared unit
  box (`TERRAIN_BOX`/`OBJECT_BOX`) at many transforms, with a custom shader
  (rim light, per-instance color/layer-tap texturing). `decor_kinds.tsx`
  instances arbitrary per-variant GLB geometry, one `InstancedMesh` per
  (chunk, decor kind) with a plain material, no rim light. Two structurally
  different rendering techniques, not two configurations of one technique.
- **Build step.** Terrain's is real per-cell voxel scanning
  (`voxel_scan.ts`), run-length meshing, occlusion culling
  (`chunk_mesher.ts`'s `is_run_occluded`), and per-LOD bucket caching
  (`chunk_bucket.ts`) — expensive, and can genuinely take multiple frames
  (`scan_deferred_ref`/`build_deferred_ref`'s one-shot defer). Decor's is a
  synchronous scatter-placement pass (`scatter.ts`) with no scan, no
  occlusion, no cache — cheap, never deferred.
- **`ready`/`pending` state.** Two fully independent `ChunkLodControl`s per
  physical chunk location — one for terrain's `Chunk`, one per decor kind's
  `ChunkDecor` (up to four: tree/car/boat/train). Nothing couples them.
  **This is why decor can visibly render before (or after) its co-located
  terrain chunk has anything to show** — reported directly by the user
  during this investigation, confirmed structurally true by code, not yet
  fixed.
- **Storage.** Decor is never stored — `scatter_decor_candidates` recomputes
  it on demand from terrain + a fixed seed every time. Placed objects live
  in `objects/store.ts` as `ObjectLayerEntry` with `kind: "solid" | "air"`.
  There's no unified "item" model yet covering procedural decor, user-placed
  decor, and placed blocks alike.

## The ready-state gate that was drafted, then rejected

A small fix was drafted for the `ready`-state gap specifically: extend
`LodSchedulerRegistry` with a `ready: Set<string>` of fully-qualified chunk
keys, have `chunk_stage.tsx`'s `Chunk` publish its own key into it whenever
`layout.ready` becomes true (a `useEffect` keyed on that React state), and
have `decor_kinds.tsx`'s `ChunkDecor.commit()` refuse to actually apply
(`set_instances_by_variant`) until the co-located terrain key is present —
returning `false` (stay pending, retry) otherwise.

This was implemented far enough to typecheck, then explicitly reverted and
not merged. Reasoning, stated directly by the user: a fix scoped to "make
two independent ready states agree" is scaffolding built around the fact
that two independent ready states exist at all — exactly the condition the
decided direction below eliminates by construction (one build step, one
`ready` flag, nothing to synchronize). Landing the patch now would mean
deleting it later once real unification happens, for no benefit in between
beyond a few days/weeks of one specific desync symptom not showing. See
`AGENTS.md`'s own "That doesn't license implementation-shaped architecture"
addition (same date) for the general principle this follows: implementation
size/effort is not a valid reason to prefer a smaller ad hoc fix over the
structurally correct one — the only real costs are server spend and
end-user device load, neither of which this patch was actually about.

## The decided direction (not started this session)

Full geometry-level unification, Minecraft's actual block model as the
literal reference (not just inspiration): every chunk bakes into **one**
merged vertex buffer (or a small fixed set — opaque/transparent), where each
block or decor item — cube or not — contributes its own real geometry
transformed into chunk-local space, not a shared primitive restyled per
instance. This is a real rendering-engine rewrite, not a wire-in:

- Replaces `chunk_stage.tsx`'s box-instancing build path
  (`build_chunk_bucket`, `ChunkInstanceMesh`) and `decor_kinds.tsx`'s
  per-variant GLB instancing (`DecorVariantMesh`) with one geometry-agnostic
  per-chunk baking pipeline.
- The rim-light shader (`inject_instance_shader`, written against
  per-instance attributes `INSTANCE_RGB_ATTR`/`INSTANCE_LAYER_TAPS_ATTR`/
  `INSTANCE_VISIBLE_ATTR`), occlusion culling (`is_run_occluded`, currently
  run-based), the LOD bucket cache (`chunk_bucket.ts`), and the blend/dither
  transition system (`module/world/lod.ts`'s `get_lod_pair_visibility`) all
  currently assume one shared instanced box and need redesigning against
  merged arbitrary geometry instead.
- The `ready`/`pending` desync above disappears as a structural consequence
  — one build step per chunk means one `ChunkLodControl` per chunk, full
  stop, not a gate keeping two states in agreement.
- Storage unification (decor becoming an `ObjectLayerEntry` `kind`) is a
  separate, independent piece of work — the baking pass can take procedural
  decor candidates (`scatter_decor_candidates`'s own output) as an
  additional input without requiring decor to be stored first. Worth doing
  eventually for user-placed decor to work at all, but not required to do
  the geometry unification above.

**Status: decided, explicitly not scheduled for this session** ("귀찮아서
안 한다", direct user call, 2026-09-03) — not because of implementation size,
which is not a valid reason per the principle above, but because the user
chose not to start it today. `decor_kinds.tsx`'s own "LONGER-TERM DIRECTION"
doc comment carries the same decision; this file is the fuller record of how
it was reached, per AGENTS.md's own convention for that.

## Does baking actually reduce device load? (instancing vs. baking tradeoff)

Asked directly: if geometry unification retires `InstancedMesh` entirely,
what happens to the performance instancing exists to protect? Baking does
not win on every axis — it trades draw calls for GPU memory/vertex
duplication, and that tradeoff should be stated plainly rather than assumed
away:

- **Instancing** uploads one copy of a geometry (a tree GLB, say) to the GPU
  once, then stores a small per-instance transform for every occurrence — a
  forest of 1,000 trees costs one geometry plus 1,000 small transform
  records.
- **Baking** appends each occurrence's own real geometry into the merged
  buffer — the same forest costs the tree's full vertex data duplicated
  1,000 times over. Terrain's own unit box is cheap to duplicate (12
  triangles), so this cost is concentrated in decor's own GLB geometry, not
  terrain's.

Net assessment: **likely still a win**, because mobile/typical GPUs are
usually bottlenecked by draw-call count and state changes rather than raw
vertex throughput. Today's per-chunk draw calls (terrain + objects + up to
four separate decor-kind `InstancedMesh`es = up to six) collapse to one or
two (opaque/transparent) under baking — a reduction that tends to dominate
frame time more than the added vertex volume costs. Minecraft itself is the
existing proof this trade works even on constrained hardware (mobile ports),
baking non-cube blocks with no instancing at all. The project's own existing
LOD thinning (`DECOR_DENSITY_BY_LOD_STEP`, terrain's own LOD tiers) already
bounds the worst case — full density only ever applies to the small, close
region, never baked across the whole visible area at once. Not measured
live; a real profile once baking exists would confirm this rather than
assert it.

## Interim fix shipped without waiting for unification: eraser hover preview

While `ready`-state unification is deferred, one specific symptom in the
same area _was_ fixed this session, and the fix was checked against the same
"does it survive unification" test before landing: the eraser tool's hover
preview needs the real target to actually stop rendering (so whatever is
behind/below it shows through), with the existing translucent ghost overlay
sitting on top of that — not a ghost merely laid over a still fully-opaque
real object.

Two implementations were compared for this specific narrower problem:

- **Per-instance visibility/alpha**, tagging `voxel_scan.ts`/`chunk_mesher.ts`
  output with the source object's id so one specific `InstancedMesh`
  instance could be toggled. Rejected: this depends entirely on
  `InstancedMesh` having individually addressable instances — a baked chunk
  has no such concept, so this would be pure throwaway work relative to the
  decided direction above.
- **Excluding the object from the paint/composite step** (`objects/store.ts`'s
  new `set_virtual_hide`, read by `render/layer/objects/adapter.ts`'s
  `ObjectWorldLayer.paint()`), triggering a normal chunk rebuild through the
  exact same `objects_revision` path a real `place()`/`remove()` already
  uses. Chosen: "the paint/composite layer can be asked to temporarily treat
  one item as absent, then rebuild" is a concept that survives baking
  unchanged — baking still goes through the same paint step, it just bakes
  the result differently. Scoped to `get_objects_in_cell`/`get_objects_near`
  staying untouched (collision, decor occupancy) — the object hasn't
  actually been removed, only its render.

This is the concrete shape of the "structure over ad hoc" test in practice:
not "which is cheaper to build today" but "which concept still exists after
the real fix lands."

**Real bug hit while landing this, worth recording on its own:** the object
paint step (`ObjectWorldLayer.paint()`) is shared by two different
compositors — `with_temporary_composite` (the real GPU-mesh build) and
`with_raycast_composite` (hover/hit-test resolution, `ray.ts`'s
`resolve_hit_object`). The first implementation filtered
`get_object_entries_in_l2_column` itself, which both compositors read from —
so hiding the hovered object also hid it from the _raycast_ that resolves
`hovered_object_id` in the first place. The moment the object hid, the
raycaster could no longer hit it, hover cleared, the hide lifted, the
raycaster hit it again, hover set again — an infinite loop, live-crashing
the canvas (`Maximum update depth exceeded`, `chunk_stage.tsx:730`).

Fixed by threading an explicit `opts?: { for_raycast?: boolean }` through
`WorldLayer.paint()` / `composite_chunk_layers()`, and giving `ObjectWorldLayer`
two entry points — `get_object_entries_in_l2_column` (real, unfiltered;
used for `revision()` and any raycast paint) and the new
`get_render_object_entries_in_l2_column` (`set_virtual_hide`'s exclusion
applied; used only for a non-raycast paint). The general lesson: a
cosmetic/render-only exclusion is never safe to apply anywhere the same data
also drives _detecting_ the condition that triggers the exclusion — check
every consumer of a shared read path before filtering it, not just the one
motivating the change.

### A fade/multi-slot system was built to mask the remaining overlap, then reverted

`set_virtual_hide`'s own hide and this ghost's own appearance are two
independent state changes — the ghost is instant (same React commit as
hover starting/ending), the real object's hide/un-hide isn't (it has to
propagate through `chunk_stage.tsx`'s `Chunk` — its own
`objects_revision`-triggered `useEffect`, a re-render, then its own rebuild
— a few React commits, not one). This produces a real, if brief (a few
milliseconds), overlap on entry and a matching brief gap on exit.

A full fix was attempted: ease the ghost's own opacity up/down over a short
window instead of snapping, which meant tracking more than one
simultaneously-hidden object (an outgoing target fading out can overlap
with a newly-entered one fading in) — `set_virtual_hide` widened from one id
to a `Set<string>`, plus a whole per-target slot lifecycle
(`EraseGhostSlot`, one independently-animated ghost mesh per slot). This
surfaced a new problem of its own: sweeping the pointer across several
objects faster than the fade's own settle time accumulated multiple
simultaneously-hidden real objects, which read as its own kind of wrong.
Bounding that would have meant adding a further rule (cap outgoing slots to
one, evict old ones immediately) — a patch on top of the patch.

**Reverted.** `useLayoutEffect` was also considered, as a way to collapse
the whole chain into one synchronous pre-paint pass, needing no fade at all
— rejected because `Chunk`'s own rebuild effect calls `build_ref.current()`,
whose `scan_deferred_ref`/`build_deferred_ref` machinery exists specifically
to let an expensive scan/build defer to a calmer frame instead of blocking
the current one; a layout effect cannot defer, so this would trade a
cosmetic overlap for a real risk of a blocked/dropped frame on an expensive
rebuild — and that effect isn't scoped to the eraser preview alone, so the
blast radius would cover every real `place()`/`remove()` too.

**Decision: accept the overlap as-is.** `set_virtual_hide` is back to a
single `id: string | null`, and the ghost is back to appearing/disappearing
instantly with no fade. A few milliseconds of the real object and its own
ghost both being visible (or a few milliseconds of neither, on exit) is a
genuinely minor cosmetic artifact; the fade/slot system that would have
masked it was real, load-bearing complexity (a widened store API, a
per-target animation lifecycle, a slot-count cap) for that alone. Per the
same principle as the ready-state gate above: don't build scaffolding
— fade or otherwise — around a problem the eventual real fix doesn't need
addressed at all, and don't let "this is achievable" stand in for "this is
worth it" when the thing being achieved is this narrow.

## Why this matters for the eventual Rust/WebGPU port

At the time `AGENTS.md` said not to volunteer that port — it was parked,
and TS + Three.js was the direction for finishing the MVP; the port has
since become the priority
([20260912-why-the-destination-is-wasm.md](./20260912-why-the-destination-is-wasm.md)).
The unification decided above is the kind of design choice that is made
for its own correctness, so a later rewrite (Rust + WebGPU or anything
else) carries the design over instead of rediscovering it.

## Status 2026-09-13

The decided direction landed two days later
([20260905-tier1-geometry-baking-implemented.md](./20260905-tier1-geometry-baking-implemented.md)),
and the instancing question in "does baking actually reduce device load"
was reopened and answered differently on 2026-09-10: decor draws from one
instanced mesh per variant written per chunk (commit af81d24), because
the measured cost was the per-chunk bake of copies rather than the draw.
