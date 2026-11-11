# WebGPU

What the WebGPU API and its specification hold on every implementation: its
limits, usage scopes, resources, and getting and losing a device.

## The API

- **A secure context.** `navigator.gpu` exists only in a secure context, and is
  absent elsewhere with no error. `localhost` and `127.0.0.1` count as secure; a
  LAN address over http does not. Android reaches the dev server through
  `adb reverse` or `chrome://inspect`; the iPhone needs real https: a
  self-signed certificate clicked through, or a tunnel with Vite's
  `allowedHosts`.
  ([WebGPU on a phone](../minutes/archive/20260917-webgpu-on-a-phone-and-the-secure-context.md):9-12, 20-30, 49-52, 63-73, 87-97;
  [the ground one device does not draw](../minutes/archive/20260923-the-ground-one-device-does-not-draw.md):94-97;
  [the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):33-36;
  [what stands over the app](../../minutes/20261103-what-stands-over-the-app.md):35-37)
- **Eight storage buffers a stage.** The default `maxStorageBuffersPerShaderStage`
  is 8, as Chrome on the Mac reports. A merged pass that needs 9 fails.
  ([the miss from a list](../../experiment/20261026-the-miss-from-a-list.md):47-50, 94-97;
  [the ring](../../experiment/20261028-the-ring.md):56, 149;
  [faces once a box](../../experiment/20261026-faces-once-a-box.md):24;
  [the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):622-632)
- **Indirect and writable storage in one scope.** A buffer used as Indirect and
  as writable Storage in one usage scope is invalid, and the submit is dropped:
  on the S23's Chrome every frame's, 7,490 error pairs in 263 s; Mac Chrome showed
  no such error. A pass may not
  write the arguments it is dispatched by; read-only storage beside its own
  indirect use is valid.
  ([the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):1648-1658, 1683-1688;
  [the ring](../../experiment/20261028-the-ring.md):54-55;
  [faces once a box](../../experiment/20261026-faces-once-a-box.md):24-25)
- **Attachment and binding.** A texture is an attachment and a binding in one
  pass only where both its aspects are read-only.
  ([the screen keeps its tiles](../../minutes/20261029-the-screen-keeps-its-tiles-as-the-light-does.md):229-232)
- **`depth32float-stencil8` is optional.** Every Apple GPU, the Quest 3 and 3S
  and about 90% of Android devices offer it (Adreno 7xx and recent Malis always;
  Adreno 6xx and older Malis by driver). Where it is missing,
  `depth24plus-stencil8` is 24-bit unorm on Android and the Quest, so reversed Z
  loses precision there.
  ([the screen keeps its tiles](../../minutes/20261029-the-screen-keeps-its-tiles-as-the-light-does.md):362-364, 524-536;
  [the tiles baseline](../../experiment/20261030-the-tiles-baseline.md):37-43, 70-79, 86-93)
- **The canvas's texture is new each frame.** WebGPU keeps nothing of the last
  frame's.
  ([what moved alone](../../experiment/20261029-what-moved-alone.md):239-241)
- **Vertex input.** An instance-stepped element is fetched at
  `first_instance + i`, so `first_vertex` can carry a record's index when
  nothing is vertex-stepped. On the S23, storage reads in the vertex stage,
  `firstVertex` and `firstInstance` were bit-exact.
  ([the ground one device does not draw](../minutes/archive/20260923-the-ground-one-device-does-not-draw.md):113, 118-122, 187-192)
- **A buffer refused for memory** is reported asynchronously, after its id is
  recorded.
  ([the ground one device does not draw](../minutes/archive/20260923-the-ground-one-device-does-not-draw.md):80-83, 220-223)
- **`layout: "auto"`** drops the bindings the shader does not read, and an
  explicit group that lists one is refused; the frame fails silently.
  ([the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):634-636;
  [a ray from the box](../../experiment/20261026-a-ray-from-the-box.md):64-67)
- **WGSL only.** WebGPU takes WGSL text, not SPIR-V, so the shaders ship as
  text, one hook away; minifying is the most that hides them.
  ([what the published build gives away](../../minutes/archive/20260918-what-the-published-build-gives-away.md):196-214;
  [what stays in JavaScript](../../minutes/archive/20260914-what-stays-in-javascript.md):318-319)
- **Out of bounds.** WGSL leaves an access out of bounds to each implementation:
  Metal through wgpu read 0 and dropped writes, Dawn clamped. A test on one does
  not show another.
  ([the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):417-418, 439-440;
  [sweep f9f66682e](../experiment/20261103-the-shaders-by-hand/supervise/sweep-f9f66682e.md):29-31;
  [sweep final world](../experiment/20261103-the-shaders-by-hand/supervise/sweep-final-world.md):26-28)
- **The memory model.** Relaxed atomics do not order plain reads, and the order
  of writes between invocations is not fixed. A multi-word slot tore on Metal
  (4 of 88 runs), and plain reads beside atomic claims answered wrong in 4 of 18.
  In one pass the whole first wave of workgroups ran before any claim landed.
  ([the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):546-554, 680-686, 881-885;
  [decor plan](../experiment/20261103-the-shaders-by-hand/decor/plan.md):41;
  [the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):1002-1006, 1010-1017)
- **A resizable ArrayBuffer.** WebIDL refuses views of one, and `writeBuffer` and
  `writeTexture` lack `[AllowResizable]`, so the wasm memory's
  `toResizableBuffer()` cannot be uploaded from. An upload reads from any other
  ArrayBuffer, not only the wasm memory.
  ([a single-thread core](../../minutes/archive/20260911-single-thread-core-and-when-to-spawn-a-worker.md):150-153, 216-226, 234-237)
- **Dynamic offsets.** `setBindGroup`'s `(data, start, length)` form over one
  view makes no garbage a call (about 530 objects a frame before). Each `set*`
  call is a crossing into the browser and a validation.
  ([what a frame asks of WebGPU](../experiment/20260922-what-a-frame-asks-of-webgpu.md):40-55;
  [the sky is the air](../../minutes/archive/20260922-the-sky-is-the-air.md):173-176)
- **A worker.** A worker gets `navigator.gpu` and a device, and draws on a
  transferred canvas (Chrome 156, Safari 27 on the Mac); its rAF runs at the
  display's rate. Untried on the phones.
  ([the core in a worker](../../experiment/20261106-the-core-in-a-worker/journal.md):12, 62, 82)
- **MSAA.** The sample count belongs to the pipeline: a change remakes the
  pipelines and the targets, not the device. A target costs pixels × samples ×
  (colour + depth bytes): 150 MB at 3.2 Mpx.
  ([the audit against the rings](../../minutes/archive/20260915-the-audit-against-the-rings-checkpoint.md):113-124, 272-277;
  [why the destination is wasm](../../minutes/20260912-why-the-destination-is-wasm.md):192-196;
  [TAA tried and closed](../../minutes/archive/20260911-taa-tried-and-closed.md):11-13, 111-116)
- **A readback's `bytesPerRow`** not a multiple of 256 returned zeros, and nothing
  showed an error until an error scope caught one.
  ([the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):979-981)
- **The queue.** A `writeBuffer` is on the queue when it returns, also where the
  call blocks first ([the GPU is shared](./the-gpu-is-shared.md)).
  ([a device is not a page](../../lesson/a-device-is-not-a-page.md))

## Getting and losing a device

- **Three ways to fail.** No `navigator.gpu`, no adapter, or `requestDevice`
  rejecting: each takes its own handling. A `lost` callback that boots again
  unconditionally loops forever.
  ([WebGPU on a phone](../minutes/archive/20260917-webgpu-on-a-phone-and-the-secure-context.md):34-45, 113-115;
  [what stands over the app](../../minutes/20261103-what-stands-over-the-app.md):35-38)
- **Other losses** are each in their implementation's file: a crash of Safari's
  GPU process ([WebKit](./webkit.md)), a dispatch aborted silently and an
  endless poll ([wgpu](./wgpu.md)), a pass too long and silent corruption
  ([Vulkan](./vulkan.md)).
