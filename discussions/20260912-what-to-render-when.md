# What to render when: one radius, one criterion, four classes

Five symptoms were reported between 2026-09-11 and 2026-09-12, and they turned
out to be four separate causes with one thing in common: the renderer decided
_what to work on_ from quantities that were either quantized to the wrong
unit, stale, or measuring the wrong thing. This document records the causes and
the rules that replace them, so the next change argues against a stated rule
rather than against whatever the code happened to do.

The rules are the ordinary ones for this kind of renderer. Where a choice here
matches a convention, that is on purpose; where it departs, the departure is
stated.

## The symptoms

1. One on-screen chunk waited sixteen seconds while the scheduler ran at 119
   bakes/s (recorded in
   [20260909-cooperative-scheduler-and-task-ledger.md](./20260909-cooperative-scheduler-and-task-ledger.md)).
2. After ageing was added, zooming in from a wide view refined in visible
   steps, 4 then 2 then 1, with the intermediate tier clearly held.
3. Sometimes on a zoom-out the far ring filled before the middle of the screen.
4. Crossing an L3 page boundary made a page-sized block of terrain vanish, on
   whichever side was behind the crossing.
5. Zoomed out, on-screen chunks waited up to eight seconds with ~1,100 pending
   and bakes running 1-2 per frame.

## Cause 1: a wait that was not the wait it looked like

`pending_since` was set when `pending` went false to true, and an off-screen
chunk stays pending for as long as its release is queued. So a chunk that sat
off screen with a queued release for several seconds came back into view still
holding that clock, and ageing treated those seconds as time spent waiting to
be built. Symptom 3 is exactly this: the far ring had been off screen, had
aged, and outranked the middle of the screen, which had just become
interesting and had no age at all. Symptom 2 has the same root on the
refinement side.

A release and a build are different jobs. The fix is that the clock belongs to
the _request_, not to the flag: when what a chunk is asking for changes, the
wait restarts.

## Cause 2: one priority key for four different kinds of work

The key was `holds_slot`, then `in_frustum`, then distance, with ageing on
distance. That puts "this chunk has nothing on screen at all" and "this chunk
would like to be sharper" in the same bucket, and lets ageing trade one
against the other. Streaming systems normally separate the two, because they
are not the same promise: a hole is a correctness problem, a refinement is a
quality problem.

The ordering, in full:

1. **Slot holder.** One scan slot and one bake slot exist for the whole world
   (`voxel_scan.ts`, `chunk_mesher.ts`); whoever holds one finishes first or
   nothing else can start.
2. **On screen before off screen.** An off-screen chunk's only pending work is
   a release, and a release can never outrank a build.
3. **Nearest first**, aged by how long it has waited.
4. **At equal distance**, a chunk with nothing drawn before one that is merely
   coarse.

Emptiness was tried as a class of its own, directly under the frustum test,
and it was wrong on screen: every hole anywhere outranked every refinement, so
ground a few metres from the camera stayed visibly blocky while chunks at the
fog edge filled in. "Already drawn" had become "done". What a viewer wants is
the nearest thing to improve next, whether improving means appearing or
sharpening — so distance decides, and emptiness only breaks a tie.

Ageing cannot lift anything across the frustum test, so a release still never
passes a build.

Class 4 gets a small guaranteed share of each frame anyway, two steps, because
"lowest priority" and "never" have to be different things. In the 2026-09-12
capture 403 releases waited 10.4 s while heap peaked at 522 MB, and a pending
release is holding a mesh the pool could reuse. A guaranteed minimum share for
the lowest class is the standard shape here, not ageing it up into the classes
above.

## Cause 3: priority computed from stale measurements

`recheck` did two jobs in one method: the cheap measurement (where is this
chunk, is it on screen) and the expensive decision (which tier, blend state,
layout). Because the expensive half has to be rationed, the cheap half was
rationed with it: at `LOD_RECHECK_INTERVAL` (0.15 s), truncated further by the
frame budget. So the commit loop sorted several thousand controls by numbers up
to 150 ms old, and during a zoom-out 150 ms is the difference between "the
middle of the screen matters" and "it did not exist yet".

These are now separate. `refresh` runs for every control every frame and only
measures; `recheck` keeps its interval and its budget and only decides.

To make the per-frame pass affordable, visibility is one bounding-sphere test
for every chunk rather than a sphere for unbuilt chunks and a transformed box
for built ones. A sphere that encloses the measured box never culls anything
the box would have kept, and costs six plane dots against eight transformed
corners. Conservative culling is the normal trade here, and one rule is worth
more than two slightly different ones.

## Cause 4: the tier walk built tiers nobody asked for

`step_lod_toward` moved one adjacent tier per commit, so a 4 to 1 zoom-in
built 2 in full first: a scan, a bake through the single global bake slot, a
commit, and only then the tier anyone wanted. That is symptom 2's other half,
and it is worse than a delay: the wasted bake blocks every other chunk in the
world, because there is one slot.

Nothing needed it. `commit` keeps the current layout while a build is in
flight (`build_buckets` returns null and the layout is left alone), so the
coarse mesh already on screen stays there until the wanted tier is ready, and
a chunk with nothing built was already exempt from the walk. The stated reason
was to avoid jumping 64 to 1, which the tier cache already covers. It was
never a recorded decision; it arrived with the first version of the pipeline.

So: straight to the tier the criterion asks for. And a bake whose tier is no
longer wanted is aborted rather than finished, for the same reason the walk was
expensive: the slot it holds belongs to everyone.

## Cause 5: visibility quantized to the page

This is symptom 4, and the old comment in `terrain/config.ts` had already
written it down: "Worst case (camera at the far edge of its own page, looking
outward) the render boundary is only (r-1) full pages away ... the seam can be
visible on a hard zoom-out, left that way on purpose."

Concretely, three radii were chosen independently:

- `LOAD_RADIUS` 1, in pages, Chebyshev around the page the view centre is in.
  Data therefore reached 1,024 u at worst.
- `RENDER_RADIUS` 2, in pages, the same shape, gating which pages may
  contribute chunks at all.
- `FOG_FAR` 1,800 u, in world units, where the picture claims ground ends.

Two consequences. The nominal view distance was never real: geometry ended
somewhere between 1,024 u and 1,800 u depending on where in its page the
camera stood. And because both page filters keyed on _which page the centre is
in_, crossing a boundary moved the whole admitted set by one page, so a
page-sized block entered or left in one step.

The replacement is one number:

- `render_cfg.RENDER_DIST`, in world units, is the range. Fog ends there, and
  chunks mount within it plus one chunk of margin.
- The pages needed are derived: every page whose 1,024 u square comes within
  that distance of the view anchor, tested as circle against rectangle, the
  same test `in_range_chunks` already applies per chunk.
- Residency is a count, not a radius: the nearest 32 pages stay decoded and
  the rest are dropped.

The page stays the streaming unit. It is one PostGIS query, one bake, one
~4 MB decode, one `tile_map` key, one eviction unit, and none of that changes.
What changes is that the _set_ of pages is computed from the range instead of
being a hand-picked page radius. This is the usual arrangement: a render
distance in chunks, with the storage unit grouping chunks for transfer, and
the loaded set derived from the distance.

Because the anchor is chunk-aligned rather than page-aligned, pages enter the
set when the disc reaches them, which is continuous as the view moves. There is
no boundary event left to pop.

### Choosing the range

Worst-case pages touched by a disc of radius R, over every anchor position
inside a page:

| R       | pages | decoded |
| ------- | ----- | ------- |
| 1,024 u | 12    | ~48 MB  |
| 1,536 u | 16    | ~65 MB  |
| 1,664 u | 21    | ~85 MB  |
| 1,800 u | 21    | ~85 MB  |
| 2,624 u | 37    | ~149 MB |

For reference, the previous fetch window was 9 pages (~36 MB) and its
eviction radius allowed 49 (~197 MB), which is the configuration that crashed
an iPhone 13 mini.

`RENDER_DIST` is 1,536 u. It is wider than the 1,024 u that was actually
guaranteed before, it is honest at every position rather than only at the
middle of a page, and it sits well under the known crash point. The nominal
number goes down from 1,800; the delivered number goes up.

One distance and one count follow from it:

- Chunks mount to 1,664 u — two chunks past the range. One chunk so a far
  edge cannot clip mid-fade, one for the margin the invariant below needs.
  That reaches 21 pages at worst.
- Which pages exist, both to fetch and to keep, is the nearest **32 pages**,
  ~129 MB. A count rather than a radius, because what is worth bounding is
  memory — a tab has already been lost to holding too much of it — and a
  count bounds it directly at any zoom.

Getting there took two wrong answers, both worth recording, because they
failed the same way on screen: a page with no tile is not drawn coarsely, it
is not mounted at all, so its whole square vanishes and returns. That reads as
terrain cut with a knife along a page boundary.

The first was a radius: a quarter page of hysteresis past the fetch range. It
held barely more than the range fetched, so panning out of a page and back
refetched it. The configuration this replaced held 49 pages (~197 MB) and hid
that by brute force.

The second was subtler and survived the first fix. Fetching and keeping were
still two different rules — a radius for one, a count for the other — so a
page could be asked for and then fall out of the fetch set while its request
was still in flight, and `fetch_scheduler` sheds demands whose category no
longer wants them. Panning cancelled its own tiles: 127 terrain requests
aborted in one session, in bursts on whichever side the view was moving away
from, which is why the two sides took turns disappearing. Making the two rules
one set fixes it by construction — a page being fetched is, by definition, a
page worth keeping.

## Cause 6: the criterion itself

Tier selection used a table of world distances: 192, 384, 768 and so on,
doubling with the sampling stride. That table is the screen-space error rule
frozen at one viewport and one field of view. Written as the rule it is:

```
handoff_distance(tier) = cell_size(tier) * viewport_h / (2 * tan(fov/2) * TARGET_CELL_PX)
```

A tier hands off when one of its cells would project to about `TARGET_CELL_PX`
pixels. This is the standard criterion for chunked terrain LOD, and the same
projection factor the billboard field and the distance fade already compute
for their own screen sizes.

`TARGET_CELL_PX` is not a fresh number: it is derived from the ladder that was
already there (192 u for the finest tier at 1080 px and 50 degrees), which
works out to about 6 px. So this is a change of reasoning, not a retune, and
the thresholds are unchanged at the reference projection.

Viewport height is in CSS pixels, not device pixels. At dpr 2 the same cell
covers twice the device pixels, and following that would double the geometry a
retina screen builds; supersampling is what dpr is spent on here
([20260911-taa-tried-and-closed.md](./20260911-taa-tried-and-closed.md)), while
tier selection is about how large a cell looks.

## Why the ranges are anchored at the view centre and not the camera

The camera orbits the view centre at up to 1,023 u of horizontal offset, so
"where the view is" has two candidate answers. They are used for different
things:

- **Which tier a chunk shows, and which chunk is built first**, are
  camera-relative, and always were. Both measure the chunk's own 3D bounds
  centre against the camera position, so a chunk whose content stands 100 u
  above the terrain is ranked where it actually is rather than where the
  ground under it is. The one exception is a chunk that has never built: its
  seed assumes z = 0 until its first real measurement replaces it.
- **Which pages exist** is anchored at the view centre, because residency has
  to be invariant under rotation. Anchored at the camera, spinning at a wide
  zoom would sweep pages in and out of the cache and refetch them, against an
  API with no cache or rate limit of its own.

Anchoring the range at the centre is only sufficient because the camera always
looks at the centre, which is what keeps the visible wedge near it. That is an
invariant rather than a coincidence, so it is measured instead of assumed:
`render/view_range.test.ts` casts the frustum at the ground across every zoom,
tilt, aspect ratio and pivot height the camera allows, and checks that ground
the camera can actually see lies inside the distance chunks are mounted to.

Pivot height is the case that reaches furthest, and it is one a user creates:
a placed object can stand above the terrain, and the camera rises with
whatever it looks at. Zoomed close onto something 50 u up, the camera sees
nearly a full fog radius in every direction and visible ground reaches
~1,532 u from the centre. Mounting to 1,664 u leaves 132 u, which also has to
cover the anchor's own chunk quantization, up to ~90 u. At 1,600 u it would
not have.

## Cause 7: the page component destroyed its chunks on every replay

Found last, and it was the one behind the symptom that survived every fix
above: pan across a page boundary and a page-sized square of terrain is cut
off and redrawn, on whichever side the view is moving away from.

React's StrictMode replays every mount as mount, cleanup, mount, in
development only. `PageChunks` owns its chunk nodes imperatively, and its
cleanup destroyed all of them, so the replayed mount rebuilt the whole page
from the scan up. Measured with the mounted-page set logged: with StrictMode
on, one zoomed-out pan unmounted five pages and disposed up to 510 nodes a
frame, none of which had left the mounted set; with it off, the same pan
disposed nothing. Every development measurement before this carried that cost
— 17,031 mounts against 16,727 disposals in one thirty-second session.

The fix defers destruction by one microtask and lets a mount arriving inside
that window cancel it. A real unmount is never followed by a mount in the same
microtask, so it destroys exactly as before. `AnchorNode` already survived
the replay for the same reason. After the fix, three minutes of load, zoom
and panning under StrictMode disposed zero nodes.

Two smaller causes were found on the way and fixed too:

- **Origin rebase.** When the floating origin crossed an L4 (4096 u), anchor
  groups had their local matrices updated but not their world matrices, which
  three recomputes at render — after the tick. The visible pass therefore
  measured every chunk 4096 u from where it was for one frame, culled all of
  them, and an off-screen chunk gives its mesh back. Now `updateMatrixWorld`
  runs on the rebase frame.
- **No hysteresis at the range edge.** A page whose nearest corner sat
  1,680 u out against a 1,664 u range was unmounted with every chunk in it,
  and panning 16 u back rebuilt them all. Chunks and pages are now built at
  the range and kept to four chunks past it (`RANGE_HYSTERESIS`).

## Cause 8: ageing that outran the queue

After everything above, a hard zoom-out filled the screen as a ring: the
middle first, then the far edge, and the band between them last.

The half-life was 500 ms. A full zoom-out queues ~2,300 visible chunks
against one scan slot, about seven seconds of work, and at that half-life a
chunk at the fog edge — 2.4 times the distance of the middle of the screen —
overtook the middle after 640 ms of waiting. So the real order was a little
of the middle, then the far edge (cheap to reach through ageing), then the
band between, which was neither first nor aged enough. Ageing had stopped
being a floor under starvation and become the order itself.

The rule is that the half-life must be long against the queue's drain time.
It is now four seconds: the same overtake takes five, so a seven-second drain
stays nearest-first almost throughout, while a chunk passed over for sixteen
seconds — the case ageing exists for — is still lifted well before that. The
overlay's `STARVED_MS` moves to ten seconds for the same reason: above any
ordinary drain, below the wait that counts as being passed over.

## Cause 9: a release committed as a build

Fixing cause 3 opened this one. Once measurement (`refresh`, every frame)
and decision (`recheck`, rationed) were separate, a chunk could be handed a
release while off screen, re-enter the frustum on the next zoom-out, and be
seen by `commit` as on screen while still holding that release — for up to
`LOD_RECHECK_INTERVAL`. `commit` took the on-screen path and baked what the
request said, and what the release said was `OFF_SCREEN_LOD_STEP`: one 64u
box per chunk, a tier the ladder never asks for inside the view, standing in
the middle of it (2026-09-12).

A request is now only valid on the side of the frustum it was made for. A
release is committed only off screen and a build only on screen; a mismatch
commits nothing and waits for the next `recheck` to ask for what that side
actually wants. The reverse mismatch — a build request that drifted off
screen — was already safe: `recheck`'s off-screen branch aborts the held
work and replaces the request, and a refused commit does not block the
queue.

Two release-order findings from the same session, recorded with it:

- **Release farthest-first.** Off-screen work was sorted by the same
  ascending key as builds, so releases went nearest-first. A release gives
  memory back and should take it from what is least likely to be wanted
  again, and nearest-first did the opposite: a short zoom-in followed by a
  wide zoom-out found the near chunks already gone and the far ones still
  holding stale meshes, drawn as a ring beyond an empty band. The off-screen
  class now sorts descending.
- **An in-flight fetch is not withdrawn for a small drift.** The 32nd-nearest
  page changed with every 64u of panning, and `fetch_sync` yields what a
  category no longer wants, so panning aborted its own requests at the edge
  of the held set — 200 terrain aborts in one session. A page whose request
  is running stays in the wishlist while it is within one page of the held
  set; a search jump still yields as designed.

## Not a cause: tile requests cancelled at boot

Listed as open after the session above and closed on measurement: the map
centre does not move during boot. Logged with its caller, it is seeded once
(879 ms) and then untouched until the first drag (12 s). The bursts of
cancelled requests that looked like boot were two other things, both
intended: a search jump, which yields every request for the place being
left (the geocode request cancelled alongside gave it away), and a
zoomed-out drag, which crosses up to 25 pages per frame — at that speed the
page set is replaced wholesale every frame and the one-page in-flight
hysteresis cannot apply, nor should it. The vite proxy also logs each
cancellation twice, so every count read off that log was double.

What would be better than cancelling is asking for less while moving, not
asking for nothing: a map still has to fetch what has just scrolled into
view mid-drag, or the user drags into blank. The usual shape is that only
the pages the view reaches right now are requested while `view_moving` is
set, and the resident margin around them — the rest of the nearest 32 — is
filled in once it clears. The pages needed at the destination arrive at
once, and the margin stops being requested and cancelled once per frame on
the way there. Worth doing; not urgent.

## Keeping what was built: a memory budget instead of a visibility rule

Every chunk already has a cache: its scanned run set and up to four baked
tiers, reference-counted so a cached mesh outlives the scene object that
draws it. What decided the cache's lifetime was visibility — leaving the
frustum dropped all of it on the first release, whatever memory looked like.
Two findings above say that is the wrong key. Chunks whose release had not
run yet came back into view with their meshes intact and drew at once; and
a camera rotation, a zoom in and out, a pan and back all leave the frustum
and return within seconds, each time rebuilding from the scan up.

The rule is now the ordinary one: keep what was built until memory says
otherwise. A release detaches the scene mesh and nothing else. What every
node retains — run sets and cached tiers, both in the core pool and sized in
bytes — is summed world-wide, and when the sum is over `RETAIN_BUDGET_BYTES`
the stage reclaims from off-screen nodes, farthest first, until it is under.
Farthest-first is the release order already established above, for the same
reason: what is least likely to be wanted again goes first.

The budget covers only what is held off screen, and that took one wrong
version to learn. The first draft budgeted the world-wide total, and the
first report under it read `kept 172 / 128MB budget` with nothing being
reclaimed: at a wide zoom, 602 on-screen chunks hold 61 MB of run sets and
697 tiers at ~163 KB each, which is 172 MB on its own, and none of it is
reclaimable because it is the layout. A budget over the whole would either
be unmeetable there or be set so high it never fired anywhere else. So bytes
are accounted by which side of the frustum their node is on — a node
crossing moves its bytes across — and the budget is drawn on the off-screen
side alone: a quarter of the core reservation, the currency the crash case
was measured in. The on-screen side is reported beside it as what it is,
the working set, and if that alone ever looks too large the lever is the
range, not the cache.

## What this does not change

- The page as the unit of fetch, decode, cache and eviction.
- The dither blend band and its per-chunk hash
  ([20260909-lod-look-and-seams.md](./20260909-lod-look-and-seams.md)).
- One scan slot and one bake slot for the world. That is the throughput cap,
  and it is deliberate: a chunk's scan is a multi-frame stepped job and two at
  once would double the peak transient memory.
- The ledger stays read-only. None of these numbers are read back into a
  scheduling decision
  ([20260909-cooperative-scheduler-and-task-ledger.md](./20260909-cooperative-scheduler-and-task-ledger.md)).

## Left open

**A coarse tier still pays a full-resolution scan, and a strided scan is not
the answer.** `begin_column_scan` takes no tier, so a chunk that will emit
sixteen boxes at step 16 still scans every column at ~3.2 ms, and zoomed out
that is the whole cost: ~2,300 visible chunks against one scan slot is a
seven-second drain whatever the order. Scanning every stride-th column would
cut it, but a coarse cell is _defined_ as the fold of every column under it
(`begin_coarse` accumulates all of them, and `layer_taps` averages four
materials out of them), so a strided scan is a different picture, not a
cheaper one — and the chunk still needs the full scan when it later refines.

The standard shape is to grow the unit with the tier rather than thin the
scan: at step 16, scan and bake a 4x4 block of chunks as one mesh. Sixteen
times fewer scans for the same picture, and the mesh count falls with it.
That is a change to what a "chunk" is at coarse tiers, and it has its own
record: [20260912-coarse-tiers-need-a-larger-unit.md](./20260912-coarse-tiers-need-a-larger-unit.md).
The same measurement that motivates it — 1,079 chunks on screen, 474 MB
live against a 512 MB reservation — is also why the core pool spills at a
wide zoom; the unit is the lever for both.

**Decor is the frame-time bottleneck, not the scheduler.** In the same capture
GPU p95 was 24.1 ms with 2.26 M triangles of decor in the last frame, which is
what made 15% of moving frames miss a vsync. That belongs to the parked decor
LOD work
([20260911-decor-identity-orientation-and-one-cell-format.md](./20260911-decor-identity-orientation-and-one-cell-format.md)).
