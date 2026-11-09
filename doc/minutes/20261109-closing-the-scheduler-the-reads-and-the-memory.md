# Closing the scheduler, the reads and the memory

**2026-11-09** — recorded with the user. Main was at `443124ecfb` when this was
written. What is done, what was decided against, and what is queued, each with
its reason. Who decided is said where it was the user; the rest was judged by
the agent working on it against the code.

## What was asked

Whether the cooperative scheduler, the fetches and the memory's limits and
budgets are closed, and whether the memory is described correctly; then to close
them structurally, with nothing ad hoc. The user's frame for it: memory is fixed
at 512 MiB on a handheld and 1 GiB on a desktop, and a heap that fills stops the
core, nothing cleans up because it is full, and each owner lets go of what it
holds. The problem was to be closed, not grown.

## What was found

The design of the memory and the scheduler stood. What was wrong were small
faults and records, listed below. The one thing whose size follows its data
without a bound is the objects.

- The scheduler: every unit is bounded, yields at a boundary and advances each
  turn. Outside the turn's budget run the location indicator, the glTF parse and
  the objects' apply, the last alone growing with a shard. A CPU ray ends at
  `STEPS_CAP`, 4096 steps. The render passes loop over fixed counts or the turn's
  draw lists, and a frame that no longer fits the GPU's stream is dropped.
- The memory: the front, `.fixed` and the heap's bounded flows have caps the
  build checks. The rest of the heap is workspace and has no sum, by design:
  the user ruled out taking the caps added up as the limit.
- The tiles: the four layers take one path (`TileCache`, `Demand::admit`, the
  farthest from the view evicted first). They differ only in where a tile's
  content lies.

## Done, and why

| What | Why | Commit |
| --- | --- | --- |
| The recorded memory limits, `EVENTS` and `turn_posts` | They did not match what the code holds | `672ff934dc` |
| `HEAP_FLOWS_BYTES` called a budget's sum, not the most live at once | Flows share credits and free blocks do not add up to one block | `a4620890f8` |
| The network's recovery wakes at `next_pump_at` | After a failure the turns spun | `17602cd358` |
| The fetch queue drops what was stopped, arrivals at 2 × credits | A path overflowed the queue | `17602cd358` |
| `TileCache::again` reports its new mark as changed | A tile asked for again was missed | `17602cd358` |
| The clipmap reports a change per brick | A coarse tile's report threw away the location indicator's memo every frame ([measured](../experiment/20261109-the-location-recomputed-less.md)) | `ef2298f898` |
| `script/perf/measure.py` reads the results in batches | The script hung on a large result | `ef2298f898` |
| The tiles' columns and rows take one set of names, plural columns and singular rows, `XxxTile` and `Xxx` | The user's discipline: tiles may differ in structure, but a field that is the same thing has the same name | `a4620890f8` |
| The place and division decoders build their rows without `Option`, and `tile_of` returns a tile | `StrList::get` fails only past the list's length, and each decoder has already held every column to one length and, for the divisions, every kind to a known one, so the failure branches could not be taken | not yet committed |

## A read the replaced runtime left

The user's word: not to leave it as a recommendation, and to secure the minimum
of symmetry.

- **The fault.** A core outlives a JS reload on a device, and a new runtime reads
  the rings from where the old one stopped (`ipc/head.ts`). A read the core had
  posted and the old runtime had not taken reaches the new one, which holds no
  such file. `FS_CLOSE` answered `EBADF` for such a descriptor and `FS_WRITE`
  returned; `FS_READ` alone threw, which ends the runtime. Read from the code, not
  run on a device. Nothing in the app reloads JS outside development.
- **Done.** `FS_READ` of a file the runtime does not hold is dropped, and its close
  answers `EBADF`, held by the trace `read.unknown` in `conformance/fs/file.tsv`.
  A file held with no bytes still throws: that is the core reading before its open
  completed, a bug on the core's side.
- **What the dropped read leaves.** The core's buffer stays zeroed. A fetch's body
  goes with its attempt, since `request::reset` on the attach ended the attempt and
  `fetch.rs` `drop_stopped` lets go of an arrival with none. An asset's read has no
  attempt to check (`loader.rs` `arrival`), so that asset takes zeros, as it would
  a corrupt file, and stays failed until the process restarts. The settings' read
  at boot, caught the same way, would be refused and set aside.
- **The descriptors an unanswered open held.** `request::reset` ended each attempt
  without closing its descriptor, so a reload with opens out left up to the
  credits' 16 of the core's 19 descriptors taken, and a later reload could stop the
  core on `ENOSPC`. It is a slot not given back in a fixed table, not fragmentation.
  On the user's word it is closed: `reset` now stops what is out through the path
  that stops anything no longer wanted (`request.rs` `stop`, `dispatch::stop`), and
  keeps a stop already pending. The new runtime answers such a close `EBADF`, which
  the core ignores. No xpute change was needed, so `FileTable::forget`, which the
  user had removed, stays out. One descriptor is not covered: the settings' open at
  boot has no attempt, and is taken once a process.

## Decided against, and why

- **An entrance check on the heap** (a watermark, an estimate per byte, shedding
  and growth, `heap_admits`). Built and removed. The user: a full heap stops the
  core, and what is held is let go by its owner. It also decides from a guess at
  what will be taken.
- **Changing the buddy allocator.** The user's word: xpute is the system library
  and this was not its problem. The block `[384, 512)` MiB cannot be split, so a
  phone's heap of about 410 MiB reaches to 384.
- **A byte budget or LRU on the tile caches.** The slots already bound them, and
  the view's order knows what comes next, which LRU only guesses. A budget on the
  places and the divisions alone gives two of four layers a rule of their own, and
  the bake already limits one tile's bytes. This withdraws the budget proposed in
  [the heap's minutes](./20261109-the-heap-if-a-full-one-need-not-stop-the-core.md).
- **Reading place and division tiles in place from the packet.** The reviewer
  found that it removes no concept and adds seven.
- **The glTF parse split into a task.** 61 files, 7.9 MiB in all, the largest
  0.26 MiB, each parsed once and all of them shipped by us. `decor` is 0.1 ms at
  p95 on the S23.
- **The objects' apply moved into the budget** (`apply_held`). A shard is applied
  whole wherever it runs, and server tiles rewrite this.
- **Extending the objects.** The user's word. Their structure is temporary and
  server tiles close its bound, its release and its apply.
- **`SupportWalk`, a second ground query for the location indicator.** It had one
  caller and no measured effect. The measured waste was how often it recomputes.
- **Work that runs to the deadline in a turn with no frame.** `4501dc3181` did
  and regressed: those turns held the CPU to the deadline and asked for the next.

## Queued, and why

| What | Why |
| --- | --- |
| The frame test's intermittent failure, "the view did not rest within 10 idle turns" | Every other check leans on it. At `443124ecfb` it did not fail: three runs of `cargo test-host`, four under a CPU load of 20 processes, 30 of six copies at once on the GPU, and 91 with 0 to 90 extra frames shifting the clock's phase. The cause is unknown, and it is looked at again when it next fails |
| The location indicator's ~10 ms a recompute, outside the budget | The largest measured CPU cost left in a turn |
| After the ways' move to the heap: the memory's record and the ways' turn budget again | The ways' pool is 64 MiB of the flows' sum, and bricks and their fill change |

## Open

Waiting on other work: the objects (server tiles) and the GPU's cost.
