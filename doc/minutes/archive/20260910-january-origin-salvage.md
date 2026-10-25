# January origin salvage: what survived the first pixelet architecture

**2026-09-10** — a retrospective salvage of the raw January 9–21 design notes,
written before the April WASM/runtime pivot and before the current application
shape existed. The source notes were exploratory conversations, not an approved
specification; this document records only the ideas that can be compared against
what was later built.

This is not a backlog. It is the missing first chapter before
[20260708-vision-and-scope-pivot.md](./20260708-vision-and-scope-pivot.md),
[20260708-concurrency-model.md](./20260708-concurrency-model.md), and
[20260909-april-checkpoint-salvage.md](./20260909-april-checkpoint-salvage.md).

---

## The January system in one picture

The earliest design was already trying to solve four distinct problems at once:

1. represent a planet-scale editable world without GPU-coordinate precision
   loss;
2. stream and render it in bounded spatial units;
3. normalize messy external GIS/API data into a compact canonical
   representation;
4. build a portable Rust/WASM runtime around that world, eventually including
   networking and distributed ownership.

The first three produced durable mechanisms. The fourth expanded into a "spatial
OS" metaphor that outran the browser and the product.

The important distinction in hindsight is therefore not _old vs new_. It is
**domain invariants that survived contact with the implementation** versus
**runtime machinery invented in advance of a demonstrated requirement**.

## What survived essentially intact

### One canonical world coordinate, local rendering coordinates

The 2026-01-09 note treated a 64-bit integer world coordinate as the single
source of truth and explicitly separated it from floating-origin rendering. The
exact bit layout and physical cell sizes changed, but the invariant did not:
absolute world identity must not depend on the renderer's small local float
frame.

Today's PVF3/IVec representation, L4 anchoring and floating-origin camera are
the mature descendant. The full later reasoning is in
[20260811-coordinate-system.md](./20260811-coordinate-system.md).

### A spatial hierarchy aligned to different costs

January already separated levels by job rather than by visual naming:

- a large network/storage page,
- a smaller render/meshing chunk,
- a logical cell/block unit,
- finer detail below it,
- and a higher slot intended for distribution/sharding.

The sizes and names moved repeatedly, but `Page`, `Chunk`, the cell hierarchy,
and the reserved higher-level shard slot survived. What proved useful was not a
particular `1024 m` or `4 m` number; it was **letting storage/network, meshing,
and world identity have different granularities while deriving them from one
address space**.

### Terrain/base data and sparse user mutation are different representations

The January design described a dense 2.5D base layer and a sparse 3D object
layer, with object state overriding/falling back to the base. The present code
has changed the concrete storage and rendering machinery considerably, but the
provenance split remains: externally sourced terrain and sparse user-authored
world mutation are not the same source even when they share a cell/material
vocabulary and converge at rendering.

The old "negative space" idea also survived in a less grand form: an explicit
AIR/erase value can override terrain rather than absence always meaning
"fallback to base".

### Page-oriented baking

The January notes proposed baking elevation and semantic ground data into a
page-scale canonical artifact before rendering. That idea survived source swaps,
format changes and several renderer rewrites. OSM/GLO-30 later became
Overture/GEDTM30, but those providers terminate at the same kind of internal
boundary: source-specific ingress is converted into pixelet-owned page data.

This is one of the clearest cases where the original problem statement was right
while nearly every implementation detail around it changed.

### Flexible ingress, static egress

A 2026-01-21 note named the principle explicitly: **"Flexible Ingress, Static
Egress."** External JSON/WKB/provider data may be irregular; once admitted into
the engine it should be normalized into a compact, stable internal vocabulary
rather than leaking provider schemas through the system.

The original plan tied that rule to Rust, `simd-json`, a slab allocator and a
binary-only pipeline. Those mechanisms did not survive as requirements. The
semantic boundary did. The 2026-09 Overture migrations are a stronger validation
of the principle than the January proposal was: Buildings/Transportation/Base,
Places and Divisions changed source vocabulary while the canonical terrain/POI/
geocode consumers remained comparatively stable.

## What survived after changing form

### Packed Int64 -> PVF3 + hierarchy + floating origin

The first `x24/y24/z14` packed-word layout was a concrete answer too early. Cell
sizes, lane widths and hierarchy extents all changed. What survived was the
separation between a deterministic world-space representation and a renderer
frame small enough for GPU floats.

The lesson is to preserve the invariant, not the bit allocation.

### L4 Region as a server shard -> L4 Anchor, L5 reserved shard

January assigned L4 directly to endpoint sharding. That was premature: there was
no deployed sharded world to justify binding the coordinate hierarchy to server
topology. The current design uses L4 for anchoring/floating-origin locality and
keeps the higher L5 slot reserved for distribution if it ever becomes real.

Again, the future requirement was not deleted; it was moved out of a level that
already had a present-tense job.

### Flat binary / zero-copy ambition -> XTP as a mechanism, not a law

January assumed the page, IPC and eventually GPU path should remain a flat
binary buffer end-to-end. That produced useful instincts — typed arrays,
relative addressing, avoiding gratuitous object graphs — and eventually XTP.

What did _not_ survive is the rule that every payload must use that format.
Terrain currently justifies XTP well; small semantic records such as POI do not
need to adopt it merely for symmetry. If WASM/worker/SAB boundaries later make a
shared relocatable representation valuable, XTP can be re-evaluated from the
measured boundary rather than from the January premise.

### Page fault / interrupt metaphor -> ordinary fetch, bake, commit

The early runtime described a missing page as `PAGE_FAULT`, routed through a
Kernel/Host/Engine interrupt path. The useful behavior underneath the metaphor
was simple and survived:

```
spatial demand
    -> missing page
    -> fetch / produce canonical page
    -> commit
    -> consumer resumes
```

The browser implementation does not need an IVT, trap frame or kernel to express
that flow. Current fetch schedulers, page stores and bake pipelines are the same
problem stated in the substrate's native vocabulary.

## What was over-generalized and should stay retired

### The browser as a miniature operating system

The January `Host / Kernel / Engine / App` topology introduced a hypervisor-like
Host, a Rust/WASM Kernel, an Engine as policy layer, page faults, ISR handlers,
interrupt-vector ranges, slab-managed shared memory and a secure binary tunnel.
It was internally coherent, but most of those abstractions existed because the
system was being made to resemble an OS rather than because pixelet had a
measured need for them.

The July concurrency retrospective explains the substrate mismatch: browser
workers remain event-loop driven; SAB does not create native preemption; and the
reference application that motivated the design, Figma, is fundamentally
cooperative. See
[20260708-concurrency-model.md](./20260708-concurrency-model.md).

### Signal/interrupt machinery as a separate control plane

Later April work made this more concrete, but the same tendency begins in the
January IVT design. The September April audit reached the cleaner conclusion:
request/response and fire-and-forget operations do not need two OS-like planes
merely because an interrupt table made that split aesthetically attractive. The
command-plane contract is salvageable; the signal plane is not.

See
[20260909-april-checkpoint-salvage.md](./20260909-april-checkpoint-salvage.md).

### Security by hiding the client runtime

January's "secure tunnel" and later WASM-obfuscation direction assumed more
security value from keeping host/application code blind to the core. That did
not survive the later trust-model review: a client holding the binary cannot be
made trustworthy by obscuring it, and that premise conflicts with the BFT-style
"do not trust clients" posture explored later.

The trusted client-server model is the current baseline. Transport security
belongs to real platform primitives, not a hand-rolled encrypted kernel tunnel.

## The interesting recurrence: cooperative scheduling came back for the opposite reason

January wanted a scheduler because an OS-like runtime was part of the proposed
architecture. July correctly parked that architecture.

In September scheduling returned, but the causal chain was reversed:

```
January:
architecture premise
    -> scheduler/runtime machinery

September:
feature growth
    -> desktop slowdown + mobile stalls
    -> measurement / task ledger
    -> explicit frame and resource budgets
    -> cooperative scheduling
```

This distinction matters. The current scheduler is not evidence that the old
Spatial-OS architecture was right after all. It is evidence that **the mechanism
became justified once a real execution-budget problem appeared**.

The current model is deliberately smaller: CPU stepped work yields against a
frame budget; I/O has its own concurrency/priority mechanism; measurement stays
read-only instead of becoming an adaptive control loop. See
[20260909-cooperative-scheduler-and-task-ledger.md](./20260909-cooperative-scheduler-and-task-ledger.md).

## What January predicted surprisingly well

The raw notes were speculative, so none of these should be retroactively treated
as implemented achievements in January. Still, several problem boundaries were
identified unusually early:

- planet-scale absolute coordinates vs local GPU coordinates;
- separate storage/network/render granularities;
- dense external terrain vs sparse editable world state;
- explicit absence/override semantics;
- canonical page baking before rendering;
- typed/binary representation as a useful systems boundary;
- provider-specific ingress vs engine-owned canonical vocabulary;
- a future shard level that must not require redesigning coordinates;
- runtime work eventually needing explicit resource budgets.

The architecture was wrong mainly where it promoted these instincts into a
complete general-purpose runtime before their actual execution constraints were
known.

## The rule carried forward

The useful lesson from January is not "return to the original design." It is the
opposite:

> Keep the invariant when implementations keep rediscovering it. Keep a
> mechanism only when a concrete boundary makes it pay for itself.

That is the pattern behind the current codebase's later consolidations: features
create pressure, the pressure exposes symmetry or a broken invariant, and only
then does the shared abstraction become part of the engine.

## Related

- [20260708-vision-and-scope-pivot.md](./20260708-vision-and-scope-pivot.md) —
  why the P2P/BFT/metaverse scope collapsed and what remained.
- [20260708-concurrency-model.md](./20260708-concurrency-model.md) — why
  SAB/preemptive browser threading was parked and what a restart would require.
- [20260811-coordinate-system.md](./20260811-coordinate-system.md) — the mature
  coordinate/floating-origin design and its actual history.
- [20260909-april-checkpoint-salvage.md](./20260909-april-checkpoint-salvage.md)
  — the later command/RPC/runtime design, split into salvage and discard.
- [20260909-cooperative-scheduler-and-task-ledger.md](./20260909-cooperative-scheduler-and-task-ledger.md)
  — the measured, budget-driven return of cooperative scheduling.
