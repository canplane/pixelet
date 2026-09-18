# The mesher in Rust, on the phone and the desktop

**2026-09-13** — the first Rust (BACKLOG P0-1): the chunk mesher translated
line for line into crates/pxl-core, run through crates/pxl-wasm's numeric
exports on the main thread, stepped under the frame budget exactly as the
TypeScript bake is, and held byte for byte to it by wasm_bake.test.ts. Both
sit behind the dev setting `bake_path`: `stepped` (TypeScript) and `wasm`
(Rust). The module keeps its own memory, so a wasm bake also pays a copy of
the run set in and a copy of the mesh out; every number below includes
both.

This measures the translation only. The algorithmic changes the port
carries (the O(k²) neighbour walks, the coarse pack's selection sort) are
not in it, so this is the baseline they are measured against.

## Procedure

**Phone.** iPhone 13 mini, iOS 26.1, Safari. `deno task dev:remote` over the
LAN (unbundled modules), a place reached by search, four sessions in the
order stepped, wasm, wasm, stepped, a page reload before each, the ledger
report taken at the end. The traces are
`pxl-ledger-2026-09-13T11-29-05-704Z.json`, `…11-31-42-821Z.json`,
`…11-33-52-368Z.json` and `…11-35-30-633Z.json`, not in the repository.

The sessions were not the same scene. 1 and 2 are close (58 chunks in the
frustum at most, 363k and 442k triangles at most). 3 zoomed out much
further (321 chunks in the frustum, 983k triangles) and baked seven times
the runs of 2. 4 zoomed out part way (208 chunks) and had a slow network
(terrain fetch 4.3 s mean against 1.5 s in the others). So the pairs are
1 with 2, and 3 with 4 loosely; nothing is read across them.

As in the worker experiment, frames were split from the traces:
**filling** is `lod_pending > 0`, **settled** is nothing pending and the
camera moving, and a frame is missed when its dt is over 25 ms. Safari's
`performance.now()` is whole milliseconds, so single-step times below 1 ms
read as 0 or 1 and the per-run figures are sums divided out.

**Desktop.** `deno bench` (V8) on an Apple M1 Max, whole bakes of two run
sets from wasm_bake.test.ts's scene generator — `flat` (4,608 runs, long
equal stretches) and `busy` (10,288 runs, stacked runs, objects, sub-cells)
— at steps 1, 2, 4 and 16. The script is [bench.ts](./bench.ts).

## Numbers

Phone, from the reports (workload-normalised lines, and the bake line):

| session             | build per 1k runs | commit per bake | longest bake step | runs baked |
| ------------------- | ----------------- | --------------- | ----------------- | ---------- |
| 1 stepped           | 0.3 ms            | 3.8 ms          | 17 ms             | 856k       |
| 2 wasm              | **0.1 ms**        | 2.6 ms          | **1 ms**          | 1,118k     |
| 3 wasm (zoomed out) | **0.1 ms**        | 0.9 ms          | **4 ms**          | 7,242k     |
| 4 stepped (part)    | 0.2 ms            | 1.9 ms          | 12 ms             | 2,101k     |

Phone, filling, from the traces:

|                  | 1 stepped | 2 wasm     | 3 wasm  | 4 stepped |
| ---------------- | --------- | ---------- | ------- | --------- |
| frames           | 107       | 146        | 535     | 208       |
| runs baked       | 467k      | 664k       | 6,930k  | 1,708k    |
| runs per frame   | 4.4k      | 4.5k       | 13.0k   | 8.2k      |
| missed           | 16%       | **8%**     | 20%     | 13%       |
| task p50 / p95   | 8 / 21    | **3** / 17 | 5 / 16  | 7 / 15    |
| commit p95 / max | 15 / 22   | 11 / 19    | 10 / 22 | 12 / 20   |
| offthread p95    | 31        | 21         | 26      | 23        |

Phone, settled and moving: missed 9%, 15%, 21%, 9%; task p95 10, 10, 9,
10; commit p95 3, 2, 1, 2; offthread p95 25, 28, 31, 25. Nothing bakes
much there (66–104 bakes per session), and the misses follow the
off-thread time and the triangles, not the path.

Desktop, mean per whole bake, V8:

| run set | step | ts     | wasm   | ts / wasm |
| ------- | ---- | ------ | ------ | --------- |
| flat    | 1    | 741 µs | 506 µs | 1.47x     |
| flat    | 2    | 759 µs | 670 µs | 1.13x     |
| flat    | 4    | 302 µs | 255 µs | 1.19x     |
| flat    | 16   | 148 µs | 92 µs  | 1.62x     |
| busy    | 1    | 4.1 ms | 4.0 ms | 1.03x     |
| busy    | 2    | 3.4 ms | 3.5 ms | 0.99x     |
| busy    | 4    | 974 µs | 819 µs | 1.19x     |
| busy    | 16   | 364 µs | 220 µs | 1.65x     |

## What it showed

- **On the phone the Rust bake costs a third to a half of the TypeScript
  one per run**, copies included: 0.1 ms per thousand runs in both wasm
  sessions against 0.2–0.3 in both stepped ones, across a sevenfold spread
  in how much was baked.
- **Its steps are short where the TypeScript's were not.** The longest
  single step was 1 and 4 ms against 17 and 12 ms. The TypeScript's 17 ms
  is over the 12 ms budget it steps under; the same rows per step now take
  a fraction of it. The worker experiment read 6 ms for the same path earlier in the
  day — the stepped bake's longest step varies with the scene, and the
  Rust one's ceiling is lower in every session read.
- **The closest pair, 1 and 2, baked the same per filling frame at less
  main-thread time**: 4.4k and 4.5k runs a frame, task p50 8 against 3 ms,
  missed 16% against 8%. Session 3 baked 13k runs a frame — half as many
  again as 4 — at a similar task p95. How fast a view fills is still set
  by the network and the scan (the pending windows are 17–25 s on every
  path, most of it fetch queue), not by the bake.
- **On the desktop the gap is smaller**: V8's JIT gets the TypeScript
  within 1.0–1.65x of the Rust. The dense fine bakes (`busy` at steps 1
  and 2) are even, which suggests — not measured — that their time is in
  work the translation kept, the per-run neighbour walks, rather than in
  the language. The coarse
  tiers gain most. JavaScriptCore on the phone is where the translation
  alone pays, which is where the decision is.

## The rewrite, measured

The same day, the two algorithmic items P0-1 carried for the mesher were
taken, with wasm_bake.test.ts as the net (its scene now also echoes
sub-cell boxes out of order, the way the scan's overflow sub-voxels trail
their band).

- **Neighbour walks as forward cursors.** A run's own column was scanned
  four times and each neighbour column from its head for every run, O(k²)
  per stacked column. Rewritten as one cursor per neighbour that only moves
  up — exact, because a column's full runs are ascending and disjoint and
  no run before a full run starts above it — the mesh stayed byte for byte,
  and bench.ts did not move: `busy` at step 1 4.1 ms before and after,
  `flat` at step 1 506 µs before and 586 µs after (the cursors cost more to
  set up per column than the walks they saved). The scenes stack at most
  four runs a column, as the scan does; k² at k ≤ 4 was never the cost.
  Reverted.
- **Where the time is.** Stepping `busy` at step 1 natively (release, M1
  Max) put 950 µs in the face masks, 20 µs in the merge, 160 µs in one step
  that zero-filled the mesh, and 1,000 µs in writing it. Split through the
  module: 12 µs copying the run set in, 3.5 ms baking, 460 µs copying a
  16 MB mesh out. That mesh is the fixture's worst case — the phone
  ledger's largest mesh block was 2 MB — and at that size a fine bake is
  memory bandwidth: the words are written once, copied once, and were
  zeroed once before either.
- **Kept:** the zero-fill is gone (the mesh buffer is grown, never cleared;
  every word read is written first, and the visibility lane is the
  renderer's, as in a pool block): that step went from 162 µs to 13 µs for
  `busy` and 16 to 6 µs for `flat`, read natively on its own since the
  machine's load average was above 5 and whole-bake times moved by more
  than that between runs. The coarse pack's selection sort is a sort;
  `merge_run`'s `-1` and the pack's `pick`/`open` are `Option`. Neither
  measured.

What would move a fine bake was not in the mesher: the copy out went the
same day, when the module was given the core memory to import — the run
set is baked where it lies and the mesh written into its block, in the map's
new module section — and `busy` at step 1 went from 4.0 to 3.3 ms through
the module (bench.ts, V8; the TypeScript 4.1). The writing itself is bounded
by how many faces survive, which the merge and the tiers decide.

## The scan, the same day

The whole-chunk scan went the same way — crates/pxl-core/src/bake/scan.rs, the
walk translated from voxel_scan.ts, reading the composite's bands and
overflow lanes where the compositor left them in the core memory through
a table of addresses, packing the set into a block acquired at the end —
and wasm_scan.test.ts holds it byte for byte to the TypeScript scan over
composites with band gaps, stacked objects, sub-cells and overflow (each
of three rules removed from the Rust fails it). On V8 the scan of the
test's busy composite went from 7.1 ms to 3.7 ms (1.9x); the phone is not
read yet.

## The phone, with the memory shared

**2026-09-13, later.** The module importing the core memory, the scan in
Rust and the coarse bake packing cell by cell (see the commits of the
afternoon), one session on the same iPhone at a wide zoom — 233 chunks in
the frustum, 1.05M triangles, 3.2M runs baked in the window — read from a
dev server, whose ledger build line then named the server's start rather
than the source (since made per page load):

|                   | 3 wasm (own memory) | 5 wasm (shared) |
| ----------------- | ------------------- | --------------- |
| in frustum / tris | 321 / 983k          | 233 / 1,054k    |
| build per 1k runs | 0.1 ms              | 0.2 ms          |
| scan per chunk    | 0.5 ms              | 0.7 ms          |
| longest bake step | 4 ms                | **21 ms**       |
| filling, task p50 | 5 ms                | 9 ms            |
| filling, missed   | 20%                 | 24%             |
| module heap       | —                   | 1.1 MB of 32    |
| mesh section      | 153 / 128 MB        | 142 / 128 MB    |

The module's heap was fine. The rest reads worse than session 3, and the
21 ms step says why: at this zoom the mesh section is past its range on
the phone (as it was in session 3 — and the page section too, 136 of
64 MB; the reservation is simply under this view, BACKLOG P1), so blocks
spill to buffers the module cannot reach, and with the memory shared each
such bake was restarted the TypeScript way. Session 3's module had its own
memory and copied its mesh into whatever block it was given, spilled or
not, so it never lost a bake to the section being full.

Taken the same evening: a mesh or a set up to 8 MB whose block is out of
the module's reach is staged in the module's heap and copied out, which
is the copy session 3 always paid, paid only when the section is over;
larger than that the bake still goes the TypeScript way. The ledger's
bake line counts both (`staged`, `fell back`).

Read again on the phone (session 6, `b6+ r185-f48ae79`, a lighter pass:
252 chunks in the frustum at the peak, 1.3M runs baked in the window,
most of it settled): 33 meshes staged, nothing fell back, build per 1k
runs back at **0.1 ms**, scan per chunk 0.8 ms, module heap 4.2 MB of 32;
filling missed 8% of frames at task p50 8 ms. The longest bake step was
still 17 ms with no TypeScript bake in the window, so that step is the
module's — or the pool's: a block past the section cuts a 64 MB region
of its own, which on a phone is an allocation of that size inside the
step. Not separated; the same cost sits on the TypeScript path, and the
section being over is P1's.

## What becomes of it

`wasm` became the default bake path on these numbers, the same day. The
same scene could not be reproduced on both paths by hand, and the per-run
cost and the longest step carry the case without it. `stepped` stays behind
the setting as the reference the byte-for-byte test and this comparison run
against, until the TypeScript mesher is retired; a module that fails to
load falls back to it. Both went on 2026-09-14
(`559fc68a`): the TypeScript bake is only the
tests' reference now, and there is no fallback. The algorithmic rewrite is next on P0-1, measured with
bench.ts and the phone again.

Not measured:

- **Memory.** The module's own memory — its input copy, the mesh it
  writes before the copy out, its scratch vectors — is outside the core
  memory and invisible to the ledger. By construction it tops out near the
  largest run set plus the largest mesh, a few megabytes, and it never
  shrinks. The ledger's core sections were alike across paths for like
  scenes (mesh live 92 and 98 MB in 1 and 2).
- The same scene on both paths, and a production build.
- A Chrome-on-Android reading, where the engine is V8 and the desktop
  bench suggests a smaller gain.
