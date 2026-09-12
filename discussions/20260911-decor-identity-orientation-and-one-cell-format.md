# Decor identity, orientation, and one cell format

**2026-09-11** — the draw census (sched/draw_census.ts) put decor at 87% of
the triangles the GPU draws. Answering "hide the cars from LOD2 on" turned up
a wider gap: decor has no convention for what an asset _is_, how big it is,
or which way it faces. This records what exists, what Minecraft does about
the same questions, and the direction settled on: one cell format for cubes
and decor, with a palette and a variable extension. Not implemented yet, and
deliberately parked — see the last section.

**Terms.** A **resource ID** (`rid`) is an asset's canonical name: a
namespaced string such as `pxl:car/sedan` or `pxl:block/red`, fixed once
given and never reused — Minecraft's resource location (`minecraft:oak_log`).
It is what a stored value ultimately names; the numbers a runtime or a cell
uses are derived from it, never the other way round.

## What started it

The census, frame mean over a 20 s pan:

| tag   | draws | triangles |
| ----- | ----: | --------: |
| decor |    22 |     1.45M |
| lod2  |    16 |      119k |
| lod4  |    17 |       44k |
| lod1  |     8 |       28k |
| rest  |    83 |       ~5k |

1,686 decor instances at ~860 triangles each. Per model:

| models                   | triangles each |
| ------------------------ | -------------: |
| cars (5, Kenney car kit) |    2,032–2,474 |
| trains (4)               |    1,130–1,742 |
| boats (4)                |        156–386 |
| trees (9)                |        126–336 |

A car costs ten trees and every kind gets the same density ladder by tier
(1, 0.5, 0.25, 0). That is the immediate problem; the rest of this document
is why it could not be fixed with one number.

## What decor is today

| concern               | now                                                                 | what goes wrong                                                                                                                    |
| --------------------- | ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| identity, scattered   | `bundle index : variant index`, from array order                    | inserting a variant renumbers every key after it, and changing `variant_count` reshuffles every placement the RNG picks            |
| identity, user-placed | `decor_types = { tree: 228 }`, one value per kind                   | the variant and heading come from a hash of the position (`user_decor_pick`), so editing the tree list changes trees users planted |
| variant count         | a literal in `*_kind.ts`, asserted against the asset list at import | the same fact kept twice                                                                                                           |
| size                  | the model's own units, scale 1 (± jitter when scattered)            | nothing declares a footprint. User decor collides as one cell (`DECOR_SIZE`); cars and trains are 2.4–3 cells long                 |
| heading               | `random` or `align_transport` per kind                              | no stated forward axis or world zero; only the Y-up to Z-up fix is written down                                                    |
| level of detail       | one density table for every kind                                    | no per-kind policy, no triangle budget                                                                                             |
| anchor                | base at the model origin, centred on X/Y                            | a comment, not a rule                                                                                                              |

## glTF orientation, checked on the assets

The glTF 2.0 spec: +Y is up, +Z is forward, −X is right; the front of an
asset faces +Z. The Kenney models follow it — every car has
`wheel-front-*` nodes at +Z and `wheel-back-*` at −Z, every train
`wheels-front` at +Z, and all of them are long along Z:

| model family | extent X (width) | extent Z (length) | height Y |
| ------------ | ---------------: | ----------------: | -------: |
| cars         |          1.3–1.5 |           2.4–3.0 |  1.2–1.6 |
| trains       |          1.1–1.4 |           2.4–2.6 |  1.9–2.0 |
| boats        |          1.8–2.7 |           2.4–3.9 |  0.8–3.9 |
| tree         |              1.0 |               1.0 |      2.4 |

The world is Z-up with north at +Y and east at +X. `instanced_model.ts`
turns a glTF asset upright with +90° about X, which sends glTF +Z to world
−Y: **at heading 0 every car faces south today.** It goes unnoticed only
because `align_transport` picks either direction along a road.

The convention: **heading 0 faces north (+Y)**, and the asset basis maps
glTF (x, y, z) to world (−x, z, y) — up to up, front to north, and the
asset's right (−X) to east. That is a proper rotation (determinant +1), so
it is one quaternion baked into the instance matrix, replacing the current
one.

## What Minecraft does

Checked against minecraft.wiki (Chunk format, Java Edition protocol/Chunk
format, Block states, Anvil file format, Pre-flattening data values).

**Identity.** Before 1.13 a block was a numeric ID and a 4-bit data value;
the ID space was 256 and grew to 4096 when Anvil added "a 4-bit data layer".
The Flattening (17w47a) removed numeric IDs for namespaced resource
locations (`oak_planks`) and split variants that had lived in the data value
into their own entries. The wiki records what changed, not why; the limits
above are what the old encoding could express. Commonly cited reasons —
mods colliding in one global number space, and four bits being too few for
the states blocks wanted — are not from that source.

**Storage.** Per 16×16×16 section, a palette of the block states used there
and one index per block. "All indices are the same length", the minimum
that fits the largest index "and then set to a minimum size of 4 bits".
Indices do not span the 64-bit words they are packed in. On the wire a
section is single-valued (0 bits), indirect (4–8 bits, local palette), or
direct (15 bits and up, global state IDs). Strings live in the palette,
once per section; the runtime works on numbers.

**Orientation** is block state, not a separate field: `facing`
(north/east/south/west/up/down), `rotation` 0–15 for standing signs, heads
and banners, `axis` x/y/z for logs.

**Multiple cells** are handled two ways. A multi-block is several blocks
that each carry their part: beds (`part` head/foot), doors and tall plants
(`half` upper/lower), double chests (`type` left/right). Anything that is
not grid-bound is an entity — boats, minecarts, paintings — with a real
position and yaw and its own bounding box.

**Extra data** does not go in the grid. Block entities (a chest's contents,
a sign's text) are a separate list per chunk, keyed by position.

## The direction

**One cell format for cubes and decor.** As in Unix everything is a file,
every placed thing is a cell value of the same shape; splitting the storage
model by kind is rejected. Where one needs more, the value points at a
variable-length extension — the IPv4 options / IPv6 next-header shape, and
the same split Minecraft makes between the block grid and block entities.

| layer         | Minecraft                           | pixelet                                                                                                         |
| ------------- | ----------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| cell value    | index into the section palette      | index into a palette, cubes and decor alike                                                                     |
| palette entry | block state `{id, properties}`      | `{rid: "pxl:car/sedan", facing, part, ...}`; a cube is `{rid: "pxl:block/red"}`                                 |
| extension     | block entity, keyed by position     | an optional record the cell points at, for the rare entry that carries more (a scale, a text)                   |
| runtime       | dense state IDs assigned at startup | dense IDs assigned from the manifest at load; the mesher expands a palette to a material table once per rebuild |

This removes the shared byte range cubes and decor currently split (cubes
128–144 and 255, decor 228) and the position-hashed variant: a stored value
names an `rid`, so editing an asset list moves nothing.

**Two placement classes**, following Minecraft's block/entity line:

- **Block-like** — user-placed decor. Stored, grid-bound, a `facing` of four
  (N = +Y, E = +X, S, W). A footprint of more than one cell is a set of
  cells each carrying its `part`, like a bed.
- **Entity-like** — scattered cars, trains and boats. Not stored, not on the
  grid, a real position and yaw. This is already what they are; the
  convention names it.

Scattered trees sit between: never stored, so free yaw or sixteen steps cost
the same.

**A manifest row per variant.** Kind-level policy (density per tier, the
last tier drawn, a triangle budget, eligible cells) sits on the kind.

```ts
{
  rid: "pxl:car/sedan",       // canonical, never reused
  kind: "car",
  model, skin,
  footprint: { w: 1, d: 3 },  // cells; collision, hover ghost, placement
  anchor: "base_center",
  forward: "+Z",              // the asset's front in glTF space
  yaw: "align_transport",     // "free" | "quad" | "align_transport"
  fit: "footprint",           // scale the bounding box to the footprint at load
  placeable: false,           // offered in the hotbar
  tris: 2032,                 // written by a script from the glb, checked in CI
}
```

## Palette scope

Where the palette lives is the open structural question. What decides it:

1. **It travels with its data.** The palette has to be persisted and loaded
   with the cells that index it, so it belongs to the unit that is stored
   and fetched together.
2. **It is resolved per rebuild.** The mesher turns a palette into a material
   table once per bake; the scope should be what a rebuild covers, or the
   table is either rebuilt for nothing or split across bakes.
3. **Few distinct states per scope.** Fewer states, fewer bits per cell —
   Minecraft sizes a section at 4,096 cells for this.
4. **Cheap growth.** A new state can force a repack to more bits; that should
   touch a small scope.

Minecraft's section satisfies all four because it is its storage, meshing
and invalidation unit at once. pixelet's equivalent is the **L2 chunk**
(the XY column): objects/store.ts keys entries and revisions by L2 column,
and a chunk rebuilds as one. An L3 page is too large for criterion 2; an
L2 band (64³) matches criterion 3 but not 2, since a chunk rebuilds all its
bands together; an octree node is too small for the palette to pay for
itself. **L2 chunk is the recommendation, not yet the decision.**

## Not migrating

Placed objects live in `localStorage` today and nothing is on a server. The
project is in development and already carries a disclaimer that saved data
may be discarded, so the format changes without a migration.

## Decided

- **One cell format** for cubes and decor; storage is not split by kind.
  Anything a cell needs beyond its value hangs off a variable extension it
  points at.
- **Palette per L2 chunk.** Confirmed on the four criteria above.
- **Variable cell width** per palette, as Minecraft does: the fewest bits
  that index the palette, repacked wider when a new state needs it.
- **Heading 0 faces north (+Y);** assets face glTF +Z per the spec and are
  mapped glTF (x, y, z) to world (−x, z, y).
- **No migration.** Development data may be discarded; the disclaimer is
  already in place.
- **Interim, until the manifest exists:** cars and trains are drawn at the
  nearest tier only (`NEAREST_TIER_ONLY` on their bundles in
  decor_layer.ts); every other kind keeps the shared ladder.

## Open

- The extension record: its shape, and what points at it.
- Footprints of more than one cell for user decor — a part per cell, and
  what erasing one part does to the others.
- ~~Per-kind level of detail as manifest policy, replacing the interim.~~
  Withdrawn 2026-09-12 (follow-up below): one projected-size criterion, a
  kind brings its extent.

## After the interim, and what the GPU cost belongs to

The census split by kind, the same afternoon, with cars and trains at the
nearest tier only:

| tag         | draws | triangles |
| ----------- | ----: | --------: |
| decor:tree  |     9 |      941k |
| decor:car   |     5 |      435k |
| lod2        |    21 |      139k |
| lod4        |    44 |      110k |
| decor:train |     4 |       82k |
| decor:boat  |     4 |       25k |

Missed frames while moving fell from 19% to 10% and GPU p95 from 13.5 to
11.8 ms, but decor's total barely moved: in a dense area two hundred cars
sit inside the nearest tier alone, and trees took the top place. The
scattered kinds are test content, so tuning them one by one buys nothing
lasting.

What does last is content-independent, and it is one problem, not three:
the storage model (what a placement is), the level-of-detail system (how
far each kind is drawn and at what density, against a triangle budget), and
low-detail meshes for the tiers where a full model is a few pixels. Culling
belongs with them — decor is drawn for the whole loaded world while 41
chunks were in the frustum. The remaining GPU bottleneck is decided there,
when this convention is taken up again.

**Correction, the same evening: the GPU is bound by pixels, not
triangles.** The trace of the next session showed GPU time in two clusters —
6–9 ms, and 16–18 ms with almost nothing between — the second only while
moving and at the same triangle count as the first; from the lowest fifth
of frames by triangles to the highest, GPU p50 rose only 6.7 to 8.3 ms.
Switching the default supersampling (dpr 2, 8.5 Mpx) to SMAA (dpr 1,
2.1 Mpx) took GPU p50 from 7.0 to 4.7 ms and p95 from 17.4 to 7.6 ms with
_more_ triangles on screen. The 16–18 ms cluster was the GPU waiting whole
vsyncs once its queue backed up, not work.

And most of that per-pixel cost was an accident: the post-processing
composer's `multisampling` defaults to 8 in @react-three/postprocessing 3,
while scene.tsx's comment said it was left at 0. Every frame rendered into
an 8x MSAA buffer on top of supersampling. Turning it off, at the same
dpr 2, gave GPU p50 5.9 and p95 8.1 ms. What it had also been doing was
smoothing distant sub-pixel geometry, which came back as shimmer without it.

The composer now runs 4x MSAA — the conventional count for a forward
renderer — which answers that shimmer as a coverage problem. A
camera-reprojection TAA was tried first and closed; see
[20260911-taa-tried-and-closed.md](./20260911-taa-tried-and-closed.md).
Decor's triangles, while still worth a policy, are
second to all of this. The paragraph above stands for what decor is; it is
wrong about what the GPU was spending.

## Why this is parked

This came out of performance work, not ahead of it: the cooperative
scheduler and the ledger cleared the CPU side, the draw census then put the
remaining missed frames on the GPU and 87% of its triangles on decor, and
deciding how far to draw cars exposed that decor had no convention at all.
It is a rendering and content-data question.

It is **not** a prerequisite for the memory model and workers in
[20260911-single-thread-core-and-when-to-spawn-a-worker.md](./20260911-single-thread-core-and-when-to-spawn-a-worker.md).
Those need big and little memory, a central scheduler that tracks buffers
that are away, and bake inputs and outputs in transferable form; the cell
format is not on that list. The two do point the same way — a
palette-indexed, bit-packed cell array is exactly the shape that sits in
linear memory and crosses to a worker whole, where the sparse tree the bake
reads today does not — but that makes the format a later beneficiary of the
memory work, not something that must come first.

So the order is: the interim above for the GPU now, the memory model and
the worker next, and this convention after, once the extension record and
multi-cell footprints have been thought through. Building the manifest
before those two are settled would likely mean building it twice.

## Follow-up, 2026-09-12: the definition, and how far to draw

Two things settled in conversation while the memory work was going on,
recorded here so the parked question resumes from them.

**The definition.** Decor is the non-voxel layer and cubes are the voxel
layer, and that heterogeneity is the right line to draw between them — not
"trees versus terrain", not "placed versus generated". A cube is a cell of
the volume the scan and the mesher read; a decor instance is a model at a
point that paints no voxel. Everything else about decor (identity, footprint,
orientation, the extension record) follows from its being the layer that is
not the volume, and any rule that treats a tree as a kind of cube is wrong
from the start.

**How far to draw.** Decor is not to be made larger to survive distance.
The rule is the one Google Earth and Google Maps apply to 3D buildings:
past some distance a thing is drawn low-poly, and past a further one not at
all. The distance is taken from what those clients do — the altitude at
which, looking straight down, a building of a given size stops being drawn
— and applied isotropically as a distance from `camera_center`, so the same
building disappears at the same range whether it is beneath the camera or
out toward the horizon. Two things this fixes at once: it gives the
per-kind level of detail (the open item above) its policy, and it makes the
cutoff a calibrated number rather than a tuned one.

What exists today is already the shape of it. `DECOR_DENSITY_BY_LOD_STEP`
is a distance rule — the tier criterion turns distance into a step, and the
table turns a step into a density that reaches zero at step 8 — so the
cutoff is a value in that table, not a new mechanism. What is missing is
the calibration, and a low-poly tier per kind between "full" and "none",
which is the manifest policy the interim stands in for. Neither is to be
done under the memory checklist
([20260912-linear-memory-what-is-left.md](./20260912-linear-memory-what-is-left.md));
this stays parked as above, one decision shorter.

**One criterion, not one per kind.** The open item above — "per-kind level
of detail as manifest policy" — is withdrawn. A kind registering its own LOD
thresholds is the wrong shape: what decides whether a thing is drawn in
full, drawn low-poly, or not drawn is how large it is to the eye, and that is
a property of the thing's size and the distance, not of its kind. The size is
its extent — the volume it occupies in cube terms, or the bounds of its
polygons — and the criterion is that extent projected to the screen: within
the eye's resolution, how much of a unit does it cover. Small enough that its
absence makes no visible difference, and it is not drawn; still large enough
to read, and it goes low-poly; large, and it is drawn in full. A tree and a
car and a building are then the same rule at different sizes, and a kind
brings its extent, never its thresholds.

This is the criterion the terrain tiers already use — screen-space error
against a pixel threshold, derived from the viewport and field of view
(render/lod.ts, `set_lod_projection`) — so decor takes the same one rather
than a second. The distance rule in the previous paragraph is this rule
stated for a given size: the altitude at which a building of a given extent
stops being drawn is the range at which that extent falls under the pixel
threshold, which is why the calibration against Google Earth and Maps is a
calibration of the threshold and not a table per kind. How those clients
represent the building meshes themselves at each range — what a low-poly
building is, when it becomes a footprint, when it is gone — is the
reference to read when the low-poly tier is built.
