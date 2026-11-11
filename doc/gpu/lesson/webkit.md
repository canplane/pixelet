# WebKit

Safari's WebGPU, and iOS, where every browser and WKWebView is WebKit.

## Shaders

- **`atomicCompareExchangeWeak`**, **`i32::MIN` as a constant** and **a struct
  rebuilt from a block's words** are in [shaders the phones take](./shaders-phones-take.md).
  A crash of Safari's GPU process loses every tab's device, and for a while no
  adapter answers.
  ([a matrix the driver reads its own way](../minutes/archive/20260927-a-matrix-the-driver-reads-its-own-way.md):122-131)
- **The Mac stands in for the iPhone** ([shaders the phones take](./shaders-phones-take.md)).
  The iPhone was not checked again after 10fe6a7e5.
  ([the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):31, 1116;
  [the shaders by hand held to the engine](../minutes/20261105-the-shaders-by-hand-held-to-the-engine.md):101)

## Memory on iOS

- **A ceiling of about 2 GB.** Others report the WebContent process killed at
  about 2048 MB on an iPhone (pressure events at 1.6–2.1 GB), and higher on an
  iPad. Memory is unified and the GPU process is charged to the page, so GPU
  memory counts against the page. Nothing found says the
  `increased-memory-limit` entitlement lifts it.
  ([why the destination is wasm](../../minutes/20260912-why-the-destination-is-wasm.md):182-185, 271-282)
- **A tab discarded under pressure** reloads, silently.
  ([mobile and perf targets](../../minutes/archive/20260810-mobile-and-perf-targets.md):142-145;
  [what to render when](../../minutes/archive/20260912-what-to-render-when.md):172-174;
  [the page pyramid](../../minutes/archive/20260921-the-page-pyramid.md):18-24)
- **Safari's figures mislead.** Its "JavaScript" figure is a collector's
  counter that also charges GPU uploads and canvas backing; JavaScriptCore
  reports the wasm reservation whole as one ArrayBuffer; the page sees no JS
  heap (`heap 0MB`). Read heap snapshots, not the timeline.
  ([why the destination is wasm](../../minutes/20260912-why-the-destination-is-wasm.md):200-204;
  [the ring, a slot and its payload](../../minutes/20260917-the-ring-a-slot-and-its-payload.md):187-191;
  [bake worker on the phone](../../experiment/20260913-bake-worker-on-the-phone.md):80-86)
- **A large canvas is read whole** on every `drawImage` or `texImage2D` of it
  (64 MB a call): Safari's JavaScript counter went from 1.3 to 3.8 GB, against a
  live heap of about 700 MB.
  ([linear memory, what is left](../../minutes/archive/20260912-linear-memory-what-is-left.md):54-62;
  [why the destination is wasm](../../minutes/20260912-why-the-destination-is-wasm.md):197-199)
- **WebGPU on iOS** is on by default from iOS 26 (iPhone 11 and later), in Safari
  and WKWebView alike; one older record says iOS 18
  ([concurrency model](../../minutes/archive/20260708-concurrency-model.md):79-84).
  ([why the destination is wasm](../../minutes/20260912-why-the-destination-is-wasm.md):225-227, 279-281;
  [WebGPU on a phone](../minutes/archive/20260917-webgpu-on-a-phone-and-the-secure-context.md):102-103)

## Stalls

- **Leftover WebKit processes hold the GPU.** A fresh page holds 410–577 MiB,
  flat; the GPU process died at 1.4–1.5 GB serving several hung pages. Quitting
  Safari freed it ([the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):1431-1453);
  [the GPU is shared](./the-gpu-is-shared.md) records one case a reboot cleared.
  Device losses once read as memory were not shown to be memory.
  ([the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):1446-1447)
- **A synchronous `writeBuffer`, the stall of 2026-11-12, and WebKit bug
  321876** are in [the GPU is shared](./the-gpu-is-shared.md).
- **A window behind others** gives no result: Safari's runs did not finish.
  ([the lights' tail](../../experiment/20261107-the-lights-tail/README.md):337-338)

## Measurement

- **The iPhone app and Safari** read the same GPU busy time, and Safari drew
  fewer frames (one run each, the temperature not read).
  ([one command buffer an encoder](../experiment/20261110-one-command-buffer-an-encoder/README.md):178-187)
- **Timestamps.** Safari 27.0.1 on the Mac reads every timestamp span.
  ([the lights' tail](../../experiment/20261107-the-lights-tail/README.md):337-340)
