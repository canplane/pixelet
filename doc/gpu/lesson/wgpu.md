# wgpu

The native path: wgpu, and naga, which writes the WGSL as Metal on Apple's GPUs
and as SPIR-V on Android's, for the apps and the pxl-render tests.

- **`break if` in `continuing`** ends a loop one iteration early on naga's Metal
  backend ([naga on Metal](./naga-on-metal.md)). The tree patches every wgpu
  crate from canplane/fork.wgpu ([Cargo.toml](../../../rust/Cargo.toml):204-213),
  and the fork's commits ([wgpu-fork.md](../../../rust/wgpu-fork.md)) do not carry
  #10459.
- **One command buffer a pass, the simulator's family, staged writes and the
  present that blocks** are in [a device is not a page](../../lesson/a-device-is-not-a-page.md).
  The one-command-buffer fix took memory down on both phones (the S23's
  `Graphics` 1.38 to 1.04 GiB, the iPhone 0.81 to 0.71) and left GPU time as it
  was; it superseded a Metal-only patch that turned indirect-call validation off
  and ran a timestamp resolve early.
  ([the patch that fixed one phone](../minutes/20261110-the-patch-that-fixed-one-phone.md):41-53, 100-128;
  [one command buffer an encoder](../experiment/20261110-one-command-buffer-an-encoder/README.md):11-27, 52-68, 75-82, 254-256)
- **The stack.** wgpu's calls inlined into one function bring large structures
  onto the stack (`skin_group` reserves 224 KiB), which overflowed React Native's
  1 MiB JS thread. The core runs on its own 8 MiB thread.
  ([a device's kernel on a thread of its own](../../minutes/archive/20260926-a-device-s-kernel-on-a-thread-of-its-own.md):11-22, 32-39)
- **Fast math on Metal.** wgpu-hal compiles Metal with fast math on by default,
  so sums are reordered: 28–981 of 65,536 light-ray starts parted even with exact
  products. [cpu-and-gpu-agree](./cpu-and-gpu-agree.md) says no flag turns fast
  math off.
  ([the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):383-388;
  [the shaders by hand held to the engine](../minutes/20261105-the-shaders-by-hand-held-to-the-engine.md):34-35)
- **An aborted dispatch reads as done.** wgpu-hal's Metal `Fence::get_latest`
  counts an errored command buffer as completed, so a dispatch can stop partway
  with no error; under contention, long dispatches lost writes. Not reproduced,
  and the cause not named.
  ([the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):688-703, 1014-1016)
- **An endless poll.** `device.poll(wait_indefinitely)` on a hung shader blocks
  forever (3 h 42 min).
  ([the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):1010-1013)
- **naga's refusals** and **`f32::MAX`** are in [WGSL](./wgsl.md).
- **naga's WGSL round trip** grows the text and is unsafe across versions.
  ([what the published build gives away](../../minutes/archive/20260918-what-the-published-build-gives-away.md):230-251)
- **wgpu on the page.** Its `webgpu` backend hands the WGSL over unchanged, and
  validation and the driver stay the browser's. It has no asynchronous pipeline
  call, and adds about 200 KB gzipped to `core.wasm`.
  ([tauri and wgpu on the page](../minutes/20261110-tauri-and-wgpu-on-the-page.md):24-26, 45;
  [pipelines are made by ticket](../minutes/20261102-pipelines-are-made-by-ticket-before-the-first-frame.md):70)
- **The app is not faster than Chrome** on the S23: per Mpx, Chrome 22.7–23.5 ms
  against the app's 24.4–26.1, at the same clock (means 695 and 697 MHz).
  ([the shaders by hand](../experiment/20261103-the-shaders-by-hand/README.md):895-899;
  [the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):408-434;
  [the lights' tail](../../experiment/20261107-the-lights-tail/README.md):244-245;
  [the app that did not move](../../experiment/20261109-the-app-that-did-not-move/README.md):34-41;
  [one command buffer an encoder](../experiment/20261110-one-command-buffer-an-encoder/README.md):84-86, 215-220)
- **The S23's surface** offers only the `Inherit` alpha mode; asking for `Opaque`
  failed `configure` (`ENOTRECOVERABLE`).
  ([the frame on the S23](../experiment/20261103-the-frame-on-the-s23/README.md):435-438)
- **A `CAMetalLayer` configured off the main thread** is flagged on iOS 27, so
  the surface is configured on the main thread.
  ([a device's kernel on a thread of its own](../../minutes/archive/20260926-a-device-s-kernel-on-a-thread-of-its-own.md):67-83)
