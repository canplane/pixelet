# Vulkan

Android's GPUs under Vulkan, learned on a Galaxy S23 (Adreno 740 unless named),
through Chrome's Tint and the app's naga alike.

## What the driver takes

- **Layouts read with room inside**: a `vec3f` then an `f32` in a storage array,
  and a storage `array<mat4x4f>`, are in [shaders the phones take](./shaders-phones-take.md).
  The `vec3f` case read right as a uniform and on every Metal device. The
  matrix stride was seen in the native app only; Chrome on the S23 was not
  tried.
  ([the ground one device does not draw](../minutes/archive/20260923-the-ground-one-device-does-not-draw.md):13-33, 55-57, 164-166;
  [a matrix the driver reads its own way](../minutes/archive/20260927-a-matrix-the-driver-reads-its-own-way.md):3-23, 27-53)
- **Why layouts part.** Each backend spells the layout as decorations (Offset,
  ArrayStride, MatrixStride), Vulkan has four block-layout rules, and a type with
  room inside it is read two ways (gfx-rs/wgpu#914, renderdoc#2378).
  ([a matrix the driver reads its own way](../minutes/archive/20260927-a-matrix-the-driver-reads-its-own-way.md):57-74)
- **Valid WGSL refused.** The driver refuses valid modules with
  `VK_ERROR_UNKNOWN`, sync or async, which reaches the page as an OperationError
  or a GPUPipelineError, reason "internal", with no validation error. No WGSL
  feature, nor size, sets the refused modules apart.
  ([the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):91-102, 217-221, 236-237)
- **Refused shapes** are in [shaders the phones take](./shaders-phones-take.md).
- **Size and load.** `decor` was taken once any one of ten functions was
  emptied; its halves were taken inlined and refused with their functions kept.
  Inlining took the scratch away from most pipelines, though `face_cs` and
  module 34 keep theirs; that the driver keeps large functions as real calls is
  an inference.
  ([the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):122-127, 174;
  [one command buffer an encoder](../experiment/20261110-one-command-buffer-an-encoder/README.md):146-148;
  [its scratch notes](../experiment/20261110-one-command-buffer-an-encoder/report/pipeline-scratch-notes.md):1847-1922)
- **Rewrites do not move one way.** A set of rewrites that fixed one module broke
  another.
  ([the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):129-139)

## What costs

- **Words live across a loop.** Time holds to 150 words, rises at 175, and the
  pipeline is refused at 200; past that a visit costs 80–240 ns. Live words count
  along the whole call path; 6,000 locals in code that never runs cost nothing.
  ([rust-gpu goes](../minutes/20261103-rust-gpu-goes.md):38-45;
  [the walk on a phone](../../minutes/20261103-the-walk-on-a-phone-and-who-owns-it.md):89-96, 174-177;
  [the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):288-359, 472;
  [the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):188-192, 596-604)
- **A scratch block a pipeline.** Every compute `VkPipeline` that uses scratch
  keeps its own 12 MiB block for its life (× ceil(S/512)): 17 pipelines held
  204 MiB. `VK_KHR_pipeline_executable_properties` gives the statistics, and the
  driver warns that scratch means poor performance. Mesa's turnip pools scratch;
  the blob does not. Merging modules that differ only in values cut `GL mtrack` by
  109 MiB.
  ([the patch that fixed one phone](../minutes/20261110-the-patch-that-fixed-one-phone.md):151-158;
  [one command buffer an encoder](../experiment/20261110-one-command-buffer-an-encoder/README.md):102-148, 165-170;
  [its scratch notes](../experiment/20261110-one-command-buffer-an-encoder/report/pipeline-scratch-notes.md):631-1922, 2906-3204, 3338-3424)
- **Spill** does not come from arrays. It starts at register footprint R=48 in
  synthetic code and at R 34–44 in real code with calls. Inlining removes it but
  doubles instructions and compile time, and inlining `queue_last` and `decor`
  raised GPU p50 from 63 to 71.5 ms.
  ([one command buffer an encoder](../experiment/20261110-one-command-buffer-an-encoder/README.md):103-105, 146-161;
  [its scratch notes](../experiment/20261110-one-command-buffer-an-encoder/report/pipeline-scratch-notes.md):810-829, 2644-2780;
  [the patch that fixed one phone](../minutes/20261110-the-patch-that-fixed-one-phone.md):177-182)
- **Occupancy** falls with register footprint: about 50% at R≤11, 12% at R≥34.
  The subgroup is 64 wide, and the driver inlines small functions itself.
  ([its scratch notes](../experiment/20261110-one-command-buffer-an-encoder/report/pipeline-scratch-notes.md):631, 686-749, 810-916, 1094-1112)
- **Compile time** grows with instructions (about 0.5 s at 16k, 3 s at 54k), and
  inlining doubles it. `spirv-opt -O` is not uniformly better.
  ([its scratch notes](../experiment/20261110-one-command-buffer-an-encoder/report/pipeline-scratch-notes.md):729-749, 1660-1712, 1885-1922, 2085-2105)
- **A node against the Mac.** Per node the S23 cost 26–62× the Mac over the same
  nodes, while plain passes cost 0.8–29×. Hand kernels brought it to about 2×,
  and the generated form cost about 3.5× the hand-written.
  ([the walk on a phone](../../minutes/20261103-the-walk-on-a-phone-and-who-owns-it.md):61-62, 78-86;
  [the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):211-254, 500-503;
  [the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):736-742, 802-807)
- **Atomics and stacks.** An atomic load costs more than a plain one (8.85,
  28.63 and 10.3 ns in three runs, against 0.53; only the order holds, and the
  Mac shows no difference); a 33-entry local stack indexed at run time costs about
  20 ns a visit, and 8 entries nothing; plain counters keep 2–15% of the true
  counts; many atomic stores into a few words serialize (one a tile a workgroup:
  14.4 to 4.4 ms).
  ([the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):276-307, 995-1009, 1105-1114;
  [the walk on a phone](../../minutes/20261103-the-walk-on-a-phone-and-who-owns-it.md):89-95)
- **Submits** cut into 4–6 a frame cost nothing.
  ([the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):1466-1469)
- **Bandwidth** is about 67 GB/s against the Mac's 400.
  ([the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):1026)

## What goes wrong silently

- **A pass too long** loses the device with no shader error; synthetic passes
  are sized to about 50 ms.
  ([the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):143-144, 288-290)
- **Corruption.** All-atomic loads with 250 or more live words changed 279 of
  6.3M words; every pass after saw an empty buffer, with no error and no device
  loss, until a new device. Not reported upstream.
  ([the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):361-406;
  [the walk on a phone](../../minutes/20261103-the-walk-on-a-phone-and-who-owns-it.md):95-96, 217)
- **A pipeline that computes something else.** After an offset shift a made
  pipeline walked 0 nodes against the Mac's 118k. Open.
  ([the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):198-209)

## The device

- **The GPU clock.** The cap is 719 MHz, stepped down by heat to 680, 615, 550,
  348 and 295; read it from kgsl. Slow runs follow the clock, not the
  temperature; CPU work heats the SoC and lowers the GPU clock; a dispatch from
  idle reads up to 3× its warmed time; p95 is about 4× p50.
  ([the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):27-28, 145-146, 1620-1646, 1759-1767;
  [the lights' tail](../../experiment/20261107-the-lights-tail/README.md):319-335, 436;
  [the app that did not move](../../experiment/20261109-the-app-that-did-not-move/README.md):74-76;
  [one command buffer an encoder](../experiment/20261110-one-command-buffer-an-encoder/README.md):84-86)
- **The swapchain as EGL** is in [a device is not a page](../../lesson/a-device-is-not-a-page.md).
- **Memory figures.** `Graphics` is `GL mtrack` plus `EGL mtrack`, and a regex
  for `GL mtrack` also matches `EGL mtrack`. `VK_EXT_device_memory_report`
  itemizes it. gpu-allocator held 704 MiB reserved against `GL mtrack`'s 1,337;
  `MemoryHints::MemoryUsage` saves about 0.15 GiB.
  ([the patch that fixed one phone](../minutes/20261110-the-patch-that-fixed-one-phone.md):88-92, 106-108;
  [one command buffer an encoder](../experiment/20261110-one-command-buffer-an-encoder/README.md):8-9, 33-50, 88-100;
  [its scratch notes](../experiment/20261110-one-command-buffer-an-encoder/report/pipeline-scratch-notes.md):2366-2368, 2627-2775)
- **Chrome's adapter on the S23** has the Mac's limits, with core features (not
  compatibility mode). It granted 40 MiB buffers up to 1,196 MiB with no OOM
  scope firing.
  ([the ground one device does not draw](../minutes/archive/20260923-the-ground-one-device-does-not-draw.md):109-111)
