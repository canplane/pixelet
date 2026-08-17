# Object Placement Silently Failing at Elevation — Unresolved

**2026-08-17** — investigation into a reported failure to place objects at
moderately elevated terrain (mountains, on the order of hundreds to a couple
thousand real-world meters). No code changed; this is a report, not a fix,
and the root cause is **not yet found**.

## First hypothesis — ruled out

The initial report read as "stacking has a height ceiling; past it, clicking
does nothing." That pointed at a first investigation into whether
`sparse_tree` storage has a fixed-size range that silently rejects
out-of-range writes.

It doesn't. Neither the sparse-tree implementation
(`packages/pxl/shared/src/spec/space/sparse_tree/{tree,mem}.ts`) nor its
consumer (`apps/web/src/modules/world/objects/store.ts`) bounds the Y
coordinate — `cells` is an unbounded `Map<string, ObjectTree>`, and
`sparse_tree.set()` only bounds the size *tier* (`OBJECT_LAYOUT.level`),
never position.

That first pass instead found a real but unrelated reach limit —
`INTERACT_RADIUS_U * 1.5` (~112 m) in `apps/web/src/modules/camera/ray.ts`,
a straight-line 3D cap on the click raycast itself, which silently returns
`null` and biases toward biting vertically first because looking up covers
that distance faster than looking across flat ground. This is real and
undocumented as a side effect, but **the user clarified it is not what they
were seeing**: the hover/candidate-block preview visibly appears and tracks
correctly at the elevations in question, meaning `ray_pick()` is succeeding.
The failure is downstream of the raycast, not the raycast itself.

## Second pass — the write path is provably magnitude-independent

Traced the full chain from the same `hovered` coordinate `update_hover()`
already computed — `handle_tap()` in `camera/input.tsx` passes it as-is to
both `find_overlapping_object()` and `place()`
(`store.ts`'s own comment confirms this is deliberate: *"i/f is taken as-is
... never re-derived or re-snapped here"*), so hover and placement share one
coordinate object, not two independently computed ones that could disagree.

`place()` (`store.ts:343-375`):

```ts
export function place(i: IVec, f: FVec, size: f64, color_id: ObjectColorId): void {
  const coord: PVF3 = { i, f };
  const level = Math.round(Math.log2(size));
  const is_whole_cell = level >= OBJECT_LAYOUT.max;
  const [ix, iy, iz] = cell_index(coord);
  const key = cell_key(ix, iy, iz);
  let tree = cells.get(key);
  if (!tree) { tree = new_tree(); cells.set(key, tree); }
  const val = object_color_num_id(color_id);
  if (is_whole_cell) {
    sparse_tree.fill(tree, val);
  } else {
    sparse_tree.write(tree, coord, val, level);
    sparse_tree.compact(tree);
  }
  save();
  reindex_cell(ix, iy, iz);
}
```

The default object size takes the `is_whole_cell` branch
(`OBJECT_LAYOUT.max = CELL_SHAMT = 0`, and `Math.log2(1) = 0`), which calls
`sparse_tree.fill()` — a `mem.data.fill(val, 0, mem.head)` that **never reads
the coordinate at all**. For that common case a write cannot fail based on
elevation; only `cell_index()` (which cell the object lands in) can differ,
and that's exact `BigInt` floor-division, no floats involved.

For sub-cell sizes, `sparse_tree.write()` descends the tree using only the
low 3 bits of the packed leaf key (`OBJECT_LAYOUT.dep = 3`) via exact BigInt
bitwise ops — magnitude-independent regardless of how large the coordinate is
before truncation.

The only two coordinate-magnitude limits found anywhere in this chain:

- `ivec.X/Y/Z` truncating to signed 24/24/16-bit lanes
  (`packages/pxl/shared/src/spec/space/pvf3/core.ts`) — the ~78 km Z bound,
  already irrelevant.
- `get_local_world_origin()` (`apps/web/src/modules/world/frame/coord.ts`)
  masking off the low 12 bits of every lane via `L4_BASE` — a ~9.8 km
  boundary, also irrelevant.

Both are three to four orders of magnitude past where the symptom actually
occurs. (Unit note: `U_PER_CELL = 1`, `M_PER_U ≈ 2.389` at the equator, more
off-equator — so "a couple thousand meters" of real elevation is only
roughly 400–1000 `u`, nowhere near either bound.)

`mem.ts`'s `set()`/`resolve()`/`fill()`/`compact()` were read in full: the
only early-returns are legitimate no-op optimizations ("this slot already
holds `val`") and the already-known level/tier bound — nothing else branches
on coordinate value.

## What's left unexplained

Static reading cannot go further than this without runtime values. Three
live possibilities remain, none confirmed or ruled out from code alone:

1. **`find_overlapping_object()`** (`objects/collision.ts`) returns a false
   positive at the hovered cell specifically at these elevations — no
   concrete mechanism found (it consumes the same `hovered` coordinate used
   for the preview), but not disprovable without real values either.
2. **A subscription-timing issue**, not a numeric one: the write and
   `reindex_cell()` succeed, but the chunk containing that cell fails to
   re-run its build effect on the very next frame at these specific
   coordinates. `chunk_stage.tsx`'s `objects_revision` gate is wired to fire
   on every `reindex_cell`, and no elevation-dependent gap was found in it,
   but this class of bug often doesn't show up from reading alone.
3. **Float32 GPU-instance-matrix precision** in
   `render/layer/pipeline/chunk_mesher.ts` — the only place in the pipeline
   where object Z is written into a `Float32Array` as an absolute,
   non-chunk-relative value (`box.min[2] + sz_z*0.5`), rather than the exact
   BigInt math everywhere upstream. By magnitude alone (float32 relative
   error ~6e-5 at ~1000u) this shouldn't be enough to make objects invisible,
   but shader precision qualifiers (`mediump` vs `highp` on some
   GPU/driver combinations) weren't audited and could amplify it —
   unconfirmed.

## Third pass — the write succeeds, so is it a whole band not rendering?

The user then confirmed the write itself does succeed (the object exists in
storage) but doesn't render, and floated a more specific mechanism: maybe an
entire Z-band of the chunk column silently isn't scanned/meshed once it's
disconnected from where terrain normally reaches — i.e. maybe the mesh
pipeline decides which bands of a column to build from terrain height or a
band list frozen at chunk-mount time, and a later write into a genuinely new
band (above local terrain, where terrain itself never allocated one) never
gets picked up.

Traced `volume.ts`, `compositor.ts`, `chunk_mesher.ts`/`voxel_scan.ts`, and
`chunk_stage.tsx`'s rebuild trigger in full. **This doesn't hold up either:**

- `get_or_create_band()` (`volume.ts:150-173`) is a plain, lazy, unbounded
  insert keyed by exact `band_z` — no terrain dependency, no pre-allocated
  band list. `min_band_z`/`max_band_z` are written but never read anywhere in
  the codebase (dead bookkeeping).
- `MAX_RAY_BANDS = 8` (`compositor.ts`) only gates whether an
  already-fully-composited volume gets **cached** for the hover-raycast LRU
  (`acquire_raycast_composite`). The render path (`with_temporary_composite`)
  calls `composite_chunk_layers()` directly — no cache, no cap, band count
  irrelevant.
- `objects_revision` changing (from `reindex_cell()`) sets
  `column_runs_ref.current = null` in `chunk_stage.tsx` (line 667),
  forcing a **fresh** composite + `scan_all_column_runs()` on the next build —
  not a reuse of a stale/cached Z range. There is no code path that derives a
  "previously known" band range and reuses it after a revision bump.
- `ObjectWorldLayer.paint()` (`render/layer/objects/adapter.ts:43-`) pulls
  every object entry currently indexed under that XY (L2) column **at any
  Z** straight from the live store — not a fixed/precomputed band loop.
- Terrain's own paint function explicitly documents itself as built to have
  no Z cap: *"sparse signed-Z bands can extend through every actual peak, so
  no z_top clamp can cut mountainous terrain or raycasts flat"*
  (`render/layer/terrain/adapter.ts`).

By this trace, a newly placed high-altitude object should be picked up on the
very next `objects_revision`-triggered rebuild, band-wise. So "an entire band
silently isn't drawn" is not supported by the code as it reads today.

## Where this leaves it

Three static passes have now traced: the write path (clean), the coordinate
magnitude/precision (clean, several orders of magnitude of headroom), and the
band/mesh-scan path (clean, deliberately Z-agnostic by design). None of them
explain the symptom. Two leads haven't been checked yet and are more
plausible than anything ruled out so far:

1. **The hit-test itself may be resolving to the wrong point at altitude** —
   not a "no hit" (already ruled out; the hover preview tracks correctly) but
   a hit whose Z doesn't match what's visually shown, e.g. if the raycast
   composite hits the wrong band or a stale composite near terrain at a steep
   slope. Worth checking what `hovered`'s actual Z value is at repro time
   versus where the object visually seems to preview.
2. **Occlusion culling** — `chunk_mesher.ts`'s `is_run_occluded`/
   `is_topmost_run` (lines ~145-176) intentionally drops instances it judges
   hidden. The code explicitly excludes the `OBJECT` class from culling logic
   per an existing 2026-08-16 bugfix comment, so this should already be
   handled — but it's the one place in the pipeline that *intentionally*
   drops instances, so it's worth confirming live rather than trusting the
   comment, especially at chunk boundaries.

## Recommended next step

Static analysis is exhausted across three passes; this needs runtime
instrumentation to disambiguate further. Logging, at a real repro at
elevation: `handle_tap()`'s resolved `hovered` coordinate (camera/input.tsx),
whether `place()` actually reaches the `sparse_tree` write, and the Z value
of the mesh instance (if any) that `chunk_mesher.ts` produces for that cell
on the next rebuild — would show directly whether this is a coordinate
mismatch or a culling drop. Not done yet; needs a go-ahead, and ideally a
repro on the device/location where this was originally seen.
