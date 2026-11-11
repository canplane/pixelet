# Dawn

Chrome's WebGPU: Dawn, and Tint, which compiles the WGSL to Metal on the Mac and
to SPIR-V for the phone's driver on Android.

- **Three compilers** take the same WGSL ([shaders the phones take](./shaders-phones-take.md)).
- **Synchronous creation holds the GPU process.** Chrome on the Mac runs
  `createShaderModule` and the synchronous `create*Pipeline` on its GPU
  process's main thread (Tint to MSL, then the Metal function). It was held
  878 ms, and the compositor's swaps waited on it: the display froze up to
  432 ms. The page's main thread was not held. Asynchronous pipelines go to
  Chrome's thread pool.
  ([pipelines are made by ticket](../minutes/20261102-pipelines-are-made-by-ticket-before-the-first-frame.md):31-48, 59-62)
- **A pass that writes only its end timestamp** loses its indirect draws,
  silently, on the S23's Chrome 157.0.8103.50; Mac Chrome and the S23 app keep
  them. Every pass writes both of its timestamps.
  ([the lights' tail](../../experiment/20261107-the-lights-tail/README.md):80, 170-228, 276-309, 431-432;
  [ray, raster, ray](../../minutes/20261108-ray-raster-ray.md):152-175)
- **Pipelines on the S23's Chrome.** 87–91 pipelines take about 15 s, and a
  reload is no faster. The slowest finish at 12–16 s, compiled side by side: where
  a module finishes is its place in the queue, not its own cost. The first
  load is 19–29 s; one minute still owes a "90 s"
  ([the shaders by hand held to the engine](../minutes/20261105-the-shaders-by-hand-held-to-the-engine.md):97),
  which came of the data format and the harness.
  ([the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):139-140, 188-191, 805-829;
  [the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):1065-1066, 1505-1522;
  [the walk on a phone](../../minutes/20261103-the-walk-on-a-phone-and-who-owns-it.md):75-76)
- **Less scratch under Chrome.** On the same WGSL, Chrome holds Adreno's scratch
  block in 6 of 17 pipelines (72 MiB against the app's 204). Why is not
  determined; Tint's output was not read.
  ([one command buffer an encoder](../experiment/20261110-one-command-buffer-an-encoder/README.md):163;
  [its scratch notes](../experiment/20261110-one-command-buffer-an-encoder/report/pipeline-scratch-notes.md):2197-2217)
- **`f32::MAX`** printed by naga is a literal Tint reads as past f32
  ([WGSL](./wgsl.md)).
- **Timestamps on the Mac** step by about 0.066 ms. A pass with no work may be
  dropped, and its query slots keep old ticks.
  ([the tiles baseline](../../experiment/20261030-the-tiles-baseline.md):531, 579-584, 712-730)
- **A window behind others.** Chrome stops rAF for it, and a background tab is
  throttled.
  ([the view is cubes about the eye](../../minutes/archive/20260926-the-view-is-cubes-about-the-eye.md):176-177;
  [the safety valves](../../minutes/archive/20260914-the-safety-valves-the-module-did-not-have.md):72-73)
