# Linear memory: the rule, what is done, what is left

A running record for the work of moving the web client's state out of the
JS heap and into the core memory. It is a checklist first and a discussion
second; the reasoning that needed its own document has one, and this points
to it. Updated as items close — the dates on each line are when they did.

## The rule

**Superseded 2026-09-14** by
[20260914-what-stays-in-javascript.md](./20260914-what-stays-in-javascript.md):
three.js and React no longer hold anything this rule left to them, and
the state it moved into lanes is the kernel's.

The JS heap keeps glue and nothing else: the objects three.js and React
require to exist (a `Mesh`, a `BufferAttribute`, a `Group`, a component), and
the handles that name a slot in linear memory. Everything the program
actually reasons over — positions, flags, distances, counts, instances,
entries, queues — is lanes in the core pool (`CORE_POOL`, one fixed
`WebAssembly.Memory`, `CORE_ALLOC` over it), so that it can move to WASM
without a second rewrite.

This is not a performance argument and is not to be judged by one. A
structure moves because the design says the client's state is linear; it
stays only where the design is not yet settled — characters and whatever
else has no decided record format — and a deferral is recorded here as a
deferral, not as a decision that the heap was fine.

Why the destination is WASM at all, and why three.js leaves with it:
[20260912-why-the-destination-is-wasm.md](../20260912-why-the-destination-is-wasm.md).
This checklist assumes that destination rather than arguing it.

Where the line runs, settled 2026-09-12 over the POI question: **a resource
the canvas reads every frame is linear memory, and a page our API emits
comes in that form (XTP) whoever the provider was** — the provider is
normalized on the server and does not reach the client. A live response
the UI shows once (a search, a reverse geocode) and the views that measure
the client (the ledger, the census) are glue. The test is who reads the
data each frame, not who produced it.

Two consequences that came up in practice and are now rules:

- **A lane set has one owner, and a consumer borrows it for the duration of
  a call.** With objects, a second reference kept the data alive; with
  pooled lanes it keeps a pointer to memory the owner will hand back. The
  decor field learned this (below): it kept the node's lanes to re-read on
  an origin shift, the node released them to the pool with its cache, and
  the next borrower's bytes drew as trees the size of a district. The fix is
  the standard one — write on arrival, keep only what you wrote, and make
  the shift a translation of what you hold — and it is the shape every
  consumer of lanes takes from now on.
- **A slot is dense and a handle is told when it moves.** Removing a slot
  swaps the last one into its place; the handle (a `ChunkNode`, say) gets
  its new index. Nothing keeps a slot index across a frame that could remove
  slots.
- **A browser call that takes a large canvas or image is an allocation**
  (2026-09-13). The rule above bounds what this code allocates; it says
  nothing about what a browser API allocates under a call. A Safari heap
  timeline read the page's JavaScript memory climbing from 1.3 to 3.8 GB
  while the live heap held 700 MB, of which 30 MB was objects — the rest
  was ArrayBuffer garbage WebKit made reading a 64 MB badge sheet back on
  every `drawImage` from it (and, before that, on every `texImage2D` of
  it). None of it was visible to the ledger, and none of it was in any
  line of ours. A canvas handed to the GPU or to another canvas is read
  whole; draw small, upload small.

## Done

- Core memory and the allocators: `BuddyAllocator` over the one memory,
  `CORE_ALLOC` as `TypedAlloc`, `Vector(ctor, {init_cap, alloc})`,
  `IndexedMap`. (Earlier; see
  [20260911-single-thread-core-and-when-to-spawn-a-worker.md](./20260911-single-thread-core-and-when-to-spawn-a-worker.md).)
- Chunk build data: composited volumes, column run sets, baked meshes and
  their vertex blocks, the coarse tiers' direct runs — all pool blocks with
  typed views, refcounted where two owners exist
  ([20260912-coarse-tiers-what-the-field-does.md](./20260912-coarse-tiers-what-the-field-does.md)).
- The object store's entries as lanes with a cursor, and the placed-object
  hit path off it.
- Decor instance matrices in the pool, one `InstancedMesh` per variant
  written per chunk run (`decor/field.ts`).
- **2026-09-12, decor instances.** One object per instance (~22,000 at a
  wide zoom, held twice) became eight lanes per (chunk, step, variant) in
  `decor/instances.ts`; the string id is not stored, only the store's own
  (band, packed position), derived back on a hit. The field writes a
  contribution as it arrives and never reads it again; an origin shift adds
  the integer delta to every live matrix. Commit c7e8080.
- **2026-09-12, the scheduler's view of every chunk.** The eight scheduler
  fields on ~2,500 nodes and the `Set` that gathered them became one slot
  per chunk in `sched/chunk_table.ts`: flags, origin, bounding sphere, world
  center, the two distances, pending-since, request, retained bytes. The
  per-frame measurement is one loop with one matrix (the anchor root's) and
  six plane dots per slot; the orders sort scratch slot lists; the memory
  budget's on/off totals are the table's own. Commit de9a8ce.

## Checklist

Ordered as agreed: A, then B, then C.

### A — the world's per-chunk state

- [x] Decor instances as lanes.
- [x] Scheduler lanes and the chunk table.
- [x] The decor field's own bookkeeping (2026-09-12). A bank's runs
      were slice objects in a Map keyed by a chunk name string, its free
      list a Map of arrays, and the field kept which banks each chunk was
      in as a Map of Sets. A bank's runs are an `IndexedMap` of chunk bases
      with start, capacity and count as lanes, its freed ranges two lanes,
      and a chunk's identity in the field is its base. Commit cd18b2f.
- [x] A band's overflow sub-voxels (2026-09-12). Entry objects on the
      band, and a map of maps built over them on every scan, became lanes
      on the band sorted by column at seal, read by binary search; the
      objects layer's claimed-voxel Set of strings is an `IndexedMap`
      scratch over packed addresses. Commit 01ecf42.
- [ ] Layout buckets — **deferred 2026-09-12**, in BACKLOG.md under
      parked. The vertex data is already a pool block; a `BuiltChunkBucket`
      is the typed views over it plus a refcount, and three.js's
      `BufferAttribute` takes exactly those views, which makes the handle
      the glue boundary itself.
- [x] The POI page (2026-09-12). The API's page is an XTP packet: x, y, z,
      category, rank and the id hash as lanes, the strings as `str` leaves
      read one at a time. The client copies it into the core memory and the
      index sweeps the lanes; a badge is keyed by the hash lane. The first
      record of "everything is a file". Commit 3dd631c.
- [x] `PageChunks`' node map (2026-09-12). The chunk table indexes slots by
      base, and a page's mount diff asks it. Commit 992fe7d.

### B — the world's per-page and per-column registries

- [x] `tile_map` and `poi_map` (2026-09-12). One `PageTable`
      (`modules/world/page_table.ts`): an `IndexedMap` of packed page keys
      in the core memory, what a page holds as a handle at its position, a
      revision React selects instead of a map identity. The POI index reads
      the table and rebuilds on the revision. What a POI _is_ stays an
      object until the record format — listed under the deferral below.
- [x] `AnchorRegistry.entries` (2026-09-12). An `IndexedMap` of L4 keys,
      the link count a lane, the group the handle. Commit 9b55d83.
- [x] Object store listeners (2026-09-12). One `ListenerTable` per
      registry: the key — a column base, a cell corner — in an
      `IndexedMap`, the callbacks as handles at the position; the cells a
      reindex touched are an `IndexedMap` scratch (`IndexedMap.clear`
      added). `placed_snapshots` stays: it is the identity-stable object
      `useSyncExternalStore` requires, a React glue cache holding a hover's
      worth of entries. Commit dd4836e.
- [x] Fetch scheduler (2026-09-12). Queued and running were two Maps of
      entries; they are one table — the key's 64-bit FNV-1a hash in an
      `IndexedMap` (a key is a string by contract, a search query included,
      and the stored key is compared on every hit), state, category,
      priority and clocks as lanes, the demand closure and its
      AbortController as handles.

### C — caches whose values are three.js objects

- [x] `ray_cache` (2026-09-12). The raycast composites, keyed by chunk
      base in an `IndexedMap`, last-use and band count as lanes, the stack,
      revisions and pooled volume as handles; least recently used evicted
      first. Commit f9ee08e.
- [x] The POI badge pipeline (2026-09-12). The billboard atlas's entries
      are slots: state, rectangle and drawn size as lanes, the shelves as
      lanes, the draw closure the one handle; a consumer keeps the slot
      index and reads the rectangle through it, since a repack moves the
      rectangle and not the slot. The billboard field's per-sprite position
      and size are lanes and its sheet windows are indexed by atlas slot.
      The badge caches are two tables — a place by the 64-bit hash of its
      id, a cluster by its count — holding the measured width and the slot,
      with the id string as the handle for the collision check and for
      eviction (the string is the POI record's, see the deferral).
- [x] Glue, left as is, by the rule: `gltf_cache` and `asset_store`
      (URL string to a loaded three.js scene, geometry or material — the
      asset loader's own table, a dozen entries), `highlight_boxes` (one
      `BoxGeometry` per size tier), the billboard field's sprites and
      materials. Their keys are URLs and tier sizes and their values are
      what three.js requires to exist.

### Left as glue, by the rule

- `placed_snapshots` in the object store: the identity-stable object
  `useSyncExternalStore` requires for the hovered entry — React's contract,
  not three.js's. The scene-side glue (`hover_cursor`, `location_puck`) is
  glue for the other reason: it is the `Mesh` and `Group` three.js draws.
- The dev instrumentation — the ledger's records, the draw census, the
  trace — is measurement of the client, not its state.

### Deferred, by design

- Characters and anything else without a decided record format. Not
  before the format exists.
- ~~The POI record~~ — done 2026-09-12, see the checklist above.

## After this: everything is a file

The follow-on, not to be started under this checklist: one top-level record
format for objects, terrain and decor, with the per-kind differences in
extension headers and glTF-sourced things reached by external reference.
The lanes above are being shaped so that they become that record's fields
rather than something to convert again. Recorded here so the target is
written down; the design gets its own document when it starts.
