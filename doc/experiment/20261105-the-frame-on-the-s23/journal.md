# Journal

The S23's frame after the shaders by hand, made cheaper where the device showed the
cost: what was tried, what came of it, and how we know, newest last. Main at
a449637a35 to start. S23 figures are Chrome on the S23 (Adreno 740), ms p50/p95,
fresh loads alternated base and change in one sitting, two loads a side unless it
says; Safari is Safari 27.0.1 on the Mac. From the visit A/B on, a load is timed
only once the ground's textures have landed (whether they had was a confound before).

Open. From 11-07 the bar is the deployed build (e317a3644b) at the same clock; where the
frame stands against it is the last section. 11-07's runs took the tools named `cmp-*`
(`tools/cmp-hot.mjs` for the S23); `tools/s23.mjs` no longer runs on main (tools/README.md).

## Where it started

- **Main a449637a35 on the S23**, shadows off: fps 35–40 (rust-gpu's 11-03: 5.9), GPU
  p50 21–24 ms, `sample` p95 5.6–7.0 (133). Three runs in a row, the phone 43 to 48 °C
  and fps drifting down: a reference, not a fixed number.

## The day, morning against night

a449637a35 against c3847db899 on the S23, both from the shelf, alternating ×2, the
phone warming to thermal 1 alike on both sides (data out/s23/day*):

| | morning | night |
|---|---|---|
| shadows off, fps | 33 / 30 | 42 / 38 |
| shadows off, busy p95 | 40–46 ms | 29–32 ms |
| shadows on, fps | 16 / 15.5 | 24 / 24 |
| shadows on, busy p95 | 110–115 ms | 50–52 ms |
| drive, cast frames' busy p95 | 175–240 ms | 122–136 ms |
| first load to the world | 9.9 / 5.2 s | 6.9 / 5.7 s |

The largest moves: `light_hit`/`light_walk` tails −90%, `light_receivers` −55%,
`sample` −65%, `world` −25–30%; eye nodes a cast sample 57k to 12–15k, miss 20k to
2–4k; the light beam's 114–153k to 7k.

## Worked

- **Safari loads again** (e0b0fadcdc): `FACE_NONE`, `i32::MIN` folded, reached Metal
  as a bare `long`; no `select` of it and an int compiled, three pipelines were
  refused and the page stood at its loading. The head word's own none (i16's lowest)
  instead; build.rs refuses an `i32::MIN`. Found from the dev console relay.
- **The tests hold the requirements, not the route** (b26607770f): −7445 lines. What
  pinned visits, floors, the cache's slots, claims, layouts and pass splits went; an
  edited brick never answered as it was is held end to end (it fails with forget off).
- **The air before a point one trilinear read** (7f88f6676a): `world` 9.6–10.8 to
  8.4–8.5 ms. Bisected on the S23 from `world` to `screen.draw` (85%), to lighting,
  to the air, to the 8 B-spline taps a pixel. Pictures alike at noon, dawn and dusk,
  high and mid, at the horizon: base against change within load-to-load noise, no
  band in contrast-stretched crops. The B-spline came with aed85fbdd7's fix for bands
  from high up; the bands were the per-froxel march, which stays fixed.
- **A ray left by the cache walk resumes where it was left** (a9d6d92d37): `beam_cs`
  4.5–5.3 to 1.8–2.6 ms. The `beam` span held three passes, so its ns a node read no
  better than rust-gpu's; split, `beam_cs` walked every left ray again from the root.
  The frame's busy fell less (−0.5 to −2 ms) than the pass did.
- **The light's asking walk takes only the slots left** (d261b5e0ca): `light_walk`
  p95 19–20 to 4.5–6.2 ms circling, 93–103 to 50–54 driving; 16 MiB of list room at a
  2048 map. The resume from the cell alone kept a sixth of the gain; the list was it.
- **A bucket scanned once, one texture tap** (36bd3f9a7f): `world` 9.0–9.1 to 7.1–7.3,
  `beam_hit` 2.3–2.4 to 1.8–2.0, fps 31.5–33.2 to 33.3–36.8 shadows off; the textured
  ground alike. From a static read of the shaders (four identical taps in paint).
- **Every asking walk resumes** (2135d70883): sample nodes 47–56k to 25–27k (p50 2.1
  to 1.2), miss 17–21k to 5–9k. The first build's beam rose 40%: its blind walks went
  back to the root. Resuming a blind walk is exact (a split's box joins all its
  descendants answer), so the restart went and beam's visits are main's.
- **A world change casts again only the light tiles it reaches** (aa2ee5efef):
  `light_hit` p95 17 to 5.2–5.4, `light_walk` 25–28 to 11.5–15, busy p95 112 to
  80–88, a character walking under the circle. Replaces the 11-02 minute by the user's
  word. No stale or missing tile seen at phone size.
- **One module for miss and miss_list** (c83fd6f67b): 84 modules, 977 KB, where 85
  were 1,036 KB; the Tint test and the probe make all 88 pipelines (they made only a
  module's first entry). The 11-03 inlining was gone with rust-gpu already.
- **Pictures before models** (2297b60d29): the Earth's sphere waited behind the whole
  model catalog; Safari's far corner showed the sky's black until 57 s. Fills once it
  lands (70 s on the dev link).
- **The app's own profiler's trace** (Perfetto export): the S23's 100–500 ms gaps
  follow runs of frames whose GPU work adds up to them; script and render 15–18 ms in
  each. GPU saturation and Chrome's queue, not turns, loads or readbacks.
- **A static read of every pass, ranked by the S23's time** (three readers over
  a9d6d92d37, the top items checked in the code): 16 items, of which the bucket scan,
  the taps, the asking walks' resume and the light footprint landed. It also showed
  `px_ray` does not walk, which the bisection had misread.
- **Readbacks ruled out:** timestamps off, `map_async`s 1.2 to 0.2–0.4 a frame, the
  gaps unchanged.

- **The light's beam split as the eye's** (880b154d54): `light_beam` 7.2/13 to
  4.3/6.3 ms, busy p95 −7, the drive's cast frames p95 200 to 141. One beam_hit/beam
  for both observers, each its own part.
- **A cell's window band in three evaluations** (514cda20b3): the `light_beam` span,
  where light_cell runs, p50 −1.5 to −2 ms. The moon's fill gated where its air
  reaches; its direct light kept (the bound proves the fill only).

- **Pixel-bound at full resolution** (main 82efe02944, `res` fixed 1080×1969 against
  low, ×0.38 a side; the phone hot, thermal 1; ×2 alternating): world 13.7 to 4.5,
  light_receivers 11.5 to 2.0, px, sraster, resolve and miss fall with the pixels;
  beam_hit, beam, sample_hit, sample and the light's walks (the angle lattice's) do
  not. Busy halves; fps 21.6 to 36.4 shadows off, 12.1 to 21.2 on. Of a hot shadows-on
  frame's ~80 ms, ~38 are per pixel and ~25 the lattice's. No scale between the two
  without code.
- **light_receivers is the stands' stores** (bisected with scratch variants of
  light_mark): footprint off and a store without its read change nothing; no stand
  gives 1.05 ms of 7–8. Every drawn pixel's thread stores into a few hundred tile
  words, 2.1 M atomic stores serialized.

- **The receivers' stands once a tile a workgroup** (repeats.patch on c83fd6f67b, the
  shelf, the S23 hot, ×2 alternating): `light_receivers` 14.4 to 4.4 ms with a character
  walking, busy −10 ms; shadow edges and exposure the same. A root line shared by
  every beam ray bought nothing (`beam_hit` even) and goes.
- **The page's GPU memory, read from the code** (main 417494fe4a): 437 MiB on the Mac
  (428 on the S23) with shadows never on, 565 once on (none of it given back), ~780
  at every cap with load transients. Five loops or GPU-written counts had no hard cap,
  way_free.wgsl:31 the worst by reading (a one-lane walk of the ways pool that leaves
  only at depth 0). After our pages the Mac's GPU stayed at 100% with Finder, the Dock
  and the menu bar frozen, and the Mac was rebooted; the next day quitting Safari
  alone freed it, so the reboot was not shown to be needed. Safari's GPU process died
  at 1.4–1.5 GB after several runs left pages open; a fresh page is well under that.

- **The shadow resources follow the setting; the loops and counts read as unbounded
  capped** (the caps were not shown to fix anything: see the hung submit). Shadows
  toggled six times a run in Safari, three runs, every allocation counted: off back
  to 431.19 MB, on to 603.40 MB each time (164 MiB swing, no leak), the same shadows
  on each turn-on, the GPU never pinned. A "t must advance" stop on the capped walks
  was not taken: a push, a corner crossing or a node under t's ulp keeps t.

- **`auto` on the S23** (main 97f694bb41, the shelf, 60 s holds, one load, thermal
  0): shadows off 54–56 fps at lattice 1; shadows on 45–48 fps, settled at 0.5 and
  lattice 2 within 10 s, softer but clean. Shadows off it swings 0.84↔0.71 every ~3 s
  (18 changes in 60 s), against the judge's stated contract: the timed branch lacks the
  untimed branch's guard.
- **A fresh Safari page holds 412 MiB, flat** at default and far zoom, with no climb
  (main 97f694bb41). So the device losses read as memory on 11-05 were not shown to be
  memory. One GPU submit never finished 80 s into the far zoom, near a sky step: in
  flight stuck at 1, GPU 100%, the GPU process uninterruptible, the canvas off. Caps
  in 9d55c0c66d did not stop it, so way_free, named likeliest in that commit, was not
  the cause. Force-quitting Safari freed the GPU at once.

- **Why frames cast, and the world query's share** (main 97f694bb41, per-frame reason
  bits, a doubled-query variant): circling or walking 97–99% of frames cast, all eye
  and jitter, every cast earned; at rest none cast, yet the GPU is busy 10.8 ms a
  frame at 60 Hz (world 4.6); driving, gated frames carry 10.2 s of GPU against cast
  frames' 2.0, with `mark` at 6.2 ms. One world query is ~0.1–0.2 ms p50 and 15–25% of
  an asking pass's p95. So neither the kernel's cast rate nor the world's rules are
  the next lever. The camera follows the character, so "character walking, camera still" were
  full casts: map-gate's no gain on receivers-only casts is unproven either way.

- **Safari's page memory measured, every phase** (af434aad6e, the shelf, each pass in
  its own submit, far zoom 10 min): default 410 MiB, far 412, back 412, shadows on
  577, flat, with no climb. No stuck pass, device loss or pin; utilization under 71%.
  The 0.6–0.7 GB gap of the code's inventory doesn't exist on a fresh page: the 1.4–1.5
  GB belonged to one GPU process serving several hung pages. The hang did not recur,
  but the face cap and the split submits changed together.
- **`auto` tries a step up on one rule** (6a9f3a2070): scale changes 18–20 a minute to
  0–1, fps 54–55 to 56–58, interval p95 33–36 to 23–30 ms; settles at 0.5 warm.

- **The drive measured a pin, and a user could hit it.** A held drag and the wheel past
  the horizon left the camera at 2 u under the level, every step refused but a
  vertical drag. Fixed in collision.rs with a test, and drive.ts tilts to just over
  the horizon. So every drive figure above is ~6 s of casts plus ~14 s of one standing
  close-up: its cast frames' and passes' tails compare only with each other, its fps
  and gated share mean nothing, and `mark` 6.2 ms is the character's fragments up close.
  The honest drive's baseline (one run): 326 frames all cast, fps 16.4, busy 52/115,
  `light_walk` p95 39, `miss` 27, `light_sample` 18.5, `sample` 17.
- **The atomics, second round** (patch held for the Mac's GPU tests): a slot's key
  atomic beside a plain entry, tallies atomic and gated, the light list, ways, decor
  and stands plain. Shadows on: `light_beam` 3.2 to 1.6 ms, `light_walk` p95 2.4 to 1.6,
  busy p95 50 to 43, fps 25 to 27.6. Shadows off, three cool pairs: 43.9 to 44.0 fps,
  even. The first round's −3 fps was run order: the first of a pair runs faster.

- **Far views' cost is the cache forgetting** (forget-bricks.patch, held for the Mac's
  GPU tests; S23, dawn, shadows on, 6 s circle at fixed zooms, ×2): a node's bricks
  were tested by entry bit modulo ACROSS, so a moving fine level dropped nodes that
  alias it anywhere in the world. A cache 4× larger changed nothing; no forgetting
  took asked 215k to 3k. Each level now hands the box its moved bricks lie in: at
  11,761 u asked 215k to 22–37k, sample 15 to 2, busy 58 to 41–47; at 131,072 u 90k to
  10k, busy 40 to 29–33. The default view is unchanged.
- **`px` over the claimed tiles** (px-tiles.patch, held likewise): at rest px 2.29 to
  0.07 ms, busy 10.4 to 8.3; moving frames even.
- **The frame in stages, 4–6 submits** (submits.patch, held likewise): on the S23 no
  cost, shadows on busy p95 53 to 46; whether it stops Safari's hang is untested.

- **The atomics' second round answers wrong on the Mac**, stacked on forget-bricks and
  px-tiles: `hand_light_beam_rays_answer_as_cast_walk` failed 4 of 18 runs, the next
  frame only, one light beam ray ending one node over; without it 10 of 10 pass. The
  plain reads under the claims were assumed safe; they are not, as built. Held; the
  plain groups are being narrowed one by one.

- **The light map recast whole on a depth step** (main e50bf2d5ec, a depth tag, two
  time-swept drives): the light frame's key changes by its depth cell alone (planes
  snapped by `DEPTH_STEP`, 0.2 of the box's depth), as the eye moves along a low sun;
  45% of light casts start a new frame so, 60–64 ms against 49–56, 48–51% of the
  drive's busy. A filter of changed columns to those reaching the map changed nothing
  and was dropped. Being built: the depth step slid as a tile shift is, the ray kept
  and its window moved, so a kept texel equals a fresh cast.
- **Only the light list and stands plain** (atomics-light.patch, held for the Mac's GPU
  tests): at equal `world`, `light_beam` 7.2 to 4.5 ms throttled, 3.7 to 2.6 cool, busy
  62 to 59–60, nothing else rose. The candidate with the ways, decor and exposure plain
  too read world +0.8, likely warmth, unsettled.

- **The depth step slid** (light-depth-slide.patch, 33 files and a contract change,
  held): at 178 u whole recasts 9–13 to 1–2 a window, but a stepped cast costs as much
  as the whole recast it replaces (82.5 ms against 62 unstepped; base's whole 69–90),
  so the tail stays. In the time-swept drive whole casts 116/109 to 85/85, most of the
  rest cheap far re-anchors past the window (51 ms, walk 0.4), busy p95 −4/−16, fps
  +0.4/+2.0. The dear casts left change the light frame's key without the sun moving
  (cast_from or half_width), 34 a drive: all cast_from, the sky re-aiming the sun from
  the eye's place as it travels tens of km at the sweep's far end (environment.rs
  `sky_anchor`), each re-aim easing through several light frames. Left as it is: a
  user rarely travels so far with shadows on.
- **Decor's model floor** (diagnostic, at rest): models down to 1 camera cell (~11 px,
  "set by eye") cost much of the 3.3 ms: world 5.24 to 3.41 at 2 cells, 2.69 at 4.

## Did not work

- **The atomics, first round** (entries apart from keys, plain tallies, a name column,
  +2 MiB): cache walks +0.2–0.5 ms, and plain tallies read 2–15% of main's on Adreno,
  useless. A person decides by those counts, so they stay atomic.

- **The receivers' stands in workgroup memory** (light_propagate): `light_receivers`
  ~7 ms either way on the S23; 1 KB and barriers for nothing, dropped.

- **The node cache without claims or atomics** (stamp-checked, plain loads): shadows
  on, fps 15.8 to 11.7, busy p95 112–131 to 212–230. Taking an entry only at the
  zooms asked (needed once entries can tear) left light misses 70× the nodes; the
  atomics saved less than that. Main's claims never tear, so its other-zoom shortcuts
  answer right.
- **One atomic a workgroup for list appends:** the eye's passes even or +0.1–0.8 ms;
  its `sample_hit` rewrite (flags, a barrier) raised `light_sample` 2–5 ms. Adreno's
  per-lane appends were not the cost.
- **The light map drawn through its gate on receivers-only casts:** `shadow` 1.6–2.0
  in all three builds.
- **A span a draw group in the world pass:** WebGPU writes timestamps at a pass's
  ends only; a split stores and reloads the tiles every frame. Attributed by
  subtraction instead: `px_ray`/`hit_of` ~0.9 ms, not the 5–6 first read.

- **The light misses resumed from their cell alone:** drive `light_walk` 116–125 to
  101, where the build with a list of the slots left reached 11–14. The list kept.

- **The cold start after a view jump, with the asking resume** (2135d70883, one
  fresh-load drive a build, ray counts from the start): the first casts' `miss` peak
  74 to 26 ms, nodes a walked pixel 11–28 to 6–8, 1.1 M to 0.5 M a cast. What is left
  is the refill itself, ~70k pixels and ~250k nodes asked a frame for ~0.5 s, spread
  over `miss`, `sample` and `beam`.

## Unsettled

- **The hung submit in Safari at the far zoom.** On af434aad6e, unsplit, the GPU
  pinned about 2 min into the far zoom, memory flat at 412 MiB; with each pass in its
  own submit the same phases ran 10 min clean, passes in the tens of ms. So the face
  cap isn't the fix; the suspect is one large command buffer stalling in WebKit or
  Metal, not a shader that never ends (two runs against one; a split run could be
  luck). Reading found every loop in that frame ending on data the code can produce.
  Next: the frame in several submits, in the app, tested in Safari. On main
  3d9521137f (forget-bricks and px-tiles, one submit a frame) it pinned again ~50 s
  into the far zoom, memory flat at 412 MiB: the cache's aliasing was not the cause,
  which puts the weight on the single large command buffer. submits.patch's Safari run
  waits for a force-quit.

- **`light_receivers`** 6.9–8.1 ms every frame with a character walking, the largest
  light cost now; receivers.patch is being measured.
- **Lighting not run under decor** (a depth-only screen draw, decor clearing a
  stencil bit): not built. The 8 stencil bits are the gate's numbers, so a coverage
  bit touches every world-pass pipeline, for an unmeasured 0.3–0.8 ms; the share of
  screen pixels decor covers is to be measured first.
- **Safari over http://localhost** asks no zstd and gets the 1 MB tile form; https
  takes zstd since Safari 26.3. Left to time.
- **plei.me** answers Korea from LAX (60–110 KB/s); left as is.

## 11-06, main ef8d1621b7 hot

The batch (kept voxel records drawn every frame, one budgeted holes' queue, owed work
woken by readback, the rolled lattice) on the user's drive, shadows on, `res fixed`,
the shelf. Two fresh loads with `s23.mjs` (`SCENES=drive,rest`, `DRIVE_S=300
FRAMES=1`; then `SCENES=drive,rest,rest`, `DRIVE_S=180`, after 3 min off). Beside
each, every 5 s the GPU's clock cap (kgsl `max_freq`, `thermal_pwrlevel`) and the AP's
and skin's temperatures, and an adb screencap every 20 s and 10 s. Beside the second, a
second DevTools client that kept every console error. The profiler keeps the last 60 s,
so each figure is the drive's last minute. Data, shots and clock logs:
out/s23/hot1*, out/s23/hot2*.

| | hot1, still throttling | hot2, settled hot |
|---|---|---|
| GPU clock cap in the window | 615 then 550 MHz (level 2–3), of 719 | 348 MHz (level 6) from 50 s on, 295 for 15 s |
| thermal status, skin | 0 to 1, 39.5–40.5 °C | 1 at the end, 39–40 °C |
| fps | 22.3 | 12.4 |
| a frame's fps p50 / p5 | 38.5 / 7.6 | 22.7 / 4.3 |
| interval p50 / p95 / max | 26 / 133 / 388 ms | 44 / 231 / 521 ms |
| cast frames' busy p50 / p95 | 77 / 130 ms | 135 / 248 ms |
| px | 29.6 / 67.4 | 50.5 / 126.6 |
| miss | 4.0 / 18.0 | 8.1 / 36.4 |
| sample | 8.3 / 16.9 | 16.5 / 33.4 |
| beam | 7.9 / 15.1 | 14.2 / 27.6 |
| world | 6.9 / 8.3 | 11.6 / 15.1 |
| cast | 6.2 / 8.1 | 9.5 / 14.0 |
| sample_box | 3.4 / 4.9 | 5.4 / 8.1 |
| light_raster | 2.3 / 2.8 | 3.8 / 4.7 |
| every other light pass, p95 | ≤ 0.5 (light_receivers 4.1) | ≤ 0.9 |

Spans are ms p50 / p95. hot2 is the hot level: the clock held at its floor for two
minutes, and its two 30 s halves read 12.6 and 12.2 fps. hot1's halves read 23.1 and 21.8
while the cap was still stepping down.

- **Against the journal:** the honest drive's baseline was 16.4 fps, busy 52/115; the
  deduced hot frame was ~80 ms. Hot now, a cast frame takes 135 ms p50 and the drive runs
  12.4 fps. Half the frames are held, with no GPU read (C h C h …), and they count in
  the fps. Long intervals mostly fall on the held frame after a cast.
- **px is the largest pass:** 37–39% of a cast frame's p50 (it was ~1–2 ms). It and
  miss follow a frame's busy (correlation 0.70 and 0.68); sample and beam do not (0.14,
  −0.01). The light's queue passes are under 1 ms. Ray counts were off in the drive,
  so a spike was not tied to the queue's asks.
- **No rest:** every frame of both rests asks the next (`wakes next` = n), with no decay
  over 15 s windows. hot1's rest ran 47 fps with GPU busy 36 ms a timed frame (px p95
  20). hot2's rest, the camera stopped 0.5 m from a grass wall, ran 22–24 fps with GPU
  130–170 ms a frame, px p95 125–147 ms. The frames at rest are kinds held, px and
  gated, and light_moved stays ~10 a frame.
- **A command buffer invalid every frame, from load:** `[Buffer (unlabeled)] usage
  (Indirect|Storage(read-write)) includes writable usage and another usage in the same
  synchronization scope. While validating compute pass usage`, then `[Invalid
  CommandBuffer] … Submit`. In hot2 there were 7,490 pairs in 263 s (25–50 a second),
  through the load, the drive and the rest. The pass is not identified. A submit's work is
  dropped each frame, so the figures above time only the submits that ran.
- **The picture:**
  - From about 100 m out (hot2-shots/s-t0086, t0096, t0136, t0166), the ground breaks
    into thin vertical slivers and fragments over a flat untextured green.
  - At 10–20 km (hot1-shots/s-t0176, s-t0291; hot2-shots/s-t0151), a bright horizontal
    line crosses the horizon, and the far ground ends at a hard horizontal edge in haze.
  - Inside the ground (hot2-shots/s-t0221, 2 m: three rock faces in a corner;
    hot1-shots/s-t0336, 100 m: a full screen of one brown texture), the camera met faces
    against solid.
  - At rest after the drive (hot1-shots/s-t0356), the mid-distance hillside is flat
    green with faint box edges, and a thin vertical sliver runs at the left edge.
  - No stale shadow was seen after a turn. The sun is low at the drive's dawn, so few
    shadows are on screen.

## 11-06, main 034ddad063 hot (the indirect scope fixed)

The fix for the invalid command buffer: an indirect call whose args are bound writable
is refused as it is encoded, and the four dispatches' counts sit in buffers of their own.

**Mac Chrome.** The same page loaded for 45 s in a fresh profile showed no GPU
validation error and no ENOTRECOVERABLE. Four React Native "touch end without a touch
start" warnings came from the pointer over the window.

**Two loads on the S23,** run as the ef8d1621b7 entry ran them:
- hot3w was a warm-up: `SCENES=drive`, `DRIVE_S=300`, ray counts off.
- hot3 was measured right after it: `SCENES=set:ray_counts=1,drive,rest,rest`,
  `DRIVE_S=180`.

Both logged the clock cap and took shots every 10 s, and hot3 also had the console
listener. Neither page logged a console error (`errs []`, and the listener wrote
nothing). The clock cap fell to 550 MHz (level 3) 81 s into hot3 and stayed there to the
end, status 1, skin 40.1–40.5 °C. It did not reach the 348 MHz of hot2, so hot3 is set
against hot1, which had the same cap (615 to 550 MHz). Data: out/s23/hot3*.

| | hot1, ef8d1621b7, 615→550 MHz | hot3w, counts off, 615 MHz | hot3, counts on, 550 MHz |
|---|---|---|---|
| fps | 22.3 | 13.4 | 10.7 |
| a frame's fps p50 / p5 | 38.5 / 7.6 | 32.3 / 3.9 | 24.4 / 3.2 |
| interval p50 / p95 / max | 26 / 133 / 388 | 31 / 256 / 405 | 41 / 315 / 452 |
| cast frames' busy p50 / p95 | 77 / 130 | 166 / 251 | 194 / 303 |
| light_beam | 0.1 / 0.5 | 73.0 / 82.7 | 81.5 / 92.3 |
| light_sample | 0.1 / 0.5 | 11.1 / 26.0 | 12.2 / 28.4 |
| light_walk | 0.1 / 0.5 | 2.0 / 34.5 | 4.1 / 40.0 |
| light_hit | 0.1 / 0.2 | 1.8 / 19.9 | 4.8 / 20.2 |
| light_raster | 2.3 / 2.8 | 2.6 / 3.6 | 2.6 / 3.5 |
| px | 29.6 / 67.4 | 17.2 / 57.0 | 26.0 / 74.4 |
| miss | 4.0 / 18.0 | 4.8 / 19.7 | 5.6 / 24.1 |
| sample | 8.3 / 16.9 | 8.7 / 16.7 | 11.1 / 19.6 |
| beam | 7.9 / 15.1 | 6.9 / 12.8 | 8.3 / 16.6 |

Pass times are ms p50 / p95, over the drive's last minute.

- **The light's work now runs.** On ef8d1621b7 the light's beam, sample, walk and hit
  passes read near zero. With every submit valid they take ~100 ms of a cast frame,
  light_beam ~80 ms of it. They follow a frame's busy most closely (light_beam 0.86,
  light_sample 0.79, light_hit 0.73, px 0.69). The ef8d1621b7 figures therefore timed a
  frame without its light. That the dropped command buffer held those passes is read off
  these timings, not traced.
- **Ray counts:** hot3 counted, per cast:

  | | beam | sample | miss | asked of the world |
  |---|---|---|---|---|
  | eye | 220k | 278k | 21k | 25k |
  | light | 177k | 191k | 193k | 31k |

  The eye reused 74% and walked 5%. The light cast again only 5% of its rays.
  Counting cost is under the clock's share: hot3w and hot3 differ by 17% in busy, and
  the cap by 12%.
- **Frame time against the queue.** The frames at or past the interval's p95 (315 ms)
  walked 125k eye nodes on average against 15k for the rest, and asked fewer (1.3k
  against 5.6k). The light's asks follow busy at 0.43. The profiler's frames give no
  owed count, so the owed queue itself is not visible here.
- **No rest:** in both rests every frame asked the next. rest2 had no motion or cast
  for 20 s before it and still ran 18 fps at 151 ms of GPU a timed frame (px p95 80.5,
  light_beam p95 71). Of its frames, 51 were light casts that asked zero nodes, with
  33k boxes a cast. It had not come to rest 35 s after the drive.
- **The picture (hot3-shots):** much as on ef8d1621b7.
  - At 2 km (s-t0111) and 200 m (s-t0211), the ground is still slivers, scattered boxes
    and dark squares over flat green.
  - At 20 km (s-t0101), the bright horizontal line still crosses the horizon.
  - Inside the ground, at 2 m (s-t0141, grey rock) and 20 m (s-t0156, the whole screen
    one grey-violet rock), the camera still meets faces against solid.
  - At rest (s-t0246, s-t0261, s-t0276), a grass wall fills the screen.
  - No stale shadow was seen after a turn.

## 11-07, what the frame carried over frames, removed

The bar is the deployed build ([the deployed build is the bar](../../minutes/20261107-the-deployed-build-is-the-bar.md)):
e317a3644b with e2b019f1b0 and b6dac91e78, so its drive is not pinned, served by Vite; the
change is main at b034c0bf54 with the working tree, served by Metro. Both dev builds, both off
the shelf. The Mac's figures are `script/compare-perf.py`'s, base and change by turns; the
S23's are `pxl.perf.drive(120)` then 65 s untouched, fresh loads by turns, the GPU's clock read
every 5 s (`tools/cmp-*`; the data under out/cmp/, not kept).

Removed: the visit budget and its caps, the rolled lattice, every ray parked across frames and
the owed words, bits and pools it took, the readback that woke frames for them, and the light's
claim budget, places, rings and carried rays. Every ray ends in its run; a ray with no room or
asks left ends as a lack. `Counter` keeps the `ray_counts` setting's copy alone.

- **The eye's holes, one lane a word, were the cost; a list, a lane a hole.** Found in the code
  first: a lane took a word of hole bits and walked its up to 32 holes one after another, so
  holes that lay together kept a 32nd of the lanes busy, about 5× short at 100k holes. A list of
  holes, as the light's: `miss` p95 29 to 6 ms on the Mac, GPU 27.5/56.8 to 19.3/36.8 ms, casts
  in 20 s 599 to 872. One run each side, the gate after it. The list takes a word a drawn pixel,
  so the most drawn falls to about 14.7 Mpx (`DRAWN_AREA`); an 8K canvas is drawn smaller and
  brought up.
- **The gate, 3 rounds: pass.** GPU p50 19.60 to 19.46 ms, p95 37.36 to 37.36, the first load's
  longest frame 878 to 175 ms, tick 1.5/2.2 to 1.5/2.1.
- **The node cache's keys and states apart from its entries,** dense arrays, the entry 44 B where
  it was 80 B. A walk's hit reads about as many lines as before (2.5 to 2.4), a miss 4 to 1; a
  pass over every slot reads 17–43 MB where it read 84 MB, two or three a cast. Expected: under
  0.5 ms on the Mac at 400 GB/s, 2–5 ms on the S23 at its 67 GB/s peak or below. The gate, 2
  rounds: GPU p50 19.79 to 18.87, p95 37.22 to 39.19, pass, within the Mac's noise as expected.
- **Standing, 8 s of the drive then 60 s, Mac:** 3258 frames gated and 342 stood, GPU 6.0/9.4
  ms, the panel idle; the deployed build 3598 gated, never resting, GPU 9.2/13.1 ms.
- **A held turn no longer draws the next frame whole where its lists moved nothing.** The kernel's
  frame test still finds the view resting 18 to 28 turns after it stops, against its bound of
  `JITTERS + FRAME_LATENCY`; it fails.
- **The asks a frame takes, 2^21.** At 2^17 a run of the light's holes over a fresh node cache
  still asked past it after 64 frames, and four GPU tests failed on it.
- **Not as first said:** the walk's budget atomics cost 0.3–8 ms on the S23 by the count of
  rounds, not the 10–30 ms a reader estimated; the S23's bandwidth was taken at 25–30 GB/s
  unchecked, its peak is about 67 GB/s.
- **The S23, the drive then 65 s untouched,** fresh loads by turns, the GPU's clock as logged:

  | run | clock, MHz | frames drawn a second | GPU p50 / p95 | standing 60 s: frames, GPU p50 |
  |---|---|---|---|---|
  | main-1 | 719 | 9.5 | 62.5 / 238.7 | 3554 (951 px, 960 held), 18.0 |
  | base-1 | 615–680 | 2.5 | 304.4 / 1282.1 | 1004 (969 gated), 20.3 |
  | base-2 | 550–719, most 550 | 1.5 | 300.2 / 437.6 | 840 (731 gated), 30.0 |
  | main-2 | 550, two reads 615 | 6.8 | 82.3 / 359.7 | 3289 (1059 px, 1109 held), 23.3 |

  At 550 MHz (main-2 against base-2) the GPU's p50 is 3.6× the deployed build's, its p95 1.2×,
  frames drawn 4.5×. The 2× bar holds at p50 and not at p95. The p95 is the light's now:
  `light_hit` 152, `light_sample` 73, `light_walk` 48 ms, together more than the deployed
  build's light (`light_walk` 174, `light_sample` 64, `light_beam` 34); the eye's `miss` 13
  against 272. With the claim budget gone, an epoch casts every tile it reads in one cast.
  Standing, main draws three times the frames the deployed build does, casting the pixels again.
- **A frame turn the GPU cannot take is not run** (the kernel's `frame_task`), its grant's time
  added to the next; the renderer's held turn, which drew the next frame whole and so started the
  jitter's sequence again, is gone. Mac gate, 2 rounds: pass, GPU 20.77/38.99 to 16.78/34.80 ms.
  Mac, 60 s standing: 1116 frames stood and 2484 gated, none casting the pixels, GPU 5.8/8.9 ms.
  S23 (main-3, 680–719 MHz, the phone cooled since main-2): the drive's GPU 63.2/249.2 ms, frames
  drawn 9.1 a second as main-1's; 60 s standing, 2604 frames gated and none casting the pixels
  again, GPU 18.0 ms, where main-2 cast them 1059 times. The deployed build's standing window
  drew 840–1004 frames at 20–30 ms; main draws the scene's animation at its own rate, more
  often. The profiler's interval now spans the turns not run.
- **Open:** fewer rocks at the far zoom in the drive's shots than the deployed build's.

## 11-08, why the light casts again

Main at 497bdf6062. Mac 4K, `pxl.perf.drive(20)` with `ray_counts` on, one fresh page a run,
each frame's `light_moved` read off the trace; a probe, not kept, split the light's frame
(`Shadow::generation`) into its key's parts and the world's whole change from its columns. The
light's spans (`light_beam_hit` to `light_walk`) summed by why the frame cast:

| why | frames | light ms | share |
|---|---|---|---|
| the box's depth stepped (the eye along the light) | 302 | 1129 | 43% |
| bricks and edits, by their columns | 144 | 956 | 37% |
| the box slid, or the eye's zooms stepped | 53 | 321 | 12% |
| tiles read anew | 217 | 192 | 7% |

- **Not as first said:** the 11-07 line put the p95 on the epoch with the sun's and the zoom
  sweep's moves among its causes. The drive holds the sky at 07:00, the box's width is a
  constant (`EFFECT_REACH`), and a step of the eye's zooms is a slide, not an epoch. Of 288
  epochs in a second run, 261 were the depth step alone and 27 the depth step with the sun.
- **A brick's columns read by the bands that read it** (`light::Column`, `brick_columns`): a
  tile answers at its band, off the finest clip level at or over the band holding the cell
  present (`clipmap::step`), so a brick at a zoom moves the tiles of its band, and of the bands
  finer only where the next zoom's bricks are not present; a band coarser reads none of it.
  Before, every band over the brick's column was cast again. The light's GPU tests hold it
  (`a_world_change_keeps_the_tiles_it_does_not_reach` with a brick over the leaf's, kept
  slots' walks alike; `hand_slide_forgets_what_a_world_change_reaches` with a column one band
  reads). Two runs after against the one before: the columns' light ms 956 to 601 and 847, the
  frames that cast the map whole 1375 to 833 and 1247 light ms. Run to run differs as much;
  the gain is not separated from it. A column now carries its bands, the head two vec4s a
  column. A column past the head's 64 is taken into the last, as the draws' stands are,
  where it cast the map whole: one branch and one cause of a whole cast fewer.
- **Open:** the depth step. A slot's word is a share of the box's depth from its near plane,
  so the box stepped along the light makes every word another depth and every ray another
  start. Keeping them across a step changes "a texel kept is the same ray"; proposed apart.

**`auto`'s lattice, removed.** The backlog's condition was that it goes if it earns nothing.
A coarser lattice moves the descent a beam ray shares onto the samples under it, four to a
cell or more, so it pays each level on more rays. The S23, 719 MHz both, one 60 s drive each
at `fixed` with the lattice forced:

| lattice | GPU p50 / p95 | `beam` | `sample` |
|---|---|---|---|
| 1 | 57.7 / 205.6 | 8.5 | 7.8 |
| 2 | 60.3 / 218.4 | 4.2 | 13.8 |

The sign is 11-03's. The 4K Mac went the other way in one run each, 26.3 / 50.9 to
22.5 / 49.3; the Mac does not reach `auto`'s floor, where the knob moved. The deployed build
has the knob: it goes as a knob that buys nothing, not as a regression. Gone: `LATTICE_MAX`,
`Grid::lattice` and `s`, three of the grid's words, the judge's G (`cell_ms`, `per_cell`),
`gpu_lattice` and the perf frame's `lattice`. `fixed` and `low` drew at lattice 1 before and
draw the same now; `auto` keeps the rate and the scale.

**A standing view, judged: no change.** 60 s standing on the S23, main drew 2605 frames, the
deployed build 840. The asks are not what differs: since 4a4441d55b, which the deployed build
has not, an actor's clip steps where its pose moves a device pixel, where the deployed build's
stepped at the clip's fastest point throughout. A gated frame asks no frame after it
(`Resolve::asks`), and costs less than the deployed build's, GPU p50 18.0 against 30.0 ms,
`world` p95 10.1 against 13.2, `mark` 6.0 against 7.1. The deployed build drew fewer because
each took longer. Fewer frames would take a cap on the animation's rate, a knob; none is added.

**What a whole cast walks.** On the Mac's drive a cast of the 1.9 M slots the receivers read
took 8% through the holes' walk (150–170 k), 42–45% off the samples' raster and 46–51% as sky.
So `light_hit`, the largest span of the S23's p95, is the holes' first walk over about 150 k
rays. Keeping tiles across a depth step was weighed and dropped: a kept slot's ray would start
a step from a fresh one's, half the slots end as sky and would owe the strip the step added,
and the tile would have to carry which plane it was cast under.

**"Fewer rocks at the far zoom" was the clouds.** The drive's shots were taken at different
zooms. Held at the far end, the same input on both, 4K: main against the deployed build differ
at rest and a second into a turn in 0.8–0.9% of the pixels, all of them the white lumps, the
panel's text and the compass. The lumps are the cumulus, the same ones shifted, which drift by
the wall clock, and the two runs were a minute and a half apart. The ground is alike.

**The dark band along the horizon, not found again.** Its cause was the parked beam rays and
the holes they left (56e388dc33), and neither exists since a ray ends in its frame. Two 4K
drives on main, a shot every 75 ms at a quarter of the size, 687 shots, 152 of them at the far
end: none has a band. Dark under the horizon there is the clouds' underside or the space over
the limb, in the shots before and after alike. At the earlier count of 5–9 a drive, about one
frame in three shot, some would have shown. The near end's dark shots are the inside of a
grass block the camera passes through.

**Safari with shadows on, rechecked (Safari 27.0.1, `measure.mjs`, a watcher to force-quit it).**
No pin. Shadows off through 120 s of the far zoom and on for 90 s at the default: GPU 79% at
most, the Mac's other work about 40–50%. Shadows on from the start through 180 s of the far
zoom, the sky's time moving: 72% at most, the GPU process 310 MB, no page error. The page's
live bytes read 0: the proxy's counters do not reach the worker's device. No shot was taken in
Safari, so that it drew is read from the load's end and the utilization moving by phase.

**The character preview shows.** The scene's work found it culled by the world's eye under the
sea (8671645700). Seen in Chrome: the drawer's Character tab draws the person and the pet.

**`auto`'s rate on the S23, judged: the floor stands.** One 60 s drive under `auto`, 719 MHz,
against the same at `fixed`:

| | rate, scale | frames a second | interval p50 / p95 | missed | GPU p50 / p95 |
|---|---|---|---|---|---|
| `fixed` | 59.9 Hz, 1.00 | 10.8 | 48 / 282 | 87% | 57.7 / 205.6 |
| `auto` | 29.9 Hz, 0.50 | 11.2 | 35 / 267 | 45% | 50.7 / 195.4 |

`auto` runs to both floors. The median frame meets the 33 ms interval, so 30 Hz is what the
frames can hold and a lower floor would slow the median for nothing. Half the pixels each way
buy 12% of the GPU's p50 and 5% of its p95: the frame is the light's and the cells', which the
scale does not reach.

**The light's box room, tried and dropped.** `record::LIGHT_CELLS_MOST`, 4096, is the claim
budget's size, gone on 11-07; `LIGHT_DRAWS` is 65,536 boxes off it, and past them a slot walks.
On the Mac's drive 11 of 71 casts over ground reached it, and those walked 220–290 k slots
against the usual 170 k. Room for every tile, one S23 drive at 719 MHz: GPU p95 205.6 to 220.7,
`light_hit` 79.8 to 82.6. No gain; not kept. The stale number stands, with the node cache's
sizing that reads it.

**What the light's list would hold, from the counts of those casts.** The 71 casts over ground
drew 30–38 k boxes at the median, 65,536 at the room. They asked the world 110–290 k nodes in
the beam, 80 k in the samples and 50–80 k in the holes, up to 2 M, and of 1.9 M slots took 790–
840 k off the raster, walked 146 k and left 890 k as sky. The boxes are what the sun sees, so a
list holds at least that many; the cells it hides are not counted. At the reach of 96 units and
a leaf of 1 unit the read disc is about 29 k columns, the sunward sweep more. That puts a list
near 100 k cells here, a tenth of the 1 M 11-06 assumed. Its time on the S23 is not measured.
