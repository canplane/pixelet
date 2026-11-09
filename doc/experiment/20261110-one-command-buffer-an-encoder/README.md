# One command buffer an encoder

**2026-11-10.** Why the S23's app held 1.38 GiB of GPU memory, and what one patch
to wgpu did to it on the S23 and the iPhone.

## Tried

- `MemoryHints::MemoryUsage` in place of `Performance` (8 to 64 MiB device blocks
  for 128 to 256 MiB), to test the Vulkan allocator's blocks. Undone.
- wgpu's allocator report, printed once at the 600th frame on the S23. Undone.
- The prototype attached to
  [gfx-rs/wgpu#5738](https://github.com/gfx-rs/wgpu/issues/5738), against wgpu
  30.0.1: [`pshtif/wgpu`](https://github.com/pshtif/wgpu/compare/v30.0.1...pass-encoding-prototype).
  wgpu-core records a pass, encodes its barriers into the encoder's open command
  buffer, then replays the pass there, so an encoder is one hal command buffer on
  every backend. It replaces the Metal-only `chain` in the vendored wgpu-hal, which
  went back to upstream but for the simulator's capabilities in `metal/adapter.rs`.
  `InstanceFlags::DISCARD_HAL_LABELS` is no longer turned off.

## The cause, as reported

wgpu-core 30.0.1 encodes each pass into a hal command buffer, with another before
it for the pass's barriers. On Vulkan each pool is reset without
`RELEASE_RESOURCES` and reused, so it keeps the memory of its heaviest frame. The
comment on #5738 reports an Adreno 840 going from about 120 MB to 2.1 GB. On Metal
the driver holds about 2.3 MiB per `MTLCommandBuffer` until it completes
([#10494](https://github.com/gfx-rs/wgpu/issues/10494)).

## How it was measured

Release builds with `PXL_PERF_DRIVE=1`, one run of each, from about `576391033e`.
While the pre-patch builds ran, another session's edits arrived in the checkout, so
they may hold some of them. The S23: `android-app.py`, `dumpsys meminfo` every 8 s.
The iPhone: `ios-app.py` for the report, then a launch of the same build under
`xctrace`'s Activity Monitor for `memory-physical-footprint`. `Graphics` and the
footprint are different measures, so the two phones compare only each with itself.

## What came of it

The allocator on the S23 before the patch, against the driver's count:

| | MiB |
| --- | --- |
| Allocated by `gpu-allocator` | 480 |
| Reserved in its 4 blocks | 704 |
| `GL mtrack` | 1,337 |
| `EGL mtrack` | 79 |

`MemoryUsage`: `Graphics` 1.20 to 1.23 GiB after the load for 1.38, 1.41 GiB in the
drive, 290 frames drawn, GPU p50 62.5 ms. The blocks are about 0.15 GiB of it.

The patch:

| | before | after |
| --- | --- | --- |
| S23 `Graphics` after the load, GiB | 1.38 | 1.04 |
| S23 `Graphics` in the drive, GiB | 1.67 | 1.31 |
| S23 PSS in the drive, GiB | 2.21 | 1.71 |
| S23 GPU p50 / p95, ms | 62.6 / 133.0 | 63.7 / 128.2 |
| S23 frames drawn | 284 | 288 |
| S23 AP, °C | 31.0 to 39.4 | 37.4 to 46.5 |
| iPhone footprint after the load, GiB | 0.81 | 0.71 |
| iPhone footprint, the drive's peak, GiB | 0.94 | 0.94 |
| iPhone footprint after the drive, GiB | 0.91 | 0.77 |
| iPhone GPU p50 / p95, ms | 51.4 / 93.4 | 50.7 / 92.0 |
| iPhone frames drawn | 360 | 366 |

A second iPhone run of the patch read GPU p50 / p95 51.5 / 95.4 ms and 358 frames.

## Looked at afterwards

Investigated by agents, and read against each other by another. Run on the code of
`2527e972f0`, with `patched` and `baseline` builds of the table above.

**CPU on `xpute-gpu`, ms a frame in the drive.** The S23, from `sched`: baseline 10.19
and 11.20, patched 8.36 and 8.33. The second baseline run was hot and drew 203
frames, so its figure does not compare; a fit to the two puts the baseline near 10.2
at 290 frames, about 18% above the patch. The iPhone, Time Profiler at 1 kHz: baseline
3.295 and 3.174, patched 3.463 and 3.525, 0.26 ms more. Each is net of what the patch
took away, per-pass command buffers on the S23 and `chain` on the iPhone, so the
recorder's own cost is not isolated, and the pre-patch builds may hold another
session's edits.

**GPU clock on the S23.** Nine runs: the app 675 to 709 MHz, mean 695; Chrome 680 to
705, mean 697. The spread follows the kernel's thermal cap, 719, 680 and 615 MHz. The
604 MHz of one earlier run did not come back; its trace is gone.

**`GL mtrack` after the patch, MiB, 40 s after a launch, no touches.** From
`VK_EXT_device_memory_report`, the allocator's report and `dumpsys`:

| | |
| --- | --- |
| Allocator blocks (479.8 of it allocated) | 704.0 |
| `VkPipeline` | 206.2 |
| Command buffers | 19.3 |
| Descriptor pools | 2.2 |
| Other Vulkan objects | 1.4 |
| Skia's cache, the Android UI | 78.1 |
| Not itemized | 11.2 |
| **`GL mtrack`** | **1,022.3** |

Of the pipelines, 204.0 MiB is one 12,582,912 B block in each of 17 compute pipelines:
`queue_last_cs`, `queue_ask_cs`, `queue_ask_parked_cs`, `decor_cs` and `face_cs`, the
entry points of the 12 modules that include `world.wgsl`. They use no `var<private>` or
`var<workgroup>` data and no function-local arrays. Register spill from the inlined
world query is a suspicion; Mesa's formula for shader private memory gives the same
size, for another driver.

**The report.** On a device and a page `n`, `fps`, `kinds`, `interval` and `gpu` are
the same arithmetic. `refresh` is not (8.3 ms against 16.7 on the same phone), so
`missed` is judged at about 20.8 and 25 ms. `load` is the session's longest long frame,
and on a device only the JavaScript thread's.

## The 12 MiB blocks

The original logs of this investigation no longer exist. The figures are those of its
report, and the commands and tables of its tool calls, recovered from the agent's
transcript into `report/pipeline-scratch-notes.md`: the standalone program's source, every
pipeline's statistics, the stubbing tables, the Chrome and in-app runs and the outside
sources, each tool output as it stood. Driver E031.41.03.64, AP 39.7 to 47.4 °C.

The block is the shader's scratch memory, held per `VkPipeline`. Each WGSL module was
compiled with naga 30.0.1 in a standalone Vulkan program run under `adb shell`, which read
`VK_KHR_pipeline_executable_properties` and `VK_EXT_device_memory_report`. The driver's
statistic is "Scratch memory usage per shader instance", in 128-bit slots, with the
warning that a shader using any scratch memory performs poorly. Of 63 base compute
pipelines, 17 have a scratch figure above 0 and each holds one 12,582,912 B block:

| Module, entry | Scratch (slots) | Registers | Program, bytes |
| --- | --- | --- | --- |
| 39, 81 to 84: `queue_ask_cs`, `queue_ask_parked_cs` (10) | 8 | 34 to 35 | 77,824 |
| 43: `face_cs` | 22 | 44 | 69,632 |
| 26: `decor_cs` | 36 | 48 | 86,016 |
| 67, 73, 36, 71: `queue_last_cs` | 64, 68, 76, 136 | 48 | 94,208 to 98,304 |
| 34: `queue_last_cs` | 328 | 48 | 266,240 |

The other pipelines read 0 and hold no such block. The block is 12,582,912 B times
`ceil(S / 512)` over the 542 pipelines of the saved outputs, 1 mismatch of them, a
synthetic shader of S = 12,124 that held 48 MiB where the rule gives 288. The 17 are the
entry points of the 12 modules whose parts include `world`.

No single part is responsible. In `queue_ask`, stubbing any one of about 30 world and ask
functions takes S to 0. In `queue_last_cs` of module 67, only stubbing the whole query
does, and stubbing the caves or the ground leaves S at 12 to 14. With naga's bounds checks,
loop bounding and division checks all off, S falls (`decor_cs` 36 to 10) and no pipeline
reaches 0. `spirv-opt --inline-entry-points-exhaustive` alone takes S to 0 on the 10
`queue_ask`, `decor_cs` and four of the five `queue_last_cs`; `face_cs` and module 34 keep
theirs. The instructions of `queue_ask` go from 16,206 to 31,093.

Not recovered: the stub results of `queue_last_cs` in modules 34, 71, 73 and 36, whose
output was cut off; the per-stub tables of modules 26, 43 and 67, of which only the lists
that reach 0 and the histograms were printed; the first base run's `GL mtrack`, which its
script read from the wrong line; any compile time in the app, as only the standalone
program's per-pipeline times exist.

| Inlined by `spirv-opt` | `GL mtrack` peak, MiB | GPU p50 / p95, ms | Frames |
| --- | --- | --- | --- |
| Nothing | 1,261.6 | 63.4 / 133.3 | 292 |
| 10 `queue_ask` | 1,143.0 | 63.2 / 136.5 | 283 |
| 4 `queue_last` and `decor` | 1,203.0 | 71.5 / 180.1 | 236 |
| All 15 | 1,082 | 67.7 / 165.9 | 260 |

Chrome holds the block in 6 of the 17 pipelines, 72 MiB against the app's 204.

## `queue_ask` as one module

Five modules and ten pipelines became one and two (`ask_run.wgsl`). One run each, the
before from an earlier commit: `GL mtrack` after the load 994 to 885 MiB, `Graphics` in
the drive 1.31 to 1.20 GiB, GPU p50 / p95 63.7 / 128.2 to 63.4 / 138.3 ms, frames drawn
288 to 283. S23 Chrome: 300 frames, 52.8 / 158.7 ms.

The derivation was reviewed against the old constants for all six queues. The GPU tests
catch a wrong lane: flipping `asked_at` fails five of them. They do not catch a wrong tag
or a wrong records offset. A tag only changes keys no test reads, and a misplaced ask reads
empty records, after which the last walk asks the world itself. `build.rs` checks the
queue-to-tag map at compile time.

## The iPhone's app beside its Safari

iPhone 13 mini, Release with the drive, one run each, temperature not read. The page came
through a `cloudflared` tunnel, for WebGPU's secure context, on `9de89bbf0e`. The app's
figures are those of the patch's table, before the `queue_ask` merge.

| | app | Safari |
| --- | --- | --- |
| Frames drawn in 20 s | 366 | 281 |
| GPU p50 / p95, ms | 50.7 / 92.0 | 50.5 / 93.1 |

The Mac's Safari did not run: its session timed out. The reports of the `queue_ask` runs
and of this one are under `report/`.

## The device's runtime beside the page

**2026-11-10.** The device's runtime off the JavaScript thread (`b7cd54d228`,
`a7e2daa22d`, `b5f4cd06ed`), against the page, and what the report had to be
corrected to say so.

### Tried

The runtime moved to a Rust thread of its own, touches reached the core without
JavaScript, and tile bodies were fetched by the operating system's network and
handed to the runtime. Then a device's frame was made to wait while the runtime
holds its stream (`3005f0a7d5`), as a page's never starts while it would have to
be put off.

### How it was measured

Release builds, `resolution: fixed`, one 20 s `perfDrive` per line, one run
each. System traces beside `s23-app` and the second Chrome run. An audit by
reading the code and comparing `kinds` with the system trace's presents:
`QueuePresentKHR` 269 against `cast` 270 on the S23's app.

### What came of it

| run | AP °C | drawn | Mpx | GPU p50 / p95 ms | GPU p50 per Mpx |
| --- | --- | --- | --- | --- | --- |
| S23 app, before `3005f0a7d5` | 36.0 to 46.3 | 270 | 2.53 | 65.9 / 140.2 | 26.1 |
| S23 app, `3005f0a7d5` | 28.9 to 39.9 | 287 | 2.53 | 61.8 / 135.5 | 24.4 |
| S23 Chrome | 36.7 to 48.2 | 314 | 2.13 | 48.2 / 139.2 | 22.7 |
| S23 Chrome, traced | 32.3 to 43.3 | 306 | 2.13 | 49.9 / 145.9 | 23.5 |
| iPhone app, before `3005f0a7d5` | not read | 360 | 2.74 | 51.6 / 95.5 | 18.8 |
| iPhone app, `3005f0a7d5` | not read | 364 | 2.74 | 50.4 / 91.2 | 18.4 |
| Mac Safari, at 30 Hz | not read | 555 | 7.90 | 22.2 / 32.0 | 2.8 |

- `fixed` does not give the app and Chrome the same pixels: Chrome's canvas is
  1080 by 1969 and the app's 1080 by 2340. Over the window the app drew 682 Mpx
  before `3005f0a7d5` and 725 after, Chrome 651 and 668.
- Before `3005f0a7d5` the device's report counted turns that drew nothing
  (`stream_busy`): `fps` 36.7 for 13.5 frames drawn a second, `interval` p50 17.3
  ms for a drawn gap of 64.7. After it, `fps` is 14.4, `interval` p50 66.6 ms, and
  the report's frames are the drawn ones on both platforms.
- In the first S23 app run's window the GPU clock averaged 604 MHz; in Chrome's
  it held about 700 MHz. The second app run has no system trace.
- JavaScript thread during drags, S23, 28 s: 1.5 to 2.2 s before the tile bodies
  moved and 1.95 s after, one run. The file's open still starts there.

### Not measured

A touch to the screen, the iPhone's Safari, and the app against the page under
touches.

### Kept

All of it. The tile fetch keeps each platform's HTTP cache: OkHttp's on Android
(`0441f85e28`), the system's on iOS.

## Not measured

Why the driver spills in those pipelines, the iPhone's share of GPU memory, a touch to
the screen, and the iPhone's temperature.

## Kept

The patch, with the vendored `wgpu-core`. Its recorder extends borrows to `'static`
with a transmute, held sound by wgpu-core's trackers until the replay; the
prototype's author does not offer it for upstream as it is.
