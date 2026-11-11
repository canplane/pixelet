# Measure

Timing a GPU frame and running a measurement, on every platform the app runs on.

## Timestamps

- **Only at a pass's ends.** A draw group inside a pass cannot be timed alone.
  ([the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):1262-1264)
- **A render pass's span overlaps.** On the Apple GPU through Dawn, a render
  pass's span includes the work it overlaps; a compute pass's span is its own
  cost. Summed spans overstate the frame (31 ms against 24 busy).
  ([the air waits](../../experiment/20261026-the-air-waits.md):47-59;
  [what the clouds cost](../../experiment/20261026-what-the-clouds-cost.md):11-14;
  [the tiles baseline](../../experiment/20261030-the-tiles-baseline.md):535-537, 613-616, 700-730, 901-934)
- **A frame's GPU time is the union of its spans** (`gpu_busy_ms`); first start
  to last end read tens of seconds. A span over several passes is the sum of
  each pass's own begin and end.
  ([the tiles baseline](../../experiment/20261030-the-tiles-baseline.md):712-730;
  [the lights' tail](../../experiment/20261107-the-lights-tail/README.md):294-295, 316-317)
- **An asynchronous query** lands on whichever frame drains it: read the
  distribution, not one frame.
  ([where the allocation goes](../../minutes/archive/20260910-where-the-allocation-goes.md):576-578)
- **Chrome's and Safari's own timestamp traits** are in [Dawn](./dawn.md) and
  [WebKit](./webkit.md).

## Reading a run

- **Not every frame is timed.** A report times 50–70% of its frames, and a window
  with no GPU read is no measurement. Frames counted in fps are in
  [a device is not a page](../../lesson/a-device-is-not-a-page.md).
  ([the tiles baseline](../../experiment/20261030-the-tiles-baseline.md):499-500, 570-572;
  [the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):170-171, 1360-1363;
  [the app that did not move](../../experiment/20261109-the-app-that-did-not-move/README.md):37-45;
  [one command buffer an encoder](../experiment/20261110-one-command-buffer-an-encoder/README.md):228-231)
- **GPU busy against the frame's interval** shows what sets the pace; when the
  GPU's queue backs up, it waits whole vsyncs.
  ([cooperative scheduler](../../minutes/archive/20260909-cooperative-scheduler-and-task-ledger.md):843-845;
  [decor identity](../../minutes/archive/20260911-decor-identity-orientation-and-one-cell-format.md):249-256;
  [the tiles baseline](../../experiment/20261030-the-tiles-baseline.md):726-728;
  [the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):250-253, 1393-1397)
- **The S23's comparability.** `refresh` reads 8.3 ms in the app against 16.7 in
  Chrome, and `resolution: fixed` does not give the same pixels: compare per Mpx.
  ([one command buffer an encoder](../experiment/20261110-one-command-buffer-an-encoder/README.md):109-112, 225-227)

## Running one

- **Runs drift.** Within a sitting, fps drifted about 20% with shadows off at
  thermal status 0, and another session read the S23's GPU p50 from 51 to 127 ms,
  about 150%; the first run of a pair is faster: run ABBA and drop the first. One drive is not read against another
  (byte-identical modules read 21.8 and 8.0 fps); a synthetic timing shows a
  shape, not a cost; load to load differs about 3 ms at p95 on the desktop.
  ([the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):186-203;
  [the S23 tool](../experiment/20261103-the-frame-on-the-s23/tool/s23/README.md):54-59;
  [the lights' tail](../../experiment/20261107-the-lights-tail/README.md):132-134, 311-314;
  [the pixels check on the S23](../../experiment/20261109-the-pixels-check-on-the-s23.md):20-25, 35-37;
  [the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):746-759, 789-798, 909-929;
  [where decor grows](../../experiment/20261027-where-decor-grows.md):25;
  [a run walks on what it parked](../../experiment/20261109-a-run-walks-on-what-it-parked.md):34-37)
- **The harness can be the load.** A 25 MB readback stalls the next frame; 32
  open tabs made the phone read low; an attached debugger took a third of the
  window; another GPU user or a busy machine spoils runs, and 3–4 jobs at once
  turned 56 of 68 mutation survivors into false kills.
  ([the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):430-434, 838-840, 931-937;
  [where the allocation goes](../../minutes/archive/20260910-where-the-allocation-goes.md):222-225;
  [the air waits](../../experiment/20261026-the-air-waits.md):21-22;
  [tauri and wgpu on the page](../minutes/20261110-tauri-and-wgpu-on-the-page.md):43, 47-48)
- **An Android system trace** and **the iPhone's memory** are in
  [a device is not a page](../../lesson/a-device-is-not-a-page.md); reading a
  Metal System Trace is in [the GPU is shared](./the-gpu-is-shared.md).
- **Tool traps.** On iOS `CLOCK_MONOTONIC_RAW` counts sleep, and
  `CLOCK_UPTIME_RAW` matches `performance.now()`. A locked phone draws and
  reports nothing. DevTools' metrics override changes the pixel ratio, not the
  canvas. A proxy over the page's WebGPU misses a device made in a worker. A
  probe must create every entry point of a module. Kernel errors reach the dev
  console, not the page's.
  ([the app that did not move](../../experiment/20261109-the-app-that-did-not-move/README.md):104-109;
  [the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):140-141, 167-169, 1475-1476, 1524-1526;
  [the lights' tail](../../experiment/20261107-the-lights-tail/README.md):286-287;
  [a ray from the box](../../experiment/20261026-a-ray-from-the-box.md):66-67)
