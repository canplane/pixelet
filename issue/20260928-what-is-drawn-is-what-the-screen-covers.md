# What is drawn is what the screen covers

**2026-09-28** — a design conversation, not built. It is the central idea
the view is to be built on from here, and every earlier idea about what a
view costs is to be read through it:
[what a view costs is the eye's](./20260925-what-a-view-costs-is-the-eye-s.md),
[the view is cubes about the eye](./20260926-the-view-is-cubes-about-the-eye.md)
and
[the view's cost, read against its invariant](./20260928-the-view-s-cost-read-against-its-invariant.md),
whose decision this replaces.

## The idea

Looking farther does not show more. Whatever lies in front of the eye, what
reaches the screen is the screen's area: a direction on the sphere about the
eye shows one thing, the first thing along it, at the size a cell has at
that distance. So what is drawn has an upper bound before anything is known
of the world, and the bound is the screen's. Where a piece came from — how
far along its direction — is its level of detail, and nothing more.

What is stored is another matter. Terrain is a heightfield in pages of a 2D
pyramid; placed objects are sparse bands in columns. There is no reason to
lay either out in 3D, or to make objects dense. Each layer keeps the
structure its data has.

The two are joined by one query. It is asked with a direction on the view
sphere — polar and azimuth, since distance is what the projection takes
away — or with a point, and each layer answers from its own structure.
Fetching and scheduling are then one mechanism for every layer: what a
query could not answer is what is wanted, in the order the screen asked.

## The invariants

With `τ` the pixels a cell may cover and `κ_lo τ` the fewest it covers once
selected (0.317 τ at a node 64 cells across, the split at 192 cells a cell
side):

1. **Drawn cells are bounded by the screen.** Each pixel shows one cell,
   the first hit along its direction, so the cells drawn are at most
   `W · H / (κ_lo τ)²`, whatever the world holds — air, layers stacked, a
   dense city — and at every level at once. Occlusion is not an addition:
   the first hit is the definition.
2. **Records wanted are bounded by the screen.** A record of level L is
   asked for where ground at the distance that level is drawn at shows,
   and at that distance it covers a solid angle that does not depend on L.
   So the records a view touches are a constant set by the screen, plus the
   ancestors each needs, a fixed count of levels.
3. **One mechanism fetches for every layer.** A record is wanted because a
   query missed it, and ranked by where on the screen and how near the miss
   was; a layer adds nothing to the order.

## The query

Two forms, one per thing asked:

- **A direction** → the first hit along it and its distance. This is what
  decides what is seen and so what is wanted; the level a layer answers at
  follows from the distance.
- **A point and a level** → a layer's value there. This is what a drawn
  piece is made from, and what a pick, a placement or a raycast asks.

The query is the only place a layer's storage shape is known. Everything
above it — what is drawn, what is wanted, in what order — speaks in
directions and points.

## Open

- **Asking before the data is there.** The first hit along a direction
  needs the data it hits. The known answer is hierarchical: the coarsest
  level is always resident, a query answers from the finest resident
  record, and the miss under it is what is fetched, so a view refines over
  frames. That is feedback from what is seen, with a frame of lag, and not
  from what memory refused. Motion reveals what was hidden, so what is held
  needs a margin past what is seen; its size is part of the bound.
- **How "draw what the screen covers" is drawn.**
  - Rasterized pieces, as now: the answers grouped into meshes, which
    pieces exist decided by what the screen showed it needs (virtual
    texturing's feedback, Nanite's streaming).
  - The records cast into directly: the shader walks the heightfield's
    pyramid by its max mipmap and the objects' sparse bands, so the records
    are the render data and there is no mesh, no mesh bake, no arena and no
    chunk table. Its cost is pixels times steps, the screen's again; what it
    costs on a phone's GPU is unmeasured.
- **What the kernel decides and what the renderer does** under the second:
  a walk in the shader executes the kernel's structure but is not listed
  by it.

## The earlier ideas, read through this

- **Shells about the eye** are a description, not a mechanism: the
  distance at which a level is drawn. Nothing enumerates them.
- **The budget's search, `VIEW_CHUNKS`, `VIEW_PAGES`, `LEVEL_CHUNKS`, the
  column bound k, a surface density, air counted or not, and the frustum's
  share** were all ways of bounding a count of 3D nodes. The screen bounds
  what is drawn directly, so each is either a derivation from `W · H` and
  `τ` or gone.
- **The pressure** answered a heap that refused. Memory bounded by the
  screen leaves nothing for it to answer. Fitting a device is choosing `τ`
  from the device's budget at boot, which is engineering inside the
  invariant, not a loop.
- **Overflow drawn a level coarser** existed because a node's payload had
  no bound. A direction shows one cell; a piece's payload is bounded by the
  pixels it covers.
- **Conservative min/max over a column** is what a hierarchical first-hit
  needs: a coarse record must not report empty where finer ground stands.
  It can be carried up from finer records as they arrive, with no rebake;
  before they arrive it is an estimate, as the pyramid's heights already
  are.
- **The chunk table's fold** (the builder's table into the slots, bounded
  tiers, a fixed index) stands or goes with the choice above: a cache of
  rasterized pieces keeps it, casting into the records removes the table.

## Reviewed

Later the same day, by three reviews — the invariants checked by proof and
simulation, the prior art, and the engineering on a phone — and a
conversation. Nothing above is edited; what they changed is here.

### What the screen bounds, and what it does not

Invariant 1 named one bound for three things, and each has its own:

- **Visible cells**, the first hit at a pixel sample, are at most `W · H`,
  one a sample. `W · H / (κ_lo τ)²` is wrong: `κ_lo τ` bounds a cell's
  side from below, not its area, and a cell seen at a grazing angle or
  edge-on covers less than a pixel.
- **Drawn cells**, rasterized, are bounded by the rule and not by the
  screen: about 1.5 · θ_h · (P / τ)² a level — some 76,500 at the reference
  screen, the same at every level and height in simulation — times the
  levels. For terrain that holds whatever the ground is. For objects it
  does not: a piece of c³ cells can expose 3c³ faces, so "a piece's
  payload is bounded by the pixels it covers" holds for casting into the
  records and not for rasterized pieces.
- **Resident cells** are whole records, a multiple of the drawn.

The count of distinct pieces visible is not bounded by the screen over a
piece's angular size either: slivers make every selected piece visible,
and terrain visibility is Θ(n√n) in the literature (Moet, van Kreveld and
van der Stappen). Pixel sampling is what caps it.

Chosen per pixel from its own distance, with no node box, a cell covers
between τ / 2 and τ; 0.317 τ was an artifact of 64-cell nodes.

### Records by the reach, not by what is hit

Invariant 2's solid-angle argument fails at a grazing angle, where a page
subtends almost nothing. The bound is a ratio of lengths. Level L is drawn
where the eye is `[r_L, 2 r_L)` away; cut through the stored ground, that
shell is a ring about the eye's foot, and widened by a chunk's diagonal it
is the level's **reach**, at most `474 · 2^L` units across against a page's
`512 · 2^L`. So a level wants at most 3 × 3 pages, whatever the tilt or
the relief, and a level whose ring the eye is too high for wants none.
The render sees the shell through the screen; the records see the same
shell in their own coordinates.

Records are wanted by the reach and not by what the screen hit, for two
reasons:

- **A coarse record under-reports what is finer.** Terrain folds by
  average and objects by majority, so a ray passing over the average, or
  through a thin tower folded to air, never descends there, and what it
  would have found is never asked for — not a fetch late, but never. "Carried
  up as finer records arrive, no rebake" above is circular: they arrive
  only if asked for. With records wanted by the reach, no fold has to be
  conservative for anything to be fetched.
- **Motion reveals what was hidden.** What is held by the reach is held
  behind a wall too, so occlusion culls drawing and never residency.

A miss asks for the level it wants directly, since a page's address is a
function of a point and a level; refining a level at a time multiplies the
latency by the levels, which Nanite names as why it does not stream its
hierarchy. The margin motion needs is speed times latency, about the eye.

### What is still missing

- **The top level has no far edge.** Nothing splits it, so it runs to the
  horizon, and on a flat world without end. The far limit is `2 · r_top`.
- **Objects have no levels.** Bands are the z16 lattice alone, so objects
  have no reach to be bounded by until they have records a level, and,
  being sparse, a resident map of where they are that is conservative —
  3D Tiles' availability, Google Earth's quadtree packets.
- **A point query misses too.** A pick, a placement or a physics step can
  ask where the screen does not, and its miss is fetched with a priority
  of its own.
- **The distance still enters** in the count of levels, about
  ½ · log₂(2R / h) from a height h, and in the top level's far edge.

### Drawing it

The prior art does not ship a global walk from the eye through one
structure. Where rays are cast at scale — Dick, Krüger and Westermann's
terrain, Teardown, Aokana — a proxy is rasterized and the ray walks only
inside it, and the demand is a traversal about the eye. Casting wins when
the data is denser than the screen, and the rule here draws a cell at
three to six pixels, where rasterizing won. So three shapes are open:

- **Rasterized pieces** in a new format. Today a box is 36 unindexed
  vertices with about 140 bytes out of each; on a tiled GPU that output
  goes through memory, which is the likelier cause of the phone's GPU time
  following triangles than rasterizing itself.
- **A proxy rasterized, rays inside it** — a page's box against its max
  pyramid — which removes the terrain's mesh bake and keeps depth as the
  join between layers.
- **A global walk**, which removes the most and is the least known.

Casting still keeps a table: meshes, their bake and the arena go, and the
chunk table becomes a table of resident records.

The experiment that decides it, on the iPhone 13 mini at ratios 2 and 3:
one frozen view from above, at 45° and at street level along the ground;
today's boxes, the same boxes as masked faces with one flat id a vertex and
the air a pixel, and a cast over the same pages writing depth. Records by
the reach and fetching by misses are the same under all three, and come
first.

### Open

- **A governor at runtime.** Every shipped system in the review has one —
  id's feedback bias, Cesium's ×1.02, Battlefield 3's blur bias — because
  a working set moved with the scene. With records bounded by the reach and
  drawing by the screen, what moved it (grazing, page granularity) is a
  constant, and what is left to follow is the device's own budget, as an
  event that chooses τ again. Whether that is enough is not yet decided.

## Perceived, not remembered

Later the same day, a conversation (the user, with another model's
framing). The eye does not hold the world and pick what it sees; light
arrives and is perceived. What is behind a wall is not kept in the eye's
working memory — at most that something is there, and roughly where. So the
storage follows how the world exists, and the view follows how an observer
perceives it, and nothing between them is a renderable copy of the world.

That splits what the kernel holds into four, each with its own bound:

| Tier            | Holds                                                                                          | Bounded by                                   |
| --------------- | ---------------------------------------------------------------------------------------------- | -------------------------------------------- |
| the world       | every record, in its layer's own structure                                                     | nothing of the view — the server, the bucket |
| the index       | what a record summarizes of what is under it: that something is there, its extent, its version | the reach: a few words a record, 9 a level   |
| the working set | the full records the view's observations have asked for                                        | what is observed, inside the reach           |
| the samples     | what the screen shows                                                                          | the screen                                   |

What decides residency is observability, not distance: a thing just behind
a wall has no need of its full record, and a mountain thirty kilometres off
on the horizon needs its coarse one. The working set is filled the way a
virtual address space is: an observation misses, and the miss is fetched.

This does not undo the review above; it places it.

- **The reach is the bound, observation is the choice.** Every record an
  observation can ask for lies in the reach of its level, so the working
  set is a subset of the reach and its bound — 3 × 3 a level — still holds.
  What is held is no longer all of the reach but what was observed in it.
- **The index is what makes observation safe.** A miss is made only where
  a query descends, and a query descends only where a coarse record says
  something is there. So the index must be conservative — a max over the
  terrain under it, an OR over the objects — or a peak or a thin tower is
  never asked for. The client cannot know what lies under what it does not
  have, so the index comes from the one who has the leaves: the bake, or
  the server. This is Nanite's always-resident hierarchy and 3D Tiles'
  availability, and it is small.
- **The eye is one observer among several.** A shadow is the sun
  observing; a pick, a placement or a physics step is a point observing.
  Each asks the same query and each miss is fetched; what the screen does
  not see can still be observed by one of them.
- **Motion is a policy over the misses.** What turns into view is at the
  index's resolution, coarse and never empty, until its miss is answered.
  Prefetching toward where the eye is going is engineering over that, not
  part of the invariant.

Drawn with the observation first, the questions left are the index's
format and where it is written, the order misses are fetched in across
observers, and — unchanged — how the samples are drawn.

## Reviewed again

The same three reviews, on "Perceived, not remembered". Nothing above is
edited.

### Observation chooses; the reach is still what is provisioned

Over open ground seen along it — the product's street-level view — every
page along a grazing ray lies within its own relief, so every page is a
"maybe" and what is observed is the reach inside the frustum. That is the
common case, not the worst. So memory is still provisioned for the reach,
and what observation saves is fetches and bytes moved; under the Worker's
100,000 requests a day that is worth having, but it is not memory.

It also costs a turn. The reach is a ring about the eye's foot, so turning
changes nothing in it and fetches nothing. Paged by what is observed, a
turn of 180° a second puts about two thirds of the screen at the fallback's
resolution for a round trip. Every shipped system keeps a floor for this —
Nanite's root pages, virtual texturing's coarse mips, VSM's coarse pages —
and occlusion lowers detail rather than evicting (Unreal's
`HiddenPrimitiveScale`).

So "perceived" is placed one hop nearer the screen than written above:

- **Network to memory by the reach.** Every record in the reach of each
  observer is held; nothing is fetched on a turn.
- **Memory to the GPU, and the samples, by observation.** That hop is a
  frame, not a round trip, and it is where a feedback of what the screen
  asked for belongs — if drawing moves to casting into records on the GPU.

### The bound is a sum over observers

The working set is inside the reach only for the eye. A shadow ray leaves
it toward the sun, by up to `min(H_max / tan e, √(2R · H_max))` at a sun
elevation e — one to three pages a level on the sun's side. A point
observer — the character's physics at the map center, a pick or a placement
far from the eye — has a reach of its own at the level it asks. The bound
is the sum of the observers' reaches, still independent of the content.

The floor has two parts, and for physics and placement it is correctness,
not prefetch: the coarse levels' records held all around, drawable and not
only a summary, and a full-resolution ring about each point observer, as
wide as its speed times the latency.

### Two numbers corrected

- **The eye's far edge includes the relief.** A peak of height H shows out
  to `√(2Rh) + √(2R · H)`, not the sea-level horizon: from 52 u up, 18.7k u
  becomes 78k u for a 1,000 m peak. The far limit is the smaller of that
  and `2 · r_top`; cut at the sea-level horizon, far mountains are never
  observed and never fetched.
- **474 · 2^L is the reach's radius, not its width.** Across it is about
  949 · 2^L, 1.85 pages, which straddles at most three a side: 3 × 3 a
  level, as written, and one derivation should own the number where
  `VIEW_PAGES`'s comment gives a third.

### The index

- **It is a lane in each page, not a tier of its own.** A page carries the
  maximum and minimum of everything under it, the interpolation's
  overshoot included, and which of its four children exist. The bake
  already folds four children into a parent; this is one more reduction in
  the same pass. The ancestors are always in the reach, so a page's index
  is held before the page is asked for, with no object and no request of
  its own, and an ocean child is never asked for — today that is a 404 a
  page, each a Worker request.
- **It is a grid, not a word.** One maximum a page lets a grazing ray
  descend everywhere and, drawn, stands a page as one box — 512 τ, the
  kilometre walls recorded on 09-26. A k × k grid of minimum and maximum,
  the top of a max mipmap (Tevs, Ihrke and Seidel), is 4 KiB a page at
  k = 32 against a record of about 1.2 MiB.
- **The minimum is what ends a ray.** A ray under a page's minimum has hit;
  without it a query keeps asking past the hit, or takes a round per
  false positive — up to about 30 along a grazing ray.
- **Objects are local for now,** so their index is folded by the client
  from its own store — occupancy by OR and the highest top, a level at a
  time — until they are shared from a server.

A new terrain version is 227,310 Class A operations; the month is already
past the free million, so it bills — about a dollar — after `deno task
usage`.

### What goes and what stays

The domain's search, `VIEW_CHUNKS`, `LEVEL_CHUNKS` and the frustum in the
wish go, for the reaches worked out a level at a time; so does the
pressure. `io/sched.rs`'s wants and credits stay — each observer is a
producer declaring its reach. `io/policy.rs`'s shells become the eye's
column at every level first, then each reach by level and distance, then a
class for point queries. `relief.rs` reads the conservative lane, which
ends its caveat that a mean lies inside what it averages. The chunk table,
the slots and the arena wait on how the samples are drawn.

The first measurement is the Worker's round trip for a page of 8 to 20 KB,
from the phone, cached at the edge and not: it decides the margin a reach
needs and how long a jump stays coarse.

## A memory hierarchy

Later the same day, a conversation. What had been thought of as one thing
— what the view needs, what is held, what is fetched ahead — is three, as
a memory hierarchy keeps demand paging and prefetch apart from the pages
themselves. Thinking of them together was the mistake; `Pyramid` and
`Domain` do all of them in one place.

| A memory hierarchy              | Here                                                                            |
| ------------------------------- | ------------------------------------------------------------------------------- |
| an access                       | a ray: a quantized direction on the view sphere, a pixel                        |
| the cache                       | what is drawn from, on the GPU: bounded by the screen, a frame away             |
| main memory                     | the records                                                                     |
| secondary storage               | the server and the bucket, hundreds of milliseconds away                        |
| a page fault, and demand paging | a ray answered by a record coarser than it wants, and that record fetched       |
| prefetch                        | the reach: more than was asked, fetched ahead so a turn or a step finds it held |
| the page table                  | the index: which records exist and the extent of what is under each             |
| the zero page                   | the open ocean, one shared record (`ABSENT`) every page of no object reads      |

### The access

What is to be drawn is seen through the quantized sphere of directions,
without r: it is ray casting, as a definition of what is asked, whatever
draws it. The distance is not an argument; it is what comes back, and it
is the level of detail:

```text
from_where(θ, φ) → (r, α, source)

level(r, α)  the level whose cell a pixel covers at distance r,
             seen at an angle α to the surface: r · Δθ / cos α
source       the layer, level and record that answered — an address
```

A **miss** is one line: the record that answered is coarser than
`level(r, α)`. What is asked for is that layer's record at that level, at
the hit — the level wanted at once, never a level at a time.

The circle — `r` needs the data it chooses the level of — is held stable
by the logarithm and by the index being conservative. The level is
`log₂ r`, so `r` has to be right within a factor of two, and a hit read off
a conservative maximum is no later than the true one, so `r` is
underestimated and the level errs finer, never coarser.

A pixel chooses its own level with no node around it, so a cell drawn
covers between τ / 2 and τ.

### Where it is not a memory hierarchy

- **A fault does not stall.** A frame cannot wait on a fetch, so a miss is
  answered by the nearest resident ancestor, coarser: a fallback in
  resolution, which a memory hierarchy has no need of. So an access always
  hits at some level, and what makes that true is the floor — the coarse
  levels held all around.
- **The address is not known before the data.** Where a ray first hits
  depends on what is there, so the page table has to be conservative and
  held over at least the range prefetched, before the records it
  describes.
- **Some observers cannot take the fallback.** Physics under a moving body
  and a placement need the record itself, so what is about them is pinned,
  as a real-time process locks its pages.

### The modules

1. **Access.** Rays — the eye's, the sun's, a point's — asked through one
   query, returning `(r, α, source)` and the misses.
2. **Paging.** Addresses to resident records, a fault answered by an
   ancestor, the capacity and what is given up under pressure, with the
   records and `ABSENT` it already has.
3. **Prefetch.** The reach, where the eye is going, and the pinned rings,
   as hints to paging and never as the demand itself.
4. **I/O.** `io/sched.rs` as it is: a demand miss and a prefetch are two
   classes of one order.
5. **The cache.** Records to what is drawn, by what the screen observed;
   shaped once how the samples are drawn is decided.

Each owns one bound: what the rays touch is inside the observers' reaches;
what is resident is the demand, the prefetch and the pins; what is drawn
is the screen.

### A fault hands the turn over, and the frame keeps it

A fetch from the server is a page fault. An operating system puts the
faulting thread to sleep and runs another; here nothing sleeps. The pixel
that faulted is drawn from its ancestor, coarser, and the frame goes on to
the rest of its work. When the record comes back, what is done with it —
decoded, folded, built, sent to the GPU — is taken in the same order under
the turn's budget, as the folds and the builds are now. D3D12's sampler
feedback streaming and sparse residency are the same shape: a tile not
resident samples coarser, the request is recorded, and residency changes
when the tile arrives.

What waits is one queue with two ends:

- **In front, demand:** a ray answered by a record coarser than it wants,
  seen before unseen and nearer first; the pins — physics, a placement —
  ahead of all of it.
- **Behind, prefetch:** the reach, and where the eye is going.

Within an end the order is the eye's, and it is the eye's now, not when the
request was made: every turn ranks what waits again and drops what has left
every reach, as `io/sched.rs`'s `declare` and `pump` already do. A record
waits at most once and only inside a reach, so the queue is bounded by the
records, and a frame never waits on it, since the floor answers every
access at some level.

### What is structure, what is mechanism, what is policy

The order of that queue is scheduling: a policy, dynamic, and orthogonal to
everything above it. So the parts are kept apart the way an operating
system keeps mechanism apart from policy:

- **The page's definition** is structure. What a record is — its layer,
  level and address, the index lane it carries, the zero page — and it does
  not move with the view.
- **Demand paging** is mechanism. A fault is a record coarser than
  `level(r, α)`; it is answered by the resident ancestor and becomes a
  request. That is fixed, and correctness rests on it and on the floor.
- **How the faults are served** is policy: the queue, its two ends, the
  order within each, what is dropped, how much a turn takes on.
- **Prefetch** is policy: which records beyond the demand are asked for,
  and when.

The invariants — what the rays touch, what is resident, what is drawn —
belong to the structure and the mechanism. A policy can change the
latency and the look while a miss is outstanding, never the bounds and
never what is correct once the records are held.

## Five concerns, reviewed

The third round of the three reviews, and the conversation after it. Two
things above are corrected here rather than there: "a fault hands the turn
over" and the queue with two ends mixed scheduling into the model, and
`level(r, α)` put the surface's angle into the geometry's level.

### The concerns

| Concern    | Asks                                                         | Kind           |
| ---------- | ------------------------------------------------------------ | -------------- |
| Page       | what moves between storage and memory                        | representation |
| Demand     | what an observation needs, and how it is answered without it | correctness    |
| Residency  | what must be held, and what may never be given up            | correctness    |
| Prefetch   | what will be needed that is not yet                          | speculation    |
| Scheduling | which of the work waiting runs now, within what budget       | dynamic policy |

**Page.** A layer's record at a level, in the layer's own shape — terrain
2D, objects sparse 3D — carrying the index lane of what is under it. A
page is not a drawn chunk.

**Demand.** A query answers from what is resident. Where that is coarser
than asked, it emits a miss and answers from the ancestor, and the caller
goes on. No order and no priority enter here; a fault is detected and
answered, and that is all.

- The geometry's level is the distance's alone, `level(r)`: a height's
  error shows on screen as `P · ε / r` whatever the angle, and with the
  angle in, the skyline — where `1 / cos α` is largest — was drawn
  coarsest, everything past about 6 km from street level at z6. The angle
  belongs to sampling colour, clamped as anisotropic filtering clamps it.
- Two hits, not one. A hit on the conservative maximum decides misses: it
  is no later than the true one, so the level errs finer. A hit on the
  drawn surface, which averages, decides what is drawn; it can come later
  than the true one and must never decide a miss.
- A hit on a bound that is not certified is a miss too — certified when
  the ray passes under the bound's minimum, or the bound is no wider than
  a pixel's footprint. Without it a ray can rest on a coarse box's top
  forever, answered and never asking.
- A round's error is at most `log₂(1 + 724 / (192 k))` levels for an index
  of k × k a page, not "a factor of two", and the rounds are one per false
  positive along a ray, some 20 to 30; held in memory, those are frames.

**Residency.** Mechanism, with a replacement policy inside it:

- **The floor** — the coarse levels held all around — is residency's, not
  prefetch's. With no prefetch at all an access must still hit at some
  level, and only the floor makes that true.
- **Ancestor closure.** A record's index is a lane in its parent, so a
  parent is resident whenever a child is; asking for a child asks for its
  ancestors, and a frame with a resident child is never given up.
- **Pins are locks.** Physics under a moving body and a placement cannot
  take a coarser answer, so what they need is locked by residency; a pin is
  not a guess and is not prefetch's.
- **Capacity** is at least `|∪ reaches ∪ pins ∪ floor|` plus what is in
  flight, stated as a constant and asserted. Thrash here is silent — a
  fault falls back and nothing fails — so it has to be impossible by size,
  not observed.
- What is given up first, within those, is policy.

**Prefetch.** The reach, where the eye is going, the sun's side. With
prefetch at zero, the result is the same once the records arrive; only the
latency and the look while they are missing change. Holding the reach
whole is a strong default of this policy, since without it a turn waits on
the network, but it is a default and not an invariant.

**Scheduling.** Work items — a demand, a prefetch, a decode, a fold, a
build, an upload — each carrying what the other concerns gave it: the
observer, how many levels short, seen or not, how near. The scheduler
orders them by that and nothing else, and may be a heap, a deque, EDF or
anything that keeps one condition: **a demand still wanted is served
eventually.** Correctness rests on convergence, so starvation is the one
thing a policy may not do. Replacing the scheduler then changes no concern
above it.

### What the prior art adds

- Unreal's texture streamer is this split shipped: wanted mips, slave
  locations and prestreaming, a budget pass, a load order, forced loads.
  An operating system's is fault, `MADV_WILLNEED`, replacement and
  `mlock`. The clipmap (Tanner, Migdal and Jones, 1998) is the reach's
  precedent.
- Under pressure, shipped systems lower the level wanted everywhere — a
  bias — rather than evicting harder; replacement alone thrashes.
- Waiting work is ranked by how many levels short it is, not by recency: a
  grazing ray touches many pages, and under LRU they all look used.
- Asking for the level wanted at once was Cesium's `skipLevelOfDetail`,
  turned off by default for its seams. Fetching directly is sound; drawing
  a coarse ancestor beside a fine descendant with nothing between needs a
  transition — a blend on arrival and a seam — and that is the cache's.
- A physics pin in the same residency as drawing has no precedent; shipped
  engines keep collision on a path of its own. This is a choice made here.

### Mapped onto the tree

- **`Pyramid<L>` is mostly residency already** — records, `ABSENT`,
  `drawn_from` and `ancestor`, pins, `record_for`, `adopt`, the folds —
  and also does demand and prefetch in `domain_for`, `cover_of` and
  `wants`. The split is moving code to its concern, not new machinery.
- **`io/sched.rs`'s `declare` replaces a category's set whole**, so the
  observers' wants are merged per layer, the best per record, before one
  declaration; that merge is also what makes a record wait once.
- **The chunk table still draws from `Domain`'s cover**, so `Domain` cannot
  go before the cache is chosen. For the time between, the reach and the
  cover come from one `split_distance`, with an assertion that every page
  the cover draws is inside the reach.
- **The device's pin is ranked last today** (`policy::pinned`); a lock for
  physics comes first. Two kinds: a hard pin, correctness; a soft one, as
  now.
- **`desired` and `backoff` are unbounded vectors today.** Bounded, they
  are the records' count plus the assets' and the geocode's own, which are
  not records.
- **`.fixed.terrain` grows from about 25 MB to about 64 MB** for some 125
  records with the index lane; POI takes a count of its own rather than
  sharing the terrain's.
- **A point observer far from the eye** adds its own ancestors, so how far
  one may be is a product number to say aloud.

### In order, before how the samples are drawn is chosen

0. The Worker's round trip for an 8 to 20 KB page from the phone, edge
   cached and not.
1. `pxl-spec`: the reach and `level(r)`, with their laws — at most 3 × 3 a
   level, closed under parent.
2. `pxl-spec`: the index lane and its fold, with its laws — a parent's
   extent holds its children's and every interpolated height — wired into
   the bake over a small area and checked, not published.
3. Residency out of `world/pyramid.rs`: the address table, frames, resolve
   to an ancestor, locks, closure, the capacity assert.
4. Prefetch and the policy's classes; `pressure.rs` and the domain's page
   wants go, the cover stays.
5. The scheduler's tables bounded.
6. The CPU query reading the lane, in place of `ground_under`, `ground_at`
   and `surface_height`.
7. One publish of the new terrain version, after `deno task usage`.
8. The cache: raster against casting, on the iPhone 13 mini.

### The plan, split by concern

The engineering review's plan, redone along the concerns:

- **The reach is two things.** Its bound is structure: any fault the rule
  can raise names a page within about `474 · 2^L` of an observer, so 3 × 3
  a level, used to size capacity and to prove. Asking for all of it is
  prefetch.
- **The mechanism's demand is the faults and nothing else**: under
  raster, a chunk of the cover drawn from an ancestor or left open, which
  `cover_of` already finds; under casting, the GPU's feedback; for a point,
  `at(point, level)` answered by an ancestor. "The reach pages that are
  seen" was policy passing as mechanism.
- **What is needed carries its kind** — `Floor`, `Pin`, `Fault`, `Hint` —
  and a policy may make only `Hint`s, which get only a frame that may be
  given up. So a slack of zero is correct and slower.
- **`io/sched.rs` splits** into a ledger — wants, attempts, backoff,
  credits, work-conserving, a fixed tie-break so the order is total — and a
  serve policy that orders, preempts, takes in and retries. `worth` becomes
  a victim policy choosing only among frames that may go, and the domain's
  page wants and the pressure become a reach prefetch, with no prefetch at
  all as the other policy the tests run.
- **The split is proved by running the mechanism under hostile policies**
  — random order, last in first out, always preempting, no prefetch, a
  flood of hints — with a resting camera: residency never past its
  capacity, a floor, pin or fault never refused a frame, every access
  exact at its level in the end, no resident page without its ancestors.
  Step 4 becomes 4a, the mechanism, and 4b, the policies; with 4a alone
  and no prefetch the view has to converge to exact.

Two corrections to that plan:

- **The floor is held, not asked for.** Adding the top page over an access
  only once it resolves to nothing leaves a hole on every turn toward
  ground not yet seen when prefetch is off. The top level inside the far
  edge — nine pages at most — is resident whatever is observed, so every
  access hits at some level; only the first moment of a cold start or a
  jump is without it.
- **The level is `level(r)`**, as above; the angle is not in the
  structure.

### Numbers are parameters

Step 0 is dropped: no measurement gates the work. Every number the
concerns depend on that is not a derivation — the prefetch slack, the
hint allowance, a pin ring's width, the motion margin, the retry delay,
τ — is a parameter, named once in one place and read by whoever uses it,
never written into the code that uses it. Tuning them later changes no
concern.

### Contracts, not words

Later the same day, the conversation again. Three of the rules above still
named what the current code needs instead of what the concern is:

- **Residency is two sets.** What is held hard — whatever a fallback needs,
  what is leased, the workspace progress needs — and a cache, which holds
  everything else: the pages demanded now, the prefetched, the recent. Only
  the hard set is correctness. Safety asks capacity for the hard set and
  the workspace. Converging to exact asks, beyond that, for what demand can
  name at once — the reach's bound, which is structure's theorem about
  faults. Asking for the reach is prefetch and enters no bound: however
  eager it is, it only pushes itself out of the cache.
- **The floor is a law, and holding it is one way to keep it.** The law is
  that every observation has an answer. Coarse levels held all around keep
  it; so could a procedural stand-in or a small root. Residency holds
  whatever the chosen fallback leases.
- **Closure and pins are contracts handed to residency.** A page says what
  it depends on — a terrain page, its parent, because its index is a lane
  there — and residency keeps whatever set it holds closed under that,
  without knowing why. A consumer that cannot take a fallback leases a
  page and releases it; residency does not know it is physics.

Scheduling's condition, stated exactly: it keeps every budget (safety),
and a demand that stays eligible while there is room for it is not starved
(liveness).

Each concern hands the others a contract and nothing more:

```text
page         dependencies(p)
demand       need(p, level)
residency    resident(p), lease(p), release(p)
prefetch     speculate(p)
scheduling   execute(job)
```
