# Everything is a file: the atom, the palette, and what stays apart

**2026-09-13** — the record format that "everything is a file" has been
pointing at since [20260910-storage-model-precedents.md](./20260910-storage-model-precedents.md)
and [20260911-decor-identity-orientation-and-one-cell-format.md](./20260911-decor-identity-orientation-and-one-cell-format.md),
decided in one sitting rather than measured, because nothing about it is a
measurement question. The linear-memory checklist it waited on closed
today ([20260912-linear-memory-what-is-left.md](./20260912-linear-memory-what-is-left.md)).

## What "a file" means here

Not Unix's regular file. What the phrase borrows from Unix is the
namespace idea: different things are not made the same, but they are
named, owned and given lifetimes the same way. In pixelet that splits into
two planes, and the split is the first decision.

**The control plane is unified.** A resource — a terrain page, an object
band, a POI page, a mesh, a slab away at a worker — is a handle: a numeric
identity, an owner, a lifetime, a revision, a residency. This exists:
`PageTable`, `ChunkTable`, the pool's blocks and their refcounts,
`LittleSlabs`' ownership lanes. The scheduler sees descriptors — what a
job reads, writes and lends, at what cost and priority — and never a
payload. What is decided is that this vocabulary stays this narrow:
identity, ownership, lifetime, revision, residency. No universal
read/write/seek interface. Operations are lifted into the common layer
only when a third kind of resource asks for one the first two already
have, which is the discipline
[20260708-vision-and-scope-pivot.md](./20260708-vision-and-scope-pivot.md)
exists to enforce.

**The data plane is specialised.** A terrain page stays dense lanes, an
object band a sparse tree, a POI page point records, a mesh vertex and
index spans. Containers are not unified. What is unified is one level
down: the **atom** — the record for one placed thing, whatever it is — and
the **palette** it points into.

## The conventions every record shares

Read from the standards and adopted as they are, since they are not
questions this project has anything to add to.

- **Columns, named.** 3D Tiles 1.1 dropped its per-instance binary
  tables for glTF extensions: `EXT_mesh_gpu_instancing` (an instance is a
  row across accessor columns), `EXT_structural_metadata` (a property
  table is typed columns under a schema, with enums), `EXT_instance_features`
  (an instance names its metadata row). That is the shape XTP's column
  layer already has — `[keys, count, columns...]` — so a record is a
  column table and nothing else is invented.
- **Extensions are named columns with a required flag**, glTF's
  `extensionsRequired` and PNG's critical bit under one rule: a reader
  skips a column it does not know unless the header marks it required, in
  which case it refuses the record. Not a chain of headers: chains are
  for streaming parsers, and a tree with named columns needs none.
- **Identity is a `u64`.** glTF references by integer index, 3D Tiles
  feature ids are integers, and this project's own `leaf_id` was numbers
  spelled as a string. An atom's id is the FNV-1a of its address, or of
  its external id where it has one (POI already does this). Strings
  exist in two columns only: a name for people, a URI for the outside.
- **Orientation is ENU, and glTF is converted at the boundary.** The world
  is x east, y north, z up, right-handed; glTF is y up, right-handed,
  front +z. The conversion is one rotation about x, already in
  `instanced_model.ts`. A heading is radians about the up axis,
  counter-clockwise, zero at +x — three's `rotation.z` and what the
  scatter already writes; a compass bearing is a derived
  `90° − heading`. Pitch and roll do not exist yet and are reserved as an
  ancillary quaternion column, not put in the atom.
- **Positions are page-local `f32`** in world units, as the POI page's
  are: a page is 1024 units across, so the precision is a ten-thousandth
  of a unit, and the page's own address carries the rest.

## The atom

One record for one placed thing — a tree, a placed cube, a bare ground
cell, a car, later a character:

```
Placement            column table, one row per thing
  id        u64      FNV-1a of the address, or of the external id
  x y z     f32      page-local, world units
  span      u8       footprint in cells
  heading   f32      radians about up, counter-clockwise from +x
  value     u32      palette id
  ── ancillary ──
  scale     f32      the scatter's jitter
  rotation  f32×4    full orientation, when something needs it
  tags      str      JSON, as the POI page carries today
```

Everything that is drawn at a place is a row of this. What differs
between a tree and a cube is the palette entry `value` names, not the
row. Scattered decor is rows a seed produces and never stores; placed
decor is rows the object store holds; the terrain page's cells are the
same row in its dense encoding, where the address is implicit and only
the value is written per cell. The object store's `(corner, size, state)`
lanes are this record already, minus the heading, which is what the
unification costs it.

## The palette

One, global, itself a record:

```
Palette              column table, one row per kind of thing
  id          u32
  kind        enum   cube | ground | decor | ... 
  name        str
  model_hash  u64    FNV-1a of the model's content hash, 0 for a cube
  model_key   str    the object key in the bucket: models/<sha256>.glb
  extent      f32×3  measured at ingest, model units = world units
  span        u8     footprint in cells
  license     str
  author      str
  ── ancillary ──
  scatter     ...    the kind's placement rule, for a scattered kind
```

`cube_types`, `decor_types` and the `*_KIND` constants become rows. The
decor range criterion reads `extent` from here rather than from a constant
beside the bundle.

**The symbolic link.** A model is never inside a record. The palette row
points at it by content: the glb's sha256 names the object in the bucket,
the same bucket the POI pages are published to. Content addressing is what
makes the rest fall out — two people uploading the same model store one
object, an object never changes so it is cached forever, and the mutable
thing is the palette row that maps a name to a hash, which is a directory
entry. A user-uploaded model is ingested on the server: validated, its
extent measured (the same bounds pass that read the GLBs today), its
triangle count and licence recorded, a palette id issued from a user
range. The client never opens a model to learn what it is; the palette
says.

## What this does not do

- It does not merge terrain, objects and POI into one container or one
  index. Their representations are right for what they are.
- It does not add operations to the control plane. Handles, owners,
  lifetimes, revisions, residency; no more until a third resource kind
  repeats a need.
- It does not migrate anything. The formats it replaces are this
  project's own and nothing is deployed that reads them; the atom and the
  palette are written fresh, and the readers move to them.

## The order

1. `Palette` and `Placement` encoders and decoders in `bake/format.ts`,
   on the XTP column layer, with the extension rule.
2. The palette record replaces the constants; the decor bundles read
   their extent from it.
3. The object store's entries become `Placement` rows; scattered decor
   emits them.
4. The bucket path for models, and the ingest that fills a palette row.
5. Then the worker, which carries `Placement` rows and mesh spans across
   its boundary in the one form they now have.
