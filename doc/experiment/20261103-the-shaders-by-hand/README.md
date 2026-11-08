# The shaders by hand

**2026-11-03 to 2026-11-05.** This record tells why the page drew nothing on a
Galaxy S23 and an iPhone 13 mini while rust-gpu generated its shaders from Rust,
what each phone's compiler refuses and what made the phones take the shaders; then
what came of writing every pass again by hand in WGSL and holding it word for word
to pxl-engine: what a test at 0 differ proves, the faults the tests found, and what
the hand shaders cost and saved on the S23. The work is closed: both phones took
every generated pipeline after `4a74a51f2`, rust-gpu went whole on 2026-11-05 with
every pass hand WGSL held to pxl-engine
([the minutes](../../minutes/20261105-the-shaders-by-hand-held-to-the-engine.md)),
and the S23 has drawn with the hand shaders since that day
([the frame on the S23](../20261103-the-frame-on-the-s23/README.md)). What holds
beyond this round is in [shaders the phones take](../../tip/shaders-phones-take.md)
and [when the CPU and a GPU must give one answer](../../tip/cpu-and-gpu-agree.md),
and what is still open is listed at the end.

Why rust-gpu went is in [rust-gpu goes](../../minutes/20261103-rust-gpu-goes.md).
The S23's own costs, with the report and the tool of each run, are in
[the walk on a phone](../20261103-the-frame-on-the-s23/README.md). An S23 figure here
comes from one load unless it says otherwise, and a pass moves up to twofold between
loads, so only a large difference says anything. The commits named here are mapped
to `original` and to main at the end.

## How it was run

| Device | What compiles the WGSL |
| --- | --- |
| Galaxy S23: SM-S911N, Adreno 740 (`adapter.info`: vendor `qualcomm`, architecture `adreno-7xx`) | Chrome 156.0.8078.73 on Android, which reports `Android 10; K`: Tint, then the phone's Vulkan driver |
| iPhone 13 mini | Safari 27.0.1: WebKit's own WGSL compiler, then Metal |
| Mac | Safari, which fails as the iPhone does and stood in for it; Chrome, through Tint to Metal; the reference tests, through wgpu to Metal |

The dev server ran on :5173. The phones reached it through a Cloudflare quick
tunnel, and the S23 also reached it over USB (`adb reverse tcp:5173`), where
`localhost` is a secure context. The Mac drove the S23's Chrome over the DevTools
protocol (`adb forward tcp:9333 localabstract:chrome_devtools_remote`).

The probe makes each shader module under `rust/client/render/src/frame/shader/` a
pipeline on the device it is opened on, one module alone, and prints what the
device said. `?only=` picks some modules,
`?bisect=` cuts a module short with `return;`, and `?src=` loads a shader under
`doc/experiment/`. In this round the probe was `ts/app/web/probe.html`
(`src/probe.ts`, dev only), and each line it printed also reached the dev server's
console (`src/remote-console.ts`) and `ts/app/web/.dev-console.log`. It is
`ts/tool/shader/probe.html` now (`pnpm probe` there, on :5173, with the same
parameters), and its console goes to `ts/tool/shader/.dev-console.log`. Tint's test
compiles every module: it was `src/shader.test.ts`, and it is
`ts/tool/shader/shader.test.ts`, which takes the assembled hand modules alone.

`tools/` holds what reduced the phones' refusals:

| Tool | What it does |
| --- | --- |
| `cdp.mjs` | sends a WGSL file from the Mac to the S23's Chrome, makes it a compute pipeline there (a render pipeline with `RENDER=1`) and prints what came back |
| `catch_render.mjs` | a hook on the page that keeps the WGSL of each pipeline the S23 refuses |
| `reduce.mjs` | shrinks a file while the device still refuses it: it empties a block, removes a loop, an assignment or a dead `let`, or makes a condition constant |
| `guard.py` | bounds every loop to 1000 turns before a reduction, so that a cut cannot leave a loop that never ends |
| `descend.py` | empties one block at a time; it read the `*.gen.wgsl` modules by default, which went with rust-gpu, and it takes a file as its second argument |
| `fnbisect.py` | empties one function at a time |
| `split.py` | rewrites each array- or struct-typed `phi_*` local as one variable per leaf |
| `forward.py` | replaces a constant access into a composite `let` with its component, function by function |
| `vecify.py` | writes an array of 2 to 4 `u32`, `i32` or `f32` as a vector, outside the structs a binding reaches |

The always-inlined set of modules was built by `pxl-shader-build` with `wgsl_of`
made to pick `inlined`.

The hand passes are tested in pxl-render on the Mac's GPU against pxl-engine, one
test binary at a time under a lock ("Running the work" tells why). `supervise/` holds
the sweeps of `world.wgsl` and their tools, which
[supervise/sweep-f9f66682e.md](supervise/sweep-f9f66682e.md) describes: `round.sh`,
`patch.py`, `gen_sweep.py` and `run_par.py` make and run the mutants, `livepath.py`
counts the live words along a call path, and `tint.mjs` compiles a module through
Tint and Dawn. `decor/` holds the plan for the decor passes
([decor/plan.md](decor/plan.md)), the supervisor's rulings on it
([decor/rulings.md](decor/rulings.md)), and the output of their tests and sweeps:
the differential of `decor_cs` against the generated module and the Rust, the frame
chains, the mutant lists and the sweeps' results, and the `regions` words read on
the page from the generated and from the hand passes.

## What the phones refused, and what they take

At `feee29054` on main, with the generated WGSL of `4a73ebdd0` (where `i32::MIN` was
already a call), the page drew nothing on either phone. Each phone had a fault of
its own: the S23's driver refused to make pipelines, and Safari's WGSL compiler
wrote Metal that does not compile. Neither fault is pixelet's. Each was worked
around here only as far as the experiment needed, and what was adopted was the
user's choice.

### The S23 refuses composites carried through branches

Every module was valid WGSL, and Tint took each one. The S23's driver failed nine
compute pipelines and one render pipeline in `vkCreateComputePipelines` and
`vkCreateGraphicsPipelines` with `VK_ERROR_UNKNOWN`, whether the pipeline was made
async or not:

| Refused | Taken |
| --- | --- |
| beam, cell, decor, face, light_miss, light_propagate, miss, miss_list, sample_box; one `vs+fs`, unnamed at first and found to be the raster | the 21 other compute modules |

No WGSL feature set the refused modules apart from the taken ones: not their size
(`cell`, 30 KB, failed, and `sample`, 380 KB, was taken), nor loops, atomics, the
`array<vec4<i32>, 46>` uniform or `vec2<u32>` members. Reduced on the device, the
refusals came in layers, each found once the one before it was removed:

1. `cell` emptied down to a pair of nested loops whose exits ride on locals, after
   an `array<u32, 2>` local that one branch of an `if` sets and whose elements are
   compared ([repro/adreno-refused.wgsl](repro/adreno-refused.wgsl), 174 lines,
   every loop bounded). The same module with that local a `vec2<u32>` is taken
   ([repro/adreno-taken.wgsl](repro/adreno-taken.wgsl)), and so is the module with
   the array computed in place instead of carried in the local.
2. With the arrays gone, a struct-typed local carried the same way (`type_30`) was
   refused. Split into its two leaves, it was taken.
3. With the locals split, a composite value was refused: the leaves reassembled
   (`let _e424 = type_30(array<u32, 2>(…), …)`) and read by member.
4. In the modules that keep their functions, a function returning a composite was
   refused (`-> array<u32, 2>`, or a struct holding arrays). Emptying either of two
   such functions made `beam` taken.
5. The raster's vertex stage read a `const array<vec3f, 72>` of box corners at an
   index computed as it runs, and the S23 refused that render pipeline. The same
   table packed into `array<u32, 8>` is taken.

`decor` was refused whatever was rewritten, and it was taken once any one of ten
unrelated functions was emptied, a buffer read of 8 lines among them: its cause was
no shape, but a size or a load. Split in two at the halves of
`generation::placement` (the column surfaces, then the pose about them; in a
worktree), each half was taken once inlined, rewritten or not, and each was refused
with its functions kept.

The rewrites, over every module, on the S23:

| Set | Refused |
| --- | --- |
| As generated | the nine above |
| Always inlined | beam, cell, light_propagate, sample_box |
| Inlined, split, forwarded, vectors | way |
| Inlined, split, vectors | cell, way_tree |

A rewrite that made the S23 take one module made it refuse another that the set
before had taken, so no one set of these was a fix. A large module took 2–5 s to
compile on the S23, and 12 s rewritten.

### Safari refuses `atomicCompareExchangeWeak`

Safari's WGSL compiler (WebKit) wrote Metal for every module, and Metal refused the
six that reach `atomicCompareExchangeWeak` (pxl-shader pyramid.rs `pyramid_swap`,
way.rs `pop`): beam, sample, miss, miss_list, light_miss and way_tree.

```
program_source:47:7: error: field may not be qualified with an address space
    T old_value;
program_source:54:12: note: in instantiation of template class
'__atomic_compare_exchange_result<thread unsigned int>' requested here
    return __atomic_compare_exchange_result<decltype(compare)> { innerCompare, exchanged };
```

The helper that WebKit writes takes the type of its result from
`decltype(compare)`, a by-value parameter, to which Metal's compiler gives an
address space. Nothing in the WGSL reaches the helper:
[repro/webkit-cas.wgsl](repro/webkit-cas.wgsl), one call, fails the same way in
Safari on the Mac (`probe.html?src=20261103-the-shaders-by-hand/repro/webkit-cas.wgsl`)
and is taken on the S23.

Before `4a73ebdd0`, Safari also refused a `select` of `i32(-2147483648)` and an
`int`. WebKit writes that constant as the bare literal, which is a C++ `long`.

### What made them take it

Each step landed on main. After each one the S23 was checked over USB, and Safari
and Chrome on the Mac were checked too.

| Commit | Change | The S23 refuses after it |
| --- | --- | --- |
| `10fe6a7e5` | no compare-and-swap (`pyramid_claim`, `pop`) | —; Safari takes every module |
| `64b6734ea` | `decor` in two passes (footing, pose); every entry inlined | beam, cell, light_propagate, sample_box; the raster |
| `f94b0017b` | an entry for each observer of a pass (`light_beam_cs`, …), with `PassObserver` gone; naga's `phi_*` composites split and small arrays made vectors (rewrite.rs) | the raster |
| `4a74a51f2` | the raster's corner table as bits (`raster_corner`) | none |

After `4a74a51f2` the S23 made all 35 compute pipelines and every render pipeline,
and it drew the page: the ground, the decor, the companion and the chrome. Chrome
and Safari on the Mac ran the page with no GPU error. The refused render pipeline
was found by opening the page on the S23 with a hook that keeps the WGSL of each
refused pipeline (`tools/catch_render.mjs`), then cutting the suspects in that
module one at a time (`tools/cdp.mjs` with `RENDER=1`).

### The hand WGSL on the phones

The hand passes keep clear of what the phones refused (the faults below give the
rules), and each one the S23 was given was taken at its first try. `world.wgsl`, which is
`world::node` by hand at `f9f66682e` (read-only, with no atomics and no arrays),
became a pipeline in 573 ms when called once a thread, and in 766 ms when called 14
times along a loop with about 28 words live across each turn. The first rust-gpu
shaders of that size had been refused, so the hand query stands under the refusal.
Adreno gave no error on `light_miss_cs` by hand at its first run (`355b65140`), and
all 91 pipelines were made. With `decor_cs` merged (`d163e34c5`), and again with all
seven decor passes by hand (`5d7113c02`), the S23 made 90 pipelines, none failed,
and no device was lost.

A pipeline that the S23 makes can still compute something other than what the Mac
computes, with no error. In September an earlier S23 read buffers laid out
otherwise than the host wrote them, and it drew wrong (the archived minutes of
2026-09-23 and 2026-09-27 are on `origin/original`;
[shaders the phones take](../../tip/shaders-phones-take.md) holds the rule). In this
round, placing the list of left samples before the misses moved `misses_cnt` and
every place after it, and that regenerated every light pass with that arithmetic
(about 8,900 lines of `light_beam.gen` changed). On the S23 the light's beam then
walked 0 nodes a cast, while the Mac's walked 118k, and no pipeline reported an
error. With the list appended at the end instead, the light passes differed from
HEAD by one line, and the S23's beam walked 148,815 nodes a cast. Which part of the
module the S23 computed wrong has not been reduced.

### The reports drafted

Two reports were drafted for the browsers' makers and not sent.

**Chrome (Dawn) / Qualcomm: Adreno 740 refuses a valid compute pipeline**

> **Summary.** On a Galaxy S23 (Adreno 740), Chrome 156 rejects
> `createComputePipelineAsync` for a valid WGSL compute shader with
> `OperationError: CreateComputePipelines failed with VK_ERROR_UNKNOWN`. The
> same module compiles on desktop Chrome (Metal, Vulkan) and with Tint
> standalone.
>
> **Reduced case.** `adreno-refused.wgsl` (attached, 174 lines): a function-scope
> `var` of type `array<u32, 2>`, assigned in one branch of an `if` and its
> elements compared, followed by two nested `loop`s whose exits are carried
> in `bool` locals (`break if`). Changing that one local to `vec2<u32>`
> (`adreno-taken.wgsl`) makes the pipeline succeed. Struct-typed locals and
> functions returning arrays or structs fail the same way in the full shaders.
> So does a render pipeline whose vertex stage reads a
> `const array<vec3f, 72>` at an index computed at run time (`vkCreateGraphicsPipelines`);
> the same table packed into `array<u32, 8>` succeeds.
>
> **Device.** SM-S911N, Android (Chrome reports `Android 10; K`), Chrome
> 156.0.8078.73, `adapter.info`: vendor `qualcomm`, architecture `adreno-7xx`.
>
> **Expected.** The pipeline is created, or a validation error names the
> cause. A Dawn workaround for Qualcomm (e.g. scalarizing function-scope
> composites before SPIR-V) would cover pages that cannot change the shape.

**WebKit: `atomicCompareExchangeWeak` produces Metal that does not compile**

> **Summary.** Any compute shader calling `atomicCompareExchangeWeak` fails
> to compile in Safari 27.0.1 on iOS and in Safari on macOS: "Failed to
> compile the shader source, generated metal", `error: field may not be
> qualified with an address space` in the helper
> `__wgslAtomicCompareExchangeWeak`, instantiated as
> `__atomic_compare_exchange_result<thread unsigned int>` from
> `decltype(compare)`.
>
> **Reduced case.** `webkit-cas.wgsl` (attached, 11 lines).
>
> **Expected.** The helper takes its result type from the template parameter
> `S` (or `remove_address_space_t<decltype(compare)>`), not from
> `decltype` of the parameter.

## Holding a hand pass to the engine

The user decided on 2026-11-05 that pxl-engine is the oracle and that the WGSL is
held to it word for word. Where the two disagree, the rule is settled on both sides
at once, so that each side does what the other does
([the minutes](../../minutes/20261105-the-shaders-by-hand-held-to-the-engine.md)).
A reference test runs a hand pass on the Mac's GPU over inputs laid from
`real_world` and compares every word the pass writes with what pxl-engine answers.
A test at 0 differ found no word apart.

The passes went by hand in bundles. The user had asked for speed, with the
instruction of 2026-11-03 to draft fast and have someone review; the agent grew that
into reference tests, a mutant sweep and an outside audit for every bundle, a bundle
took half a day, and the user had to dig the instruction out again. On the evening
of 2026-11-04, after the user's word "그냥 번역해" ("just translate it"), four
translators in four worktrees took the last 15 passes at once and built and checked
them on the CPU only. Each branch landed by fast-forward, and the next rebased once
onto it (miss `0b978c7e53`; light `c0e206a08d`; beam, cast and face `2cc580cd88`;
the rest `3baa9e9fd5`). It took about an hour from the start to 6 generated modules
left. The passes translated without a reference test got one each afterwards.
rust-gpu went out whole at `97b46505fb`: the shader crate, the vendored codegen, the
tool's `shader-wgsl`, `gen.digest` and the hook in check-rules. Every pointer at a
pxl-shader file now names the hand WGSL or pxl-engine, and `shader.test.ts` takes the
assembled modules alone. The render tests passed at that commit, one binary at a
time under the lock; Tint was not run.

### The tests

Every test below ends at 0 differ, apart from the GPU rounding it names (the next
part). tests/light_reads.rs, the box cases in tests/sample.rs, tests/cast.rs and
tests/light.rs were written on `small`, the worktree the round was integrated in,
and were uncommitted when they ran; none of the four changed any WGSL, and they
reached main with the merge.

| Hand passes | Test | What it compared | Mutants |
| --- | --- | --- | --- |
| `world.wgsl`, the world query | tests/world.rs | asks against `world::node`, counted below | counted below |
| `light_miss_cs` | tests/light_miss.rs | every word against `Inheritance::of`, a case over X = 0 among them | |
| `sample_cs`, `light_sample_cs` and their hit passes | tests/sample.rs | samples over the real world, a case over X = 0 among them; a listed case at `shadow::SIZE` took 18,512 samples past the first row over two frames | |
| the seven decor passes | tests/decor.rs | `decor_cs` 421,248 records against `generation::placement` and a Rust model of every pass; 177 frame chains through every pass, threaded repeats included; the tiles stood on, counted below | 2,091 over the reach of `decor_cs`: 1,032 killed, 962 survived |
| `miss_cs`, `miss_list_cs`, `miss_hit_cs` | tests/sample.rs, `hand_missed_pixels_answer_as_cast_walk` | 27,663 marked pixels from three eyes by two screens, against `cast::walk` | 11 of 11 killed |
| `fold_cs`, `height_cs`, `bound_cs`, `cell_cs`, `hit_lead_cs`, `hit_faces_cs` | tests/fold.rs, fill.rs, cell.rs and hit.rs, at `6b2ba06d84` | fold 16 tables; fill 32 top cases and 112 level cases; cell 214,999 cells; hit 25 cases with 75,564 leads | 215; survivors: fold 32, height 4, bound 10, hit_lead 1, cell and hit_faces 0 |
| `light_propagate_cs`, `light_slide_cs` | tests/light_reads.rs | propagation 16 cases over four lights, 301.8M words; slide 7 cases (across X = 0, across the i32 seam, nine tiles, past the box), 132.0M words | 21 of 22 killed |
| `sample_box_cs`, `light_sample_box_cs` | tests/sample.rs | against `cast::asset_box`, `raster::is_eyed` and `raster::id_of`: eye 13 cases, 27,281 boxes, 19,031 asks; light 7 cases, 22,100 boxes; the rest of the beams buffer held (123M and 21M words) | 29 of 29 killed |
| `face_cs`, `cast_cs` | tests/cast.rs | off the real eye passes over three worlds, one by X's seam: face 16 cases, 168.9M words; cast 6 screens (one lacking everything past the list's room, one at exactly `MARKS + 1`, one 1×1), 48 comparisons, 221.2M words | face 10 of 14, cast 17 of 19 killed |
| `light_cs`, `light_mark_cs` | tests/light.rs | light 5 cases, 26 comparisons, 2.34M words; light_mark 9 looks, 18 comparisons, 3.72M words; both also checked against the rust-gpu passes at `c0e206a08d~1` | 20 of 25 killed |
| the end of the walk in `light_beam_cs` | tests/margin.rs | every word of pxl-engine's walk at `STEPS_MOST` and at 24 (hand, visits, node, every inheritance): 1,629 and 1,120 rays cut short, 653,020 words | 7 of 7 killed |
| `forget` | tests/forget.rs | `moved_from` by its effect over the real world | |

`light_miss_cs` is one module for every case and keeps no stack: a node asked of the
world finds what is handed to it from the top (`world_root`). rust-gpu's own module,
run through the same test, keeps the same nodes with the same kind and asset, and it
differs from the Rust's chain only in the place and the cursor of the ways of a cell
placed whole, which hands nothing on. The walker moved out of `light_miss.wgsl` into
`ask.wgsl`, with far and near apart, and the whole test of `light_miss` passed at the
first run with no change of its own. The Rust side of tests/light_reads.rs lists
what feeds a claimed tile by the forward rule of `light_needs`, so the inverse
ranges of the WGSL are under test. The seven decor passes were first held at
`bfd83b127`, before any mutant sweep over them; on the Mac's page their `regions`
read word for word the same as at `355b65140` (2,576 drawn), and on the S23 a
difference map at rest showed the trees in place.

At `97b95981b8` the whole pxl-render suite passed, one test at a time under the lock
(`--test-threads=1`): 12 binaries and 45 tests, decor's
`hand_frames_answer_as_the_rust` among them. The decor binary took 90 s and every
other binary under 25 s. At the tip of the merge into main, CPU 315 tests, render
62, vitest 28 and xpute 114 passed, and the page drew with no console error.

### Where the GPU rounds otherwise

Floats are compared exactly, except where the GPU is known to round otherwise; each
test names the ulps it allows and counts what took them. Metal fuses a product into
a sum: `height` takes ±1 cell only for a top within 4 ulps of a half (214 tops
qualify, and 13 move), and `cell` and `hit_lead` allow a window or a cell over only
within 32 and 8 ulps of an edge, where none moved. Each moved top was checked against
a fused sum in Rust.

Metal's division is not correctly rounded. An entry appended to the module ran
`cast::entry` on the device's own reciprocal and found it 13 ulps off on a component
of 0.0137, so the t of `cast_cs` takes 4 ulps plus 4/|d| on the entered axis, and
every other word is exact. A band edge on a power of four parted the two sides:
`zoom_at` divided `r2` by `ZOOM_SPLIT²` (walk_node.wgsl, and pxl-engine
`grid::cell_zoom_at`, which it copies), and at r² = 9216 Metal landed on the other
side of the edge, against the Rust comment that such a distance falls on one side
everywhere.
The test first took ±3 ulps, with the change of the rule on both sides left to the
user. The zoom is now read on both sides off the distance squared scaled by a
product, `r2 * INV_SPLIT2` (build.rs hands `INV_SPLIT2` where it handed
`ZOOM_SPLIT`), since WGSL's product rounds half to even and its division need not.
rust-gpu had compiled the Rust's division into a product by the reciprocal
(`* 0.00043402778f`), so the CPU and the GPU had parted there before too, unseen.
The split squared is 9 · 2⁸, and 9 · fl(1/9) rounds to 1, so a square on a split
distance still begins its zoom (`zoom_cell_dist`). The tests of pxl-engine,
pxl-spec and pxl-kernel passed, the render suite passed except tests/light.rs, and
Tint passed.

The light's band had a second cause. With the product, the same two slots of the
claimed-tiles case still took another band, since `length` in `tile_band` is a
square root and its sum of squares may be fused. The band is the GPU's alone (every
pass goes through `tile_band`), so the test first took ±3 ulps named for those
slots, and the eye's zoom, which the kernel decides too, keeps the product. On the
user's word the light's texel width then became a word the host rounds,
`light::W_WIDTH` (the fourth word of the bias row), with `Light::width` =
`length(across)`, across and down alike since the box's texels are square.
`Light::spacing`, `band_dist2` and `ring` read it, and `light_beam`, `tile_band` in
`light_aim` and `light_mark_draws` read the word, so no square root is left in
them; `BAND_ULPS` in tests/light.rs fell from 3 to 1, for the fused sum alone. The
rule is in xpute ARCHITECTURE.md beside the golden records.

On the user's word (symmetry), divisions came out of the rules that both sides
decide, and the host hands over what a frame holds constant. `eye::LIGHT_WORDS`
grew from 20 to 28 (`W_INVERSE`, `W_T_MAX_INVERSE`, `W_ACROSS2_INVERSE`,
`W_DOWN2_INVERSE`, `W_WIDTH_INVERSE`), and `beam::GRID_WORDS` from 20 to 24
(`GRID_S_INVERSE`, `GRID_SLOPE`). `texel_at`, `ring`, `light::depth`, the light
ray's reciprocal (one function, `light_aim` in light_aim.wgsl), `Grid::cell` and
`Grid::window`, `sample_spacing` and `sees` take a product. The steps of `up` are a
power of two (`ldexp`, up to twice the steps). `texel_start` is in fixed point
(`light::fixed`, 16 bits of a Q3 step, summed in i32), after tests/margin.rs had
found it one Q3 step off `Starts::start` on 47 rays (allowed then as `START_ULPS`
in tests/light.rs): the device's starts now equal the Rust's on all 65,536, and the
engine's start moves a step on about 5% of rays, rounding floor(x+½). The
allowances came out: `INV_ULPS`, `START_ULPS` and `starts_about` of light.rs, and
"a step over" in light_miss, sample and walk; `WITHIN` went from 1024 to 2, and the
six margins of tests/margin.rs to 2. `DIVISION_ULPS` now covers the two fused sums
of `depth_words` alone (1,482 slots). The eye's per-pixel ray stays the GPU's,
through `eye_aim` alone, and cast.rs and cell.rs keep their ulps for it.

A fixed order of sums does not hold on Metal. wgpu-hal 30 compiles Metal with its
default fast math (`metal/device.rs:272`), which may reorder and simplify sums, not
only fuse them: with halves making every product exact and one order written on
both sides, 28–981 of 65,536 starts still parted. Integers and the host's
reciprocals hold. The rule in xpute ARCHITECTURE.md names fusion alone, and widening
it is the user's to decide.

A ray's own rounding decides two more things. A node that one ray grazes is entered
or not by the GPU's rounding, so traps on such a node are not compared (`SURE` of
tests/light_miss.rs). And `resumed` cannot climb on the eye's rays: with a cell's t
moved to where its ray leaves the node by the finest grid, the Rust climbs for 3,330
samples and 827 then differ, because the eye's directions are the GPU's own
rounding and a face's t one ulp off flips the climb. Climbing is a question of ulps
on any ray not along an axis. Along an axis it holds: with the light straight down,
one band, and each cell's t moved to a whole unit where its ray leaves its line's
node, the planes and the finest grid part exactly, and 552 samples climb, 314 of
them left, with 0 differ.

### What a test at 0 differ does not prove

A mutant is one small change to a hand module, an operator or a constant. A test
that still passes with it does not hold that line. The first test of `world.wgsl`
compared 1,849,742 asks with the Rust at 0 differ, and 35 of 93 hand mutants
survived it. What closed them was what real callers vary: a zoom per node rather
than one for a whole descent, a clip level not laid, a concave service area, rows
past the poles and heights past the world. Widening the test's world later stopped
it reaching one value, and an old mutant came back, so a sweep runs again after the
test world changes.

At `f9f66682e` the test compared 2,700,150 asks. Of 443 operator mutants, 388 failed
it, 7 hung it and 48 survived. A survivor is one of four kinds, and a report keeps
them apart: reachable, and missed by the test (3); not reached, at the very end of
an i16 (4); held only by what callers or geometry give (about 6); or equal outright
(the rest, read and not each proved). [supervise/sweep-f9f66682e.md](supervise/sweep-f9f66682e.md)
lists the survivors line by line and files one of them apart, a read past the table
that gives 0 on Metal and that Dawn clamps. The last sweep of the query, over the
assembled `world_asked` module, is
[supervise/sweep-final-world.md](supervise/sweep-final-world.md): 438 mutants, 381
killed, 7 hung, 50 survived.

The kinds are sorted by reading, and a reading can be wrong, so the one who writes,
the one who reviews and the one who samples the review are three parties. An outside
sample of 14 mutants drawn at random found 13 failing and one surviving, at
`world.wgsl` line 457, which the review had filed as equal outright: flipping one
side of it changes an answer at the end of an i16, which the test world does not
have. For the last six passes, an outside sample of 16 of the 215 mutants agreed
with the supervisor on all 16 (14 killed; `height` 119 held by the exact dispatch,
and `fold` 48 equal outright at `ACROSS` 8). A verdict comes only from runs one at a
time under the lock: a rerun of the first pass's 68 survivors at three and four jobs
beside other sessions' GPU work reported 56 killed, an unread variable among them
(`enter` in `cave_holds`), so the load killed them, not the mutation, and a kill
from a loaded run can hide a survivor.

A survivor leads to a hole in the test. In the last six passes, a case each closed
three reachable survivors: the count guard of `hit_faces` (an earlier cast's hits
past the count would write onto real pixels), the workgroup opener of `hit_lead`
(asks one past a multiple of the lanes dispatch one group short), and a write by
`fold` past its table (Metal drops it, and the test now keeps a spare row). For the
decor passes, the Rust's answers were recorded once and each mutant ran on the GPU
against them: the 2,091 mutants of the table ran in about 25 minutes with 5
processes, each run bounded, and hangs counted as a kind of their own. Of the 886
survivors in `world.wgsl`, the world test `world.rs` kills 756. The 16 reachable survivors in the new modules are all gaps of
the test world, and reading each survivor against the test world's data tells why:
the end of the vote survives because `real_world` lays transports 37..54 of 37..55
(`TRANSPORTS[0] + h % 18`), which no count says. A shared part's survivors are rerun
under every test that assembles it: `decor_box` left 15 survivors under the test of
mark that the test of pose may kill. A first sweep (43: 113 of 458 survive; 79: 67 of
113; 81: 41 of 62) showed three holes, which the test then closed: up to 512 left
samples could be a visit or two off, so a start a level too coarse passed (now 16);
nodes kept by one side alone were not bounded; and nothing held what the hit pass
leaves, so a hit pass that left everything passed. Each left sample must now ask the
world in the walk of `Writing` over the same table, and in all 36 frames none fails
to.

Reading each case against the engine before writing catches what the test world
would pass. Traced against `world::node`, a way over a void column hits
(`world.rs:256`, where `!fills(val)` takes the way), so the decor plan's order, idx
first, would answer a miss, and with no void column in the test world it would have
passed at 0 differ. Patching void columns into one test did not cover it: the void
path of `world.wgsl` goes unmet in `world.rs`, and a mutant that fills a void column
with soil survives both, so a case that one consumer needs belongs in the shared
test world. A reduction is judged with the merge beside it: the plan's "`r+1..r+4`
already hold" is true of two passes and false of one, since in the merged pass
nothing writes `r+3` and `r+4` and the seed that thin leaves there would read as z.
The merged `decor_cs` writes `r`, `r+3` and `r+4` ([decor/rulings.md](decor/rulings.md)).
Still open for the world bundle: `world:1127` (`!fills(val)` in the placed branch, a
placed fold sharing a cell with more ground than it fills) and `world:487` (the edge
of `held_of`) survive `world.rs`, and the tie of `halve` (`760`) has been open since
the first bundle.

A comparison passes over zeros, so a test counts what it compared. The frames of
tests/decor.rs laid only `HEAD_CAST` and `HEAD_SIZE` of the light's head, so
`light_mark_draws_cs` read zero tiles and stood the decor on nothing. The comparison
of what stood held zeros on both sides and passed, until an assert on its count
failed: `hand_frames_answer_as_the_rust` failed on "tiles decor stands on > 0" at
`6b2ba06d84` and at `97b46505fb` alike (main at `3baa9e9fd5` was not run), and the
read-only review of 2026-11-04 night (under "The shape of a pass") named the cause.
With `light::head_layout` laid, 6,969 tiles stand, 0 apart over 24 chains
(`97b95981b8`). A light case at the shipped map's
side laid only the tiles' reads. A beam ray's corner whose feeds span more than 16
tiles reads the squares (`light::coarse_at`), which were left 0, so its line went
unwritten while its cell resumed off it; the Rust read the zeros as what was handed,
the hand pass asked from the root, and 3,101 slots came apart. The test lays the
squares now, and on the 256 map that moved the left of "the box wrapping" past
their reach from 2 to 0. The eye words of tests/margin.rs had zeroed cuts (a cut at
zoom 0, with everything carved) and hold `Cut::NONE` now.

A translation with no reference test hides its faults. The first test written for
one, of the miss passes, found `select(FACE_BOX, keep_hit(...), fill == 0)` running
`keep_hit` on every hit, since `select` evaluates both sides: a cell short of its
cube kept a hit, 155 of 18,677, whose faces `hit_faces_cs` would write over its
`FACE_BOX`. The call runs under an `if` now. Until that test, a rebase had moved the pixel ray of `miss_word.wgsl` onto
`grid.wgsl` and `hit.wgsl`, bit for bit by reading and under no test. Eight
translated passes still had no test after that (cast, face, light, light_mark,
light_propagate, light_slide, sample_box, light_sample_box); four workers gave each
of them one, every run under the lock and a 120 s kill.

The later tests closed their survivors the same way. In `sample_box` the first
version let five hand mutants through (`1.0 +` in `is_eyed`, `hi` for `lo`, `alike`
from word 1, and `sample_needed` dropped across and up); cases two steps over and
under a cell, a node one zoom coarser and a listed cast's earlier words closed them.
Its cases are still thin where "down a bore" and the eyed cases draw one box each,
and the sign of `ASK_FACE` for a direction exactly 0 on the entered axis is
untested. In `face`, five of 14 mutants were left at first, which were holes of the
three worlds and not equivalents (X roots −1 and 0 inherit alike; the seam's asks
answer alike unwrapped; nothing is carved or hidden where asked), and they were sent
back to close. A carved hole with a hidden leaf beside it kills CARVE and HIDE
swapped (890 asks answer otherwise). The face test grew from 14 cases and 146.3M
words, and the cast test from 44 comparisons and 202.4M words, to the counts in the
table. The four face mutants still left change nothing the engine's rules
allow: X roots −1 and 0 read one overwrite node (`overwrite::path(0, c)` masks X),
so "held" and `meet.x <= low` never part; the world masks X in its path, so
`wrap_idx` on the beside cell never changes an answer; and `face_cs` asks a cell at
its own zoom, so `far` is never read. In `cast` two of 19 are left: `>` to `>=` in
`enters`, and the `t_max` of `span_of` with no zero component. In
`light_propagate` and `light_slide`, one survivor at first (rows counted from `e`)
was closed by a case whose claim opens a new row, and one is left: the read disk's
`<=` against `<`, equal only where r² is hit exactly. In `light` and `light_mark`,
three survivors at first were closed by cases, and an exact touch and three
equivalents are left.

An equivalent mutant marks code that holds nothing, and on the user's word
("symmetric, and what is unneeded out") such code came out of both sides rather than
being tested around. The clamp of `sample_before` in cast.wgsl, `n - 2` against
`n - 1`, was equal: a drawn pixel's tangent lies within the screen give or take an
ulp, parting the two needs a point a sample spacing (1/384) past its edge, and the
largest index the engine reaches is `n - 3`. The clamp was the engine's
(`Grid::sample_before`), and its `min(n - 2)` came out of pxl-engine and cast.wgsl
alike. In decor_pose.wgsl, `lookup_here(found) && lookup_from(found) > cell` was
equal: without `here`, `from` is `FIRST_CELL`, never above `cell`, and `>=` picks
`cell` either way. The guard as a whole holds nothing the listing allows, since a
listed pair's row is held that frame, which the Rust of tests/decor.rs asserts; two
counts were added (a finer level holds the column: 97,202; its own: 395,769), and the
control `zoom = cell` is killed with 5,742 words apart. The guard came out with the
Rust of tests/decor.rs, and the assert stays. In sample_box.wgsl, `along > 0.0`
against `>=` was equal, since `SAMPLE_AXIS` is set by `crossing`, which picks only
an axis the ray moves along, and `Ray::new` zeroes a component under 2^-126. The one
path read as leaving the axis unset, the step cap of walk.wgsl writing a box with
`ends` 0 and axis 0, was a wrong reading: `ends` 0 went to the left list, never to a
box. walk.wgsl now hands a walk out of steps on as left (`ends` 4), as it hands on
one that meets a node no slot keeps, and its four callers test `== 4` alone. Whether
the four face survivors are WGSL that holds nothing, and so comes out, is open.

The tests plant false answers in the pyramid's table as traps for the walk that
writes. A trap has to sit where the order of the walks cannot decide the word: false
answers in slots that a walk may write over made a flake (6 slots, one run in three),
and they now sit only where a node is alone in its bucket, with the truth in the
slot its hash names. Two failures of a test whose false answers lay in a table the
tolerance read were the test's own, and a fix was taken for the cure until the
supervisor's 24 runs showed it was not. The traps (own pass, torn words, kept
false) belong to the walk that writes alone: `sample_hit_cs` reads no `Writing`
stamp, so it took them, and the hit pass runs over a table without them.

The test world has to be laid where the cases are. X's roots part at X = 0, not at
the i32 seam, and `real_world::build` panicked within 2000 of X = 0 ("a brick whose
tree lies about sea level") on every row tried, which was first read as a reason to
leave root crossings out. The cause was that no shard about X = 0 was served, so
there was no ground and no way tree; shards −1 to 1 of rows 40–43 are served now,
and the cases over X = 0 end rays in another root (4,401 in tests/light_miss.rs and
571 of the light's in tests/sample.rs). The cases over the i32 seam stay, for the
world's X wrapping. Over X = 0, 1,724 samples resume off a line from the other root,
every one at t = 0: the least t of the window is a beam ray stopped at X = 0 within
its margin, and a ray at its origin lies in its own root, so the walk starts from
the root either way. A box a quarter unit a side slanting east across X = 0 holds 24
off the root over, which shows where `moved_roots` acts. A test eye east of the seam
kept nothing at first, since `pyramid::keeps` kept no node of negative X and every
visit asked; the count of asks by reason told it, and no word did. An odd light
corner in a listed case, tried to reach the mask of `sample_taken`, took samples past
the lattice (64 differ); the box's corner is `TILE`-aligned by contract
(`light::HEAD_CORNER_WAS`), so that fault was the case's. The listed case at
`shadow::SIZE` lays the list as three rows of `LIST_ACROSS` workgroups; the dispatch
of light_miss at 256 is already 16 rows of `MISS_ACROSS`, and its next frame's 3.

## Faults found, and the rule each holds

Each rule here comes from a fault met in this round, and the later passes are held
to it. render build.rs now refuses in every assembled module what of these naga can
see, as [shaders the phones take](../../tip/shaders-phones-take.md) describes.

### The shape of a pass

No pass holds a `var` of a struct type, a struct nested in a struct, or an array
carried through branches: these are the S23's refusals. A read-only review on the
night of 2026-11-04, which read `e85332699f..97b46505fb` against the record of what
had failed and the faults listed then, found the rule broken in merged passes: an
array carried through the loop of `forget`, and a nested struct in `miss_word`. At
`97b95981b8` the moved mask of `forget` is read a word at a time inside the loop,
and the missed pixel's nested `Px` is two functions. Nested structs in render-pass
shaders (`Air` holding `Light`; `Surface` holding `Air`, `Pixel` holding `Hit`) lay
outside the compute passes the rule was found in and were left unsettled here;
[shaders the phones take](../../tip/shaders-phones-take.md) keeps them out of
render modules too.

Live words are counted along the whole call path, the entry's included, and the
synthetic shows that the cost lies in the words live across a loop; the reading that
code which never runs was the largest cost was wrong. By source count, the worst
path of `world_node` holds 76–94 words, and `walk` holds 61 at its call of
`slot_of`. Asking first and working out the node's spans afterwards took
`light_miss_cs` from 132 live words across a loop to 113, by `livepath.py`, and the
shared walker in `ask.wgsl` raised it to 124. The decor passes peak at 112
(`decor_cs`) with the rest under 30, and the last six passes peak at 37
(`bound_cs`).

A constant goes only into the passes that read it, checked in the assembled output.
A layout is read from a `pub` item, never inferred by arithmetic. naga, Tint and the
S23 are three gates, and passing one says nothing of the next; Tint reads every
assembled module now (`ts/tool/shader/shader.test.ts`). One rule lives in one place:
the review found one rule written in two places, the tile side of `light_cs`
hard-coded with its assert gone, and about 15 comments citing deleted Rust, all
fixed at `97b95981b8`. One take/needs/sample word sits in light_list, and the sample
passes call it; the side of `light_cs` comes from `light::TILE`, held equal to
`SAMPLE_SIDE` in build.rs. Each function has one definition, held by a script over
every shader's parts with naga beside it. A call with a side effect does not go
inside a `select`, which evaluates both sides (the `keep_hit` of the miss passes,
above). While rust-gpu still compiled pxl-engine, `self.brick != was.brick` failed
for want of `raw_eq` on an array, and the comparison was written word by word.

### Bindings and layouts

A merged pass's bindings are counted against floor.rs before it is written. The
decor supervisor, reading only, found that `decor_cs` on `world.wgsl` as it stood
would bind 9 storage buffers against a limit of 8, and main's `shader.test.ts` makes
each pipeline on a device of default limits. The generated `decor_cs` held 8 through
`NoPalette`, and the hand one does the same: the one read of the palette in
`ground_cell` sits behind a function in a small part, with a binding form for
light_miss and `world_asked` and a form returning 0 for decor (ruling R8 in
[decor/rulings.md](decor/rulings.md)). The no-palette form did not suit
`light_miss_cs`: `Map::assets()` went true with `e8d616ba4`, so a cell the light
keeps is read by the eye and needs its palette words. The form was undone before it
ran on a phone, and light_miss keeps binding 4 and 8 storage buffers.

An explicit bind group lists exactly what the shader reads. wgpu refused the group
of `decor_cnt`, 6 entries against 5, because an automatic layout drops an unread
binding.

A render pipeline's groups are checked against what the entry points it runs read
(its vertex entry, and `fs` where it has one) as the crate is built (record.rs
`lays_out`). build.rs writes a fifth word to `STAGES`, the vertex entries reading
each binding (`part::VERTEX_ENTRY`, moved from record.rs). Three mutants (a read
entry dropped, its kind changed, a depth frame's kind changed) stop the build. The
check found entries that no pipeline reads: `LIT_FRAME`'s 10 (in eight
pipelines), the sky's 12, `ao_downsample`'s 6 and `atmos_sky_view`'s 0, where the
comments named the first as laid out on purpose. Held both ways, as
compute groups are, each entry is now read by a pipeline sharing its group, and the
rest came out: the air's sky view, moon view, and the Earth's picture and tiles from
every painter's group and the sheets' groups (record.rs `SKY_AIR`, the sky's alone;
gpu/atmosphere.rs `sky_entries`); the table of multiple scattering from them too
(`MULTISCATTER`, the tables'); the frame from the sky views' tables, whose comment
said they were measured from it; and the AO's uniform from its downsample. An entry
put back stops the build.

The air's split at first broke the page. The froxel's group `of(&[8, 9, 11, 12], …)`
filtered the painter's air, which no longer held 12, so it made five entries against
six, and the group was invalid every frame, as Chrome's console showed; no test made
the renderer's bind groups. `of` now takes each binding the air holds or stops
(`or_bug`), the froxel names the entry of the multiple scattering, and the page
draws with no console error. On the user's word (symmetry), a bind group is made off
its layout's own group: pxl-render gpu/api.rs holds `Layout { id, group }`,
`Gpu::layout` is the one way to make a layout, and
`Gpu::bind_group(Layout, entries)` stops at a wrong count, a binding the group lacks, a binding given twice
or a resource of another kind, naming the call site. All 72 calls take a `Layout`.
The owners pick the air off the layout (`Layout::binds`), and the hand lists of
atmosphere.rs and `MEDIUM` are gone. gpu/renderer.test.rs makes the 23 groups of
`Renderer::new`, and a dropped froxel entry or an extra star entry each stop it. The
groups made at a resize or in a frame are checked as they are made, and no test
makes them.

### The pyramid and the walks

A brick that moves forgets what was read from it. Reading `clipmap::fold` showed
that the brick of an absent entry is read, and a slide case found 8 of 7,776 nodes
stale (`d11dc095c0`); an audit caught it, not the bundle's tests. tests/forget.rs
now holds `moved_from` by its effect: over the real world, a brick turning present
or a slot changing forgets every node that a ray down over it visits, a ticket alone
forgets none, and every answer stands. The old rule fails the test at the ticket,
and a rule that moves nothing fails it at the first case.

A slot read while another thread overwrites it can hand the reader a torn word. Told
apart from a flake on Metal, a wrong word came in 4 of 88 runs with traps off, and
in 0 of 168 with no write-over of an occupied slot; `Writing` has the same recheck,
which relaxed atomics do not hold, and the phone's rate has not been read. A stamp in
the kind word now marks a slot read while another writes: 201 full runs had no
failure, against 3 of 48 before; with verification off every run fails; and
rust-gpu's own module gave 0 in 60 word lines against 1 in 20.

A dispatch can stop mid-claim with no error. One light_miss run panicked in
`ground.rs:318`, an overflow in the Rust reference, and two reruns passed; it was
read at first as the reference reading a torn entry the GPU wrote. The panic is
`truth` asked at `key_zooms(CLAIMED)`, [63, 63], for a slot left claimed: the check
then read every changed slot, a claimed one too, so the panic was a dispatch that
stopped mid-claim. Both tests now fail a case whose slot is left `CLAIMED` after a
pass, as a dispatch that did not run to its end, and they do not read its words;
the panic did not come back in 5 serial runs and 12 runs four at a time. wgpu-hal's
Metal fence (`Fence::get_latest`) takes a command buffer at
`MTLCommandBufferStatus::Error` as completed and reports nothing, so a test dispatch
now clears a word in the same command buffer after the pass and fails where it
stands; 32 runs four at a time had none aborted or lost. Four test runs at once with
a 122k-sample eye had left some left samples unwritten in 2 of 24 runs, with no
error reported; that was read as Metal's watchdog aborting a long dispatch under
contention, and the case was cut to 47k samples. Which load aborts a buffer has not
been reproduced, and the cause of the panic has not been named.

On the user's word of 2026-11-05, the list's mark is swapped, not raised. `list_swap`
in light_stand.wgsl is an `atomicExchange`, and a place is listed where what the mark
held is not the cast's number. A mark above that number, after the number wrapped,
had been raised to nothing by `atomicMax`, which relisted the place at every claim,
as rust-gpu's `crate::raise` did; light_reads differed by 252,989 words a case at
`97b95981b8`. The Rust of tests/light_reads.rs swaps the same way.

A walk ends at `STEPS_MOST` on both sides. The end of beam_walk.wgsl was first held
under tests/margin.rs with the test's own bound, `STEPS_FEW` = 24, set in the
assembled module: a ray cut short handed `t_max` bit for bit, the rest matched their
whole walks (1,629 and 1,120 rays cut short), and the mutants `hit_t = t` and
`hit_t = 0.0` were killed. Real worlds take at most 71 visits against 4,096. The line
and the inheritance that a cut walk leaves were open then, since no engine walk gave
them. On the user's word (symmetry), pxl-engine's walk now ends at `STEPS_MOST` as
its WGSL twins do (cast.rs `walk_at_most(…, steps)`, and `walk` with `STEPS_MOST`):
`End::Miss`, `t_max`, and its line kept. The iterations of `beam_walk` and
`walk_asking` are the engine loop's one for one, and tests/margin.rs holds
`light_beam_cs` to it word for word.

### Beside the passes

`CANVAS_ALPHA_MODE` came out of CONFIGURE whole, in the `opaque` worktree, since
nothing but `Gpu::configure` asked for it. World shots at the same stub read alike
against main on the Mac, and the empty preview box reads the same pixel,
(202,198,192), on both. The clear itself was not seen: the preview does not draw on
main or in that worktree (its canvas stays 300×150 and is never configured), so the
box's color is the wrapper's, and the clear rests on a test of the bytes' round trip
and a reading of the path.

## What the hand shaders cost and saved on the S23

A query by hand costs what the generated passes cost, so asking the world on every
visit is no cheaper by hand. A pass therefore answers from the observed pyramid and
asks the world only for what is left, and what is left is often a choice made in
the pyramid, not the size of the shader. The size of the generated shader counts as
well: on the same work and tables, a hand pass took about 3.5 times less time than
the generated one. A reading from a made-up table, that the shape costs and the
compiler does not, was wrong for that reason; both count.

### Asking the world

Along a loop of 14 on the S23, with `world.wgsl` at `f9f66682e`:

| Body | Time a visit |
| --- | --- |
| The query, tables all zero | 108.6 ns |
| The query, tables pseudo-random | 104.9 ns |
| A trivial body | 0.09 ns |
| `sample_hit` by hand, over the pyramid (an earlier run) | 2.4 ns |
| `sample` and `beam`, generated (an earlier run) | 111–138 ns |

The tables are made up, and zero and random agree, so this does not show that the
worst path was taken, and it does not split the query's own work from a cost of
registers: it is a shape. The trivial body is a floor and not a walk's own cost, as
the driver can reduce it to almost nothing.

The hand walks over the pyramid (`sample_hit`, `miss_hit`, `beam_hit`) took the S23
from 6–7 fps to about 24–28 with shadows off.

### The light keeps the cells it stops on

`light_miss_hit_cs` by hand (`6e9a7cb83`) answers a missed slot from the pyramid and
leaves what it cannot answer to `light_miss_cs`. By itself it was cheap and took
nothing off the frame: `light_walk` did not move, because every ray that stopped on
a filled cell was still left. The light's walks wrote no filled cell
(`Observer::assets` false), which kept the pyramid's slots for the eye; a ray that
stopped on a filled cell found no slot there, the hand pass left it, and the
generated pass walked it whole from the root and did not keep the cell for the next
cast. A count first said that 5% of slots held all the cost, and reading `Writing`
then found the cause. `e8d616ba4` is that one word, and the hand pass paid only with
it:

| Shadows on, 25 s circle | `f9f66682e` | `e8d616ba4` |
| --- | --- | --- |
| `light_hit` p50 / p95 | 7.5 / 15.3 ms | 6.1 / 14.9 ms |
| `light_walk` p50 / p95 | 403 / 875 ms | 42.1 / 131.5 ms |
| `miss` nodes a cast | 4,291,609 | 140,074 |
| Slots walked | 5.13%, 20.0 nodes each | 3.42%, 1.0 nodes each |
| Total GPU p50 / p95 | 345 / 1,155 ms | 171 / 450 ms |
| fps | 2.3 | 5.0 |

The `miss` and slot rows come from 3 casts and 8, and the spans from 30 frames and
from 67–71.

The eye does not pay for the light's cells in this view. In one sitting with shadows
on, alternating `f9f66682e` and `e8d616ba4` twice, the four loads read 2.6, 7.0, 2.2
and 6.6 fps and GPU p50 239, 124, 331 and 132 ms, and `light_walk` p50 fell from
283–536 to 20–25 ms. The eye's `walked` read 0.64%, 0.49%, 0.60% and 0.42%, so the
light's cells serve the eye and do not push it out. The picture at rest differs
between the commits by a mean of 0.56, and each commit differs from itself by 0.50
and 0.59, which is noise. Casts run at each commit's own pace, so the nodes a cast
partly follow from casting more often. A first answer that the kept cells cost the
eye nothing came from a run with shadows off, where the light does not walk and the
run could not have shown it.

### The generated shader's own cost

`light_miss_cs` by hand (`355b65140`) against the generated one, alternating in one
sitting with shadows on, over about the same nodes: `light_walk` p50 fell from about
26 to 7 ms and p95 from about 118 to 31, roughly 285 ns a node against 75, and the
frame rose from 7.2 fps to 8.0–8.8. The module went from 397 KB and a span of 12.5 s
in the cold first load to 50 KB and 6.9 s. About 70% of that pass's time was the
generated shader itself, on the same work.

The merged `decor_cs` by hand (`d163e34c5`) is 47 KB, with a span of 9.6 s in the
cold first load, against 349 KB and 13.5 s generated. The wall of the first load fell from 13.5
to 11.4 s, one cold load each, and it ends at `miss_cs` now; `generate` p95 fell from
17.4 to 2.8 ms on the circle and from 14.2 to 1.6 on the drive. With all seven decor
passes by hand (`5d7113c02`), the decor modules are 2–47 KB, where two generated ones
were 349 KB each; the wall fell from 13.1 to 10.4 s, one cold load each, and it ends
at `face_cs`, then `light_sample_cs` and `sample_cs`; `generate` p95 on the circle
fell from 14.0 to 0.9 ms. Each figure comes from one cold load, the two of a pair
in that order, as `pixelet-ec` recorded them.

`sample_cs` and `light_sample_cs` by hand cut their spans: 9.4 fps against the
base's 9.6, `sample` p95 13.7 ms against 50.3, and `light_sample` 5.8 against 64.6,
one load each. The 14–16 fps that the same tree had first read on the S23 was not a
gain: the light's beam was not running (the left list above). The light's hit pass
has no span of its own, since the spans are 32, a bit each of a mask the host holds,
and `TimestampWrites::pass` keeps a span's begin across passes, so both light passes
sit in `light_sample`.

The first load had been blamed on rust-gpu at 90 s; that figure came from the data's
format and the agent's harness. Measured, it takes 19–29 s, 15 s of it making
pipelines, and a warm load is no faster.

After `e8d616ba4`, `sample`, `light_sample`, `beam`, `light_beam` and `miss` each
hold 50–90 ms at p95: there is no one wall.

### What the pyramid leaves to the walk that asks

A brick that moves now leaves a split passable (`forget`): whole-grid frames fell
from 30 of 328 to 1 of 153, and the worst frames grew shorter, while p95 moved on one
load of two. A claim that `forget` was not the cause had come from a read taken with
a 25 MB copy, which takes the frame after a stall and misses whole-grid frames;
`MODE=scan` showed that 29 of 30 whole frames came with the root zeroed.

Air that the pyramid holds at other zooms is now gone past, with one ray in 16
asking again. Asked again, 99.5% of nodes answered the same, and no hole showed by
eye at a stop, after 1 s or after 6 s. The gain was not measured apart from the rest,
and it may have cost the eye something.

Why samples are left was measured before any port. In a scratch tree instrumented
so that `sample_hit` counts by reason at the first stop, on the Mac's page over a
15 s drive at `355b65140`, 25.9% of samples were left; for 95.4% of those no slot
held the node, and two thirds of these were forgotten (key 0, with the node's words
still there), not new. `c.moved` set a bit for any row that differs, a clip level's
new brick not yet present and a fill's ticket among them, and no query reads either.
With bits for a present brick or slot alone, the left samples fell from 25.9% to
18.8%, the forgotten from 4.07M to 1.33M rays in the window, and the light's asks
from 110k to 28k a cast, one load each.

`pyramid::keeps` takes X's −1 now. West of X = 0 no node had been kept: on the Mac
over open sea, the light asked for 796k of 814k visits a cast before and 475k of
715k after, and the eye's `sample` asked 808k before and 206k after.

Most asks at a first filling of the test's world are of nodes the same pass kept,
since `Writing`'s rule asks them again as their words may be half written: 1.86M of
1.95M about the eye, and 105k of 113k the next frame, with slots at other zooms at
3–5%. Reading the pass's own slots under their stamp, as `Writing` reads any
other's, removed nearly all of these asks in the light test and in `sample`'s first
frame, and `light_miss`'s test passed unchanged; but that was done in `ask.wgsl`
and `Keeping` alone while `Writing` on rust-gpu still asked them, so the mirror
differed, and the old rule is back. Seeding a dispatch did not end the own-pass
asks: one ray in 16 first takes the first frame from 1.94M to 724k asks and the next
from 169k to 110k, four levels give 498k and 114k, and the own-pass asks stay the
largest column, since the new nodes of the left slots are shared by few rays each.
On the Mac's page the asks lane showed 28–30% of `light_miss` visits asking the
world, with `light_walk` p95 at 2.1–3.2 ms, both pipelines made, no console or GPU
error, and shadows darkening 5.3% of pixels at noon. With that first dispatch and
without it, asks were 27–31% of visits over a 6 s circle, one load each, and a
cast's visits moved threefold between loads, more than the dispatch moved them. The
dispatch came out again: it showed no gain on the page, and it was another entry and
a tag standing in for another pass. No count of asks exists on a phone, since the
lane of `ray_counts` holds visits alone.

One node can leave a whole grid. In three cases of the tests (an eye down a bore,
where no clip level holds the ground, and the placed case's next frame) the hit pass
leaves every sample, and `Writing` asks 1 to 4 nodes for all of them: one shared
node that no slot holds leaves the whole grid. The hand pass asks 12,288 times for
them, the first wave of 192 workgroups, before any claim lands.

Three changes to the walks did not help and were reverted: a slot's far relaxed, so
that a coarser far is taken as the same near; a node queue, with the nodes of beam
rays asked once and then walked again, which did no good or worse against an
estimate of 20–40%; and a short stack in `sample`, which gained nothing once `sample`
left only what the hand pass left.

### The app against the page, and the drive

The browser is not what makes the phone slow. The Android app at `e8d616ba4` (wgpu on
Vulkan, Release, the same place and motion) ran at 14.8 fps with shadows off, 4.3
with them on, and 1.7 casts a second on the drive, against the page's 16.4, 5.0 and
2.5. The app draws 2.5 Mpx against 2.1 (19% more) and was paced at about 25 ms, so
this does not place naga against Tint. The run also found that the app did not
start on the S23; [the walk on a phone](../20261103-the-frame-on-the-s23/README.md),
"The app against the page", has the cause and the fix.

The drive with shadows on casts on few frames now, 61 of 707 in 24.8 s: 2.5 casts a
second against the 2.7 fps of a drive that cast throughout, so there is no gain
there yet.

### Reading the S23

A sitting drifts. With shadows never on, where the light's walks do not run, four
loads in order read `06a46035c` 24.1 fps, `6e9a7cb83` 22.1, `e8d616ba4` 19.8, and
`06a46035c` again 19.4. Every pass rose together, those the commits did not touch
too, and the thermal status stayed 0 throughout: a phone at the end of a sitting
reads about 20% below itself at the start. So `Aim` and the kept cells are not shown
to cost anything, and a few percent would not show. A figure from a warm phone is
not set against one from a cold phone: 16.4 fps against 28.1, one load each, had
been read as a regression and was mostly this drift. The same sitting was first told
as the answer on what `e8d616ba4` costs the eye, with shadows never on, where it
could not have come out against the conclusion; the alternating sitting with
shadows on (above) is the answer. A measure is first asked whether it could have
come out against the conclusion.

One drive is not read against one drive. At `d163e34c5` a drive read 21.8 to 8.0
fps, with `light_sample` p95 50 to 424 ms, in passes whose modules were
byte-identical (no `*.gen.wgsl` in the diff); each drive had ended at another
azimuth, and the second ran on a warmer phone. Mirrored, each commit read both low
and high: 3.9 and 18.9 fps at `d163e34c5`, 13.6 and 4.3 at `355b65140`. The drive's
fps follows the count of `gated` frames, and its path differs from one input (the
end azimuth went from −38 to −124), so it measures neither commit. Casts a second,
2.9 to 3.9, are steadier.

A read has to see every frame. `MODE=scan` on the S23 found whole-grid frames that a
read every second frame had missed, and a read taken with a 25 MB copy misses them
too (`forget`, above).

The harness can be what is measured. Numbers taken with 32 of the agent's tabs left
open on the phone read low, and the user saw a blank page; the harness now keeps one
tab. Walk visits were measured for hours on format 8 against data 9, and nobody
checked which division localhost served until the user asked. The first runs of the
app got no numbers, three times: a setting was sent before the kernel took it
(`EAGAIN`), the dev rows were left out of the Release bundle, and a position was fed
where the kernel had not asked. Logcat showed each.

A plan starts from a current figure. `beam`'s floor of 4.36 ms, taken as a target,
came from before the hand passes and was 2.2–2.5 ms when looked at. Closing the loop
on that floor missed that the tail and `light_walk` were the inefficiency, until the
user asked what the goal was.

## Whether the trees cast shadows

They do, and the claim that they did not is closed as no defect. The claim, filed
as P1 "trees cast no shadow", rested on a reading of two screenshots with no
control, among them an S23 shot at `6e9a7cb83`. The first implementer briefed to fix
it spent its run at noon, not at the reported dawn and S23 cases; its report and the
source of the two shots, from the sessions that took them, show it. A claim's
evidence is checked before a fix is briefed.

On the Mac at noon at `e8d616ba4`, a no-decor control build was set against shadows
on and off: the trees' blobs are in the difference with the decor in the map and
gone without it, so the map's decor draws cast. An on/off shot alone could not have
said so. On the S23 at noon at `e8d616ba4`, shadows off, on and off with no camera
move (`pixelet-ec`), the crown's dark blob reads on the ground in the difference:
5.55% of pixels differ by more than 8 between on and off, against 0.14% between off
and off. The same
crown had read as no shadow by eye at `6e9a7cb83`; the shadow is soft and the eye
misses it. The phone had no no-decor control, and the blob's shape and place say it
is the tree's. On the Mac at dawn, a visibility paint shows streaks running west from
the trees on a sunlit slope; on that patch, on against off reads 0.785 against a
floor of 0.763, and the no-decor build 0.981. At Yeouido and Seoraksan the near
ground is already in terrain shadow (visibility 0 with or without decor), so a tree
has nothing to darken there. The decor session's visibility shots are the evidence.

Shadows are not judged by eye: two sessions and an implementer misread them, in both
directions, until per-pixel ratios corrected them. Two tools settled what shots had
not. Patching the WGSL and the frame's words at run time through wrapped WebGPU
calls gave several paints on one camera with no source edit, and the visibility
paint at dawn settled the question in one shot. Unprojecting the eye's depth settled
what casts and what cuts independently of the light: the occluders' heights match
the eye's terrain tops, and the wall that cuts the character lies at x = 0 and moves
off the column with a 10.8° turn. A depth readback with a `bytesPerRow` that is not a
multiple of 256 returned zeros silently, until an error scope showed it. Three
questions came out of this and are in `doc/backlog.md` as P2: the character half
inside a step at load, dawn's strength on flat ground (a decision about the look),
and the map's depth-0 texels.

## Running the work

### The GPU on the Mac

A GPU test can hang the machine. At 18:55, 13 GPU test processes at once
(light_miss ×5, way ×4, fill ×4) hung WindowServer. At 19:45–19:48 one mutant alone
under the shared lock (decor's sweep binary) held the GPU, WindowServer missed its
watchdog, and the kernel panicked at 19:48, taking Docker down. A lock does not stop
a mutant whose loop never ends, so no GPU mutant runs until every loop in the mutated
WGSL breaks past a cap and each run has a wall-clock kill. The serial sweep of the
last six passes ran so, with every loop capped at 65,536 turns a dispatch, a 60 s
kill and one mutant a hold of the lock: of 215 mutants none reached the kill, and
the caps broke 4 (the supervisor's runner, `sweep.py`, and its records). Three GPU
sweeps at once (m28 with two workers, and the implementer's `decor_cs` and frames
phases) beside another session's test drove the load average to 141, and the machine
stalled until the Claude process went down, taking every background run with it.
Since then one GPU job runs at a time, with one worker and no build beside a sweep,
and every run that opens a GPU device, across sessions, holds
`lockf -k /Users/sanghoon/git/canplane/.gpu.lock`, built first, one mutant a hold.
The kill goes on the test binary, inside the lock: a brief's
`perl -e 'alarm 120; exec @ARGV' lockf … cargo test` killed `lockf` alone, and a
mutant's test binary ran six minutes past it, outside the lock, beside a second sweep
the same worker had started. The form that holds is
`lockf -k <lock> perl -e 'alarm 120; exec @ARGV' <binary>`.

A GPU test runs under a time limit and with a poll that gives up, since a hand
shader's loop can hang the wait. `hand_decor` under `device.poll(wait_indefinitely)`
with no bound on the run sat 3 h 42 min in `wgpu_hal::metal::Device::wait` at 0% CPU
while the implementer waited on it; the subagent's tally showed 4.2 h and 38 tool
calls. The generated modules under a shared GPU (load average 25) once hit a 60 s
timeout and twice lost a dispatch, and the hand side never did; that was test-only,
and it went with the deletion. The decor binary runs past the 60 s kill as a whole,
so it is run test by test.

A sweep with 3 jobs on the full test ran about 2.4 mutants a minute, and stopping a
mutant's run at its first failed case raised that to about 9. The sweep's `par.py`
read every `MUT` line as tab-separated, and a survivor line with no tab
(`MUT SURVIVED ID240 | `) killed one of its two workers, so the run went on at half
speed unseen; lines without a tab are skipped now, and the `ID…` lines are dropped
from the mutant lists. `pgrep -f` on a path inside a loop in the same shell command
matched its own command line and never ended. One face mutant (F17) ran to the 120 s
kill after it had printed its difference, where it had been killed outright in two
runs before.

### Agents, worktrees and the history

Four agents working at once is the cap: five or more ran at once while the user
watched, and the user had them killed. On the user's rule, passed on by the
coordinator, the S23 load runs once at the end over all bundles, not per bundle, and
the bundle commits stay separate until then, each building and running on its own,
so that a fault is bisected across them. A main checkout that others edit is no
place to land: `small` could not fast-forward there, since two other sessions held
uncommitted edits to review.md, the naming skill, ARCHITECTURE.md and Cargo.lock,
and on the user's word the round was integrated in a worktree of its own, `small`.
An agent set to rebase `small` while its owner session was rebasing it stopped mid
cherry-pick with 15 files in conflict, and it was deleted.

Merges broke what no GPU test ran over. In the merge of decor onto way, a
replacement of `side` by `resume` done by text in part.rs broke decor's generated
`side` part, and the world test got the world's parts twice; the build and
`pixelet-3b` found it, and nothing ran a GPU test after it. Since the decor merge,
the "world and walk" module of main's tests/world.rs listed clipmap, seed, clip and
palette and then `hand`, which held them again, so naga refused the world test
before any case ran; the supervisor found it while running world.rs for a shared
part, and it was fixed on `small`. Splitting `world.wgsl` on a branch while two other
branches split it too made each of two rebases redo the split (decor's clipmap,
seed, clip and palette; way's world_rule), and `small`'s own height code gave way to
main's `ground_height`, so tests/fill.rs ran again after the rebase. A supervisor's
run cut short by the stall left uncommitted edits in the worktree, one of them in
`clipmap.rs`, which it had been told to leave; they were saved to a patch and dropped
before the rebase, and the run after it was briefed again.

An implementer's `pkill -f build-wasm` matches every checkout's builds, so an agent
stops its own task or matches by path, and briefs say so now. LSP in a worktree whose
symbols main lacks gives no references at all: in `small` it found none for any
pxl-engine symbol (`beam::GRID_S`, `observer::cell`), so the reviews' call paths
rest on grep, and the WGSL's on naga. A rename by text replace (`asked` →
`column_zoom`) ran into four comments. A saved copy of the test put back over newer
work (`cp .clean`) undid that work, and the edits were replayed from their own
scripts; a copy is taken only just before the change it undoes. `make spec` fails in
a fresh worktree, because `xpute/ts` has no `node_modules` and the tool's prettier
reports "No such file"; `make -C xpute install-ts` comes first.

Git dropped or took files unseen three times. `git commit -- <dir>` leaves untracked
files out, and the sample commit missed its seven new files and was amended.
`git add -A` in a worktree where cargo had once run from the root took 209,872 build
files under `target/` into the WIP squash, and no report said so; listing each
commit's files before reading its code found it, and files are added by path now.
`xargs git add` over a list holding a deleted path stopped at that path, and the
commit took 67 of 158 files; the file count showed it, and the commit was redone by
path at once. World-split and void-columns came onto main without a stash: the diff
was saved to scratch, the branch's files were applied with `git apply`,
`git reset --soft` moved to the branch, and then the index was reset.

The round went into main as genre commits, with the other session's commits beside
them. `small`'s work was split into nine commits (the passes by hand, rust-gpu out,
the tests, the shared rules, groups from one definition, names, ts, docs), with the
tree equal at every step's end, and rebased onto the xpute session's eight. The
nineteen commits before the branch point were rewritten by genre the same way, with
the tree equal to the branch point's, so the seventeen after it replayed unchanged.
Each commit passes `cargo check`.

### What slipped out to production

A server whose table shape had changed was deployed without the search tables
copied, and `/geo/search` returned 500 until the user restored them and ran
`VACUUM`. `infra/README.md` gives the copy as one step, and a deploy now checks
first whether a table's shape moved. `VACUUM` then failed on `/dev/shm`, running in
parallel within Docker's 64 MB, and passed with `PARALLEL 0`. A report said "pushed
to deploy" when the push had failed, because a `tail` hid the error.

## Names

On the user's word, pxl-engine's `tally` became `count`, `TOOK` became `REUSED`,
`pyramid::tally_at` became `lane_at`, `TALLY_ON`, `TALLY_AT` and `TALLY_WORDS` became
`COUNT_*`, and `ray::MISS_ARGS_TALLY` became `MISS_ARGS_COUNT`. In pxl-render,
gpu/ray/tally.rs became count.rs, its `Tally` became `Counter`, and `tally_back`
became `counts_back`; perf.json's `eye_took` and `light_took` became `eye_reused` and
`light_reused`, generated again. In the WGSL, `*_TALLY` became `*_COUNT`,
`light_take` became `light_item`, and `sample_taken` became `sample_item`.
light.wgsl's `took` and `Took`, kept at first, became `visible_entry` and `Entry`
(after pxl-engine's `cast::Entry`), and `Answer::Took` in tests/light.rs became
`Answer::Entry`. The tests' comparison record `Tally` keeps its name, and the word
now names that alone. `grid::INV_SPLIT2` stays, an abbreviation the user takes.

## Open

On the phones:

- Whether what the S23 draws is what the Mac draws, pixel for pixel, and the shadows
  in particular, which the `light_*` entries cast now.
- The iPhone itself after `10fe6a7e5`: Safari on the Mac stood in for it.
- Which part of the light's modules the S23 computed wrong, with no error, when the
  left list moved before the misses.
- The native app on the iPhone, and where naga stands against Tint; the Android app
  on the S23 was measured at `e8d616ba4` (above).

In the frame's cost:

- The pyramid under pressure with the light's cells kept: a long drive, where the
  pyramid's slots run out, has not been run.
- The eye reading a light's cell in motion: no test covers it, and it has been seen
  by eye only at phone size, where a small patch, or one that shows only in motion,
  would not be seen.
- The drive with shadows on: 2.5 casts a second against 2.7, no gain yet.
- Which visits ask, counted on a phone.

In the passes and their tests:

- The lost writes: which load makes Metal abort a command buffer, and the cause of
  the `ground.rs:318` panic.
- The survivors left: `world:1127`, `world:487` and the tie of `halve` (`760`); the
  read disk's `<=` against `<` in the light's reads; `>` to `>=` in `enters` and the
  `t_max` of `span_of` in `cast`; the four face survivors, and whether they are WGSL
  that holds nothing and comes out; an exact touch and three equivalents in `light`.
- The thin cases of `sample_box` ("down a bore" and the eyed cases draw one box
  each), and the sign of `ASK_FACE` for a direction exactly 0 on the entered axis.
- The bind groups made at a resize or in a frame, which no test makes.
- The clear of the opaque surface, to be looked at once the preview draws again.

In the arithmetic:

- Metal's fast math reorders sums: the rule in xpute ARCHITECTURE.md names fusion
  alone, and the user decides.
- The square root of `spacing_width` (cast.rs `spacing.width(sqrtf(…))`,
  walk_node.wgsl), and the divisions by a segment's length in way.rs and their twins
  in way.wgsl, on both sides and per thread. `stop_word` with its division put back
  survives, as a 1-ulp quotient inside the fused words the test takes.

Of the questions the two records had left open, the cost of the larger generated
WGSL (the pipelines of a phone's first load, a frame's time on the Mac and the
phones) went with rust-gpu, and `world.wgsl` has read a real world on the S23 since
every asking pass went by hand, from 2026-11-05
([the frame on the S23](../20261103-the-frame-on-the-s23/README.md)).

## The commits named here

The commits named here were main's while the work went on. main has been squashed
since, and `git log --all` no longer shows most of them: each change is on main in a
squash, and most are on `original` as they were made. The six commits of the phones'
round are on `original` under other hashes, each with the same tree. The commits of
the hand passes are named as main had them when its history was rewritten by genre
on 2026-11-05 (the round's xpute side is in
[its minutes](../../minutes/20261105-xpute-not-yet-for-other-programs.md)). A state
that was never kept is named as the work it belonged to: `d163e34c5`, `5d7113c02` and
`bfd83b127` were commits of the decor work, not kept, and that work is
`e85332699f`.

| Named here | On `original` | On main |
| --- | --- | --- |
| `feee29054` | `cad791eade` | `dafc5a2322` |
| `4a73ebdd0` | `826f4908a9` | `dafc5a2322` |
| `10fe6a7e5` | `a60fd30b33` | `dafc5a2322` |
| `64b6734ea` | `c920e687e2` | `dafc5a2322` |
| `f94b0017b` | `b4dc1e14f0` | `dafc5a2322` |
| `4a74a51f2` | `4a56b8b526` | `dafc5a2322` |
| `f9f66682e`, `e8d616ba4` | | `47d238aab4`, `e8d616ba4`'s tree |
| `355b65140` | | `4cd32d38ef`, the same tree |
| `d11dc095c0` | `b7ec6dd8df` | `ecee58d6c6`, the same tree |
| `e85332699f` | `9f7839e040` | `9e9b30b924`, the same tree |
| `0b978c7e53`, `c0e206a08d`, `2cc580cd88` | `dcb723f058`, `0e1418b9f6`, `28ea81aad3` | `3d9aa5b91e` |
| `3baa9e9fd5` | `728020efcc` | `3d9aa5b91e`, the same tree |
| `6b2ba06d84`, `97b46505fb` | `725bc70025`, `6ebe1349fa` | `4399ef1ce5` |
| `97b95981b8` | | `4399ef1ce5`, the same tree |
