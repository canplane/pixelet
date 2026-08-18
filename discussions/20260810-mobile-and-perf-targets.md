# Mobile strategy and the measured performance bottlenecks

**2026-08-10** — why Capacitor rather than React Native, and the two performance
bottlenecks that were confirmed with live measurement rather than code reading.

---

## Mobile: Capacitor, not React Native

Capacitor wraps the existing Vite + Three.js/React Three Fiber app in a native
WebView shell **as it is** — no rendering-layer rewrite.

React Native was rejected because R3F/WebGL doesn't run natively there. Getting
this app onto it would mean either a WebView bridge — which is what Capacitor
already is — or rewriting the renderer against something like `expo-gl`. Neither
is worth doing to reach the same place.

Two things to handle when this is actually pursued:

- `modules/device/pose/use_geolocation.ts` and `use_device_orientation.ts` call
  raw browser APIs (`navigator.geolocation`, `deviceorientation`) directly. They
  should move to Capacitor's Geolocation/Motion plugins for proper native
  permission dialogs and background behavior. The change is scoped to those two
  files, since both are already encapsulated behind `useGeolocation()` /
  `useDeviceOrientation()`.
- System WebView rendering has a lower performance and battery ceiling than a
  true native renderer. Acceptable at the current casual-voxel visual complexity;
  worth revisiting if geometry gets much heavier.

Note the interaction with [20260708-concurrency-model.md](./20260708-concurrency-model.md):
Capacitor is a plain WebView wrapper, not Electron, which is precisely why
SAB/COOP can't be relied on and the worker model has to stay cooperative.

## The two measured bottlenecks

These were found by measuring, and they superseded an earlier speculative list.
Both were confirmed on an M1 Max — the concern is that mobile SoCs, with much
lower single-core JS throughput and thermal throttling under sustained load, make
both worse rather than better.

### A. Main-thread contention when tiles arrive — still open

A `PerformanceObserver({entryTypes:['longtask']})` (no source changes) was
installed and the camera panned to force fresh tile loads. **Four of five
captured long tasks (57–122 ms each) landed within ~50 ms of a batch of tile
network responses completing**, cross-checked against
`performance.getEntriesByType('resource')` timestamps.

The cause is a chain that runs **synchronously inside React's render/commit
phase**, on the same thread as the rAF render loop: `set_tile` fires →
`layer.tsx`'s `page_to_chunks` (16-chunk slice) → `render/terrain/elevation.ts`'s
`dense_ele` (bicubic, 4096 samples/chunk) → `shader_meta.ts`'s
`decode_ground_rgba` (per-texel color resolve, up to 65536 texels × 4 channels)
→ texture and shader-material creation. React batches multiple same-tick
`set_tile` calls into one commit, so a burst of ~5 simultaneous tile arrivals —
routine during a fast pan — becomes a single large synchronous block. At 60 fps
that is 3–7+ dropped frames.

This is exactly the class of work the abandoned WASM-worker architecture existed
to absorb. The solution space, ranked by scope:

- **Not the answer:** full offscreen rendering (OffscreenCanvas + WASM + the
  renderer itself in a worker). That is the engine-scale rewrite already ruled
  out.
- **The actual answer, far smaller:** `page_to_chunks`, `dense_ele`, and
  `decode_ground_rgba` are **pure functions** — typed arrays in, typed arrays
  out, zero Three.js or DOM dependency. Move only those into a plain Web Worker.
  No OffscreenCanvas, no WASM, no change to Three.js/R3F code. The main thread
  keeps rendering and does the cheap `DataTexture`/`ShaderMaterial` construction
  from results handed back over `postMessage` with transferable `ArrayBuffer`s.
  This is one dedicated long-lived world/terrain worker, not a pool — matching
  the concurrency model already agreed on.
- **WASM inside that worker, only if measurement warrants it.** If it happens,
  batch **per page** (one call in, a whole page's chunks/elevation/colors
  computed inside, one buffer out) — never per chunk or per texel, where FFI
  crossing cost dominates. AssemblyScript is the lower-friction toolchain for
  this particular code, since the sources are already plain numeric TypeScript
  loops over typed arrays, which is AssemblyScript's sweet spot rather than
  Rust + wasm-bindgen's.
- **Cheap interim mitigation, no threading:** spread each page's 16 chunks across
  several frames. Reduces the length of any single task without reducing total
  CPU time.

**Order of operations when this starts:** build the plain-worker split first, then
re-measure with the same longtask-observer technique, and only then consider
WASM. The worker is what gets the work off the main thread; WASM only makes the
work itself faster. Two independent, stackable levers — not one fix.

### B. GPU fill rate scaling with screen area — addressed

A large viewport with only *small* camera movements — no tile loading at all —
still stuttered, while shrinking the window to phone size made it smooth. A
different bottleneck from A: present on every frame, scaling with on-screen pixel
count.

The cause was `dpr={[1,2]}` — genuine spatial supersampling, up to 4× the pixels
on a DPR-2 display — plus a seven-pass full-screen `EffectComposer` stack (N8AO,
Bloom, BrightnessContrast, HueSaturation, TiltShift2, Vignette, SMAA), on top of
the terrain fragment shader's own substantial per-pixel cost. The `dpr` prop's
comment had already flagged this exact risk; this was the first confirmation it
was real.

**The strategy chosen, and since shipped: invert the process.** Rather than
shaving cost off an everything-on baseline, start from a Minecraft-like minimal
baseline and reintroduce selectively. `render_settings_store.ts` now does this —
`low_spec` defaults true, `high_dpr` defaults false (pinning dpr to `[1,1]`), and
every post-stack effect (`show_ao`, `show_bloom`, `show_tilt_shift`, `show_smaa`,
`show_chromatic_aberration`, `show_god_rays`) plus `show_shadow_map` and
`show_blob_shadow` default off. `frameloop="demand"` also landed, so a fully
static scene stops re-rendering.

Note one honest gap in that store: `show_chromatic_aberration` was defaulted off
by the same principle as the rest but, unlike `show_ao`/`show_bloom`/
`show_tilt_shift`/`show_smaa`, was never itself profiled.

## Device reports, 2026-08-17

Two failures observed on real hardware while the UI was being sized. Neither was
profiled — the mechanism in each case is inferred from what changed and what
broke — so both are recorded as leads, not conclusions.

### Galaxy S23 janks where an iPhone 13 mini does not

Same build, same scene. The leading suspect is `backdrop-filter`: every blurred
element over the canvas has to re-read and re-blur its backdrop, and the backdrop
here repaints every frame, so the cost is continuous rather than one-off. WebKit
and Chromium do not pay it the same way, which matches the shape of the report.

`backdrop-blur` was removed from the map's circular controls and the address
pill as a first experiment; the fill went from `/50` to `/70` to keep the icons
legible without it. Three blurred elements remain — the size flyout, the toast,
and `app/styles/index.css`'s glass utility — deliberately, so that if the
symptom is unchanged it is evidence against the hypothesis rather than a partial
fix that proves nothing.

This bears directly on the parked idea of doing tilt-shift as a CSS
`backdrop-filter` over the canvas instead of a WebGL pass. That idea now has
evidence against it from the same surface it would live on.

### iPhone 13 mini reloads itself after wandering

Silent reload with no crash dialog: WebKit discarding the tab under memory
pressure. It appeared immediately after a pass that doubled every view distance.

The chain matters more than the numbers. `MAX_DIST`, fog and `RENDER_RADIUS` are
not independent: the camera sits `MAX_DIST` from its pivot, so fog has to reach
past that or full zoom-out is a screen of solid fog, and geometry has to exist as
far as fog can see. Doubling `MAX_DIST` therefore dragged `FOG_FAR` to 1800 and
`RENDER_RADIUS` to 3 — and `CHUNK_RENDER_DIST` derives from `FOG_FAR`, so the
ground **area** that builds chunk geometry went up as its square: 4x.

What it settled at: distances 2x, fog and `VIEW_DIST` 1.5x, `RENDER_RADIUS` back
at 2. Fog does not have to match `MAX_DIST` one for one — it only has to leave
something visible at full zoom-out, and 480/1350 against a 1024 pivot puts the
ground at ~62% haze there. That buys the full 2x zoom range for 2.25x area
instead of 4x. `RENDER_RADIUS=2` now knowingly breaks its own
`(r-1)*1024 >= FOG_FAR` rule; geometry ends at 1024u where fog is already ~62%
opaque, so the seam sits inside the haze. **If that seam reads on a device,
lower `FOG_FAR` — do not raise `RENDER_RADIUS`, which is the move that failed.**

### A decoded page is 4.01 MiB, always

Measured by fetching a published tile and decoding it with the shared package,
rather than by arithmetic. Four dense 1024x1024 `U8` ground channels plus a
68x68 `I16` elevation grid. **Including the all-ocean tile** — the sparse tree
compresses on the wire (~21 KB average in the bucket, ~195:1) and expands to the
same dense arrays regardless of content, so no page is cheap.

That makes `CLEANUP_RADIUS` a straight memory dial: `(2r+1)^2 * 4.01 MiB`, i.e.
100 MiB at r=2 and 197 MiB at r=3 (its current value). It is a retention cap,
not a working set — `LOAD_RADIUS=1` only prefetches 9 pages; the rest accumulate
by panning.

### Client-side tile caching: mostly redundant

Raised as "should tiles go into localStorage with an LRU?". They should not, and
neither should Cache Storage or OPFS for the same job: tiles are served
`immutable, max-age=31536000`, so the browser's own HTTP cache already answers a
repeat request from disk without revalidating. `localStorage` is additionally
disqualified on its own terms — synchronous, so it blocks the very thread that
is already the bottleneck, and capped around 5–10 MB against tiles that decode
to 4 MiB each.

The one version that is not redundant is persisting the **decoded** structures,
which nothing does today (`tile_map` dies with the tab) and which is where the
expensive work actually is. `IndexedDB` fits that best, not Cache Storage:
structured clone stores typed arrays directly, while a Response-shaped store
would need them serialised and parsed back. It is unusually promising here
because the app opens at the user's real location, so the same pages repeat.

**Gated on A above.** Move the decode off the main thread first, then measure
whether cold start still hurts. In the other order it is impossible to tell
whether the persistence bought anything.

## Superseded: the original speculative list

Kept for the record, since two of three have since been addressed:

1. Shadow mapping on by default — now settings-driven and off by default.
2. `enableHighAccuracy: true` in `use_geolocation.ts`'s `watchPosition`, forcing
   continuous high-power GPS regardless of whether the device is moving. **Still
   open**, and a real battery cost.
3. No `frameloop="demand"` — landed in the 2026-08-09 perf audit.

The reference point for all three was Pokémon GO: adaptive GPS polling tied to
movement state rather than constant max-precision polling; a deliberately
low-detail always-on world with heavy 3D reserved for short bursts rather than a
continuously rendered open world; and sensor pausing on backgrounding, which this
project already does.

## How to use this

Don't implement any of it preemptively — this is a forward-looking analysis, not
a task list. When a real mobile-perf pass starts, A is the lead item. Re-measure
before cutting anything here; the claims came from instrumentation, not from
reading code and guessing.
