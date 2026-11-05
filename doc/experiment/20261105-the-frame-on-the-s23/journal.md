# Journal

The S23's frame after the shaders by hand, made cheaper where the device showed the
cost: what was tried, what came of it, and how we know, newest last. Main at
a449637a35 to start. S23 figures are Chrome on the S23 (Adreno 740), ms p50/p95,
fresh loads alternated base and change in one sitting, two loads a side unless it
says; Safari is Safari 27.0.1 on the Mac. From the visit A/B on, a load is timed
only once the ground's textures have landed (whether they had was a confound before).

## Where it started

- **Main a449637a35 on the S23**, shadows off: fps 35–40 (rust-gpu's 11-03: 5.9), GPU
  p50 21–24 ms, `sample` p95 5.6–7.0 (133). Three runs in a row, the phone 43 to 48 °C
  and fps drifting down: a reference, not a fixed number.

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
  80–88, an actor walking under the circle. Replaces the 11-02 minute by the user's
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

## Did not work

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

## Unsettled

- **The asking walks' resume as committed** (without the blind restart) is measured
  on test worlds (beam's visits main's) but not yet on the S23.
- **Cold miss spikes after a view jump** (74, 63, 46 ms in the first casts): the
  resume should cut the 11–28 nodes a pixel; a fresh-load drive on main is owed.
- **`light_receivers`** 6.9–8.1 ms every frame with an actor walking, the largest
  light cost now; receivers.patch is being measured.
- **Lighting not run under decor** (a depth-only screen draw, decor clearing a
  stencil bit): not built. The 8 stencil bits are the gate's numbers, so a coverage
  bit touches every world-pass pipeline, for an unmeasured 0.3–0.8 ms; the share of
  screen pixels decor covers is to be measured first.
- **Safari over http://localhost** asks no zstd and gets the 1 MB tile form; https
  takes zstd since Safari 26.3. Left to time.
- **plei.me** answers Korea from LAX (60–110 KB/s); left as is.
