# The Safari stall

**2026-11-11 to 11-12.** Why the Mac Safari page's canvas stopped while its UI stayed
alive, what was changed for it, and how far it was traced. Safari 27.0.1
(22625.1.29.11.28), macOS 27.0.1, Apple M1 Max. Release builds with `PXL_PERF_DRIVE=1`,
`script/perf/safari.py`, one run to a row below. The decisions it led to are in
[the GPU's other users are not the app's](../../minutes/20261112-the-gpus-other-users-are-not-the-apps.md).

## Tried

- **The page's buffer writes on a belt** (`32186b37bd`). Before it, each `writeBuffer`
  was a synchronous call to WebKit's GPU process, and a stalled run showed the core's
  worker blocked in it for 15 s at a time (f-2: `[gpu] op 13 took 15002 ms`, op 13
  being `WRITE_BUFFER`), WebKit's `gpuProcessTimeoutDuration` (WebProcess.cpp). Kept.
- **A slow line from the runtimes** (`f0b858683d`, `32186b37bd`, `d17fc17abb`): a GPU
  record, a GPU run, a completion's landing (map, remap, image) or a turn slower than
  perf.json's `LIMIT.SLOW_MS` is said, on the page and the device alike, in a measuring
  build only; a core's trap says its error with its stack. Kept.
- **The GPU's use in every run's state** (`c46876c841`): `gpu_util`, the Mac's
  IOAccelerator "Device Utilization %", kgsl's `gpu_busy_percentage` on Android, none on
  an iPhone. Kept.
- **A static review of the WGSL**, three readers: every loop has a bound, no lane waits
  on another, and three GPU counts were read without being held to their list's room.
  Those three were clamped, the holes' queue was assembled per observer, and the world
  group was split per observer (`29b260cc38`). Reading the node cache in place of asking
  the world in queue_last.wgsl `shows` failed `hand_a_lack_shows_the_deepest_node_held`
  and was dropped.
- **Instruments' Metal System Trace**: over all processes in a 40 s window, and attached
  to Safari's GPU process from the run's start. Kept as the way to catch one.
- **A GPU load beside Safari**: pxl-render's `decor` GPU test run without pause. It froze
  the whole Mac (below). Dropped.

## What came of it

| batch | build, as each run's report.txt gives it | runs | stalled | beside it |
|---|---|---|---|---|
| c, d, e, f, g | `35e6280154`, `b6af256d04`, `f97b86384f`, `f0b858683d-dirty`, `32186b37bd-dirty`; the belt from g on | 30 | 13 did not finish (c-3 to c-6, d-2, d-3, d-6, e-5, f-2 to f-5, g-6) | WebKit processes left from earlier runs (read in the session, below) |
| h | `29b260cc38-dirty` | 6 | 1 (h-2) | not recorded |
| m | `29b260cc38-dirty`, a 40 s trace window | 5 | 1 (m-5) | another session's `decor` test and an unnamed process, seen in m-5's trace |
| n | `29b260cc38-dirty`, the trace attached | 10 | 0 | the other sessions' GPU work stopped, as the session coordinating them said |
| c2 | `29b260cc38-dirty`, `decor` looped as load | 2 | the Mac froze | the load |

- **h-2.** Stalled at about 81 s (its read unanswered 1005 ms at 82.4 s; turns 2873, one
  frame unfinished) and stayed so, the read unanswered 158 s at the run's 240 s limit.
  `gpu_util` read 95% at 86.5 s and 100% from 96.7 s on, and the page's, the GPU
  process's and the Metal compiler's CPU 0%. No `took` line, no trap, no Metal error.
  After Safari quit, the next run started at 35%.
- **m-5.** Stalled at about 30 s (its read unanswered 1004 ms at 31.3 s). The read was
  answered at 44.4 s and at 59.5 s, after 13 to 15 s each (turns 466, 468, 469; frames
  187, 188), and at 73.7 s, 43 s after the stall began, the core trapped:
  `InvalidStateError: GPUDevice.createCommandEncoder: Unable to make command encoder.`,
  in the record's `createCommandEncoder` (g-6 trapped at the same call). Its trace (read
  by `xctrace export`, not in the run's logs) kept only the last ~4 s, all inside the
  stall: vertex work in 1246 slices (3.5 s) under submission 193,285,514, and fragment
  work as 663 partial renders (1.7 s) under 193,285,513, neither with a process named,
  both submitted before the window. That the two are one render job is read from their
  neighbouring numbers. WebKit's GPU process made no command buffer, encoder or label in
  those 4 s.
- **The attached traces.** Ten runs, none stalled, their traces deleted. An eleventh run,
  l-1, kept its trace (1.1 GB, not kept here), which shows every pass's WebGPU label on
  its Metal encoder: `render ....#58:1437`, the
  file numbered as a published build numbers it, and `ts/client/core.paths.json` turning
  `#58` back into `renderer.rs`.
- **c2.** c2-1 ran clean. c2-2 began at 01:19:51 with the load running, and its page
  never said a progress line. Its screen captures timed out from 145 s, and the Mac
  froze. The reset counter of 01:30 holds `Boot faults: btn_rst,finger_reset force_off`,
  and there is no panic report.
- **Leftover WebKit processes.** Before the first reboot, ten `(com.apple.WebKit)`
  processes stood exiting (`?Es`, no memory); four had started with runs d-3, f-1, g-3
  and g-6. The GPU read 100% while the live processes' GPU time summed to 28%. Only a
  reboot cleared them. After it, every run ended with none (`stuck_webkit 0` in the
  batches' logs). The processes, the 100% and the 28% were read in the session, by `ps`
  and `ioreg`, and are in no run's log.
- **The GPU's resets.** IOAccelerator's `recoveryCount`, read in the session by `ioreg`
  and in no run's log: 2 at 22:31 on 11-11, the last about 17:12, as run r1 ran, which
  ran clean; 0 after each reboot. So the 13 failed runs from c-1 (18:19) to g-6 (22:00)
  came with no reset. It was not read during h-2 or m-5.

## How we know

- The 15 s: WebKit WebProcess.cpp `gpuProcessTimeoutDuration()` returns `15_s`, and the
  `took` lines of the runs before the belt.
- Each run's `said.txt` (`PXLRUN` progress, `state` lines, `took` lines, the trap) and
  `batch.log`, in the session's `tmp/ss`, not kept here.
- The traces: `xctrace export` of `metal-gpu-intervals`,
  `metal-application-encoders-list` and `metal-object-label`.
- Per-process GPU time: `ioreg -r -c AGXDeviceUserClient`, each client's
  `AppUsage[].accumulatedGPUTime`, by `IOUserClientCreator`.

## Not established

- Whose render job held the GPU in m-5, and so which pass, if any of ours, never ended.
- Whether the stalls need another GPU user: 2 in 11 with one possible beside them, 0 in
  10 with none, the last ten with a trace attached, which may change the timing too.
- Whether WebKit bug 321876 is it. Its shape is decor's model draw, and its signs are
  these: utilization held at 100%, no reset through the failed runs of 11-11, and a hang
  outliving the browser. Its report saw it on M3 and later, and this is an M1 Max.
