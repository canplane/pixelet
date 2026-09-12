# Ten thousand billboards in one place, and the order everything else has to come in

**2026-09-10** — a design conversation, not a code change. It started from
"should character models move to R2" (see
[20260910-asset-identity-and-storage-model.md](./20260910-asset-identity-and-storage-model.md))
and ended somewhere else: the question that actually decides the storage
model is what happens at ten thousand billboards, or ten thousand actors,
in a small space. Nothing here is built. This records the shape of the
problem and the order the pieces have to arrive in, so the next pass does
not re-derive it.

## The question that sets the regime

Not "how many entities" but **how many in how small a space**, and in how
many dimensions.

An octree is the obvious answer and the wrong one here. It pays off on
sparse, unevenly distributed points; ten thousand in a small area all land
in one or two leaves, so the tree prunes nothing and charges traversal for
it. The regime where an octree earns its keep is the opposite of the regime
in the question.

A uniform grid sized to the query radius is the usual replacement, and in
3D it fails differently: a dense grid costs cells whether or not anything
is in them. For sparse, moving, three-dimensional points the convention is
a **spatial hash** — quantise the position, hash it, bucket the entity —
which is insensitive to both density and emptiness and makes re-insertion
on movement cheap. Actors move every frame; that last property is not
optional.

## Three dimensions, not two and a half

Characters sit on terrain today. Assuming they always will is the mistake
to avoid: flight, interiors, and stacking all break it, and an index chosen
for ground-attached points cannot be widened afterwards without redoing the
things built on it.

Two consequences are already visible in the current code.

**The hierarchy is 3D only up to L4.** `L5_SHAMT` is 16 and the Z lane is
16 bits wide, so L5's Z collapses and it can only ever address X/Y
(`indexer.ts`, and [20260811-coordinate-system.md](./20260811-coordinate-system.md)).
A spatial index that quantises against the existing address space — which
it should, rather than inventing a second one — has a 3D ceiling at L4 and
horizontal-only sharding above it. That is a constraint to design with, not
a bug.

**Billboard occlusion currently assumes the anchor is on the ground.**
Today's work resolved that a sprite cannot be partially occluded, so it is
all-or-nothing against one anchor point
([20260909-lod-look-and-seams.md](./20260909-lod-look-and-seams.md)). That
anchor is derived from terrain: `poi_layer.tsx` grounds candidates with
`with_ele` before the frustum test and offsets by a fixed `ANCHOR_Z_U`, and
`anchor_occlusion.ts` recovers the anchor by dropping `u_anchor_drop` below
the sprite centre. A billboard free in Z has to carry its own anchor
instead, and "which point represents this thing for occlusion" reopens.

## Drawing ten thousand is not the hard part

One instanced draw handles them. The cost is **decluttering** — deciding
which of the overlapping labels to show — and the established answer, in
map renderers that have had to solve it for years, is a collision pass over
placed label boxes with priority-based hiding, so the work scales with what
reaches the screen rather than with what exists.

`frustum_budget.ts` is already a primitive version of that: rank by
distance from the eye, cut at a budget. What it lacks is an index, so it
still considers every candidate. That is the piece a spatial hash supplies,
and it is why the index question and the decluttering question are the same
question.

In 3D the priority function also stops being obvious. Distance from the
camera and overlap on screen diverge much more when things are stacked
vertically than when they all sit on a heightfield.

## Store, index, batch are three things

The structural point under all of it:

|       | what it answers          | fits                                    |
| :---- | :----------------------- | :-------------------------------------- |
| store | what exists              | cell key to value — today's sparse tree |
| index | what is near me          | spatial hash over the L-hierarchy       |
| batch | what is drawn this frame | instance buffer plus declutter          |

Engines that collapse store and index break when density changes. pixelet
collapses them for placed objects today and gets away with it because
objects are cell-quantised and static. Billboards and actors are neither.

## The order, and why this is not the moment

The pieces of the longer arc mostly already have their own documents. What
was missing is that none of them says which has to come first:

1. **Storage model** — density, dimensionality, index. Open; this document.
2. **Resource identity** — a billboard has to name the asset it draws, and
   what can name it depends on where it lives.
   [20260910-asset-identity-and-storage-model.md](./20260910-asset-identity-and-storage-model.md)
3. **User-supplied content** — an uploaded glTF cannot be named by a build
   hash. This is trigger 1 in that same document.
4. **Command unification** — one dispatch interface for world-shared
   mutation, with local storage and a server as backends.
   [20260908-command-channel-for-state-mutation.md](./20260908-command-channel-for-state-mutation.md),
   whose own conclusion is that this is what erases the single/multiplayer
   split. The transport-agnostic `CommandPort` contract salvaged from April
   ([20260909-april-checkpoint-salvage.md](./20260909-april-checkpoint-salvage.md))
   is the shape it would take.
5. **Multiplayer** — not a project of its own, but what falls out once each
   of the above has been paid for.

The dependency runs one way. Fixing a command interface before the storage
model is settled freezes a carrier around a payload whose shape is not
decided, and server persistence turns the numeric catalog
(`decor_types.tree = 228`) into a wire contract before anyone has decided
what an id is. That is the April failure mode exactly, and
[20260910-january-origin-salvage.md](./20260910-january-origin-salvage.md)
states the rule it broke: keep a mechanism only when a concrete boundary
makes it pay for itself.

So this stays unstarted deliberately. Opening it now would be opening all
of it.

## The prerequisite that is not on the list

The order above assumes something the project cannot currently do: notice
when any of it breaks.

The evidence is the same day this was written. A terrain-styled cube placed
on matching ground was silently dropped — `place()` wrote VOID instead of
the pick, so nothing was stored and nothing rendered, and a reload changed
nothing. It survived because it is invisible from every direction the
project can currently look. The 177 tests are pure logic and passed
throughout; the mesher's own unit tests were right the whole time, since
the mesher was never at fault. It was found by adding temporary
instrumentation to a running browser and reading the values back.

That is not a gap in test count. It is that the render and interaction
layer — click to store to mesh to pixels — has no automated coverage at
all, and eyeballing a browser is what stands in for it. At the entity
counts this document is about, a defect of that class is found by users.

Two halves, and they are not equally hard.

**Invariants are already tractable.** `chunk_mesher.test.ts` asserts real
things about baked geometry — a plateau emits five faces rather than
twenty-five, a user cube flush against terrain draws no seam — by building
a run set and reading the vertex buffer back. That pattern extends: it needs
no GPU, no browser, and no golden image, and it caught real bugs this week.

**The interaction chain is the hole.** Nothing exercises "a click at this
world position, with this pick, against this terrain, ends with these bytes
in the object store and this box in the mesh". Every link in that chain
exists as a pure function; what is missing is a harness that drives them in
sequence without a browser.

The instrumentation that found the bug is close to the shape that would
have prevented it: a trace of decisions — the pick, what the terrain shows,
the redundancy verdict, the cell written, the value before and after —
pushed somewhere readable. Written as an assertable trace rather than a
console line, that is a test.

Golden-image comparison is the obvious idea and the wrong one to start
with: it is GPU- and driver-dependent, and it fails on antialiasing noise
long before it fails on a missing cube.

This does not belong at a position in the order above, because it is not a
step. It is the thing that decides whether the steps can be taken safely,
and it is worth having before the storage model changes underneath
everything.

## How to approach it when it is time

By reading, not by paraphrase. The conventions named above — spatial
hashing for sparse dynamic 3D sets, symbol-collision decluttering in map
renderers, grid-based area-of-interest in networked worlds — are named here
from general knowledge, at the level of "this is the family of solution to
look at". They are not a specification, and adopting them from this
description would be reinventing them badly.

The stance the conversation landed on: at this size the useful move is to
follow the platforms that have already been through it, and to keep the
project's own invention for the places where pixelet is genuinely unlike
them. That is also the reading of the January and April salvages —
what survived was the domain invariants, and what did not was the machinery
built ahead of the constraint that would have shaped it.

## Status 2026-09-13

Steps 1 and 2 of the order are paid for: the storage model is one row over
volumes and points
([20260913-everything-is-a-file-the-placement-and-the-palette.md](./20260913-everything-is-a-file-the-placement-and-the-palette.md)),
and resource identity is the palette's `model_hash`/`model_key`. Step 3 is
decided and not built (the bucket and the ingest route); 4 and 5 stay
unstarted, and the dependency still runs one way.
