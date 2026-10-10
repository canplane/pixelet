# What is drawn is what the screen covers

**2026-09-28** — a design conversation, not built. It is the central idea
the view is to be built on from here, and every earlier idea about what a
view costs is to be read through it:
[what a view costs is the eye's](./closed/20260925-what-a-view-costs-is-the-eye-s.md),
[the view is cubes about the eye](./closed/20260926-the-view-is-cubes-about-the-eye.md)
and
[the view's cost, read against its invariant](./closed/20260928-the-view-s-cost-read-against-its-invariant.md),
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

## Built, and what the merge settled

Built on `observation-paging`: the structure (the level, the reach, the
page's index lane, terrain v7) and the mechanisms (residency, demand, the
ledger and its serve policy, prefetch as a policy), merged and through the
gate. What the conversation settled while it was merged:

- **Demand is held only while it is asked.** A frame is held for demand
  while the current observation asks for its page; when the eye leaves, it
  is cache and may go. A fault leaves no state behind.
- **Prefetch is off by default.** With none, a view still converges to
  exact — fault, fallback, fetch, refine — which is what shows prefetch is
  not part of the meaning. The reach is a policy to compare against it.
- **Nothing is leased for the device.** Its marker takes a coarse answer;
  a lease is for a consumer that cannot, and none exists yet.
- **The client's fold is gone.** A page's dependency is its parent, so a
  parent with its children held and itself not cannot happen, and folding
  was only a fetch saved — a cache optimization to bring back if the
  requests show it is worth one.
- **The frustum is two things.** For what is drawn it is the domain: a
  direction outside it is not asked. For the records it is a predictor:
  inside it is demand, around it is prefetch's. The floor is neither — it
  answers every direction, turned to or not, so it is held all around.
  Occlusion is not a stage: the first hit is the answer, and a culling of
  what is hidden is an acceleration that must be conservative.
- **Demand free of what is hidden needs demand from hits.** While the
  raster cover is what raises faults, a hidden chunk raises one too; and a
  hit that decides demand is taken on the conservative maximum, since the
  drawn average can hide what stands behind it.
- **A drawn chunk is derived.** Time is carried by the records; what is
  drawn is made from them for the view. A cache of meshes may stay, if it
  can be dropped at any moment and rebuilt, and holds no invariant.
- **The demand bound is the observation's, not the chunk's.** A sample
  hitting at `[r_L, 2 r_L)` asks for the page it hit, so the bound is
  `(⌊4 · SPLIT / PAGE_CELLS⌋ + 2)²` a level, a function of the criterion
  and the page alone. The raster cover reaches a chunk's diagonal further,
  and that term is kept apart and marked as the one place the drawing's
  representation still couples to the paging's bound; it goes when demand
  comes from samples. Changing the chunk's size must not move the bound.
- **Memory is a formula.** `.fixed.terrain` is `F × a frame's bytes` with
  `F ≥ hard + D_max`; a tighter observation bound lowers it without a
  heuristic.

Each concern can be turned off or swapped to test that it is orthogonal:
prefetch off, another scheduler, another victim policy, no mesh cache, no
consumer leasing — and another chunk size — each leaving the meaning and
the bounds as they were.

### Run: a page depends on nothing over it

Run against the local servers, the built branch drew nothing. Pages of
levels 0 to 2 arrived; none was read. Residency held a page as read only
once its parent was, and the dev server answers nothing past its top level,
so no chain ever closed — and with every level served, one ancestor late
would still leave everything under it undrawn. That is the invariant the
tree states against it: a missing ancestor cannot make a resident
descendant unreachable.

The dependency was written for an index in the parent's lanes. The index
is in each page's own, for everything under it; a page that is held
exists; so nothing over a page is needed to read it. What the page over a
missing one is for is the fallback and the query's descent, both from the
nearest resident page. A page of the pyramid depends on nothing, and the
demand is a fault's own page — the pages over it, held for a better answer
meanwhile, are prefetch's to ask for, as a policy.

With no dev exception: past the dev server's top level the floor is not
served, so there only what the view asks for at the served levels answers.

## Four sentences before the next step

Settled in the conversation before the access module and the demand's
estimator are built, so that neither drags a solver or a representation
back into the meaning:

1. **An observation's meaning is the first visible answer for an angular
   sample** — an eye, a direction, a footprint. Ray casting and
   rasterizing with a depth buffer are two solvers of it, and a layer may
   use either; `first_hit(ray)` is the query's vocabulary, not a choice of
   renderer.
2. **Demand is first what an observation requires** — the information, at
   the resolution its footprint allows within τ — and only then pages: the
   storage's own representation maps a requirement to the pages that meet
   it, so a page's shape can change and the observation's law does not.
3. **A query answers and says what it lacked; it does nothing else.** It
   returns an exact answer, a coarser one with what it would have needed,
   or nothing. Turning a lack into a request is demand's, when to fetch it
   is scheduling's, and a query never fetches, prefetches or holds.
4. **An estimator of demand is a conservative superset of the exact
   demand.** The first one works from bounds — a page's box in the frustum
   and in its level's band of distance — and asks for hidden pages too;
   later ones, from hits or the GPU's feedback, are smaller supersets. None
   changes what an observation means or what residency does.

What is left at the top is only this: the world, what an observation
requires, the pages that meet it, what is resident, and what is made from
it to be drawn — the chunk now a materialization, made for a frame and
cached only if that is cheaper, never a thing the meaning rests on.

## Materialization, and the quality it is held to

The access query and the observation's demand are merged; the view draws.
Taking the budget's search out of the domain showed what it had been
standing in for: the rule's cut of a view has no bound in three dimensions
short of a ball — a column can hold any number of cubes — so the pieces a
view can need came to about 65,000 against today's 1,024, and a mesh
arena sized for them to some 7 GB. Kept at its size, the arena makes a
steep view wait for slots, and the wait shows as holes.

**Materialization is a concern of its own.** It makes what is drawn from
the observation, the resident records and a budget on the device, and its
budget is not residency's: a hundred records resident is no reason to hold
a hundred pieces drawn. Its contract:

1. It never takes a record to be there that is not.
2. It meets the observation's quality where it can.
3. Where it cannot yet, a defined coarser answer stands — never a hole.
4. It never goes past the arena.
5. Without a cache the result is the same.

**What is on the screen is a frontier with no gap.** Every visible region
is covered by some piece that is ready, and a refinement replaces a piece
with its children whole, at once, only when they are all ready — the
stale-until-replaced the chunk table already does, as its law. So the
arena holds the frontier and the refinements under way beside it:
`arena ≥ frontier(τ_eff) + refinements in flight`.

**Waiting and choosing coarser are two things.** A fault, a build backlog,
a slot not yet free: the goal is unchanged and a coarser ancestor shows
while the frontier converges. A frontier that at the goal cannot fit the
arena at all: no amount of waiting converges, and the quality itself is
chosen coarser. That choice is not the materializer's to make alone — it
would draw coarser than demand fetched — but a **quality policy**'s, whose
effective quality both demand and materialization read:

```text
τ_eff = the finest τ ≥ τ_target with frontier(O, K, τ) + headroom ≤ arena
```

with O the observation and K what the index lanes know. It is decided from
the view before anything is allocated, never from a refusal. It moves as an
AIMD does: coarser at once when the view needs it, finer a step at a time
once there is room — so the arena is never over, and the quality comes back
without flapping. K tightens only as finer lanes arrive, so what it adds is
convergence in one direction, not a loop.

**The workspace is not what is kept.** Building a piece uses a workspace
(`.fixed.bake`, one mesh open); what is drawn across frames is a derived
render residency (the arena's slots). They may share memory; their
contracts are two.

**A leak marked.** Places' badges and decor standing on far, coarse tiers
still read their height off the drawn mesh (`Scheduler::surface_at`): a
derived representation deciding what a query answers. Either the coarse
fold is a rule the query can reproduce, or nothing may stand on it.

**Knobs are outside.** Every number a person would turn — the arena's size,
the refinements in flight, AIMD's step and wait, τ_target, the prefetch's
slack and allowance, the retry delay — is a parameter, named once where
parameters are, with the reason beside it, and read by what uses it; none
is written into a mechanism.

## The frontier's goal is what is observed, not what is occupied

The materializer merged draws: whole moves, admission by free slots, one
owner of quality. What it converges to is still the volume's: every
occupied piece the rule cuts the view into, hidden ones included, so its
worst case is a ball, r³, and the quality policy would coarsen a steep view
far more than its screen asks. That count was never the observation's.
Looking does not take in a volume; a direction answers with its first
visible surface, what is smaller than a sample is not asked for, and what
is far is folded by the level. What must be drawn at once is bounded by the
screen.

So visibility decides depth, not existence:

- **A coarse cover always stands.** The floor's pages are drawn as pieces
  all around, so every region of the view enters the frontier coarse and
  only then refines. A region seen for the first time shows the floor's
  answer, not a hole. Before the floor has arrived — a cold start, a jump —
  nothing is known at all, and that is a stated state, "no answer yet", not
  a gap in a frontier: the frontier is without gaps once the floor is
  resident.
- **Refinement follows what is observed.** A piece is refined only where an
  observation of the screen lands on it. Hidden, smaller than a sample, or
  not yet known: it stays at the depth it has.
- **An observation is an angular cell**, a small area of directions, not a
  ray. A ray through its center is the first estimator of what the cell
  sees; a cone, several samples, or the GPU's feedback are better ones, and
  none changes what a cell means. A center ray can miss a sliver between
  two cells; the sliver then stays coarse, which is quality, not a hole.
- **The cells are derived.** A piece of any level covers about the same
  size on the screen — its cells times τ — so a cell half that size sees
  every piece at least once; the grid follows τ_eff, and the half is the
  knob.

The frontier then holds `cover + cells × (children × levels)` at most — a
bound of the screen, whatever the world holds. What the world's depth still
costs is the traversal that finds each first hit, not what is held drawn.
The quality policy, which answered a volume's worst case, becomes the last
valve for a device whose arena is small.

A region of the view is in one of three states: **unresolved**, nothing
known of it yet; **covered coarse**, the floor's answer or any coarser
piece; **covered fine**, refined as far as it is observed. The floor moves
a region out of unresolved, and nothing moves it back: once covered, a
region's coverage is never lost. A server that does not answer only keeps a
region unresolved longer; the drawing never loses what it had.

**Prior art, and where this is not it.** Unreal's Nanite shipped the same
principles at scale: detail chosen by error on the screen, a cut through a
hierarchy that changes by whole groups, a coarse root always resident, a
fixed pool, visibility limiting refinement, streaming driven by what the
selection needed. It is a sanity check, not a model to follow: its
hierarchy of triangle clusters is its world, while here the world is
records of different shapes, an observation is defined before any of them,
and what draws — raster, rays or both — is a solver behind the
materializer.

## What a cell must see, and what it may leave coarse

Built and merged: the goal is observed, and on realistic views it is 46 to
264 pieces where the volume rule asked 115 to 729; two hundred stacks
behind a ridge do not move it. One ray through each cell's center showed
where the estimator is short: from 2u over flat ground, the ground from
30u to the horizon stayed coarse, because a piece seen at a grazing angle
is squashed below a cell's height and no center ray lands on it.

A cell is a finite resolution, so nothing a cell cannot tell apart has to
be kept — a far pole thinner than a pixel is folded into the coarse piece
around it, and that is the observation, not a loss. The rule is the
projected error:

```text
refine P  ⇔  P contributes to some cell's footprint  and  P's error on the screen > τ
```

At a grazing angle the error is two things. Along the screen's depth a
pixel's footprint stretches over many cells, and there the detail is
averaged into the pixel whatever is held. Across the screen, and at a
silhouette, it does not stretch: a coarse cell at 100u to 400u is 12 to
23 pixels wide against τ of 6, and a height's error shows as `P · ε / r`.
The level by distance alone is the choice by the footprint's short axis, as
anisotropic filtering chooses a mip, so the ground the center ray missed is
error over τ and must refine; the estimator missed it, the rule did not.

So an estimator reports what a cell's footprint touches, not a point in
it. For terrain the first one casts the cell's top and bottom rays and
takes the stretch of ground between their hits: every piece on it
contributes to the cell. That is what the cell covers, not every geometry
in it; a piece refines only where its error is over τ. A corner's rays, a
cone, or the GPU's feedback are later estimators of the same thing.

**The floor is the source's.** The floor is the coarsest level the source
supplies whole — the pyramid's top from the bucket, the dev server's
`--top-level` locally — declared by the source, not guessed from what
fails. It is a property of the storage, not an exception for dev; the law
is unchanged: once the source's floor is resident, coverage is never lost.

**What costs, and where it is cut.** A rebuild cast a cell grid for every
step of the quality ladder, 131 rays, 2.5 to 3.1 ms natively. The policy
needs only whether the step in force still fits, and whether one finer
does; and a small move of the eye can reuse the last cells' answers, to be
cast again where they are stale — acceleration, not a cache anything rests
on.

**Decor is its own solver.** Its record is placements over a shared asset;
what is drawn is instances, each at the asset's level of detail its
footprint needs, and far enough away an impostor. Nanite's clusters fit
exactly that, and its weakness — aggregates, like a far forest — is where
the impostor goes. What is common across layers is the observation's
contract, not the representation.

## A cell's inside is don't-care

The conversation went one step further, and this replaces "error over τ"
above with what it meant. A cell of the observation — τ pixels, the least
the observer promises to tell apart — is don't-care inside: drawn finer,
exact, averaged, procedural, some other shape altogether, its own value
included. Optimization comes before precision there. What is not allowed is
for an approximation to leak: to change what a neighbouring cell observes.
So τ is not a tolerance but the observation's quantum, and the one rule is

```text
refine P  ⇔  P contributes to some cell  and  P's error, projected, crosses a cell's edge
```

tested on each axis of the screen. A coarse piece's error is bounded by
what its page already says: its cell's size across, and the span of its
index lane's minimum and maximum in height. Projected, that is `P · δ / r`
on each axis; under a cell, the piece stays as it is. The grazing ground is
the same rule and needs nothing of its own: along the screen's depth the
error projects small and is inside; across it, and at a silhouette, it
crosses and refines. A far pole inside a cell is inside. What is hidden
contributes to no cell and is not tested at all. A cell's top and bottom
rays remain only to find what contributes.

Don't-care is the current observation's. The world's records keep what the
renderer folds away, because another observer can tell it apart: physics,
a shadow, an edit, the camera one frame later. Each has its own quantum or
its own lease; the renderer quotients the world by its own and by nothing
else.

## A box is all the view asks

Until now the domain read the world by column: the ground's bound and the
objects' span over a column, then the z-slices they crossed. Height was a
different kind of thing from x and y in the one place that decides what is
drawn. Now what a box of the world holds is one question
(`pxl-spec space/observation.rs` `Bounds::contents`), and both the demand's
estimator and the view's domain ask it. The kernel answers it
(`world/observation.rs` `Known`) from the terrain's index lanes and the
object store's bands. Those are stored flat, and only the answer leaves.

```text
Contents::Unknown                    nothing over it is resident
Contents::Empty                      provably nothing in it
Contents::Holds { bound, error }     all of it inside `bound`; drawn at the
                                     level, off by up to `error` on each axis
```

Three points came out of reviewing it.

- **The bound is conservative, not tight.** Only `truth ⊆ bound` is
  required. A tighter bound saves work and nothing rests on it, so a layer
  for which tight is expensive may answer loosely.
- **The error is a box, one extent per axis.** The lens projects it as a
  box is projected: its extent on each world axis, weighted by how much of
  that axis the screen's axis takes (`Lens::error`). Terrain happens to
  answer a cell across and its rise up; the domain does not know which axis
  is up. A scalar radius would have lost the grazing case, where depth
  projects small and height whole. The disc the horizontal error used to be
  was an underestimate on the diagonal; the box is exact there. A
  projection divides by depth, so a displacement that moves a point across
  can also bring it nearer. The extent across is therefore taken over the
  distance less the box's diagonal, so the bound is held over, never under.
  A property test holds every corner of random boxes to it: near the eye,
  at a grazing angle, and with depth and breadth together.
- **The cover starts from the observation, not from the world.** An octree
  from boxes coarser than the source's top was considered and dropped:
  nothing coarser than the declared top is supplied, so no box over it has
  an answer. The cover is every chunk of the top meeting the box the far
  edge is stored in (`Observation::reach`). That box is the reach on every
  axis, plus the planet's drop over it, since the planet has a down. It is
  cut to what a position can name (`pxl-spec space/pvf3.rs` `REACH`).
  The number is the observation's alone. Along each axis it is
  ⌊2·far/side⌋ + 2 chunks, or the lane's where the lane ends first
  (`cover_pieces`). The far edge is `SPLIT` cells of the level over the
  top and a chunk `CHUNK_CELLS` of the top's own, so `far / side` is
  2 · `SPLIT` / `CHUNK_CELLS` = 6 at every level. That makes 14 a side, and
  it moves with the criterion: `SPLIT` 256 would make it 18. Height is where
  the lane ends first today: the 16-bit lane and a chunk of the published
  top are both 65,536 units, which meet two chunks either side of zero.
  So the published cover is 14 × 14 × 2 as before, a bound derived from the
  criterion, the lattice, the lane and the source's top, and no knob of its
  own. The knob is the arena it is checked against. At a finer top the bound falls
  from 128 slices of the lane to 14, because the reach ends before the lane
  does. The arena is sized to this bound, not to what is drawn: what is
  drawn is still bounded by what the screen's cells observe.

Occupied is not yet contributes. Terrain answers a surface — a cube wholly
under the ground's lowest holds nothing — and objects answer by the bands
they are in. A solid mass of objects would still answer every inner cube
as held. What contributes is the observer's to judge, above this query,
and the query stays the access primitive it is.

The estimator asks the same question of the page lattice's cubes. A
requirement names a region, and a layer's storage maps it to its pages,
by their squares for the pyramid's. What is required lies nearer than its
level's band's end, so along the ground too, and the fault bound holds as
it did.

## A ray passes over what a coarser page already bounds

The query's walk read the ground a region at a time, a region being one
index cell of the level asked: sixteen of its cells across. The criterion
puts a level's band at `SPLIT` of its cells, so a ray crossed about twelve
regions a level, however far over the ground it went. Every page's index
lane already bounds its whole subtree, so the pages resident over the one
asked are a max-mipmap of it, and nothing new is stored to use them.

Before a region is read, the resident pages over it are asked, coarsest
first, whether the ray passes over all they hold (`Access::clear`). The
first that proves it carries the ray to its cell's far edge. That cell is
`2^k` of the level's regions across for a page `k` levels up, and never
past where the level asked changes. What proves it differs by layer:

- the ground is solid under its surface, so it is passed only above its
  highest;
- objects stand in their span with air under it, so they are passed under
  as well as over.

A page that is not resident proves nothing and is passed by. It never lets
the ray skip what it would have held, and the answer, exact, coarser or
nothing, and the page it lacks are what they were without it. The descent is
as deep as the levels over the one asked; what a ray costs is the nodes it
visits, fewest far over the ground and most along a slope it grazes. No
bound on a whole ray is claimed.

The GPU `Observer` is held back. A rebuild casts about a hundred corner
rays, and moving them would mean the records on the GPU and a readback a
frame or two late. It becomes worth it when tiles shrink toward the cell;
then it would be a second `Observer`, perhaps one that leaves only a
compact list of pages to read back.

## The cell is an angle, and a screen only calibrates it

"τ pixels" was a translation. In the code the cell was always an angle,
`τ / SPLIT` radians. Pixels came in only through the reference view
`SPLIT` was stated at: 1080 tall at 50°. The observer's frame was already
the eye's own, built from the frustum as forward, right and up
(`Lens::of`). The leak rule measures an error as an angle on the screen's
two axes, square to its direction. So the observation was already a chart
of directions about the optical axis, not of the world's axes. Only two
things tied it to a plane, and both are now changed.

- **The target is set from the screen.** The cell is to span
  `CENTER_CELL_LOGICAL_PX` logical pixels centered on the optical axis.
  The plane stands `height / (2 · tan(fov / 2))` of them from the eye, so
  the angle is exact, not a small-angle approximation:

  ```text
  Δα       = 2 · atan(CENTER_CELL_LOGICAL_PX · tan(fov / 2) / height)
  τ_target = max(1, SPLIT · Δα)
  ```

  (`world/quality.rs` `target`). The pixels are logical ones: CSS pixels
  on a page, points on a device. A device pixel is how finely the screen
  is rasterized, which the resolution policy owns. It is not how finely
  the world is told apart, so detail does not follow the device's pixel
  ratio. The logical pixel is not taken for a fixed angle of the eye;
  viewing distance and zoom differ from one person and device to the next.
  It is the product's unit of observation, independent of the raster. The
  six pixels hold only at the optical axis: a perspective screen draws the
  same angle wider toward its edges, and the angle is what is canonical.
- **The floor of τ is a sizing, not a limit of the data.** τ = 1 is
  `SPLIT`, the finest cell the fault bound, the cover and the arena are
  sized for. A screen that asks finer is held there. A finer cell
  everywhere is a smaller `SPLIT`, and every bound moves with it.
- **The estimator's tiles are equal steps of angle** about the optical
  axis on each screen axis, not equal steps of the screen's plane
  (`Grid::corner`). They are equal steps of angle, not of solid angle; over
  a field of view the two differ little, and a tile is the estimator's
  resolution, held to no bound.

The screen is still drawn in perspective. Nothing is resampled through
another lens; the angular chart is where it is decided what to draw and how
finely.

The orbit that moves the eye — a distance and a polar angle about the map
center, with the world turned under it by the azimuth — is orthogonal to
all of this. It makes the frustum, and the observation reads only the
frustum.

## Demand from what the tiles reach

The demand was estimated from bounds alone (`ByBounds`): every page whose
box the frustum can see, in the band of its level. That includes ground
behind a hill or a wall, which the screen never reaches. From street level
it asked for about 37 pages where 3 were seen. Materialization already
refined only where the screen's tiles reach. Demand did not use that, so
the two concerns shared what is known (`Bounds`) but not what is seen.

Now demand reads the same sight (`world/observation.rs` `ByFootprints`).
Its requirements are the ground each tile's footprint touches, at every
level whose band some of that ground lies in, plus what the tiles' rays
lacked. Three rules keep it a demand and not a guess.

- **What a ray lacked is required.** A footprint is what the resident
  records say a tile meets. A coarse page averages its peaks away, so a
  hit on it is not proof the ray stops there. The query already names
  what such a hit lacked (`Answer::Coarser`, `Answer::Nothing`), and that
  lack is required. When the lacked page comes, the tiles over it are
  cast again, so the requirement grows until it is what the exact records
  make it. An exact answer that meets nothing requires nothing.
- **The observer names ground, never a page.** A lack travels as a
  requirement, a level and a region, and each layer's storage maps it to
  its own pages. Places are asked for by the same ground the terrain's rays
  lacked.
- **It is demand, not prefetch.** Only what the current observation reaches
  or lacks is required. What it may reach later stays the prefetch policy's.

The bands are shared with `ByBounds` (`pxl-spec space/observation.rs`
`band`), so the fault bound holds as before. A lack asked at a finer
quality than the effective one is lifted to the level that quality puts
its ground at, so nothing finer is fetched than what is drawn.

Hidden ground is not required at any level: the floor answers it. On the
fixtures, a flat street with boxes, the pages required went from 25 to 21
at street level, and from 25 to 13 in front of a wall. They went from 28
to 18 at 40u, and from 24 to 10 at 600u, looking down. A dense grid of
samples finds every page it answers from among those required.
`ByBounds` stays the estimator the spec's own laws are held against.

## The quality is the policy's alone: a tier fills the slots it takes

Every mesh slot was one fixed size. A tier whose mesh had more boxes than
its slot held was baked a step coarser until it fit (`BuildEvent::Coarser`,
`Slot::fit`). That was a second quality policy, inside materialization,
deciding for itself. A step coarser can leak past a cell. Terrain alone
fits, since a column is a box, so what overflowed was placed objects,
which have no practical bound per chunk. The quality policy only counted
the retreat afterward. Decor stood on the drawn tier and not on the query's
surface, and that was the compensation.

The contract now is that the representation the quality chose is never
changed by a local capacity. Five mechanisms were weighed; a variable
allocation was not assumed.

| Mechanism                    | What it costs                                            |
| ---------------------------- | -------------------------------------------------------- |
| Split instead of coarsen     | the goal would need the mesh's size; a leaf cannot split |
| The policy picks a cube each | moves the retreat to the policy; still leaks past a cell |
| Fewer boxes (faces alone)    | lowers how often a mesh overflows; sets no contract      |
| Several fixed slots, chained | a draw per slot a heavy tier fills                       |
| Slab or buddy allocation     | fragmentation, relocation, stable handles, GPU sync      |

The chained slots were taken, because the allocation knows slots and
nothing of why a tier needs them.

- When a bake knows its size, every slot the mesh fills is held for it at
  once, or none is (`BuildEvent::Reserve`, table.rs `reserve`). Holding
  some and waiting for the rest would let two tiers each hold part of
  what the other needs. Where there is not room, the bake is thrown away
  rather than wait with the one bake open. How many slots the tier needs is
  kept for its generation and cube (`Slot::need`), and it is not asked for
  again until that many are free.
- The mesher writes a slot's worth at a time (`Step::Full`), so the
  workspace stays one slot however large the mesh. Each block goes to its
  slot as it is written, and the slots may be anywhere in the pool,
  chained in the order they are written (`Meshes::next`).
- The frontier and the quality policy count each piece by the slots it is
  known to fill (`Env::slots`, `need`). A piece not yet baked counts as one,
  and is counted right from its first bake on. Where the goal is past the
  arena, the one policy draws it coarser.
- A heavy tier is a draw per slot, sharing its uniform.
- Progress is proved in slots, not pieces. The frontier's old argument was
  that one coarsening is always possible because it takes a slot and gives
  one back. That no longer held once a piece could fill several. It holds
  again from a bound on the representation, not on the meshes met so far.
  A coarsening lands one piece over z16, so in the coarse pool. Every tier
  there is drawn at a step of two or more, where a cell is at most its box
  and a floor, so a chunk bakes to at most 2·32³ boxes, which is 15 slots
  (`COARSE_TIER_SLOTS`). Refinements, fills and changes of cube leave that
  many coarse slots free (`RESERVE`), and none in the fine pool, where no
  coarsening lands. What moves already under way hold is counted out of the
  free slots and not again in the reserve. The quality policy's headroom is
  that reserve, which is progress, and a slot for each refinement under
  way, which is throughput.
- A bake's slots are its own until it lands, and free the moment it
  closes without landing (`settle`, after every call that can close one),
  not at the next bake's reservation.

A mesh written through blocks of any size is the mesh written whole
(`a_mesh_written_through_blocks_is_the_mesh_written_whole`). A move whose
pieces fill several slots starts only with every one of them free
(`a_piece_filling_several_slots_starts_only_with_all_of_them_free`), and
a coarsening starts with only the reserve free where a fill does not
(`a_coarsening_is_always_started_with_only_the_reserve_free`). A claim is
all of its slots or none, each chained once with no cycle, and letting a
chain go frees exactly what it took
(`a_claim_is_all_of_its_slots_or_none_and_letting_it_go_frees_them_all`).
Decor's reading of its tier now covers only the tier's cells averaged
2 × 2, inside a cell, and goes when decor becomes its own solver.

## The cover holds completeness; refinement holds quality

The arena's assert, the cover and the headroom inside the coarse pool,
added pieces to slots. That held while a piece was a slot. Once a tier can
fill several, the cover's 392 pieces could, at worst, be 392 × 15 slots. A
piece of the top was drawn from the level's cells averaged 2 × 2 up to the
coarsest cube, by distance, so it was not structurally one slot.

The cover's job is only that no ground the floor answers goes undrawn. So
it is drawn in the coarsest cube, one cell a chunk, at most two boxes, one
slot, whatever it holds (world/chunk/frontier.rs `drawn_cube`). Detail
comes from refinement alone. The cover's count is its slots by its
representation, and the assert is a proof in one unit.

A piece is measured as it is drawn. A level's own cell is half the cube
its cells are drawn averaged into — the 2 × 2 is by design, like a
retina's pitch against its pooling — so the error of a cube is half its
side across, which for every level under the top is the level's own cell
as before. A piece of the cover is therefore half a chunk across, and up
the one box's whole height from the layer's bound (world/pyramid/domain.rs
`look`). Where it is seen that closely it splits; where it is hidden it
stays as the floor. The size of the primitive drawn and the error it can
make are not the same thing: a cube twelve pixels across whose error is
held to six is inside the cell.

## How the samples are drawn: the steps, measured (10-01)

Before a shader is written, the query itself says how many steps a cast
pays. `pxl-kernel examples/ray_steps.rs` counts what `first_hit` visits
(world/access.rs `Access::counting`: each region read and each cell
walked) over real terrain from the tile server, at Namsan, from the
experiment's three views on an iPhone 13 mini's screen, one ray every three
points:

| view                      | steps a ray: mean / p50 / p90 / p99 / max | a frame at ratio 3 |
| ------------------------- | ----------------------------------------- | ------------------ |
| from above, 1,500 u up    | 18 / 17 / 26 / 35 / 46                    | 50 M               |
| at 45°, 400 u up          | 27 / 27 / 36 / 44 / 55                    | 75 M               |
| at street level, 1 u, −5° | 22 / 4 / 87 / 116 / 136                   | 60 M               |

The mean is tens of steps whatever the view, so the walk over the max
pyramid is not ruled out as a phone's cost. What a GPU pays is not the
mean: rays run in groups that wait on their longest, so where on the
screen the long rays lie decides it. Drawn as heatmaps
(`crates/target/ray_steps/`):

- The steps are near constant in blocks that are the index cells projected
  onto the screen, and neighbours within a block differ by a few steps: a
  group pays about what each of its rays pays, except along a block's
  edge.
- From above, a disc about the nadir costs more: the finer band of
  distance, a level more under it.
- At street level the near ground below the horizon is cheap, a few steps a
  ray. What is dear is not a line at the horizon but all that lies above
  the near ground — the far rays and the rays into the sky. Part of that is
  the dev server's: it serves pages to level 3 only, so a ray past what
  level 3 covers finds nothing over it resident, cannot be passed over, and
  walks region by region to its reach. With the top resident, a ray over
  the highest ground clears there in a few tests. And a ray rising above
  the highest ground in its reach can meet nothing more, an exit the query
  does not yet take.

Two things this review corrected:

- **A cell is an angular footprint, not a sample point.** One ray through a
  cell's centre, its answer spread over the cell, is not the rule: a pole
  thinner than the cell that the centre ray misses still covers part of
  it. What the rule asks is a bounded amount of observation a cell — O(1)
  work that stands conservatively for the cell's footprint — and how that
  is done (a centre ray with the hierarchy's bounds, corner rays, adaptive
  subdivision, a cone through the pyramid, reuse over time) is the
  experiment's to choose. Fixing the primitive at one ray a cell too early
  would fix the representation before the measurement.
- **A raster is not bound to the world.** Frustum and occlusion culling,
  levels of detail and clusters bound a good raster pipeline near the
  screen too; Nanite is one. What differs is the direction of evaluation: a
  raster materializes the world's visible geometry and then samples it, an
  observation starts at the screen's samples and queries the world. This
  tree already has the second's parts — storage, the query, demand,
  residency and materialization apart, and a query (`first_hit`) that no
  renderer owns.

The prior art for drawing from the screen into a heightfield, beside Dick,
Krüger and Westermann's terrain, Teardown and Aokana above: NovaLogic's
Comanche (1992), whose Voxel Space engine walked a ray a screen column
through a height map and drew the column from the nearest hit up, the
first shipped renderer whose cost was the screen's and not the terrain's;
and Nanite, which holds a frame's cost to its pixels through clusters
chosen by their projected error — the same bound reached from the raster's
side.

So the experiment compares four, on the same pages and views:

| shape                      | what it answers                                    |
| -------------------------- | -------------------------------------------------- |
| today's boxes, rasterized  | the baseline                                       |
| the same as masked faces   | whether the vertices' output was the phone's cost  |
| a ray a pixel              | the upper bound of observation done by brute force |
| bounded observation a cell | what this architecture asks for                    |
