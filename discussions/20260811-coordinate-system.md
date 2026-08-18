# Coordinate system and floating origin: why it is built this way

**2026-08-11**, reconstructed from git history — the reasoning behind PVF3, the
L4 anchor registry, the three-tier camera frame, and the 3857/4326 adapters.

[ARCHITECTURE.md](../ARCHITECTURE.md) describes *what* these are. This document
covers *why*, and what the design went through to get there.

---

## Eight independent justifications

No single reason carries this design on its own. Each of the following would be
sufficient motivation by itself, and they all point the same way:

1. **GPU float32 precision.** At Web Mercator's full extent (2×10⁷ m), one
   float32 ULP is about 2 m; float64's is 3.7 nm. Local coordinates have to be
   kept small for float32 to be safe on the GPU, which is what floating origin
   is for.
2. **BigInt has zero drift under repeated operations.** Accumulating float64
   with `+=` can in theory drift over a very long session. Negligible in
   practice — but "unproven" was the objection, not "observed".
3. **The L4 grid bounds the cost of an origin update** to the number of live
   anchor cells, rather than the whole scene. The earlier design moved every
   object.
4. **Fixing the camera at the origin and splitting rotation from translation**
   turns "which coordinate frame is this in" into a tree-nesting choice rather
   than a math problem.
5. **The guard bit absorbs transient overflow at the antimeridian wrap**, not
   just SIMD headroom — stated explicitly in `3857.ts`.
6. **`Z_SCALE` is fixed per L3 page on purpose**, deliberately coarse. Mercator's
   latitude-dependent vertical distortion could be corrected per point, but then
   terrain, objects, and baking would each land on slightly different scales and
   fail to line up. Consistency between subsystems was chosen over precision.
7. **The L5 (shard) slot is reserved ahead of need**, so server-side distribution
   later doesn't force a coordinate redesign.
8. **AR/VR was anticipated.** Splitting the view into three frames was partly
   about a later XR transition. On desktop and touch the camera is fixed and the
   world moves; in AR/VR the device genuinely moves with 6DOF and the world is
   fixed. With content already separated by which invariance it needs, switching
   *who moves* becomes a swap rather than a rewrite. **No WebXR code exists — this
   is design motivation only, not implementation.**

## The three-tier camera frame, and the combination it can't express

Translation and rotation each being variant or invariant gives four theoretical
combinations. The tree has room for three:

| Combination | Where it lives | Actually used for |
| --- | --- | --- |
| Both invariant | `view-frame` (no hook touches it) | Screen-fixed HUD/gizmos — no real content yet, debug markers only |
| Rotation variant only | `world-frame` (`useFrameRotation`) | **In real use** — `environment.tsx` reads `world_frame_ref.rotation.z` to spin the sky with bearing; celestial bodies are "infinitely far", so translating them is meaningless |
| Both variant | `local-world-frame` (nested inside `world-frame`, `useFrameTranslation`) | All actual world content — terrain, markers |
| **Translation variant, rotation invariant** | **no slot** | Unimplemented |

The missing one is the interesting case: POI place labels, which should follow a
position but stay upright regardless of bearing — the same counter-rotation
pattern Google Maps and Mapbox use for labels. `poi/layer.tsx` and `poi_item.tsx`
exist, but there is no text-label feature yet, so the need hasn't surfaced.

**Why the slot doesn't exist:** `local-world-frame` is nested *inside*
`world-frame`, and there is no way to inherit a parent's translation without its
rotation.

If it's needed, two options — the first is preferred because it stays consistent
with the existing "hooks are independent, you just choose a group" pattern:

1. Add a sibling group to `world-frame` and attach `useFrameTranslation` to it
   independently.
2. Multiply per-object by the inverse of `world-frame`'s current rotation. This
   is per-object math rather than structure, which cuts against the design.

## History

**2025-12-23 (`d0b1dc9`) — the first floating origin.** `origin`/`originOffset`
were plain float64 in a Zustand store; crossing a `RESET_THRESHOLD` of 1125 m
moved world, camera, and target together inside one `useFrame`. `originOffset`
accumulated with `+=` — mutable state, theoretically drift-prone, almost
certainly harmless in practice. Neither PVF3 nor AnchorRegistry existed. With
only a handful of objects in the scene, moving everything at once was fine at
that size.

**2026-01 (`361c796`, "full platform rewrite") — PVF3 and AnchorRegistry arrive
in the same commit,** designed as a pair: PVF3 handles correctness, L4 handles
scalability.

**2026-03~04 — Z-up unification and basis transpose.** Three separate
transformation concerns were pulled apart into three separate layers in this
period:

- **Y-up → Z-up** (`5d4c50c`, 2026-04-03, 26 files): before this, camera
  pose/ground-align/orbit sync, markers, highlighters, and beacons each carried
  their own "legacy Y-up adaptation". Flipping the whole app to Z-up made ENU
  (east/north/up) map straight onto the render engine's axes and deleted the
  scattered helpers. Y-up conversion survives only for GLTF asset loading, since
  assets are mostly authored Y-up.
- **PVF3 basis transpose** (`pvf3/projection.ts`): converts between the per-axis
  `[[x_i,x_f],[y_i,y_f],[z_i,z_f]]` representation and the lane-packed
  `{i: IVec, f: FVec}` one. Deliberately domain-agnostic — pure scalar math, no
  assumption of a coordinate system. Metric scaling lives outside it, in the
  adapters.
- **Local ↔ absolute** stays with the L4 anchor system (`ivec.sub`).

**Later — a `useFrame` ordering bug, fixed by merging.** `useFrameTranslation`'s
registry rebase was once two independent `useFrame` subscriptions. Their
execution order wasn't guaranteed, so the anchor and the local frame could
momentarily disagree on screen. Merging them into one callback made the update
atomic. Recorded in the code comment.

## Note on level sizes

Level extents have changed over the project's life — the page was 128 u before it
became 1024 u, and the chunk 32 u before 64 u. Current values live in
`indexer.ts` (`L2_SHAMT` 6 → 64 u, `L3_SHAMT` 10 → 1024 u, `L4_SHAMT` 12 →
4096 u, L5 reserved at 16 → 65536 u but commented out). Anything quoting other
numbers is stale; read the constants, and note that some derived tuning constants
(`modules/world/terrain/config.ts`) carry their own re-derivation notes tied to
the page size change.

## Related

- Why BigInt was chosen, and what the abandoned Rust port would have done about
  its cost — [20260708-vision-and-scope-pivot.md](./20260708-vision-and-scope-pivot.md#corrections-to-earlier-readings)
- Why PVF3 stayed in `pxl` rather than moving to `xpute` — same document,
  "Why `xpute` and `pxl` are separate packages"
