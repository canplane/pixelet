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

## Not measured

What `vkPipeline`'s 12 MiB blocks hold, the iPhone's share of GPU memory, a touch to
the screen, and the iPhone's temperature.

## Kept

The patch, with the vendored `wgpu-core`. Its recorder extends borrows to `'static`
with a transmute, held sound by wgpu-core's trackers until the replay; the
prototype's author does not offer it for upstream as it is.
