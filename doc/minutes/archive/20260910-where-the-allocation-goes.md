# 2026-09-10 — what the day found, and what it could not

One day's record, in two halves that turned out to be about the same thing.

The first half is four bugs that looked unrelated and were not. The second
is an evening of profiling that named a cost and failed to name its owner.
Both end at a question neither of them could answer, which is recorded in
[20260910-point-density-and-the-order-to-multiplayer.md](./20260910-point-density-and-the-order-to-multiplayer.md).

---

# Part 1 — Four bugs, one shape

Four bugs surfaced that looked unrelated: the camera passing through a hill,
scattered decor not rendering, a settings toggle doing nothing, and the
eraser's hover flickering. They were the same bug wearing four costumes, and
the costume is worth keeping because the cure is not more tests of the kind
this project already has.

## The four

**The camera walked through terrain sideways.** "Stop at terrain" gated the
three orbit controls and nothing else. The eye is `target + orbit offset`;
panning moves the first term, and the gate only ever saw the second. The
reason nobody noticed is the shape of the thing: `required_rise(dist, polar,
azimuth)` read the target out of a store instead of taking it, so the term it
was missing did not appear in any signature. Rewriting the gate around one
`ViewPose` made the pan path obvious the moment the type existed.

**Decor did not render at all, sometimes.** Models loaded through a React
hook and reached the chunk pipeline as a prop. The effect that would have
delivered a later arrival only created nodes that did not exist yet, so a
chunk built before the models finished loading kept the null geometries it
was constructed with, forever. Whether decor appeared depended on whether the
browser cache beat the first chunk build — which is why it read as
intermittent rather than broken.

**A settings toggle did nothing.** `show_decor` was in no rebuild condition.
The node subscribes to the settings store and re-checks on every change, but
the check compares a hand-written list — tile, `show_structures`, objects
revision — and returns early when all three match. A fourth setting existed
and no line mentioned it.

**The eraser's hover fed itself.** Hovering a tree hid it so the ghost would
read as translucent; hiding it rebuilt the decor mesh without it; hit testing
raycasts that mesh; a hover that cannot find what it is hovering releases the
hide, which brings the tree back. `set_virtual_hide`'s own doc comment states
the invariant this breaks — render only, never raycast — and the invariant
holds for the voxel raycast, which reads the store. Decor raycasts the render
output. Then the fix for it was itself dropped by a filter that excluded
invisible meshes, because two different reasons for a mesh to be invisible
had been collapsed into one flag.

## What they share

Not one of them is a wrong computation. Every function involved returned the
correct answer for what it was handed. `required_rise` computed the right
clearance for the target it was given; `attach_decor` correctly skipped a
null geometry; `on_source_change` correctly found nothing changed among the
three things it compared. The defect is upstream of the computation, in what
arrived.

So each is a fact that had to travel — a pan happened, a model finished
loading, a setting flipped, an entry is hidden — from where it became true to
where it was needed, along a path that could silently drop it. And each path
dropped it a different way:

| the fact            | how it was dropped                                  |
| :------------------ | :-------------------------------------------------- |
| the target moved    | not named in the signature that needed it           |
| the model arrived   | delivered by a code path that only handled creation |
| the setting flipped | absent from a hand-written equality list            |
| the entry is hidden | one flag carrying two meanings                      |

## Why the tests could not see any of it

There are 181 of them and they were green throughout. They are also all pure:
given this run set, assert these boxes; given this input, assert this output.
A pure test asserts that a computation is right about its inputs. Every one
of these bugs is about an input that never came. The test cannot see it by
construction, and adding more of the same kind moves nothing.

[20260910-point-density-and-the-order-to-multiplayer.md](./20260910-point-density-and-the-order-to-multiplayer.md)
already says the interaction chain has no automated coverage. This says
something narrower about what that coverage would have to assert: not "does
this function compute the right answer" but "after this happened, was that
told". Those are different assertions and only the first is currently
writable.

## Where they live

All four cross the same boundary: the line between the React tree and the
imperative renderer. `chunk_node.ts` is a plain class; `chunk_stage.tsx` is
the last component above it. Decor's models came down as a prop, decor's
settings came through a subscription, the camera gate straddled a store and a
frame loop.

That is not a coincidence. Inside the React tree, "who tells whom" is
enforced — a dependency array is wrong loudly, and a re-render reaches every
consumer. Inside the imperative half, it is enforced too: a revision number
either bumped or it did not. It is only at the seam that the enforcement is a
convention, and every one of these was a convention nobody had written down.

The repair that generalizes is not a rule about React. It is that at that
seam, a fact should arrive as a **notification the receiver subscribes to**,
never as a **delivery the sender remembers to make**. `decor/asset_store.ts`
is now the first piece built that way on purpose: the models are a store,
arrival is a notification, and a chunk node hears it the same way it hears a
tile arriving. Nothing has to be told twice, and nothing has to remember to
tell.

## The smaller lesson, which is about comments

One more from the same day, close enough in kind to belong here. The hover
ghost's `coplanar_with_terrain` carried a comment explaining why carving
needed a depth bias: the ghost sat inside geometry still being drawn. True
when written. It stopped being true when the carve preview started injecting
a real AIR entry, and the code kept the bias, which made a ghost inside a
carved hole draw lines out across the surface.

A comment that states a premise ages exactly as badly as code that assumes
one, and nothing checks it. The same day's `origin: "published" | "live"`
field and `get_ele`'s name are the same defect at different scales — a name
or a comment asserting something the system no longer guarantees. Worth
naming as its own class, because it is the one the type checker will never
catch.

---

# Part 2 — A measurement record

Four Chrome Performance traces on the deployed site, two ledger sessions on
a dev server, one allocation sampling profile, and the code reading that
followed. Numbers with the conditions they were taken under, what each
establishes and what it does not, and the hypotheses the data killed.

The traces are the source of truth and are listed at the end. Nothing here
needs a new recording to be checked.

## 0. Scale of the thing being measured

From `U_PER_CELL = 1`, `M_PER_U = 2.389`, Seoul's latitude (EPSG:3857 planar
meters are inflated by `1/cos(lat)`; real meters shown).

```
1 cell   1.89 m
1 chunk  121 m       (L2, 64 cells)
1 page   1.94 km     (L3, 1024 cells)
fog far  3.41 km     (FOG_FAR = 1800u)

RENDER_RADIUS  = 2   5x5 pages   9.70 x  9.70 km    94.0 km²   mounted
LOAD_RADIUS    = 1   3x3 pages   5.82 x  5.82 km    33.8 km²   fetched
CLEANUP_RADIUS = 3   7x7 pages  13.57 x 13.57 km   184.2 km²   retained
```

Visible disc at fog radius: **36.5 km²**. Minecraft at 12 chunks of 1 m
blocks is 0.147 km², so ~248x by visible area — though this terrain is a
2.5D heightfield against a full 3D volume, so per-area cost does not
compare. Wide and thin against small and deep.

Resident during the sessions: 15 tiles at 4,203,672 B each (~60 MB), 2422
chunk nodes, 6621 POIs.

---

## 1. Chrome Performance, deployed site

Production build: no StrictMode, no dev React, minified, no instrumentation
of ours involved. Four recordings, POI toggled for B1.

|                    |          A jump |    B pan |    B1 pan |   C rotate |
| :----------------- | --------------: | -------: | --------: | ---------: |
| POI                |              on |       on |   **off** |         on |
| span               |        18618 ms |  7517 ms |   7588 ms |   17146 ms |
| longest task       |   **4903.9 ms** | 219.6 ms |  155.9 ms |   136.7 ms |
| FunctionCall       |         5012 ms |  3346 ms |   2322 ms |    8255 ms |
| FireAnimationFrame |         3151 ms |  1661 ms |   1558 ms |    6173 ms |
| GPUTask            |          988 ms |   756 ms |    729 ms |    2332 ms |
| MinorGC            |               — |      ×64 |       ×26 |          — |
| MajorGC            | **×4, 4982 ms** |        — | ×1, 22 ms | ×4, 126 ms |
| busy JS (sampled)  |         8999 ms |  2861 ms |   1825 ms |    7549 ms |

### 1a. The jump: 4.9 s, and it is Blink's heap, not V8's

Opening the single 4904 ms `RunTask` and reading only what nests inside it:

```
MajorGC                                4903.8 ms
V8.GCFinalizeMC                        4903.8 ms
V8.GC_MARK_COMPACTOR                   4903.8 ms
V8.GC_HEAP_EMBEDDER_TRACING_EPILOGUE   4898.2 ms    <- 99.9%
CppGC.AtomicSweep                 x2   4898.2 ms
CppGC.SweepInvokePreFinalizers         4898.1 ms
---
V8.GC_MC_CLEAR                            2.4 ms
V8.GC_MC_EVACUATE                 x2      1.7 ms
V8.GC_MC_EVACUATE_UPDATE_POINTERS         1.2 ms
V8.GC_MC_MARK                             0.8 ms
```

**V8's own mark-compact work totals about 7 ms.** The other 4898 ms is the
embedder-tracing epilogue: CppGC (Oilpan, Blink's C++ heap) running
pre-finalizers and then an _atomic_ sweep. CppGC normally sweeps
incrementally and concurrently; an atomic sweep is the whole heap, at once,
on the main thread.

So the stall is not JS garbage. It is Blink finalizing and sweeping its own
C++ objects. Pre-finalizers are Blink's mechanism for objects that must
release something before being swept, which is what WebGL resource wrappers
use — but the trace does not name the objects, so _which_ C++ objects is
inference, not measurement. Chrome emits no per-resource delete events in
the default categories.

### 1b. What ran in the three seconds before it

```
RunTask                      x189527   3177.4 ms
v8.callFunction               x10069   1927.8 ms
FunctionCall                   x5404   1476.3 ms
PageAnimator::serviceScriptedAnimations x179  985.9 ms
v8::Debugger::AsyncTaskRun      x303    982.7 ms
FireAnimationFrame              x249    981.1 ms
GPUTask                         x491    196.4 ms
V8.GC_MC_BACKGROUND_MARKING      x14     54.4 ms
```

Two things worth carrying: 189,527 tasks in three seconds, and
`v8::Debugger::AsyncTaskRun` at 982 ms of those three seconds. **The
recording had the debugger attached and that is roughly a third of the
window.** Every figure in §1 carries that observer effect.

### 1c. POI on vs off, same action (B vs B1)

The only controlled A/B of the evening, on the shipped build.

```
busy JS         2861 -> 1825 ms   -36%
FunctionCall    3346 -> 2322 ms   -31%
MinorGC          x64 -> x26       -59%
longest task   219.6 -> 155.9 ms  -29%
GPUTask          756 ->  729 ms    -4%
```

Rendering POI is about a third of a pan's main-thread JS and over half its
minor collections. GPU time barely moves.

### 1d. Pan vs rotate (B vs C), per second of span

```
B  pan     FunctionCall 446 ms/s   MinorGC 8.5/s
C  rotate  FunctionCall 482 ms/s   MajorGC ×4 in 17 s, MC background marking ×70
```

Rotate is slightly heavier than pan.

### 1e. Names that could not be resolved

The largest own-code entries are minified: `S7` (index:4111, in all four),
`zW` (index:5268, 552 ms in C), `Yve`, `xm`, `FRe`, `mDe`, `pDe`, `Mwe`.
`vite.config.ts` sets `sourcemap: mode !== "production"`. Resolvable three.js
frames that did appear — `updateMatrixWorld`, `multiplyMatrices`,
`renderBufferDirect`, `bindVertexArray`, `intersectTriangle` — are each under
200 ms.

A first parse merged CPU-profile node ids across processes, which put Chrome
extension frames (`proxy.js`, `setUninstallURL`, `getUILanguage`,
`connectPort`, `handleDisconnect`) at the top. Grouping by `(pid, tid)` and
picking the renderer thread removed them; they still appear at 100–200 ms in
the corrected tables and are not ours.

### 1f. Production heap, allocation rate, and frame cadence

The recordings carried memory counters. Grouped by process (pid 13841 is the
renderer; the other is ~8 MB and flat):

|                  |  heap min–max |    end |      gross alloc |           rate | frames |
| :--------------- | ------------: | -----: | ---------------: | -------------: | -----: |
| A jump           |    124–171 MB |  48 MB | 1227 MB / 18.4 s |      66.6 MB/s |   52/s |
| B pan (POI on)   | 94–**146** MB | 125 MB |  1741 MB / 7.3 s | **238.6 MB/s** |  131/s |
| B1 pan (POI off) |  55–**73** MB |  50 MB |   538 MB / 7.4 s |  **72.4 MB/s** |  122/s |
| C rotate         |     52–106 MB |  64 MB | 3054 MB / 17.0 s |     179.6 MB/s |  121/s |

Three things fall out.

**The shipped build's JS heap peaks at 171 MB.** The ledger's 1348 MB and
its 1740 MB peak (§2c) are a dev artifact — unminified modules, the React
dev build, StrictMode's doubled work, and the ledger's own retained rings.
Nothing about production memory should be read from §2.

**POI is the allocator, by 3.3x.** Turning it off takes a pan from
238.6 MB/s to 72.4 MB/s and halves the resident heap, 146 MB to 73 MB, with
everything else held constant. §3 measured POI at 0.35% of allocation and
that measurement was taken under dev + StrictMode, where R3F's doubled
reconciliation drowns it. In production it is most of the allocation.

**The display is 120 Hz and pan and rotate hold it.** 131, 122 and 121
`FireAnimationFrame` per second for B, B1 and C. The jump averages 52/s
across a window containing the 4.9 s freeze.

Also worth keeping: what is left of §2a's 78 MB/s baseline turns out to be
real. B1, with POI off in production, allocates 72.4 MB/s.

### 1g. Every major collection in the four traces

```
A jump            MajorGC  4903.8  35.7  21.5  21.0 ms
                  embedder 4898.2  34.6  20.3  19.9 ms
B pan             MajorGC    20.7 ms      embedder  19.2 ms
B1 pan (POI off)  MajorGC    21.5 ms      embedder  20.4 ms
C rotate          MajorGC    46.7  41.6  31.4   6.3 ms
                  embedder   42.5  40.3  27.6   0.7 ms
```

A major collection normally costs 20–47 ms here, and the embedder-tracing
epilogue tracks it almost exactly in every case. **The jump's 4903.8 ms is a
100x outlier against the other nine, not the going rate.** Whatever made
that one collection expensive was specific to that moment's embedder heap;
the mechanism is the same one that costs 20 ms elsewhere.

The jump's own allocation rate is 66.6 MB/s, _lower_ than a pan's. It is not
a JS allocation storm. Whatever it built up is not in these counters, which
count only the JS heap.

### 1h. Compositor

```
A   Commit  62 ms   RasterTask 21 ms   Paint 12 ms
B   Commit  49 ms   Paint      29 ms   RasterTask 13 ms
B1  Commit  46 ms   Paint      25 ms   RasterTask 11 ms
C   Commit 120 ms   Paint      76 ms   RasterTask 69 ms
```

Under 2% of every span. Not a factor.

---

## 2. Ledger sessions, dev server

Conditions: Vite dev build, React dev build, **StrictMode on**
(`main.tsx:30`), scheduler ledger overlay visible, tiles proxied from the
published deployment. StrictMode double-invokes React render and effect work;
it does not double `useFrame`.

|                  | run A (violent) | run B (lighter) |
| :--------------- | --------------: | --------------: |
| frames / span    |   1596 / 32.5 s |   1532 / 27.0 s |
| gross allocation |         5185 MB |         2629 MB |
| reclaimed        |         4456 MB |         2258 MB |
| net              |         +729 MB |         +371 MB |
| bakes / scans    |     1930 / 1231 |       579 / 455 |
| runs             |       7,905,280 |       2,371,584 |
| MB per bake      |            2.69 |            4.54 |

Parsed from the Perfetto traces the ledger emits, by summing positive and
negative per-frame `heap_mb` deltas.

### 2a. Allocation during frames with zero chunk work

Consecutive stretches where `runs == 0` and `bakes == 0`:

```
run A    0.0s +0.7s    42 frames   gross  59 MB   =  82.8 MB/s
run A   15.5s +0.8s    25 frames   gross  96 MB   = 120.1 MB/s
run B    0.0s +0.6s    39 frames   gross  37 MB   =  58.2 MB/s
run B    0.8s +1.2s    68 frames   gross  70 MB   =  60.8 MB/s
run B    2.2s +7.8s   460 frames   gross 609 MB   =  78.2 MB/s
run B   10.2s +0.4s    25 frames   gross  61 MB   = 137.0 MB/s
run B   11.3s +1.3s    73 frames   gross 117 MB   =  92.0 MB/s
run B   19.5s +1.9s   104 frames   gross 160 MB   =  84.5 MB/s
```

Session averages are 160 and 97 MB/s, so this baseline is most of it.
MB-per-bake differs 1.7x between profiles. Neither session was idle: the
camera was moving in both, and `lod pending` was 767–791, so LOD was still
draining.

### 2b. Worst frames

```
run A  dt  79.7 = task 10.7 + offthread  69.0  gpu  20.9  mount +859/-998  Δgeom -3034
run A  dt  74.2 = task  9.3 + offthread  64.9  gpu 117.9  mount +0/-0
run B  dt 147.1 = task  2.4 + offthread 144.7  gpu   5.1  mount +0/-0   Δheap -49 MB
run B  dt  40.6 = task  3.1 + offthread  37.5  gpu   5.6  mount +0/-0   Δheap -88 MB
run B  dt  40.9 = task 11.7 + offthread  29.2  gpu   5.9  mount +0/-0   Δheap  +0 MB
```

Some carry a large heap drop and some do not, so reclamation explains part
of the off-thread time and not all of it.

### 2c. Steady state, run A

```
dt        p50 10.4  p95 26.0  max 79.7 ms   over budget 324/1596 (20%)
task      p50 12.6  p95 22.1 ms
gpu       p50  7.0  p95 19.0 ms
cpu       rck p95 2.4   cmt p95 12.4   rnd p95 0.1 ms
gpu       draws 1300 (peak 3813)   tris 1854k (peak 6103k)   tex 87
mem       heap 1348 MB (+729, peak 1740)   tiles 15 (~60 MB)   poi 6621
faces     kept 3,637,575 / 12,275,340 (30%)
runs      culled 0 / 2,455,068
pool      live 308 MB   idle 265 MB   hit 100%   biggest 2 MB
bands     reused 2257 / 2263 (100%)
retain    nodes 2422   runsets 912 (93 MB)   tiers 764   decor 764 steps / 7614 instances
lod       chunks 2422   in-frustum 654 (peak 1463)   pending 767 (peak 1550)
per       commit/bake 5.6 ms   build/krun 0.3 ms   scan/chunk 2.8 ms
rate      59.4 bakes/s   37.9 scans/s   243k runs/s
```

### 2d. Network

```
terrain  n=6  p50 2055  p95 3006 ms   4,203,672 B each   decode 0.0 ms
poi      n=6  p50 1672  p95 1796 ms   3–152 kB           parse 0.1–0.4 ms
geocode  n=2  p50 2221 ms                                q 834–1108 ms
```

Every terrain tile is the same size. Six were issued at one timestamp and
completed 1.67–2.06 s later with `q0.7`, so the time is round trip plus R2
read, not queueing or decode. POI latency is independent of payload size.
Geocode spends about a second queued, which is its scheduler priority.

---

## 3. Allocation sampling, dev server

Same conditions as §2. 30 s of smooth pan/rotate over already-built terrain.
Percentages are of sampled self size; the sampled total (~42.8 MB) is far
below the 78 MB/s of §2a because sampling does not capture every allocation.
The proportions are the datum, not the sizes.

```
17.2 MB  40.18% self   31.3 MB  73.42% total   onMutate        events-…esm.js:15920
10.4 MB  24.45%                                assign
 3.5 MB   8.22%                                (anonymous)     events-…esm.js:15867
 1.6 MB   3.81%                                exports.jsxDEV  react-jsx-dev
 1.2 MB   2.81%                                _Object3D       three.core.js:11703
 574 kB   1.35%         2.5 MB   5.84%         Portal          events-…esm.js:15849
 394 kB   0.92%        36.5 MB  85.49%         setState        vanilla.mjs:4
 361 kB   0.85%         1.6 MB   3.84%         (anonymous)     chunk_stage.tsx:252
 235 kB   0.55%                                SpriteBillboard2 sprite_billboard.tsx:65
 151 kB   0.35%          233 kB  0.55%         (anonymous)     poi_layer.tsx:335
 115 kB   0.27%                                collect_procedural_decor_instances  decor_kinds.ts:238
  82 kB   0.19%                                Raycaster / Billboard / ChunkNode
  60 kB   0.14%                                add             ivec.ts:146
```

`commitLayoutEffectOnFiber` accounts for 40.14%. Expanded bottom-up, each row
being the caller:

```
onMutate
  (anonymous) vanilla.mjs:9
    forEach
      setState   vanilla.mjs:4
        setSize    events-…esm.js:1017
          configure  events-…esm.js:15545
```

`configure` and `setSize` are R3F's own; `configure` is what `<Canvas>` runs
from a layout effect. By area: R3F reconciliation ~76%, React ~7%, three.js
object construction ~2%, and everything under `apps/web/src` together under
2%. StrictMode doubles the React side and not `useFrame`, so that ratio is
inflated by roughly 2x on one side.

---

## 4. Read from the source afterward

Facts about the code, not measurements.

**POI selection walks everything loaded.** `select_nearby_pois`'s own comment
says so: "runs over every loaded POI, not just the ones that end up
rendered". It is a `useMemo` keyed on `view_state`, and `poi_count` was 6621.
No spatial index; `frustum_budget.ts` walks the full candidate list.

**`frustum_budget.ts` reuses its scratch and not its results.** `$frustum`,
`$sphere`, `$local_pos`, `$camera_pos`, `$scratch_matrix` are module-level.
`const in_frustum: {item,id,d2}[] = []` is fresh per call, one object literal
is pushed per in-frustum candidate, and `.map()` allocates one or two more
arrays.

**Each POI badge builds its own canvas and texture.**
`build_poi_badge_canvas` calls `document.createElement("canvas")`; it is
`useMemo(…, [poi])`, so per badge mount rather than per frame. Ledger frames
carry `Δtex` swings of −72 to −82.

**Characters and billboard fades use module scratch.** `companion.tsx` holds
`$parent_quat` / `$parent_euler` at module scope, with a comment that they are
never held across the `useFrame` body; `cam_world_pos` is a
`useRef(new THREE.Vector3()).current`. `use_distance_fade.ts` uses a
module-level `$world_pos`. `sprite_billboard.tsx` memoises its material and
disposes it on unmount. No per-frame allocation is visible in these paths.

**Geometry is destroyed on detach.** `chunk_node.ts`'s `detach_all` calls
`entry.mesh.geometry.dispose()`; each attach builds a fresh `BufferGeometry`.
The mesh pool pools the CPU-side typed arrays — `hit 100%`, buddy split and
merge — and not the GPU-side objects.

**`MapScene` subscribes to `map_center` through four paths.**
`useTileFetcher` (`use_tile_fetcher.ts:83`) and `usePoiFetcher`
(`use_poi_fetcher.ts:83`) each call `useMapStore((s) => s.map_center)`, and
`useMapNavigation` — called from both `useMapBootstrap` and `useTileFetcher`
— selects it through `useShallow`. `map_center` is a fresh PVF3 object per pan
tick and the store's setter has no equality check, so `useShallow` does not
stop it. `<Canvas>`'s `gl`, `camera` and `dpr` props are inline literals.

**No instrumentation exists in production.** Every ledger call site is behind
`import.meta.env.DEV`. The overlay's `show` flag gates only the 500 ms panel
repaint; recording and the console handles run regardless.

---

## 5. What the measurements establish

- A jump on the deployed build produces one ~4.9 s task, of which 4898 ms is
  CppGC pre-finalizers plus an atomic sweep of Blink's C++ heap. V8's own
  mark-compact phases total about 7 ms. The stall is embedder-heap
  finalization, not JS garbage collection.
- Rendering POI costs ~36% of a pan's main-thread JS and ~59% of its minor
  collections, measured by toggling it with everything else held constant on
  the shipped build.
- Pan and rotate cost about the same per second; rotate is slightly higher.
- Under dev + StrictMode, ~76% of sampled allocation is R3F reconciliation
  reached through `<Canvas>`'s layout effect, and everything under
  `apps/web/src` is under 2%. That ratio does not survive to production —
  see §1f, where POI alone is most of the allocation.
- The shipped build's JS heap peaks at 171 MB and holds 120 fps through pan
  and rotate. A major collection costs 20–47 ms; the jump's 4903.8 ms is a
  100x outlier against the other nine in these traces.
- Terrain fetch is 2–3 s per 4.2 MB tile, latency-bound; decode is 0.0 ms.
- The chunk pipeline's normalized costs are 5.6 ms per bake, 2.8 ms per chunk
  scan, 0.3 ms per thousand runs, at a 100% mesh-pool hit rate and 100% band
  reuse.

## 6. What they do not establish

- **Which C++ objects the 4.9 s sweep is finalizing.** The time is provably
  in `CppGC.SweepInvokePreFinalizers` and `CppGC.AtomicSweep`; the trace does
  not name the objects, and Chrome emits no per-resource delete events in the
  default categories. Geometry disposal is a plausible source, and the
  correlation with the ledger's `Δgeom -3034` in one frame supports it,
  but nothing here measures the link.
- **What re-renders `<Canvas>` in dev.** The allocation stack ends at
  `commitLayoutEffectOnFiber`; the `setState` that scheduled the render ran in
  a different task and the sampler cannot span it. The `map_center` paths in
  §4 are a reading of the source, and §1d is evidence against them being the
  dominant cost, since rotate does not write `map_center` and is not cheaper.
- **What allocates the 78 MB/s dev baseline.** The largest own-code entry is
  0.85%, so no single site in `apps/web/src` accounts for it.
- **Whether the dev numbers describe the shipped build.** Production has no
  StrictMode, no dev React and no instrumentation, so §2 and §3 measured
  something that is not shipped.
- **The observer effect's size.** `v8::Debugger::AsyncTaskRun` is 982 ms of
  the 3 s preceding the jump stall. Every §1 figure includes the cost of
  being recorded.
- **What the minified frames are.** No source map in the production bundle.

## 7. Hypotheses the data falsified

- **The ledger's own instrumentation as a significant allocator.**
  `chunk_stage.tsx:252` — the whole DEV `useFrame` including
  `ledger_record_frame` and a per-frame `JSON.stringify` into a DOM dataset —
  is 0.85% self, 3.84% total; `JSON.stringify` does not appear at all. The
  overlay repaints twice a second over a 240-sample ring: tens of kB/s
  against 78 MB/s.
- **`raycast_decor` as an allocator.** `Raycaster` is 0.19%.
- **POI as an allocator — falsified, then reinstated.** §3 puts
  `poi_layer.tsx:335` at 0.35% and the whole billboard path under 1%, and
  that reading held only under dev + StrictMode, where R3F's doubled
  reconciliation drowns everything. §1f measures the shipped build: POI
  costs 238.6 MB/s against 72.4 with it off. It is both a third of the CPU
  and most of the allocation.
- **Mount storms as the cause of the worst stalls.** Run A's worst frames
  carry `mount +859/-998`; run B reaches a worse frame (147 ms) at
  `mount +0/-0`.
- **A JS-heap explanation for the jump.** V8's mark-compact phases are 7 ms
  of 4904.

## 8. Defects in the ledger's own reporting

- `offthread` is `max(0, dt - task)`, but `task_ms` spans from
  `chunk_stage`'s `useFrame` start to the end of the whole rAF task,
  `gl.render` included, so it is not nested inside `dt`. `offthd p50 0.0`
  means `task >= dt` for half the frames. Reported off-thread time is a lower
  bound.
- Per-frame `gpu_ms` is stamped on whichever frame drains a completed timer
  query, which is how a 105.5 ms GPU figure lands on a 33.7 ms frame. The
  distribution is usable; individual frames are not.
- `runs culled` read 0 across 2.45 M runs for a whole session while
  face-level culling removed 70%. Either the counter measures something
  structurally unreachable or run-level culling did not run; not determined.

---

# Part 3 — Why this stops here

Two repairs follow directly from Part 2: pack POI labels into an atlas so
the texture count stops tracking the visible badge count, and pool
`BufferGeometry` so the `WebGLBuffer` count stops tracking the chunk count.
Both were rejected on 2026-09-10 as premature, and the reason is the right
one.

An atlas is a decision about how billboards are batched. Pooling geometry is
a decision about how a chunk's residency is expressed. Both are answers to
the question that is actually open — what happens at ten thousand billboards,
ten thousand chat bubbles, or ten thousand actors in one small space — and
answering it in two subsystems separately, ahead of the model, is how the
answers end up disagreeing.

The measurement is not wasted, because it changes what that model has to be
judged against. Whatever storage, index and batch design comes out of
[20260910-point-density-and-the-order-to-multiplayer.md](./20260910-point-density-and-the-order-to-multiplayer.md),
it now has a number to beat: a jump must not spend 4.9 s in
`CppGC.SweepInvokePreFinalizers`, and it will not, if the design keeps the
count of browser-owned objects constant rather than proportional to what is
on screen. That constraint is the residue worth carrying, not the two
repairs.

## Sources

```
~/Downloads/A-Trace-20260910T080510.json.gz             jump,   POI on
~/Downloads/B-Trace-20260910T080618.json.gz             pan,    POI on
~/Downloads/B1-Trace-20260910T080716.json.gz            pan,    POI off
~/Downloads/C-Trace-20260910T080816.json.gz             rotate, POI on
~/Downloads/pxl-ledger-2026-09-09T22-28-36-003Z.json    ledger run A
~/Downloads/pxl-ledger-2026-09-09T22-34-38-426Z.json    ledger run B
```

The Chrome files are gzipped Chrome trace JSON; the ledger files are
Perfetto JSON from `sched/trace.ts`, carrying one counter track with
per-frame `heap_mb`, `runs`, `bakes`, `mounted`, `geometries`, `gpu_ms`,
`commit_ms` and the rest of `FrameSample`.
