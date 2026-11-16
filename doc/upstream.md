# Upstream defects

The defects found in what pixelet builds on and does not own: the browsers'
WebGPU, the GPUs' drivers, wgpu, the Rust toolchain and the libraries the app
takes. It gathers them in one place, grouped by upstream project; it is a
collection, not a queue of reports to file. An entry goes when the defect is
found fixed upstream, or found to be pixelet's own. What the records concluded
was pixelet's fault is not listed.

Each entry says what upstream and which part of it, the symptom, the date it was
found and what it was seen on, the records it rests on, what the tree does about
it now, and what a later record says of it. **Reported** appears only where a
record links an issue. What the records did not establish is marked
**unconfirmed**. A record is cited as the lessons cite one, its lines after the
link.

## WebKit (Safari's WebGPU)

### `atomicCompareExchangeWeak` writes Metal that does not compile

- **Upstream:** WebKit, the WGSL compiler (WGSL to Metal).
- **Symptom:** any shader that calls `atomicCompareExchangeWeak` fails to
  compile: the helper WebKit writes takes its result type from `decltype` of a
  by-value parameter, and Metal refuses the `thread`-qualified field
  (`field may not be qualified with an address space`).
- **Found:** 2026-11-03. **Seen on:** Safari 27.0.1 on an iPhone 13 mini, and
  Safari on the Mac (an M1 Max); the S23's Chrome takes the same module.
- **Evidence:** [the shaders by hand](./gpu/experiment/20261103-the-shaders-by-hand/README.md):142-162,
  with an 11-line repro, [webkit-cas.wgsl](./gpu/experiment/20261103-the-shaders-by-hand/repro/webkit-cas.wgsl),
  and a report drafted there and not sent (240-255);
  [shaders the phones take](./gpu/lesson/shaders-phones-take.md).
- **In the tree:** no compare-and-swap: a walk claims a slot with an exchange
  ([ask.wgsl](../rust/client/render/src/frame/shader/ask.wgsl)), and
  `refuse_faults` in [check.rs](../rust/client/render/build/check.rs):90-99
  refuses a module that holds the call.

### `i32::MIN` is written as a C++ `long`

- **Upstream:** WebKit, the WGSL compiler.
- **Symptom:** `i32::MIN`, written as a constant or folded into one, reaches
  Metal as a bare literal, a `long`, and a `select` of it and an `int` fails to
  compile; the page stood at its loading.
- **Found:** 2026-11-03, and again 2026-11-05 (`way_tree_cs` and `bound_cs`).
  **Seen on:** Safari 27.0.1, the Mac and the iPhone.
- **Evidence:** [the shaders by hand](./gpu/experiment/20261103-the-shaders-by-hand/README.md):163-164;
  [the frame on the S23](./gpu/experiment/20261103-the-frame-on-the-s23/README.md):1426-1429;
  [shaders the phones take](./gpu/lesson/shaders-phones-take.md). No reduced
  repro is kept.
- **In the tree:** a sentinel takes the end of its own range instead, and
  `refuse_faults` ([check.rs](../rust/client/render/build/check.rs):90-99)
  refuses a module that holds `i32::MIN`.

### A struct built back from a block's words loses the GPU process

- **Upstream:** WebKit, the WGSL compiler or the GPU process (which is not
  established).
- **Symptom:** a shader that builds a whole uniform struct back from its words
  (`fn frame() -> Frame`) made Safari's GPU process crash on the terrain's
  pipeline: every tab's device was lost, and for a while no adapter answered.
  The same block read field by field compiled.
- **Found:** 2026-09-27. **Seen on:** Safari on a Mac and on an iPhone; the
  Safari versions are not recorded (**unconfirmed**; the record of 2026-09-23
  names Safari 26.1 on the iPhone and 26.6.1 on the Mac).
- **Evidence:** [a matrix the driver reads its own way](./gpu/minutes/archive/20260927-a-matrix-the-driver-reads-its-own-way.md):122-131;
  [shaders the phones take](./gpu/lesson/shaders-phones-take.md). No reduced
  repro is kept.
- **In the tree:** blocks are bound as `array<vec4f, N>` and each field is read
  by an accessor of its own, which build.rs writes from word.rs; the comment in
  [skinned_place.wgsl](../rust/client/render/src/frame/shader/skinned_place.wgsl):8-10
  names the crash.

### Indirect draws through `clampIndirectBufferToValidValues` hang the GPU (bug 321876)

- **Upstream:** WebKit, WebGPU on Metal.
- **Symptom:** in one render pass, indirect draws from one arguments buffer
  through a pipeline with a vertex buffer layout all go through one scratch
  slot, and the GPU hangs past the browser's end until a reboot. Whether this
  Mac carries it, and whether the Mac Safari stall was this, is
  **unconfirmed**: the report saw it on M3 and later, and this is an M1 Max.
- **Found:** met on 2026-11-12, while chasing the Mac Safari stall. **Seen on:**
  the report's M3 Max; the stall was on Safari 27.0.1 (22625.1.29.11.28),
  macOS 27.0.1, an M1 Max.
- **Evidence:** [the GPU is shared](./gpu/lesson/the-gpu-is-shared.md);
  [the Safari stall](./gpu/experiment/20261111-the-safari-stall/README.md):95-97.
  decor draws in this shape (`MODEL_BUFS` in record.rs, pass/decor.rs).
- **In the tree:** nothing, on the user's decision
  ([the GPU's other users are not the app's](./gpu/minutes/20261112-the-gpus-other-users-are-not-the-apps.md):15-16).
- **Reported:** https://bugs.webkit.org/show_bug.cgi?id=321876, by others.
- **Since:** marked fixed upstream (319317@main, WebKit PR 71794); whether a
  Safari release carries the fix is not recorded.

### An indirect call reading args written in its own command buffer hangs the GPU

- **Upstream:** WebKit, WebGPU on Metal, or Metal itself (suspected).
- **Symptom:** at Safari's far zoom one submit never finished: in flight stuck
  at 1, the GPU at 100%, the GPU process uninterruptible, the canvas off;
  force-quitting Safari freed the GPU. The suspect is one large command buffer
  stalling in WebKit or Metal, not a shader that never ends. **Unconfirmed:**
  it rests on two runs against one, and a split run could be luck; a run of the
  frame in 4–6 submits waited for a force-quit, so whether splitting stops the
  hang is untested.
- **Found:** 2026-11-05 to 2026-11-06. **Seen on:** Safari on the Mac (an
  M1 Max); the Safari version is not given in that passage.
- **Evidence:** [the frame on the S23](./gpu/experiment/20261103-the-frame-on-the-s23/README.md):1455-1469;
  [the frame is what it must draw](./minutes/20261106-the-frame-is-what-it-must-draw.md):184.
- **In the tree:** nothing. The renderer cut an encoder's work so that no
  indirect call read args written earlier in its command buffer (`split.rs`,
  `b8bd0993f0` on `original`); the cut went on 2026-11-16, as the user had
  decided against a workaround for WebKit's hang and that runs which did not
  stall prove nothing
  ([the GPU's other users are not the app's](./gpu/minutes/20261112-the-gpus-other-users-are-not-the-apps.md)).
  Each encoder is one command buffer again.

### `writeBuffer` blocks the page for 15 s behind a busy GPU

- **Upstream:** WebKit, the WebContent process's call into the GPU process.
- **Symptom:** each `writeBuffer` was a synchronous call into the GPU process,
  and behind a busy GPU it blocked for `gpuProcessTimeoutDuration`, 15 s
  (`[gpu] op 13 took 15002 ms`). Whether this is a defect or WebKit's design is
  **unconfirmed**.
- **Found:** 2026-11-11. **Seen on:** Safari 27.0.1, macOS 27.0.1, an M1 Max.
- **Evidence:** [the Safari stall](./gpu/experiment/20261111-the-safari-stall/README.md):10-14, 81-82;
  [the GPU is shared](./gpu/lesson/the-gpu-is-shared.md).
- **In the tree:** the page stages its writes on a belt, as a device does
  ([belt.ts](../ts/gpu/host/src/belt.ts)).

### WebKit processes left exiting hold the GPU until a reboot

- **Upstream:** WebKit, or macOS (which is not established).
- **Symptom:** ten `com.apple.WebKit` processes left by earlier runs stood
  exiting with no memory; the GPU read 100% while the live processes' GPU time
  summed to 28%, and only a reboot cleared them. Earlier, quitting Safari alone
  freed a GPU held at 100%. **Unconfirmed:** read in the session, in no run's
  log.
- **Found:** 2026-11-11 (and the GPU held after runs' pages, 2026-11-05).
  **Seen on:** Safari 27.0.1, macOS 27.0.1, an M1 Max.
- **Evidence:** [the Safari stall](./gpu/experiment/20261111-the-safari-stall/README.md):69-73;
  [the frame on the S23](./gpu/experiment/20261103-the-frame-on-the-s23/README.md):1434-1437;
  [WebKit](./gpu/lesson/webkit.md).
- **In the tree:** nothing; the repository's tools hold the device they drive,
  one user at a time (`script/perf/measure.py` `hold`).

## Apple (macOS, Metal)

### A GPU job beside Safari froze the whole Mac

- **Upstream:** Apple, macOS's GPU driver (AGX) or WindowServer (which is not
  established).
- **Symptom:** pxl-render's `decor` GPU test, looped without pause beside a
  Safari run, froze the whole Mac on its second run: the screen captures timed
  out from about 145 s, and the Mac took a forced power-off with no panic
  report (`Boot faults: btn_rst,finger_reset force_off`). Whose job held the GPU
  is **unconfirmed**.
- **Found:** 2026-11-12. **Seen on:** macOS 27.0.1, Safari 27.0.1, an M1 Max.
- **Evidence:** [the Safari stall](./gpu/experiment/20261111-the-safari-stall/README.md):30-31, 64-67;
  [the GPU is shared](./gpu/lesson/the-gpu-is-shared.md).
- **In the tree:** nothing; the tools hold the device one user at a time.

## Chrome (Dawn and Tint)

### A render pass that writes only its end timestamp loses its indirect draws

- **Upstream:** Chrome on Android: Dawn, Tint or the Adreno driver, not told
  apart.
- **Symptom:** a render pass whose timestamp writes hold an end and no begin
  draws nothing from its indirect calls, with no console error; its direct
  draws and a pass with both timestamps are kept.
- **Found:** 2026-11-10 to 2026-11-11. **Seen on:** a Galaxy S23 (Adreno 740),
  Chrome 157.0.8103.50; Chrome 157.0.8103.39 on the Mac and the S23's app
  (wgpu on Vulkan) keep the draws.
- **Evidence:** [the light's tail](./experiment/20261107-the-lights-tail/README.md):80, 170-228, 276-287, 431-432;
  [ray, raster, ray](./minutes/20261108-ray-raster-ray.md):152-175;
  [Dawn](./gpu/lesson/dawn.md). No reduced repro is kept.
- **In the tree:** every pass writes both of its timestamps (`Stamp` in
  [api.rs](../rust/client/render/src/gpu/api.rs):93-95).

### Synchronous pipeline creation holds the GPU process's main thread

- **Upstream:** Chrome, Dawn's GPU process on the Mac.
- **Symptom:** `createShaderModule` and the synchronous `create*Pipeline` run
  Tint and the Metal compile on the GPU process's main thread: 878 ms held, the
  compositor's swaps waiting on it, the display frozen for up to 432 ms.
  Whether this is a defect or Chrome's design is **unconfirmed**.
- **Found:** 2026-11-02. **Seen on:** Chrome 156 on the Mac (Apple Metal).
- **Evidence:** [pipelines are made by ticket](./gpu/minutes/20261102-pipelines-are-made-by-ticket-before-the-first-frame.md):31-48, 59-62;
  [Dawn](./gpu/lesson/dawn.md).
- **In the tree:** pipelines are made asynchronously, by ticket, before the
  first frame (the same minutes).

### A pass with no work drops its timestamps and keeps old ticks

- **Upstream:** Chrome, Dawn's timestamp queries on the Mac.
- **Symptom:** a pass with no dispatch may be dropped, and the query slots it
  was to write keep earlier ticks, so a span reads an old time. **Unconfirmed**
  as a defect: the record says only that the browser may drop such a pass.
- **Found:** 2026-10-30. **Seen on:** Chrome on the Mac; the version is not
  recorded.
- **Evidence:** [the tiles baseline](./experiment/20261030-the-tiles-baseline.md):579-584, 712-730;
  [Dawn](./gpu/lesson/dawn.md).
- **In the tree:** nothing aimed at it.

### An invalid indirect and writable usage passes without error on the Mac

- **Upstream:** Chrome, Dawn's validation on the Mac.
- **Symptom:** a buffer used as Indirect and as writable Storage in one usage
  scope invalidated every frame's submit on the S23's Chrome (7,490 error
  pairs in 263 s); the same page in Mac Chrome showed no validation error.
  **Unconfirmed:** the record does not say that the offending pass ran on the
  Mac. The invalid usage itself was pixelet's, and is fixed (`034ddad063`).
- **Found:** 2026-11-07. **Seen on:** the S23's Chrome against Chrome on the
  Mac; the versions are not given in that passage.
- **Evidence:** [the frame on the S23](./gpu/experiment/20261103-the-frame-on-the-s23/README.md):1648-1658;
  [WebGPU](./gpu/lesson/webgpu.md).
- **In the tree:** nothing; an indirect call whose args are bound writable is
  refused as it is encoded.

## Qualcomm (the Adreno Vulkan driver)

### Valid modules refused with `VK_ERROR_UNKNOWN`

- **Upstream:** Qualcomm, the Adreno 740's Vulkan driver, reached through
  Chrome (Tint to SPIR-V); Dawn could work around it.
- **Symptom:** `vkCreateComputePipelines` and `vkCreateGraphicsPipelines` fail
  with `VK_ERROR_UNKNOWN`, sync or async, for valid WGSL that Tint takes: an
  array- or struct-typed local assigned in a branch and read after it about
  loops, a function returning an array or a struct holding one, a struct
  holding a struct, a constant table of vectors indexed at run time, and one
  module (`decor`) refused for its size or load alone.
- **Found:** 2026-11-03. **Seen on:** a Galaxy S23, SM-S911N (Adreno 740,
  `adreno-7xx`), Chrome 156.0.8078.73 on Android.
- **Evidence:** [the shaders by hand](./gpu/experiment/20261103-the-shaders-by-hand/README.md):89-140,
  with a 174-line repro, [adreno-refused.wgsl](./gpu/experiment/20261103-the-shaders-by-hand/repro/adreno-refused.wgsl),
  and its taken twin, [adreno-taken.wgsl](./gpu/experiment/20261103-the-shaders-by-hand/repro/adreno-taken.wgsl);
  a report drafted there and not sent (215-238);
  [shaders the phones take](./gpu/lesson/shaders-phones-take.md);
  [Vulkan](./gpu/lesson/vulkan.md).
- **In the tree:** the hand WGSL keeps clear of each shape, and `refuse_faults`
  ([check.rs](../rust/client/render/build/check.rs):90-99) refuses a struct
  that holds a struct and a struct- or array-typed local; the raster's corner
  table is packed into words (`raster_box` in
  [shared.rs](../rust/client/render/build/shared.rs):928-935); decor is split
  (decor_pose.wgsl).

### A `vec3f` then an `f32` in a storage array read one slot along

- **Upstream:** Qualcomm, the Adreno Vulkan driver (the layout's decorations
  from Tint, read back by the driver).
- **Symptom:** in a struct inside a `var<storage>` array, the float after a
  `vec3f` is read at byte 16 where WGSL puts it at 12, and every field after it
  one slot along; the terrain did not draw, with no error. The same struct
  read right as a uniform and on every Metal device.
- **Found:** 2026-09-23. **Seen on:** a Galaxy S23 (Adreno 7xx), Chrome 154;
  Safari 26.1 on an iPhone 13 mini, an iPad, and Safari 26.6.1 and Chrome on a
  Mac drew it.
- **Evidence:** [the ground one device does not draw](./gpu/minutes/archive/20260923-the-ground-one-device-does-not-draw.md):1-60;
  [shaders the phones take](./gpu/lesson/shaders-phones-take.md);
  [Vulkan](./gpu/lesson/vulkan.md).
- **In the tree:** nothing shared with the host has room inside it;
  `has_no_room` in [check.rs](../rust/client/render/build/check.rs):60-66
  refuses a binding that does.

### A storage `array<mat4x4f>` read with another stride

- **Upstream:** Qualcomm, the Adreno Vulkan driver, through naga's SPIR-V
  (whether naga's decorations or the driver's reading is wrong was not read;
  **unconfirmed**).
- **Symptom:** the joint palette, declared `array<mat4x4f>` in storage, was read
  as another layout: every skinned character lay on the ground, with no error.
  Read as `array<vec4f>` and assembled column by column, it drew right.
- **Found:** 2026-09-27. **Seen on:** a Galaxy S23 (Adreno 740), One UI 8.5,
  the device app (wgpu on Vulkan); the iPhone app and Chrome on a Mac drew it.
  Chrome on the S23 was not tried.
- **Evidence:** [a matrix the driver reads its own way](./gpu/minutes/archive/20260927-a-matrix-the-driver-reads-its-own-way.md):1-74,
  which cites gfx-rs/wgpu#914 and baldurk/renderdoc#2378 as reports of the same
  class on other hardware.
- **In the tree:** the palette is `array<vec4f>`
  ([skinned_place.wgsl](../rust/client/render/src/frame/shader/skinned_place.wgsl):13-19),
  and `has_no_room` refuses a matrix in a binding.

### Atomic loads with many live words corrupt a buffer

- **Upstream:** Qualcomm, the Adreno Vulkan driver.
- **Symptom:** a pass that only loads, every load atomic on a read-write
  binding with 250 or more locals live, left 279 of 6.3 million words changed;
  from that pass on every pass in that device answered as if the buffer were
  empty, with no error and no device loss, until a new device.
- **Found:** 2026-11-03. **Seen on:** a Galaxy S23 (Adreno 740), Chrome 156;
  the Mac's table was untouched.
- **Evidence:** [the frame on the S23](./gpu/experiment/20261103-the-frame-on-the-s23/README.md):361-406,
  six runs; [Vulkan](./gpu/lesson/vulkan.md). No reduced repro outside the
  record's tools.
- **In the tree:** nothing aimed at it; the record found no sign that the page
  suffers it.

### A pipeline computes something else after an offset moves

- **Upstream:** Qualcomm, the Adreno Vulkan driver (**unconfirmed**: not
  reduced, and the record does not rule out the shaders).
- **Symptom:** placing one list before the misses moved every offset after it;
  the S23's light beam then walked 0 nodes a cast against the Mac's 118k, with
  no error. With the list at the end it walked 148,815.
- **Found:** 2026-11-03 to 2026-11-05. **Seen on:** a Galaxy S23, Chrome 156,
  under rust-gpu's generated shaders.
- **Evidence:** [the shaders by hand](./gpu/experiment/20261103-the-shaders-by-hand/README.md):198-209, 1117-1118.
- **In the tree:** nothing; the generated shaders are gone.

### Each compute pipeline keeps its own scratch block

- **Upstream:** Qualcomm, the Adreno Vulkan driver.
- **Symptom:** every compute pipeline that uses scratch holds its own 12 MiB
  block for its life, where Mesa's turnip pools them: 17 pipelines held 204 MiB.
  It is a cost, not a wrong answer.
- **Found:** 2026-11-10. **Seen on:** a Galaxy S23 (Adreno 740), the app (wgpu
  on Vulkan); Chrome held it in 6 of 17 pipelines.
- **Evidence:** [one command buffer an encoder](./gpu/experiment/20261110-one-command-buffer-an-encoder/README.md):102-170;
  [its scratch notes](./gpu/experiment/20261110-one-command-buffer-an-encoder/report/pipeline-scratch-notes.md);
  [Vulkan](./gpu/lesson/vulkan.md).
- **In the tree:** modules that differ only in values are merged.

## wgpu and naga

### `break if` in `continuing` ends a loop one iteration early

- **Upstream:** wgpu, naga's Metal backend.
- **Symptom:** in `loop { … continuing { break if cond; } }`, naga writes a
  `let` of the body that the `break if` evaluates after the counter has moved,
  so the loop ends one iteration early.
- **Found:** 2026-10-14. **Seen on:** the native path on Apple GPUs; wgpu
  30.0.1, and the fork on 30.0.2.
- **Evidence:** [the GPU casts a ray a pixel](./minutes/20261014-the-gpu-casts-a-ray-a-pixel.md):86-90;
  [naga on Metal](./gpu/lesson/naga-on-metal.md).
- **In the tree:** no hand WGSL has a `continuing` block or a `break if`; the
  fork does not carry the fix ([wgpu-fork.md](../rust/gpu/wgpu-fork.md)).
- **Reported:** https://github.com/gfx-rs/wgpu/issues/4558; fixed by
  https://github.com/gfx-rs/wgpu/pull/10459 on wgpu's trunk, in no release as
  of the lesson.

### One command buffer a pass

- **Upstream:** wgpu, wgpu-core's pass encoding.
- **Symptom:** each pass is a hal command buffer of its own, with another for
  its barriers. On Metal the driver holds about 2.3 MiB for each until it
  completes, and the iPhone app died at boot at its memory limit; on Vulkan
  each pool keeps the memory of its heaviest frame (the S23's `Graphics` at
  1.38 GiB).
- **Found:** 2026-11-09 to 2026-11-10. **Seen on:** wgpu-core 30.0.1, an iPhone
  13 mini and a Galaxy S23.
- **Evidence:** [the patch that fixed one phone](./gpu/minutes/20261110-the-patch-that-fixed-one-phone.md):25-60, 100-128;
  [one command buffer an encoder](./gpu/experiment/20261110-one-command-buffer-an-encoder/README.md):1-30;
  [a device is not a page](./lesson/a-device-is-not-a-page.md).
- **In the tree:** a fork commit records a pass and replays it into the
  encoder's open buffer ([wgpu-fork.md](../rust/gpu/wgpu-fork.md), the
  prototype attached to #5738).
- **Reported:** https://github.com/gfx-rs/wgpu/issues/5738,
  https://github.com/gfx-rs/wgpu/issues/10494,
  https://github.com/gfx-rs/wgpu/issues/10506, and femtovg#367.

### The iOS Simulator's Metal capabilities

- **Upstream:** wgpu, wgpu-hal's Metal adapter.
- **Symptom:** the Simulator reports the Apple2 family yet runs indirect calls
  on the Mac's GPU, which `supportsFamily` cannot say, and needs 256-byte
  constant buffer offsets.
- **Found:** before 2026-11-09 (the date is not recorded). **Seen on:** the
  iOS Simulator on an Apple silicon Mac.
- **Evidence:** [a device is not a page](./lesson/a-device-is-not-a-page.md);
  [wgpu-fork.md](../rust/gpu/wgpu-fork.md).
- **In the tree:** a fork commit decides both by `target_abi = "sim"` in
  `wgpu-hal/src/metal/adapter.rs`.
- **Reported:** https://github.com/gfx-rs/wgpu/issues/10189.

### No pipeline cache on Metal

- **Upstream:** wgpu, wgpu-hal's Metal backend. A missing feature, not a wrong
  answer.
- **Symptom:** wgpu keeps a pipeline cache on Vulkan alone; on Metal its cache
  is empty.
- **Found:** by 2026-11-16 (the date is not recorded). **Seen on:** wgpu 30.0.2.
- **Evidence:** [wgpu-fork.md](../rust/gpu/wgpu-fork.md).
- **In the tree:** a fork commit makes a Metal cache a binary archive
  (`MTLBinaryArchive`), the design proposed on the issue.
- **Reported:** https://github.com/gfx-rs/wgpu/issues/8526.

### An aborted command buffer reads as completed

- **Upstream:** wgpu, wgpu-hal's Metal fence.
- **Symptom:** `Fence::get_latest` takes a command buffer at
  `MTLCommandBufferStatus::Error` as completed and reports nothing, so a
  dispatch can stop partway with no error. **Unconfirmed:** which load makes
  Metal abort was not reproduced.
- **Found:** 2026-11-04 to 2026-11-05. **Seen on:** wgpu-hal 30, an M1 Max,
  under contention from parallel test runs.
- **Evidence:** [the shaders by hand](./gpu/experiment/20261103-the-shaders-by-hand/README.md):688-703, 1134-1135;
  [wgpu](./gpu/lesson/wgpu.md).
- **In the tree:** a test dispatch clears a word in the same command buffer
  after its pass and fails where the word stands
  ([tests/gpu/mod.rs](../rust/client/render/tests/gpu/mod.rs):150-152, 183).

### A wait on a hung shader never returns

- **Upstream:** wgpu, `Device::poll` on Metal.
- **Symptom:** `device.poll(wait_indefinitely)` on a shader that does not end
  sat 3 h 42 min in `wgpu_hal::metal::Device::wait` at 0% CPU. Whether a bound
  is wgpu's to set is **unconfirmed**.
- **Found:** 2026-11-04. **Seen on:** wgpu 30, an M1 Max.
- **Evidence:** [the shaders by hand](./gpu/experiment/20261103-the-shaders-by-hand/README.md):1010-1013;
  [wgpu](./gpu/lesson/wgpu.md).
- **In the tree:** the GPU tests poll with a timeout and run under a time limit
  ([tests/gpu/mod.rs](../rust/client/render/tests/gpu/mod.rs):492).

### Metal's fast math cannot be turned off

- **Upstream:** wgpu, wgpu-hal's Metal backend. A missing option, not a wrong
  answer.
- **Symptom:** wgpu-hal compiles Metal with fast math on (`metal/device.rs:272`
  in 30), which reorders and simplifies sums; with every product exact and one
  order written on both sides, 28–981 of 65,536 light-ray starts still parted
  from the CPU's. No flag turns it off.
- **Found:** 2026-11-04 to 2026-11-05. **Seen on:** wgpu-hal 30, upstream's and
  the fork's.
- **Evidence:** [the shaders by hand](./gpu/experiment/20261103-the-shaders-by-hand/README.md):383-388;
  [when the CPU and a GPU must give one answer](./gpu/lesson/cpu-and-gpu-agree.md).
- **In the tree:** what must agree is computed in integers.

### `f32::MAX` printed as a literal Tint reads as past f32's range

- **Upstream:** naga's WGSL writer or Tint's literal parsing; which one is
  wrong was not settled (**unconfirmed**).
- **Symptom:** naga prints `f32::MAX` as a literal that Tint refuses as past
  f32's range, and the frame failed silently.
- **Found:** 2026-10-26 to 2026-10-28. **Seen on:** Chrome on the Mac, under
  rust-gpu's generated shaders; the versions are not recorded.
- **Evidence:** [the cone touches](./experiment/20261028-the-cone-touches.md):87-93;
  [a ray from the box](./experiment/20261026-a-ray-from-the-box.md):64-65;
  [WGSL](./gpu/lesson/wgsl.md).
- **In the tree:** every float the build writes is finite
  ([shared.rs](../rust/client/render/build/shared.rs):935), and
  [shader.test.ts](../ts/tool/shader/shader.test.ts):3-13 compiles every module
  with Tint.

### naga's SPIR-V input made a pointer-typed local and refused it

- **Upstream:** naga, its SPIR-V front end.
- **Symptom:** from rust-gpu's SPIR-V, a per-level array read and written
  across branches became a pointer-typed local, which naga's own validator
  refused (`LocalVariable InvalidType`).
- **Found:** 2026-10-28. **Seen on:** naga of that date (the version is not
  recorded).
- **Evidence:** [the cone touches](./experiment/20261028-the-cone-touches.md):87-90;
  [WGSL](./gpu/lesson/wgsl.md).
- **In the tree:** nothing; rust-gpu and the SPIR-V path went on 2026-11-05.

### An atomic in a branch of its own

- **Upstream:** naga.
- **Symptom:** naga's SPIR-V front end read an atomic's pointer out of scope
  where the atomic was asked in a branch of its own, in the WGSL it wrote from
  rust-gpu's output. **Unconfirmed:** one line of the lesson, from the rust-gpu
  round, holds it; no experiment records the case.
- **Found:** 2026-11-03. **Seen on:** naga of that date.
- **Evidence:** [shaders the phones take](./gpu/lesson/shaders-phones-take.md),
  and the note beneath that line, which places the fault in the SPIR-V path.
- **In the tree:** nothing. The SPIR-V path went with rust-gpu on 2026-11-05;
  the hand WGSL forms each atomic's pointer at its call, and an atomic in a
  branch is kept where it stands.

## Expo

### `fetch` on Android completes a body after a dropped connection

- **Upstream:** Expo, `expo/fetch` on Android (`NativeResponse.kt`).
- **Symptom:** when the connection drops while a body comes in, the native
  response marks the error and then marks the body complete, so `text()`
  resolves with a short or empty body. **Unconfirmed:** read from Expo's
  source, not reproduced on the phone.
- **Found:** 2026-11-15. **Seen on:** a Galaxy S23, the app's first launch; the
  Expo version is not recorded.
- **Evidence:** [xpute's parts cut by what they are](./minutes/20261115-xpute-s-parts-cut-by-what-they-are-and-what-they-vary-on.md):61-70.
- **In the tree:** the source service takes a body that is not JSON as a
  failed ask and asks again
  ([source.ts](../ts/client/src/service/sys/fs/source.ts):181-184).

## Rust (cargo, clippy)

### `missing_lints_inheritance` reads the wrong workspace

- **Upstream:** cargo, the `missing_lints_inheritance` lint (stabilized with
  cargo-lints, PR 17298; the rule added in PR 16588).
- **Symptom:** the lint reads the current workspace's `[workspace.lints]`
  (`ws.root_maybe()` in `src/diagnostics/rules/missing_lints_inheritance.rs`),
  so building rust/'s workspace warns once for each path dependency from
  xpute's own workspace, 8 crates. Cargo 1.98.1 printed none.
- **Found:** 2026-11-16. **Seen on:** cargo 1.100.0.
- **Evidence:** no record under doc/ holds it yet; found while building on
  2026-11-16. Related upstream issues: rust-lang/cargo 17733 and 14645.
- **In the tree:** nothing.

### `missing_const_for_thread_local` fires on a `const` initializer on Android

- **Upstream:** clippy, the `missing_const_for_thread_local` lint.
- **Symptom:** on `aarch64-linux-android` the lint fires on a `thread_local!`
  already initialized in a `const` block, since std's os-based TLS expansion
  drops the `const` block; for example `BLOCK` in
  [admit.rs](../rust/gpu/guest/src/admit.rs):18.
- **Found:** 2026-09-27 (the allow's date); rechecked 2026-11-16. **Seen on:**
  the toolchain 1.100.0.
- **Evidence:** the comments on `lint-android` in
  [rust/.cargo/config.toml](../rust/.cargo/config.toml):16-21 and
  [xpute/.cargo/config.toml](../xpute/.cargo/config.toml):5-10.
- **In the tree:** both `lint-android` aliases pass
  `-A clippy::missing_const_for_thread_local`; the flag goes when the lint
  passes without it.
