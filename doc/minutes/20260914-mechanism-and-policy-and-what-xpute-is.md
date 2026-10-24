# Mechanism and policy, and what xpute is

**2026-09-14** — the evening `@pxl/shared` was mirrored whole into
`crates/pxl/shared` and the line between the two crates came up: what is
`xpute-core`, what is `pxl-core`, whether anything like xpute exists in
JavaScript, and whether xpute is worth opening. Worked out in a
conversation with a model, kept here as the roadmap after the WASM move,
with the decisions the author took from it.

## The line: mechanism is xpute, policy is pxl

The scheduler and the memory are xpute's; what fills them and in what
order is pxl's. The OS vocabulary fits: a _mechanism_ is the general device
that makes a thing possible, a _policy_ is how the device is used.

```
xpute — the execution substrate            pxl — the spatial runtime
  scheduler (budget, yield, resume)          chunk, page, terrain, object,
  memory: buddy, pool, arena, block/handle,   decor, actor
    acquire/release, pressure and refusal    LOD policy, residency policy
  IPC: rings, ports, frames                   world lifecycle, the build
  change journal, wake-ups                     pipeline, the raycast
  lanes, containers, errno                    the environment, the draw list
  (a host/worker abstraction, later)
```

Concretely:

- xpute's scheduler says "run resumable jobs within this budget; keep
  blocked and starved tasks; yield and resume." pxl says "this chunk
  builds first; reclaim off-screen farthest-first; lower the LOD under
  memory pressure; protect the visible tier."
- xpute's memory is the buddy allocator, the pool, the arena, a block and
  its handle, acquire/release, and pressure as a _refusal_ the caller
  sees. pxl is the page section, the mesh section, terrain residency, the
  tier cache, the reclaim target and the LOD response to pressure.

So "kernel" needs care: `xpute-core` is the runtime's mechanisms,
`pxl-core` is the spatial kernel's policy and the world's semantics, and
the WebAssembly module that holds both is the pixelet kernel. Inside it
there is a layer:

```
 pxl        domain / policy
  ↑
xpute       mechanism / substrate
```

xpute's return is not the old general runtime restored; it is the
mechanisms that proved themselves under pixelet's workload, pulled down one
layer.

## The scheduler has levels, and a turn

Think of it as an OS, not as a web page. On the web the central scheduler
is JavaScript because nothing else can be; natively it would be Rust. So
xpute's mechanisms live on both sides, the same file in each language
(`packages/xpute/core/src/sched`, `crates/xpute/core/src/sched`), and each
side uses the half it runs.

```
central scheduler   owns the clock, the memory (big and little) and the
  (the host)        connections; grants a guest a quantum, memory once, and
                    I/O credits                          sched/quantum, sys/memory, sys/slab
guest scheduler     inside the guest: its phases in priority order on the
  (the kernel)      quantum, its I/O queue within the credits
                                                         sched/tick, sched/io
pass                one list of steps within a phase's share
                                                         sched/frame_budget
```

The inside knows little of the outside. A turn is a strobe with a hint of
how long it should take; the guest takes what waits on its fixed submission
ring, runs its own tasks cooperatively, and leaves on its completion ring
what goes back — what its turn spent among it. The central scheduler sizes
the next hint from what only it sees: the display's frame interval and what
the turn cost outside the guest's tick.

The quantum is not real time. It says "about this long"; a step runs to
completion, the first step always runs, and a turn that runs over costs the
next grant what it overran. Two quanta are granted, interactive and batch,
and the guest picks by whether its view moved — the split a multilevel
feedback queue makes — because which applies is known only partway into the
turn. The numbers are the guest's policy, declared to the central scheduler
at boot (the boot block's quantum words), not the mechanism's.

### Precedents

Recalled, not re-read against their sources when this was written.

| Precedent                                                               | What matches                                                                                                                                              | What to take                                                                                                                                                          |
| ----------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Chromium's BeginFrame (cc/viz `BeginFrameArgs`)                         | the strobe and the grant: each vsync the compositor sends the renderer a frame time, a deadline and an interval; the renderer works within and hands back | a grant as an absolute deadline and interval rather than a duration, so no phase's time needs separate accounting; a missed-frame flag                                |
| Android Choreographer                                                   | the phases: callbacks run in a fixed order — input, animation, insets animation, traversal, commit                                                        | the interaction → visible → … order is the platform's; commit, the correction after drawing, is the report phase                                                      |
| `requestIdleCallback`'s `IdleDeadline`                                  | the batch quantum: `timeRemaining()` is a hint for idle work, capped at 50 ms                                                                             | a hint, not a deadline; a ceiling for the batch quantum                                                                                                               |
| Seastar (ScyllaDB)                                                      | cooperative with a quota: the reactor's task quota, tasks checking whether to yield; scheduling groups with CPU and I/O shares                            | `content_share` is one fixed reserve; shares per phase are its general form, and the I/O queue can take the same shares                                               |
| Scheduler activations (Anderson et al., 1991)                           | the two levels themselves: the kernel grants resources and upcalls events; a user-level scheduler decides within                                          | the inside knows little of the outside; revocation is told by upcall, not done behind the scheduler's back                                                            |
| Exokernel (Engler et al., 1995)                                         | mechanism and policy split over granted resources: the kernel multiplexes securely, policy sits above                                                     | visible revocation: memory the central scheduler takes back, the guest learns of and gives back itself                                                                |
| Linux `SCHED_DEADLINE` with CBS; cgroup `cpu.max`                       | the guest declaring its numbers: a task sets runtime, deadline and period with `sched_setattr`; a quota over a period, hierarchically                     | the boot block's quantum is `sched_setattr`; CBS postponing an overrunning task's deadline is "an overrun costs the next grant"; cgroups when there is a second guest |
| Solaris IA class, FreeBSD ULE interactivity, Windows foreground quantum | interactive against batch: the focused or interactive process gets short, responsive quanta                                                               | the slice/batch choice is this family's                                                                                                                               |
| io_uring; virtio notification suppression                               | the turn: one `io_uring_enter` submits and reaps; completion overflow is kept aside; a doorbell only when needed                                          | one strobe a turn, and what goes back accumulates on the completion ring                                                                                              |
| Erlang BEAM reductions; Tokio's coop budget                             | a quantum counted in work, not wall time                                                                                                                  | a work-count quota as an option, so a turn can be replayed deterministically                                                                                          |

Where they differ: the OS precedents assume preemption, and nothing here
preempts — CBS's guarantee does not carry over, only its "an overrun is paid
for next time". Figma has the same host and WebAssembly-renderer arrangement,
but no published account of its scheduler was at hand to compare.

**Not xpute's:** WebGPU, the renderer, assets, the spatial model. WebGPU
may sit on it later, and even then as middleware at most — not xpute's
substance. xpute is the cooperative resource runtime and the primitive
kit — scheduler, memory, IPC, containers — and a name from the other side
appearing under `crates/xpute/core` is a layer violation.

## Is there a JavaScript xpute?

Not as one thing. The pieces exist apart:

- data-oriented, typed-array storage: bitECS, Apecs, oecs (oecs can even
  back its archetypes with a SharedArrayBuffer or a WebAssembly.Memory);
- a cooperative scheduler: the browser's event loop and task scheduler,
  and in idea, Tokio;
- shared memory and worker coordination: SharedArrayBuffer and Atomics,
  composed by hand;
- an allocator, memory pressure, reclaim: in JavaScript, almost always
  written per project.

The ECS libraries stop at entity/component storage and system execution;
they do not go down to an allocator, a scheduler with budgets, rings, a
change journal, refusal and yield, resource ownership.

The Tokio comparison holds where it holds. Tokio is a cooperative async
runtime, not a preemptive one, and it keeps a per-task operation budget:
each resource spends from it and reports _not ready_ when it runs out, so
the task yields to the scheduler. In xpute's terms:

```
Tokio: task → operation budget → await a resource → budget spent → yield
xpute: job  → frame budget     → acquire memory   → refused/starved → yield → retry next tick
```

The difference is the domain: Tokio is an I/O runtime, this is a
frame-bound resource scheduler, and it takes memory pressure itself as a
scheduling signal (allocate → REFUSED → reclaim → lower LOD → retry).
That is closer to an OS resource manager crossed with a game runtime's
scheduler than to Tokio.

What xpute is, then, in one line: _data-oriented cooperative runtime
primitives for constrained interactive systems_ — neither an ECS, nor an
async runtime, nor an allocator library, but the point where the OS's
mechanism/policy split and Tokio's cooperative budget meet. The set
together — DOD storage, an explicit allocator, a cooperative scheduler,
resource budgets, SPSC rings, a change journal, a shared-memory and
WASM-friendly ABI — is what no one JavaScript library provides.

## Opening xpute

Worth doing; pixelet need not open with it. The split just made is the
one the strategy wants:

```
open                                   private
  xpute: generic mechanisms              pixelet: the spatial product
    scheduler                              world scheduling policy
    buddy, arena                           LOD policy
    ring, port                             terrain and object semantics
    change journal                         the GIS pipeline
    lanes, containers                      the content and storage model
    shared-memory contracts                rendering decisions
    resource primitives                    the product's UX
```

Why open xpute:

1. It is the cleaner artifact of the systems work. Opened whole, pixelet
   makes a reader find the design under GIS, rendering, React and asset
   code; xpute alone says on its first page what it is and why.
2. It has a real user already. It is not a toy framework with a benchmark
   in its README: _xpute was extracted from the runtime mechanisms used by
   pixelet_ — and that is what makes a framework credible.
3. Publication is an abstraction test. A `Chunk`, an `LOD`, a `Terrain`,
   a `WebGPU` or a `PVF3` inside `xpute-core` would be visible at once as
   a layer violation; keeping a public API keeps the mechanism/policy
   line.

Why not now. xpute in its present form is a day old, and the extraction
of generic mechanism from pixelet's workload has only begun. Taking on a
package, documentation, semver, a public API, compatibility, examples and
support at this point invites the trap of April: an outside demand that
it be a _general_ runtime, and abstractions built for that rather than
for pixelet. The order instead:

```
xpute inside the pixelet monorepo
  → used by the pxl workload, continuously
  → the WebGPU/WASM migration stabilized
  → a second workload proves it
  → the xpute boundary corrected from that
  → then the public extraction
```

The second workload is the checkpoint, and it need not be large: one
compute demo — a WebGPU compute pass or a WASM simulation over the
rings, the scheduler and the allocator — that uses xpute without the
word `pxl` appearing anywhere. When that reads naturally, the genericity
is shown and the case for opening is made.

```
xpute
 ├ pixelet        the spatial workload
 └ compute-demo   a non-spatial workload
```

pixelet stays private without contradiction: React and Tokio are open
and the products on them are not. The product-specific GIS, world and
content strategy stays in; the general engineering that came out of it
goes out. As a portfolio it reads as "a spatial runtime needed
scheduling, memory and IPC mechanisms repeatedly; they were separated
into a data-oriented cooperative runtime and published, and pixelet is a
production user" — not "a library was studied and used."

Two cautions for the day it opens. Keep the scope small — `ds/`, `ipc/`,
`memory/`, `sched/` — and leave WebGPU, spatial, renderer, asset and
storage policy out at first. And avoid the grand positioning ("Tokio for
JavaScript"): the problem domain differs. _A small data-oriented runtime
toolkit for cooperative, resource-bounded workloads_ fits what is
implemented.

## What this decides now

- The crate line is mechanism/policy and stays that way; the pair check
  and the reshaping after the move both follow it
  ([where the pair differs](./archive/20260914-where-the-pair-differs.md)).
- Nothing about opening xpute is scheduled. It is recorded so the
  checkpoint — a second, non-spatial workload — is recognized when it
  comes, after the WASM move is done and the boundary has been corrected
  by use.
