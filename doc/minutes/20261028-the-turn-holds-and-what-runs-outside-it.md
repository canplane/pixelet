# The turn holds its budget, and what runs outside it is cut

**2026-10-28** — recorded with the user, by pixelet-d6 with pixelet-44, -53 and
-81. Main was at `d9ce6dc35`. Measured on the session's release builds (no
`pxl`, the panel's Report and Trace) and on a probe export of the scheduler that
was not kept.

## What was asked

The user doubted that the memory layout and the scheduler's quotas hold. A
cooperative scheduler that is never mentioned may be working, or it may be
failing quietly, since nothing had been said of starving, priority, or what is
fetched and in what order. They asked whether it runs stably, and whether any
step should be split. Later they asked how the fetch order should be set, with
outside references.

## How it was read

Three saved traces and the code were read against each other: a zoom-out on
HEAD, the same zoom-out on a baseline, and a release stress run of 1,603 frames.
Those traces fetched no place, asset, earth or division tile, so a second run
turned places on and panned and zoomed at Myeongdong with the pointer moving.
The probe split the `scene` step into its parts, through a note written into the
turn's worst effect.

## Memory holds

- The heap peaked at 67 MB, with 0 `heap_refusals`. The largest free block
  never fell below 4 MB, room for a 1 MB tile body.
- At most 28 of 512 SQ entries and 1 of 1,024 CQ entries were used, and at
  most 8 body slots.
- `cap_terrain_refused` was 0.

Nothing here needs a change.

## The turn holds

- In the three saved traces, no turn ran past its interval (`long 0`), and
  busy peaked at 19 ms. The run with places on did not hold (below).
- The code does what it says. Phases run in their order. The visible phase
  keeps `CONTENT_SHARE`, 20% (pxl-kernel `sys/sched/param.rs`), back for
  content. A pass always runs its first item, so one costly item cannot block
  what comes after it (xpute `sched/pass.rs`). The turn's tasks, arrivals and
  background alike, get that share again past a full frame (pxl-kernel
  `sys/sched/edge.rs` `tasks_after_frame`).
- Content and cosmetic read 0 ms. This is not starvation: no step is
  registered there (pxl-kernel `turn.rs` `boot`), so the share kept back for
  them goes unspent. The fetch arrival task peaked at 0.2 ms.
- The turns are paced at an interval of at most 1000/60 ms
  (`@xpute/host/sched/quantum.ts` `paced`, `PACED_MAX_MS`). The host runs a turn
  on every `divisor`-th refresh, the most refreshes that fit in that interval
  and at least one. So a 60 Hz or 90 Hz display gets its own rate, a 120 Hz
  one 60 and a 144 Hz one 72. The display these were measured on runs at
  120 Hz, so 60 was the ceiling here.

## What runs outside the budget, and is cut

The scheduler cannot cut a step in the middle, so a step that is not budgeted
and does real work is the hazard.

- **Places' `show`.** With places on, the visible phase's `scene` step took
  125–140 ms in a frame, and 59–62 frames passed their tick budget. With
  places off, it took 1.7 ms. The probe put up to 77 ms in
  `domain/scene/marker/place.rs` `show`. The design, the choice among overlaps
  and the instances took 0.2 ms or less.
  - When the zoom changes the detail asked, every visible place asks its
    ground's height again (`place.rs` `top_at`, then
    `domain/scene/marker/arrangement.rs` `surface`). Each ask is a vertical ray
    from the top of the world, and the loop runs whole in one turn. The cells'
    read just before it already runs under the pass, and took 0.1 ms.
  - Two causes add up. The loop is not split. And each vertical ray is long: a
    coarse node at the windows' frontier is unbounded over the whole height,
    because pxl-spec-core `window::fold` writes a zero entry where a block
    holds an absent square.
- **The `puck` effect** took 3.4–4.7 ms a frame. pxl-kernel `turn.rs` holds an
  effect to no real work, since it has no budget. Its cost is one call of
  `domain/scene/indicator/location_puck.rs` `ground`, as main has it. Each of
  about 37 cells asks `surface_support` for itself and again for each of its
  four neighbors, about 185 asks. Grounding is asked again only when the fix
  moves or a change touches the columns it read (`domain/query/changed.rs`
  `Asked`, `within`). The beacon's bob and slide re-ground nothing, since `Seen`
  holds the target and not what is shown.
- **The interaction phase** reached 12–13 ms. xpute holds that phase to
  unbudgeted O(1) work (xpute `sched/tick.rs`). Pointer hover took up to 9.4 ms
  (`camera_input_tick`, `pointer::hover`, `picked`, a CPU ray). Its cause was
  not looked into.

## The fetch order

- **As it stands.** The wants are declared whole each turn (pxl-kernel
  `sys/io/request.rs` `declare`). A want declared again keeps the time it was
  first wanted and its retry wait. A pump first stops what nobody wants any
  more, then starts the ready wants in rank order into free credits. Nothing
  running is preempted. The rank is lexicographic (`sys/io/policy.rs` `rank`):
  the class, then distance, then the finer tile. The classes run terrain seen,
  terrain unseen, assets, then places and earth, seen before unseen, and last
  divisions and what is asked last. `rank` takes `enqueue_ms` and does not
  read it, so there is no aging.
- **Measured.** A terrain tile takes 1.2–1.6 s on the network (median). The
  wait in the queue has a median of 1–3 ms and a maximum of 6.9 s. While
  the camera moves, 70–90% of terrain requests are stopped, and every place
  request (3 of 3) was. Each place request had started and was stopped when
  the camera passed it before its tile came back. That is cancellation, not
  starvation.
- **The references.**
  - CesiumJS's `RequestScheduler` re-ranks every frame by the caller's
    function, caps requests at 50 in all and 18 per server, cancels what it
    does not issue so the caller asks again, never preempts what runs, and
    has no aging.
  - Cesium's tile priority is a key of factors packed one above another,
    the shape of the rank here. Its foveated deferral holds back the tiles
    off the view's center until the camera stops.
  - Mapbox GL 1.3.0 cancels requests aggressively on a fast pan or zoom.
    MapTiler offers to cancel pending tiles while zooming.
  - None of them ages a request.
- **Weighed.** Aging would help a want that stays wanted and is passed over.
  Here a want passed over is one that nearer or finer wants of the same view
  keep ranking ahead of, the order a view is meant to build up in, and ranking
  it ahead for its wait would put the wrong tile first. The places measured were
  not passed over: they started and were stopped, and aging does not keep a want
  nobody holds. The one place the references differ is that Cesium caps each
  server, so no single source holds every credit. Here the layer class is
  strict, so places start only once terrain has no ready want. Over a long pan
  places do not show, and they fill when the camera stops.
- **The option, not taken.** A cap on each layer's credits, in the form of
  Cesium's per-server cap, keeps the order and lets no layer hold every
  credit. It changes the order a view builds up in, which is the user's rule
  (`policy.rs`, 2026-09-26), so it is the user's to take.

## Resolved

- Memory and the turn stay as they are.
- The fetch order stays as it is: no aging, and no cap on a layer. The user
  agreed to go on with this.
- pixelet-81 corrects `window::fold` so that an absent square is bounded by
  the finest window holding it, and keeps the zero entry only where no window
  holds the square.
  - The CPU read the GPU's fold back a frame or more late
    (`domain/layer/terrain/window.rs` `joined`, pxl-spec-core `window::unfold`).
    A stale entry would then have held a coarser window's bound over cells
    already landed, too tight for them. So the CPU folds its own entries with
    the same `window::fold` over its own squares, in the turn its table
    changes, and the read-back is retired.
  - This shortens places' surface rays and every vertical ray from the top
    alike.
- pixelet-44 runs places' `show` under the pass, nearest first, with what is
  left carried to the next turn, in the form the cells' read already takes.
- pixelet-53 makes the grounding ask each cell's height once, from a grid of
  the zone's cells and their ring, about 57 asks instead of 185, the
  instances byte-identical. Before that, 53 counts `ground` calls on a still
  view and on a pan, to tell a moving fix from a change in the columns.
- pixelet-53 also steps the puck's bob and spin as the clouds drift, under one
  rule:
  - `seen_step_s` sits in `domain/view/lens.rs` beside `cell_there`. It is
    the time a motion takes to carry what it moves a share of the camera cell
    where it is drawn.
  - The share becomes the view's one constant, 1/12 as the clouds have it.
  - `domain/scene/environment/cloud.rs` `drift_step_s` calls it with its
    output unchanged.
  - A still view with the puck in it asks for a frame only once a step's time
    comes.

## Open

- What makes pointer hover cost 9.4 ms in the interaction phase.
- The `enqueue_ms` argument of `Serve::rank`, which no policy reads. The
  profiler reads the same field off the want, not through `rank`.
