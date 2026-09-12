# Cooperative scheduler and the task ledger

**2026-09-09** — the shape of a top-down deferred-work model for the web client:
one cooperative scheduler (really two, plus one profiler), driven by a
read-only instrument that exists so optimization decisions can be made on
measurement rather than code-reading.

**Status: direction agreed, not built.** The first deliverable is the ledger
(observability only); the scheduler unification is downstream of what the ledger
shows and is not pre-committed. This does **not** reopen
[20260708-concurrency-model.md](./20260708-concurrency-model.md) — everything
here is main-thread-only, no workers, no WASM, no SAB. Only the WASM/worker
migration itself is deferred: the scheduling/budget mental model and its
abstractions are designed now to be identical before and after that migration,
so the migration is a transport swap under a fixed model, not a redesign (see
[The abstraction is migration-invariant](#the-abstraction-is-migration-invariant-only-the-executor-changes)).

---

## How this came up

Tracing where the POI "show POI" setting cuts the wiring (it cuts at the
earliest point — `use_poi_fetcher.ts`'s effect early-returns, so no fetch, no
parse; render is independently gated too) surfaced one honest gap: fetches
already in flight when the toggle flips still run to completion — full HTTP
receive, `res.json()`, `set_pois`. Bounded (`LOAD_RADIUS=1` → ≤9 pages),
one-shot, invisible.

**Decision on that specific gap: do not add an `AbortController` to POI.** It is
a local optimization that buys almost nothing (the response is already buffered
by the time you'd abort; render is double-gated) and creates asymmetry — the
sibling `terrain/use_tile_fetcher.ts` has no abort either, and terrain payloads
are ~4 MB/page, so if cancellation ever matters it matters for terrain first.

The fetch call sites are already split along a coherent line, and POI sits on
the correct side of it:

| fetch                           | `signal`                            | nature                                                        |
| ------------------------------- | ----------------------------------- | ------------------------------------------------------------- |
| `fetch_search`, `fetch_geocode` | **yes**, wired to `AbortController` | user-input driven, latest-wins, single-slot                   |
| `fetch_terrain`, `fetch_poi`    | **no**                              | spatial streaming, result is cached and reused, "let it land" |

So the question isn't "POI vs everything else" — it's whether the streaming
category should get a shared scheduler. That opened into the rest of this
document.

## The decomposition: two schedulers, one profiler

Folding all deferrable work into "one cooperative scheduler" is a trap, because
two kinds of work with different natures are hiding under it.

**A. CPU-bound stepped work** — chunk meshing, tile decode
(`render/layer/terrain/elevation.ts`'s `dense_ele`, `shader_meta.ts`'s
`decode_ground_rgba`), decor scatter, billboard canvas building
(`poi_layer.tsx`'s `build_poi_badge_canvas`, `chat_bubble.tsx`). This is the
part that feels like a computer-architecture simulator. It needs: a per-frame
wall-time budget ("≤ N ms/frame on deferred CPU work"), tasks sliced into
resumable steps or naturally small units, priority ordering, and
preempt-at-step-boundary (finish the current step, re-sort, possibly switch
task). `chunk_stage.tsx`'s `LodSchedulerProvider` (recheck/commit steps,
`scheduler_peak_stats`) is already a working prototype of exactly this, scoped
to one work type.

**B. I/O-bound work** — the fetches. This is **not** CPU scheduling: a request
in flight doesn't occupy the main thread, it occupies a connection slot and
eventually a decode. The scheduler here is a priority queue + a concurrency
ceiling + abort-on-stale. The _decode_ of the result then becomes a category-A
task.

These do not share an execution mechanism. What they share is the **priority
model** (distance-derived + category weight; the three radii in
`terrain/config.ts` are policy inputs to it, not replaced) and the
**observability layer**. One task ledger that both feed; two execution
strategies underneath.

## The abstraction is migration-invariant; only the executor changes

Deferring the WASM/worker migration
([20260708-concurrency-model.md](./20260708-concurrency-model.md),
[20260810-mobile-and-perf-targets.md](./20260810-mobile-and-perf-targets.md)'s
bottleneck A) must not mean designing a scheduler a worker can't later fit. The
rule: **the mental model — task, category, priority, budget, ledger schema,
overlay — is designed once, now, to be identical before and after the migration.
Only the executor implementation changes.**

- **A category-A task is a black box with a serializable contract:** `(input
  payload) → (output payload)`, payload transferable-friendly (typed arrays, not
  live object graphs). Today `run(input)` is a plain inline call inside the frame
  budget; post-migration the same task is `postMessage`'d to a worker with a
  transfer list and its result returns the same way. The scheduler's view —
  enqueue, priority, completion signal, latency accounting — is unchanged. The
  mobile-perf doc already notes `page_to_chunks` / `dense_ele` /
  `decode_ground_rgba` are exactly this shape (typed arrays in and out, no
  Three.js/DOM); holding every category-A task to that contract from the start
  makes the migration a transport swap.

- **A task must not read app state mid-run.** No `useTileStore.getState()`
  inside a step — that does not survive a worker boundary. Everything a task
  needs is in its input payload, captured at enqueue time on the main thread.

- **The ledger schema reserves a `transport_ms` field that is 0 today.**
  Post-migration it holds the `postMessage`/transfer cost and worker-side queue
  wait. Categories, span model, and overlay do not otherwise change — a worker
  span is still `{category, id, enqueue, start, end}`, with a different clock
  source and one extra hop measured.

- **"Budget" generalizes per-executor.** Today: one main-thread frame budget.
  Post-migration: the main thread's budget shrinks (it only builds
  `DataTexture`/`ShaderMaterial` from results) and the worker carries its own
  budget and backpressure. "Each executor yields against a time budget" is the
  invariant; the executor count is not.

- **Priority is untouched either way** — policy computed on the main thread from
  map state, independent of where the work runs.

What this does _not_ license: building the worker, the transfer plumbing, or an
executor-selection layer now. There is one executor (inline); the interface has
room for a second. That is the whole move.

## Monitor, do not feedback-control

The requirement is precisely _"must be able to monitor / display"_ each task's
main-thread time share, latency, queue depth — **not** _"the scheduler must know
its own load and adapt."_ That narrowing matters:

- The ledger is a **read-only instrument**, decoupled from scheduling decisions.
  Separate module, independently testable, removable.
- Priority stays **policy-driven** — static formulas. No adaptive controller, no
  feedback loop that can oscillate or resist reasoning.
- The scheduler's own timing knowledge shrinks to **one scalar**: "ms consumed
  this frame", for the budget cutoff (`chunk_stage.tsx` already does this).
  Per-task attribution, latency distributions, queue depths — all the monitor's
  job, not the scheduler's.

|         | scheduler knows               | monitor knows                                               |
| ------- | ----------------------------- | ----------------------------------------------------------- |
| timing  | ms used this frame (1 number) | per-task / per-category ms, p50/p95 latency, frames spanned |
| queue   | what to pull next             | per-category depth, wait time, in-flight count              |
| purpose | budget cutoff                 | display, trace                                              |

If adaptive budgeting ("device is struggling → shrink the CPU budget") is ever
wanted, it is a deliberate separate step whose signal is **coarse** (frame rate
/ LoAF), not per-task CPU% feedback. The moment the scheduler starts consuming
the ledger's output, that's the drift signal. The observation→execution edge is
one-directional on purpose.

## Why: so optimization is a decision, not a guess

The point of the instrument is to be able to say _"the bottleneck is here, go
down and fix this,"_ and to justify (or reject) each optimization on
measurement.

**The repo already has a proof of this workflow.**
[20260810-mobile-and-perf-targets.md](./20260810-mobile-and-perf-targets.md)'s
two bottlenecks were _found by measuring_ and "superseded an earlier
speculative list." A hand-installed `PerformanceObserver({entryTypes:
['longtask']})` cross-checked against `performance.getEntriesByType('resource')`
timestamps produced the finding: _four of five captured long tasks (57–122 ms)
landed within ~50 ms of a tile network-response batch completing_ — which
localized the cost to the synchronous `set_tile → page_to_chunks → dense_ele →
decode_ground_rgba → texture creation` chain inside React's commit phase, and
named the fix (move those three pure functions to a plain worker). Bottleneck A
there is still open.

The ledger is that ad-hoc technique made permanent and always-on — so nobody has
to re-install a longtask observer and eyeball resource timings by hand each time.

## What makes a profiler decision-grade rather than decorative

1. **Spans sit at fix boundaries.** `decode_ground_rgba` as its own span is
   actionable; "terrain: 40 ms" is not. The mobile-perf chain above is the right
   granularity — each link is a pure function you could move or defer
   independently. Test for a new span: "would I actually change code at this
   seam?"

2. **A timeline, not just aggregates.** p95 says "meshing is slow." The trace
   says "meshing is slow _because_ five `set_tile` calls batched into one commit
   during a pan" — which was the actual finding. Bottleneck reasoning is about
   ordering, contention, and causation, which only a timeline shows. Hence
   Chrome-tracing / Perfetto JSON export as a first-class output.

3. **Baseline / target reference lines.** "Where's the bottleneck" is relative
   to a budget. 60 fps = 16.7 ms drawn on the overlay; the ability to snapshot a
   trace and compare before/after a fix. Otherwise you shave something from 8 ms
   to 5 ms that was never on the critical path.

4. **"Slow" ≠ "on the critical path."** Mobile-perf bottleneck B (GPU fill rate)
   produced **no long tasks at all** — "every frame, small movements, still
   stutters." A longtask-only monitor misses it. So the monitor needs both:
   main-thread task spans **and** per-frame render cost / dropped-frame count.
   The metric that decides whether to optimize is intent-to-first-correct-frame
   latency (e.g. `map_center` change → first frame that reflects it), not "how
   long did this task take."

5. **Cheap enough to leave on.** If instrumentation costs 2 ms it has changed
   what it measures. Coarse spans at fix boundaries (rule 1 again) keep it
   near-free; `performance.now()` at step granularity is fine, per-tiny-op is
   not.

## The GPU half is a per-frame gauge, not a task log

Rule 4 says the monitor needs per-frame render cost, not just task spans —
because mobile-perf bottleneck B (GPU fill rate) produced no long tasks at all.
That half of the monitor has a different shape from the CPU ledger and can't be
built the same way:

- **GPU work isn't decomposable into scheduler-driven named tasks.** It's
  "render the frame," one blob, and the browser hides its timing on purpose
  (Spectre-era fingerprinting defense). So the GPU section is a per-frame gauge
  panel, not a span log.
- **`scene.tsx` runs `frameloop="demand"`** — a static scene costs ~0 GPU. GPU
  budget only means anything during active frames (pan, animation, an effect
  running).

What's measurable, by availability:

| tier                               | signal                                                               | note                                                                                                    |
| ---------------------------------- | -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| always, cheap                      | `renderer.info` — draw calls, triangles, textures, programs          | what the GPU is being asked to do; the CPU-side queue-depth analogue                                    |
| always, cheap                      | rAF frame interval + a demand/continuous flag                        | under `frameloop="demand"` with near-zero per-frame CPU, the interval ≈ GPU + compositor                |
| always, cheap                      | pixel load = `drawingBufferWidth·Height` × active post-fx pass count | `dpr` and every effect toggle live in `render_settings_store`; estimates B with no GPU timing at all    |
| always                             | LoAF `PerformanceObserver` — long frame with a script/render split   | makes CPU-vs-GPU attribution possible                                                                   |
| dev-only, absent on the iOS target | `EXT_disjoint_timer_query_webgl2` — real GPU-ns                      | Chrome desktop only, coarse; WKWebView does not expose it, so the shipped monitor must not depend on it |

**Decision rule:** optimize GPU when frame interval > budget (16.7 ms) **and**
no matching CPU / LoAF-script span **and** pixel load or draw count is high —
exactly bottleneck B's signature ("shrink the window → smooth").

**Scope now:** bottleneck B is already mitigated (`low_spec` defaults on, `dpr`
pinned to `[1,1]`, every post-fx pass defaults off, `frameloop="demand"`). GPU
is not on fire. So build the gauge — so that re-enabling an effect, heavier
geometry, or a device stutter report is visible rather than a mystery — but do
**not** build GPU timer queries speculatively; they're a dev instrument for a
real GPU regression that hasn't appeared, and the iOS target has none anyway.

## Factors the monitor accounts for

GPU fill is one row of a wider set. The roofline framing the question came from
— arithmetic intensity, bandwidth, memory — mostly does not become a live FLOP
count (unmeasurable in JS) but a **per-category classification** that steers
which optimization to reach for. Grouped:

**Time & scheduling** (all live)

| factor                   | signal                                                         | what it tells you                                                                                         |
| ------------------------ | -------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| main-thread time         | `performance.now()` spans, per task / per category / per frame | the base "what's expensive" number                                                                        |
| latency (enqueue→usable) | timestamps in the task record                                  | slow to _start_ vs slow to _run_ — prefetch/reprioritize vs cheapen                                       |
| queue depth / contention | per-category depth sampled each frame, over time               | "the scheduler is the bottleneck, not any one task" — only a timeline shows it                            |
| budget occupancy         | `used_ms / budget_ms` per frame                                | budget full → cheapen the work; budget idle but frames still drop → the stall is upstream (I/O), not here |

**Memory**

| factor                | signal                                                                                    | what it tells you                                                                                                                     |
| --------------------- | ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| footprint             | summed decoded bytes held (`tile_map`, `poi_map`, geometry, textures) vs a device ceiling | **the crash factor, not a slowdown factor** — `terrain/config.ts` already records r=3 ≈ 197 MB decoded, r=3 crashed an iPhone 13 mini |
| GC / allocation churn | `performance.memory.usedJSHeapSize` delta per frame (Chrome only), LoAF attribution       | a burst of typed-array allocs → GC pause → dropped frame; best-effort                                                                 |

**Bandwidth**

| factor         | signal                                                                                          | what it tells you                                                                                                  |
| -------------- | ----------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| network        | response bytes/s; TTFB vs total download                                                        | often _the_ cold-load bottleneck on cellular; latency-bound (RTT) vs throughput-bound (pipe) needs different fixes |
| decode         | bytes in+out / wall-time of the decode step → effective MB/s vs a rough platform memcpy ceiling | memory-bound (WASM won't help) vs compute-bound (WASM/SIMD pays)                                                   |
| CPU→GPU upload | count × size of new `DataTexture` / geometry buffers per frame                                  | a pan hitch distinct from both CPU meshing and GPU rendering                                                       |
| GPU fill       | `drawingBuffer² × dpr² × post-fx passes`, `renderer.info` counts                                | bottleneck B ("shrink the window → smooth")                                                                        |

**Classification / shape** (mostly static labels, set once from profiling)

| factor                             | form                                                                        | what it tells you                                                                                                                                         |
| ---------------------------------- | --------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| arithmetic intensity / bound-class | static label per category: `memory` \| `compute` \| `network` \| `gpu-fill` | which lever: touch less data / overlap · WASM+SIMD · prefetch+prioritize · fewer passes / lower dpr                                                       |
| batching efficiency                | static label + live units-per-batch count                                   | fixed per-unit cost (React commit, future FFI crossing) vs marginal per-byte — mobile-perf's "batch per page, never per chunk or per texel"               |
| burst vs sustained                 | rolling-window percentiles vs session peak                                  | a one-time cold burst is acceptable; a sustained overrun is mobile-SoC throttling territory (`scheduler_peak_stats` only records the peak, not the shape) |
| idle ratio (power proxy)           | fraction of wall-clock the `frameloop="demand"` loop actually runs          | watts are unmeasurable on web; a never-idle scene drains battery even fully at-budget                                                                     |

The `FrameSample` record the frame ring stores carries the live fields; the
static labels live next to each task category's registration.

## Build order

1. **Task ledger — observability only.** Every existing work site reports
   `ledger.begin(category, id)` / `ledger.end(...)` plus `performance.now()`
   wrappers. Ring buffer of the last N frames: `{frame_ms, budget_used,
   per_category_ms, started/completed, queue_depths, inflight, dropped}` plus the
   per-frame GPU gauge fields (`renderer.info` counts, rAF interval, pixel load —
   see "The GPU half is a per-frame gauge"). Dev overlay: stacked frame-time
   bars, queue depths, per-category p50/p95 latency, 60 fps reference line.
   `scheduler_peak_stats` folds in as a degenerate case (it records only the
   maxes). Zero behavior change. ~1–2 days.

   _Done, including the fold-in:_ chunk_stage no longer keeps its own peak
   object. It reports each frame's raw numbers (LOD counts included) and the
   ledger keeps **both** views — the ~4s ring for the shape of a stutter, and
   `ledger_frame_peak()` for one noticed too late to still be in the ring,
   which is the reason that object existed. `__PXL_RENDER_STATS__` keeps its
   exact shape (it is read from the console and scraped off the canvas
   dataset) and now derives `scheduler_peak` from the ledger. The cumulative
   sub-operation counters (`lod_build`, `mesh_mount`, `commit_step`,
   `render_call`, `compositor`) stay where they are on purpose: they count
   calls across a session, not per-frame values, and forcing them into
   `FrameSample` would bloat the ring for a different kind of measurement.

2. **Findings doc from the ledger.** Run it, capture traces during pan/zoom
   storms, produce a ranked list of what's worth fixing. _This_ is what
   justifies step 3+ — not a blind commitment to a scheduler design.

3. **Unified fetch scheduler (category B).** Priority queue + concurrency
   ceiling (~4–6) + `AbortController`-on-stale, shared by terrain and POI;
   geocode/search fold in later (they already carry signals). Fed from a single
   "current demand set" effect rather than each subsystem's own `useEffect` +
   module `pending` Set. Derives `map_status` from one in-flight count — retiring
   the `pending_page_demands.size` heuristic and its "newer batch stomps SUCCESS"
   hazard (`use_tile_fetcher.ts`).

4. **Unified CPU task scheduler (category A).** Split in two once it was
   clear only half of it needs a working render:
   - **4a — the kernel.** `sched/frame_budget.ts`'s `run_under_budget`: run
     an ordered list of steps until a wall-time budget or a step cap, with
     forward progress guaranteed (the first step that does work always runs,
     so one expensive item can never permanently starve the queue) and a
     falsy step counting for nothing. Eligibility and ordering stay with each
     caller — those are domain work. Unit-testable, no render needed.
     `chunk_stage.tsx`'s recheck and commit loops are its first consumers, so
     it never existed without one.
   - **4b — the integration.** Pool the per-pass budgets into one frame-wide
     allowance, and move the `set_tile` commit cascade onto it. A real
     behaviour change; needs a render to tune and verify against.

   (Tile _decode_ was on this list until it was measured at 0.6 ms; see the
   second finding below.)

5. **Trace export.** Ring buffer → Chrome-tracing JSON, openable in Perfetto.

Steps 3 and 4 are independently shippable and their order is set by the findings
doc, not by this document.

## Update 2026-09-09: step 3 landed first

The build order above was reordered on the day: the **fetch scheduler (step 3)
shipped first**, carrying a minimal step-1 ledger inside it, rather than the
full frame-rollup ledger + findings doc before it. The reasoning: the fetch
scheduler is small, its correctness is unit-testable without any measurement,
and laying the shared `Demand` / ledger-record vocabulary now is what keeps the
later CPU scheduler and the full ledger from being designed in isolation. The
full ledger (per-frame rollup, dev overlay, Perfetto export) and the findings
doc still come before step 4.

What landed, in `apps/web/src/modules/world/sched/`:

- **`demand.ts`** — the category-B task contract: `{ key, category, priority,
  run(signal) }`. `run` owns the fetch and its store write; the scheduler never
  touches a store.
- **`fetch_scheduler.ts`** — `fetch_sync(category, demands)` replaces one
  category's wishlist; `MAX_INFLIGHT = 6` global quota; anything dropped from a
  category's wishlist is aborted (in flight) or forgotten (queued). `pump()`
  sorts the queue by priority on each `sync` and each settle. `fetch_subscribe`
  - `fetch_pending_count` are the one reactive readout.
- **`ledger.ts`** — finished-work ring buffer (`TaskRecord`, cap 256), with the
  migration-stable `transport_ms: 0` field. No overlay yet.
- **`fetch_scheduler.test.ts`** — quota, priority order, yield (abort in-flight
  / drop queued), dedup, ledger outcomes.

Rewired: `terrain/use_tile_fetcher.ts` and `poi/use_poi_fetcher.ts` each build a
`Demand[]` and call `fetch_sync` — their module-level `pending` sets and
`Promise.all` batches are gone. `fetch_terrain` / `fetch_poi` took an optional
`AbortSignal`. `map_status` now derives from `fetch_pending_count("terrain")`,
retiring the `pending_page_demands.size` heuristic and its "newer batch stomps
SUCCESS" hazard. Terrain page keys are now `TO_XY`-normalized like POI's always
were, so an elevation-only `map_center` change no longer re-fetches the ring.

Side effect: the POI in-flight-fetch gap that opened this whole discussion
(fetches surviving a "hide POI" toggle) is closed for free — hiding POI calls
`fetch_sync("poi", [])`, and the scheduler's yield aborts whatever was in
flight. No bespoke `AbortController` on POI, as decided.

Priorities as shipped: terrain `chebyshev_dist * 2`, POI `chebyshev_dist * 2 +
1` (POI one step behind terrain at equal distance, since a badge grounds against
its page's terrain), device-pinned terrain page `1_000` (loads, but never jumps
the line). Geocode `-5` and search `-10` were folded in afterwards, both single
slot — see below.

## Update 2026-09-09: the ledger's first real finding

Everything above is the instrument. This is the first thing it caught, and it
is recorded here because the loop it closes — measure, find, fix, re-measure —
is the whole point of building it.

`ledger_report_text()` printed, right next to each other:

```
[   4613.3] terrain:6359575375314944   ok   363.7ms q0.0    r363.7 p2
[   4618.7] poi:6360674887008256       ok   454.8ms q358.7  r96.1  p1
```

A POI page at priority **1** sat 358.7 ms in the queue while terrain pages at
priority **2** ran with essentially no wait. Priority was inverted in practice.

**Cause.** `fetch_sync` pumped synchronously. `MapScene` calls
`useTileFetcher()` before `usePoiFetcher()`, so terrain's nine demands were
admitted and filled all six quota slots before POI had submitted anything at
all. Priority can only order the _queue_; it cannot preempt a request already
in flight. So submission order silently beat it — and nothing about the
priority numbers was wrong.

**Fix.** Coalesce the pump to one per tick (`queueMicrotask`). Every `sync()`
in the same tick lands first, and the single pump that follows picks the top
`MAX_INFLIGHT` across all categories. The diff half of `sync()` stays
synchronous: dropping and aborting superseded demands must not wait.

Rejected alternatives: reserving slots per category (invents a second policy
knob next to priority, for a problem priority already expresses), and merging
the fetchers into one "current demand set" effect (couples two subsystems to
fix a scheduling artifact).

**Re-measured**, same cold load:

```
geocode:6360674887008256  p-5  q0.0
poi:6360674887008256      p1   q6.1     <- was q358.7
terrain:6359575375314944  p2   q7.1
terrain:6359575375380480  p2   q7.7
terrain:6359575375446016  p2   q7.9
terrain:6360674886942720  p2   q7.9
```

The six admitted slots are now strictly the six highest-priority demands
across every category. Locked in by a test ("a later category submitting in
the same tick still wins on priority").

**The generalizable part:** a quota that saturates within one tick makes
submission order, not priority, the real scheduler. Worth remembering when the
category-A CPU scheduler lands with the same shape.

## Update 2026-09-09: the second finding retires a planned work item

`TaskRecord` gained an optional `phases` split (a demand's `run` may hand back
its own sub-timings; terrain and POI report network vs decode/parse). One cold
load, nine pages each:

```
poi      network  n=9   p50 865.5  p95 931.6 ms
                  mean net 269.3   parse  2.2   bytes    114,047
terrain  network  n=9   p50 428.4  p95 618.7 ms
                  mean net 322.5   decode 0.6   bytes  4,203,672
```

**`decode_terrain` costs 0.6 ms on a 4.2 MB page.** It is an XTP cursor read
over an already-shaped ArrayBuffer, so this is what it should cost, but it was
being assumed expensive. 99.8% of a terrain demand's run is network. POI's
`JSON.parse` is 2.2 ms on a ~114 KB page (295 KB worst case). Both categories
are network-bound end to end, and the static labels in `categories.ts` now
carry these numbers instead of a guess.

**What this retires.** The planned "#9 — split terrain decode out of the fetch
demand into a category-A task" is dropped: there is 0.6 ms to split. The
assumption behind it — that
[20260810-mobile-and-perf-targets.md](./20260810-mobile-and-perf-targets.md)'s
bottleneck A lived inside the demand's `run()` — was wrong. That cascade
(`page_to_chunks` → `dense_ele` → `decode_ground_rgba` → texture/material
creation) hangs off `set_tile` inside React's commit phase, downstream of the
fetch entirely. The ledger already measures it under a different name:
chunk_stage's `commit_ms`, seen at p95 470 ms and max 2724 ms.

**Where the category-A work actually is**, then: the commit cascade, which
already runs partly under `LodScheduler`'s 4 ms frame budget and blows through
it. That is the thing #8 has to generalize and the worker migration has to
absorb — not decode.

**Caveat on the numbers.** `net_ms` here is `deno task dev:remote`, so it
includes a local vite proxy hop plus WAN to the published deployment; a CDN
edge would be far lower. `decode_ms` is local CPU and honest regardless, and
the conclusion survives an order of magnitude of network improvement (0.6 ms
against 32 ms is still 2%).

## Update 2026-09-09: what the instrument actually found

Measured against a working render (live API, so tiles match the current
decoder). Every number below is observed, not reasoned about — which matters,
because **four hypotheses formed by reading the code were wrong**, and the
ledger refuted each one:

| hypothesis (from reading code)               | measured                            |
| -------------------------------------------- | ----------------------------------- |
| `decode_terrain` is expensive                | **0.1 ms** on a 4.2 MB page         |
| AnchorNode's double render is the bottleneck | removing it left `dt max` unchanged |
| zoom-out mounts chunks in bulk               | `mount +0/-0` throughout            |
| `set_layout` re-renders drive the stall      | `rerender 0` on every stalled frame |

Three distinct problems remain, with distinct signatures and distinct fixes.

**1. Tile arrival freezes the UI — React mount storm. Confirmed.**

```
dt 966.5  sched 21.5 (cmt 17.4)  draws 5  geom 2  mount +2524/-1262 rerender 2524
```

`PageChunks` renders every one of a page's ~256 chunks in one reconciliation,
and nothing budgets it. This is what a user hits as "tiles render and the map
stops moving", and it is mobile-perf bottleneck A's trigger — _tile response
batch completing_, not zoom-out. Fix: bound the mount (a reveal budget), then
stop making a React component per chunk at all. The LOD controls are already
imperative, so only the R3F wrapper has to go — and the WASM roadmap wants
imperative regardless.

**2. Zoom-out stalls seconds — garbage collection. Resolved.**

Closing the last instrumentation gaps settled it. `render_ms` had been reading
0 because the post-processing composer renders through its own `useFrame` and
never touches the wrapped `gl.render`; `task_ms` (measured in a microtask, so
it sees the whole rAF task including that render) and `gpu_ms` (a real
`EXT_disjoint_timer_query_webgl2`) replaced the guesswork:

```
t+36.61  dt 8419 = task 15.3 + offthread 8404   gpu  58.4   Δheap  +344MB
t+38.89  dt 2193 = task  9.5 + offthread 2184   gpu   9.0   Δheap  +709MB
t+40.73  dt 1736 = task 25.3 + offthread 1710   gpu  22.7   Δheap  +565MB
t+42.07  dt 1339 = task 43.1 + offthread 1296   gpu  22.7   Δheap -1379MB
```

Neither the main thread (15 ms) nor the GPU (58 ms) is doing the eight
seconds. And off-thread time tracks allocation volume, nothing else:

```
offthread > 100ms  (n= 17)   mean Δheap  +86.2 MB
offthread <= 20ms  (n=831)   mean Δheap   -3.6 MB
```

Heap swings between 1.4 GB and 4.3 GB, with seven collections over 300 MB
(worst −2160 MB). Eighteen frames account for 72% of the window's wall time.

What allocates that much with `mount 0`, `rerender 0`? `geometries` churns
±30–50 every frame (971 → 1000 → 1015 → 1029 → 1042 → 1085 → 1061) as LOD
transitions rebuild chunk geometry. Building the typed arrays is cheap —
`commit_ms` stays at 4–10 ms — but the garbage is not, and the ~730-deep
pending backlog keeps feeding it. (The correlation is measured; attributing
the allocation specifically to chunk rebuilds is read off that `geometries`
churn.)

**3. Steady state is GPU-bound — now measured, not inferred.**

```
gpu  p50 16.37  p95 24.08 ms        (against a 16.7ms budget, 914/914 frames)
task p50  8.0   p95 17.7  ms
```

The GPU alone fills the frame budget at rest. Independent of React and of the
stall above; the levers are dpr, post-fx pass count and overdraw. Separately,
the LOD full recheck sweeps all ~1985 controls every `LOD_RECHECK_INTERVAL`
(~167 ms) for ~3.4 ms **even with `pending 0`**.

**What this changes about the plan.** Removing React from the render layer is
still the right move, but the honest accounting is:

- it fixes **1** outright — the mount storm is React, and nothing else is.
- it does not by itself fix **2**. GC pressure comes from the allocation, not
  from React.
- it is nonetheless the _prerequisite_ for fixing 2: geometry is currently
  created inside `ChunkMesh`'s `useLayoutEffect`, so its lifetime is React's.
  Owning that lifetime imperatively is what makes buffer reuse possible at
  all, and reuse — not a faster rebuild — is what removes the garbage.
- it does nothing for **3**, which is a separate GPU-side conversation.

**Already fixed or ruled out:** the scheduler pass (4711 ms → bounded), tile
decode (0.1 ms), fetch priority ordering (behaves as designed).

## Update 2026-09-09: the GC finding's fix — a pool, not a buddy allocator

The stall the instrument pinned (`dt 8419 = task 15.3 + offthread 8404`,
`Δheap +344MB`, seven collections over 300MB) is collection pressure from
rebuilding chunk geometry, not from building it. The fix is to stop producing
the garbage. Two lifetimes hide inside `bake_chunk_mesh`, and only one of them
is what `ds/arena.ts` already serves:

- **Scratch, dies at the end of the call.** `surviving` (one `{cx, cy, run}`
  object per surviving run) and `per_cell` (`res^2` arrays plus an object per
  Z bucket). Small objects in bulk — the kind the collector must trace, not
  just free. This is exactly Arena/Vector's shape and now uses them: a
  `Vector(Uint32Array)` of (cx, cy, run index) triples, and one `Arena` of
  bucket accumulators whose per-cell ranges are contiguous because a cell is
  filled before the next one starts, so `CELL_RANGE` holds (start, count) and
  no per-cell container exists at all. `emit_box` also took its box min/size
  and taps as scalars rather than tuples — two throwaway arrays per box, at
  thousands of boxes per chunk.

- **Retained, dies with its owner, in no particular order.** The eight typed
  arrays a baked mesh hands to `BufferGeometry`. Tempting to call these
  scratch too ("three.js copies them on upload"), but it does not: upload is
  lazy, on first draw, so a frustum-culled chunk holds them for many frames,
  a context loss needs them again, and `visibility` is written for the whole
  life of the geometry by the LOD blend. Unordered death is the one shape a
  bump allocator structurally cannot reclaim.

So the second lifetime got `ds/pool.ts`: power-of-two size classes, a free
list per class, no splitting and no coalescing, and a hard reserved-bytes
ceiling above which a released block goes back to the collector instead of
being retained. All eight spans of a mesh are views into **one** block, since
they share a lifetime exactly — eight allocations bought nothing.

**Why not a buddy allocator.** A buddy allocator is a size-class pool _plus_
migration of bytes between classes. The migration buys bounded residency when
the class mix shifts; it does not lower churn, and churn is what the 8.4s was.
The ceiling bounds residency without it. The contract is deliberately
blocks-and-handles rather than raw views precisely so splitting/merging can be
added later behind `acquire`/`release` without touching a consumer — if
per-class idle residency ever shows up in the ledger's own `pool` row as a
real cost, which is the condition to check before building it.

**Ownership.** A mesh is owned twice and the two owners die independently:
`chunk_stage.tsx`'s per-chunk LOD cache, and the mounted geometry rendering
it. `recheck()` prunes the cache on frustum exit while the same tier is still
on screen, so "the cache dropped it" cannot mean "the block is free" — hence
`rc` on `BuiltChunkMesh` (`Rc<MeshBlock>` in the Rust reading, not a novelty).
Geometry disposal moved into `ChunkMesh`'s effect cleanup alongside the
release, because the GPU buffers and the block have to be surrendered in the
same step, and only the cleanup runs on a real unmount — where the previous
"replace, then dispose the previous one" shape leaked the last geometry.

**Rust/WASM.** `Vec<u8>` slabs with per-class free lists is the same shape;
under WASM, where linear memory grows and never shrinks, the reserved ceiling
stops being a nicety and becomes the whole point. The pool introduces exactly
one new failure mode — a view outliving its release, which is silent
corruption rather than a stale read — so `pool.ts` carries a `poison` option
(off by default: it is a memset per release, which would distort the very
storm being measured) and a double-release guard that is always on.

The ledger gained a `pool live/rsv hit` row for this. Reading it against
`heap_mb`: `live` tracking the visible chunk count while `reserved` stays flat
is reuse working; a low hit rate with `reserved` churning is the pool being
defeated, by class thrash or by the ceiling.

## Update 2026-09-09: what it took to actually land, and the numbers

The GC finding took four attempts to fix, and three of them failed for the
same reason. Recorded here because the reason is not about allocators.

**The pool never engaged.** Its ceiling counted borrowed bytes, and a full
frustum legitimately holds ~600MB of live chunk geometry, so every release
took the eviction path — the pool switched itself off exactly under the load
it exists for. Measured hit rate 13-36%. Borrowed bytes are not a surplus to
reclaim; only the idle surplus is.

**Per-class free lists were the wrong allocator, and the argument against
buddy was wrong.** "This workload does not migrate between size classes" was
false by construction: dropping a chunk's fine LOD tier and asking for a
coarse one in the same breath frees a large block and requests a small one,
and a zoom back in does the reverse. The free lists sat full of blocks
exactly one order above every request. Splitting fixes one direction and
merging the other; needing both is what makes it a buddy allocator rather
than a pool with a hack in it. A one-class upward reach was tried first and
is strictly worse than either alternative.

**Pooling was unsafe while React owned mesh lifetime.** A chunk mesh had
three owners — the LOD cache, the layout naming it, and the mounted geometry
— and React decides _later_ when a layout mounts, so all three lifetimes were
deferred by an unknown amount. Three reference-counting bugs came out of the
windows between them: a build during render (StrictMode's setup/cleanup/setup
replay dropped the last reference to meshes the pending mount then retained),
a layout that named meshes it did not hold, and an unmount teardown with no
setup. Each fix was real and each was followed by another, because the class
of bug is "an owner whose lifetime someone else decides", not any one
instance of it. Recycling was switched off (one flag, not a revert) and the
chunk layer went imperative: `chunk_node.ts`, a plain object with a group, a
dispose, and a commit that attaches geometry in the same call. Two owners,
both mutated synchronously, no window.

Then recycling went back on. Measured, same zoom-out:

|                             | before       | after            |
| --------------------------- | ------------ | ---------------- |
| dt max                      | 8419ms       | **85.7ms**       |
| offthread p50 / max         | 4.2 / 4981ms | **1.4 / 63.0ms** |
| pool hit rate               | 13-36%       | **99%**          |
| Δheap on a committing frame | +4.28MB      | **+1.43MB**      |
| Δheap on a quiet frame      | +0.18MB      | **+0.03MB**      |

The multi-second stalls are gone. What was bought with churn is residency:
the pool now holds ~1.8GB steadily (439MB live, ~1.4GB idle) because buddy
regions are only reclaimed when _fully_ empty, and one live block strands a
256MB region. That is the standard pooling trade and the right side of it,
but it is a number to keep in view.

**Method note.** Three of these rounds cost a measure/report cycle each
because the fix was landed and measured one step at a time. That was the
wrong instinct: the instrument's job is to say _where_ to look, and it had
already said so. The rule for next time is the one that finally worked —
land the whole change, then measure once — and the corollary that a
measurement of a mechanism that was never actually enabled (the bypassed
pool, the ceiling-disabled pool) is not evidence about that mechanism.

**Also found, unrelated:** `deno check` reports green off a warm cache.
`deno check --reload` finds 77 pre-existing errors, all R3F JSX intrinsics
failing to resolve. Twelve went away with the components that used them; the
rest are still there and the task cannot be trusted until they are dealt with.

## Update 2026-09-09: the rest of it, and what the instrument was for

Four more changes after the imperative conversion, each one found by the
ledger rather than by reading code. Recorded mostly for the pattern.

**Faces nothing can see.** Every cube emitted all five or six of its faces
regardless of its neighbours, so a flat plateau baked four hidden faces per
cell. Culling a face when a neighbouring column spans the run's whole z
range keeps **23-24%** of the faces on real terrain — measured identically
across three sessions, which is what a workload-normalised counter is for.
A run-level occlusion test already existed and had fired **zero times in
half a million runs**: its condition was strictly narrower, and narrower in
the wrong direction, since it refused to consider chunk-edge columns at all.

**Column runs.** `MeshRun[][]` was 4096 arrays plus an object per run,
retained per chunk. Packed into parallel spans over one pooled block: 17
bytes a run against a hundred, one allocation against ten thousand.

**Region size.** Buddy reclaims a region only when it is fully empty, so at
256MB one surviving chunk held a quarter-gigabyte hostage — 339MB live
against 1502MB idle. Face culling shrank the largest block, so the region
shrank with it. `max_block_bytes` now reports the number the next choice
should be made from, rather than the last guess.

**The band pool.** The one that mattered. Heap was growing 188MB/s while
the chunk layer retained almost nothing, so it was transient, and the
retention counters that proved it also refuted the guess that came with
them — decor, the "likely culprit", was three orders of magnitude too
small. `MAX_POOLED_BANDS` was **2**. A band is 1.31MB and a composite makes
several, so the first two came from the pool and every one after was a
fresh pair of typed arrays. Correct when compositing was rare; at ~80 chunk
scans a second it _was_ the allocation.

|                    | start of the day                  | now                   |
| ------------------ | --------------------------------- | --------------------- |
| dt max             | 8419ms                            | **85.1ms**            |
| resting heap       | 1.4-4.3GB, collections over 300MB | **730MB, +555MB/15s** |
| frames over budget | —                                 | **6%**                |
| pool hit / idle    | n/a                               | **98% / 34MB**        |
| faces emitted      | all of them                       | **23%**               |

**What the instrument actually did.** It never measured "GC" or "React". It
measured one thing — `dt - task_ms`, whether a frame's time was inside the
main thread's own task or outside it — and that split excluded half the
search space on the first look: an 8.4s frame with `task 15.3ms` cannot be
the scheduler, the build, or the mounting, because all of those are inside.
Everything after was narrowing. Four hypotheses formed by reading code were
refuted by it (decode cost, a double render, mount storms, re-renders), and
so was the last guess in this list. The lesson is not "instrument GC" but
"find a cheap axis that is exclusive, and let it delete half the space".

The counters that earned their keep have a shape in common: they are
_normalised_ (faces kept as a ratio, cost per bake, cost per thousand runs)
or they _name a specific owner_ (what the pool holds, what a chunk retains,
how often a band was reused). Absolute per-window totals were the ones that
kept failing to settle arguments, because they mix how much work a window
happened to contain with what one unit of it costs.

## Resolved 2026-09-12: one on-screen chunk could wait sixteen seconds

In a zoomed-out session with
500–590 chunks in the frustum and the view moving almost the whole time,
the ledger's `oldest_visible_ms` rose by exactly 1000 ms per second from
t = 3 s to the end, reaching 15 994 ms: one pending chunk on screen was
passed over for the entire session while the pending count stayed between
230 and 760 and bakes ran at 119/s during the backlog. An earlier session's
window peak read 17 180 ms, so it predates the core-memory work that session
tested. The rate says the scheduler is working; the monotonic wait says its
order can starve one chunk indefinitely — the unified priority
(`holds_slot`, `in_frustum`, `lead_distance2`) has no ageing term, so a chunk
that is always slightly farther from the lead point than whatever keeps
arriving never reaches the front while the view keeps moving. Trace:
pxl-ledger-2026-09-11T14-35-25-187Z.json.

Fixed with ageing (`sched/priority.ts`'s `AGE_HALF_MS`), the textbook cure
for starvation under a static priority: every 500 ms a pending item waits
halves its effective distance from the lead point, so one k times farther
than a fresh arrival overtakes it after log2(k) half-lives — two seconds at
sixteen times, the overlay's `STARVED_MS`. Items that began waiting together
age alike, so a jump or a load still fills nearest-first. Ageing reorders
within a tier and never across one: slot holders still go first and an
off-screen release still cannot pass a visible build, so releases can in
principle still wait out an endless stream of on-screen work — accepted, as
before, since what they cost is memory the pool can spare. The comparator
takes `now` once per sort, so its keys hold still while a sort runs.

Reviewed alongside, the ledger's three readers: the overlay and the report
reduced frames through `summary.ts` already, but the side counters the report
prints (faces, bands, build and scan cost, badge builds, placement runs,
atlas repacks, index rebuilds) counted from load, so after a restart they
carried every earlier window beside frames that did not; they are now read
as differences from the window's start (`log.ts`'s `window_counters`). The
pool's hit share was likewise since load and is now a difference over the
window's frames. The overlay reduced the whole task and long-frame rings on
each repaint, the cost the frame rows had already been cut from; all three
now read its last ten seconds. The trace builds a frame's args once for both
the counter track and the worst-frame spans.

## Update 2026-09-13: the stats blob is gone

`__PXL_RENDER_STATS__` and the canvas `dataset` copy were removed. Nothing
read them once the report existed: the overlay and `__pxl_ledger_log` print
the same numbers with a window around them, and a blob rewritten every
frame — a JSON stringify of a dozen sub-objects — was itself a per-frame
cost paid for a reading nobody took. What only fed the blob went with it:
`mesh_mount_stats`, `mesh_unmount_stats`, `commit_step_stats` and their
`performance.now()` pairs in the attach and commit paths. The counters the
report does print stay (`lod_build_stats`, `occlusion_stats`, the band
pool, the badge pipeline), and `coarse_instance_stats`, which had only the
blob, is now a line of the report. `ledger_frame_peak()` stays: the report
prints it.

The console stream (`ledger_set_logging`) is no longer a setting of its
own. It is a button on the overlay, beside report and trace, since a
stream is something one turns on while looking at the panel, and the
one `show_ledger` setting is what opens the panel.

## Update 2026-09-13: the axis has a precondition, and a phone showed it

Four ledger reports from an iPhone (dpr 2, no GPU timer, no LoAF, no
longtask) were read in a row, and the first reading was wrong in the way
this document warns about: `task 7 + offthread 279` on a still frame was
called GPU, then GC. The A/B settled it — decor off cut triangles 58% and
moved nothing; POI off took dt p95 from 222 to 54 ms. The 250 ms was
main-thread work that the tick never sees: POI's placement (a React
effect on an epoch), its badge canvases, and the atlas upload three does
at render. `dt − task` excludes half the space only when every piece of
main-thread work is inside the task; work outside it is indistinguishable
from the GPU and the collector, and on iOS there is no second instrument
to tell them apart. The category-A contract already said this work belongs
under the budget — "billboard canvas building" is on the list by name —
so the finding is not a new rule but the old one unmet, and the fix is the
one planned: POI under the tick (BACKLOG, P1).

The tick's steps are named now and the report prints each one's mean and
max (`steps` line). Its first reading named a step that runs under the
budget and does not yield: `content/poi_index` at a mean of 1.5 ms and a
max of 45, because its unit of work is a page and forward progress runs
the first unit whole. A budgeted phase is only as cooperative as the
granularity of its steps; the phase total could not show that, and the
step table can. Monitor-side only — the scheduler still knows one number.

The steady state, once loading is done, is fine on that phone: dt p50 15
ms on a 13 ms display, the tick 5 ms, the interaction phase 3. What the
phone exposed is where the instrument was blind, not where the scheduler
was slow.

## Guardrail

[20260708-vision-and-scope-pivot.md](./20260708-vision-and-scope-pivot.md)
records that the most design energy in the WASM era went into the runtime and
network layers — "the place where generalization was least needed" — from trying
to imitate systems-level architecture. This proposal is _more runtime-layer
generalization_, so the justification has to hold:

- Real contention, present tense: 4+ live fetch sources (`fetch_terrain`,
  `fetch_poi`, `fetch_geocode`, `fetch_search`), 2+ CPU-task sources (chunk
  mesh, tile decode) actually competing — not hypothetically.
- A concrete observed bug class: stale-demand download/decode waste, the
  `map_status` hazard, the "no cache / concurrency-limit / rate-limit there yet"
  gap `terrain/config.ts` already flags.
- Main-thread only. No worker, no WASM, no SAB — the parked rewrite stays
  parked.

The smell to stop on: this pulling toward an abstract "task runtime tier" (the
`xpute` orchestration-layer mistake). Scope is the ledger plus two concrete
schedulers. No orchestration layer above them.

## Relationship to the parked worker model

[20260810-mobile-and-perf-targets.md](./20260810-mobile-and-perf-targets.md)'s
bottleneck A fix — moving `page_to_chunks` / `dense_ele` / `decode_ground_rgba`
into one long-lived plain Web Worker via transferable `ArrayBuffer`s — is
compatible with this and downstream of it. The ledger is what tells you whether
that move is worth its cost, and later whether WASM inside that worker is
warranted (the doc's "measure, split, re-measure, only then WASM" order). The
CPU task scheduler (step 4) is the main-thread half; a worker is where a
category-A task's body runs once the profiler says it should. Neither requires
the engine-scale rewrite that
[20260708-concurrency-model.md](./20260708-concurrency-model.md) parked.

## Related

- [20260708-concurrency-model.md](./20260708-concurrency-model.md) — the
  worker/WASM/SAB axis, parked until the post-MVP milestone; this document is
  the main-thread work that precedes it and does not reopen it.
- [20260810-mobile-and-perf-targets.md](./20260810-mobile-and-perf-targets.md) —
  the two measured bottlenecks; the precedent for measure-then-fix, and the
  first entries the ledger's findings doc should re-confirm or retire.
- [20260708-vision-and-scope-pivot.md](./20260708-vision-and-scope-pivot.md) —
  why over-investing in the runtime/network layer is the specific historical
  mistake this must not repeat.
- [20260907-world-rendering-tiers.md](./20260907-world-rendering-tiers.md) — the
  billboard tier (POI, chat bubbles) whose canvas-build cost is a category-A
  task.
