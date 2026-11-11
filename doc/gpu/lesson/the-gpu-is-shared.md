# The GPU is shared

**2026-11-12.** The Mac Safari stall, and the hang that took the whole Mac: what
was seen, what was found, and what is still open. Every program on the Mac draws
through one GPU: Safari's WebKit GPU process, the pxl-render tests, the frame
harness, the shader probe, another browser or app, and WindowServer, which
composites the screen. A job that does not end holds the GPU for all of them.
The runs and their numbers are in
[the Safari stall](../experiment/20261111-the-safari-stall/README.md).

## What was seen

- **The Safari stall.** The canvas stands and the UI lives. `gpu_util` (the
  IOAccelerator's `Device Utilization %`, the whole GPU's, whoever keeps it
  busy) stays busy, and the page's, the GPU process's and the Metal compiler's
  CPU stand at 0%. No `took` line comes. In one stall (m-5) the core trapped
  about 43 s in, in `createCommandEncoder` ("Unable to make command encoder"),
  the turn's next record, and its unanswered read fell back every 15 s or so;
  in the other (h-2) the read stayed unanswered, climbing to 158 s, with no
  trap, until the run's 240 s cap.
- **How often.** The Safari runs stalled 1 in 6 (no trace) and 1 in 5 (a Metal
  System Trace of all processes, its window 40 s), and 0 in 10 once the other
  sessions' GPU work had stopped (the trace attached to WebKit's GPU process
  from the start). Another session's `decor` GPU test ran during the stall in
  the 1 in 5 (m-5); whether one ran during the 1 in 6 is not recorded. The
  clean ten differ in both the other users and the trace's attachment, so the
  two are not told apart.
- **The whole Mac.** pxl-render's `decor` GPU test, looped without pause beside
  Safari, ran clean once and froze the whole Mac the second time. The second
  run's page never gave its first progress line, and from about 145 s the
  screen captures timed out. The Mac took a forced power-off: no panic report,
  and `Boot faults: btn_rst,finger_reset force_off` in
  `/Library/Logs/DiagnosticReports/ResetCounter-*.diag`. The Mac is an M1 Max
  on macOS 27.0.1, Safari 27.0.1 (22625.1.29.11.28).
- **In a Metal System Trace** of one stall, a render job ran for seconds: its
  vertex work, and its fragment work as repeated partial renders. Its
  submission came before the trace's window, so the trace named no process for
  it, and whose job it was is not known.
- `recoveryCount` (IOAccelerator), the driver's resets of the GPU, read 2 at 22:31
  on 2026-11-11, the last about 17:12 during a clean run, so the failed runs
  after it came with no reset, and 0 after each reboot. It was read in the
  session, not by the runs, and was not read during the later stalls.
- Before the reboot of 2026-11-11, ten WebKit processes left by earlier runs held
  the GPU until the reboot cleared them; the runs since logged none
  (`stuck_webkit 0`).

## What was found

- **WebKit bug 321876** matches the hang's shape: in one render pass, indirect
  draws from one arguments buffer, through a pipeline that declares a required
  vertex buffer layout, go through WebKit's `clampIndirectBufferToValidValues`,
  which writes every draw's arguments to one scratch slot. On an M3 Max the GPU
  then hangs past the browser's end: `Device Utilization %` latched at 100,
  `recoveryCount` never moving, a firmware "progress timeout", and a reboot to
  clear it. The reporter's way round is no vertex layout on an indirect
  pipeline, its attributes read from storage buffers. It is marked fixed
  (319317@main, WebKit PR 71794) and was reported on M3 and later, not on an
  M2 Ultra. decor draws so: its model meshes come through vertex buffers
  (`rust/client/render/src/frame/shader/record.rs` `MODEL_BUFS`), each group an indexed indirect draw from one
  arguments buffer in one pass (`rust/client/render/src/pass/decor.rs`). Whether this Mac carries the
  fault, and whether its hangs were this, is not known.
- **A synchronous call behind a busy GPU.** Each `writeBuffer` from the page
  was a synchronous call into WebKit's GPU process, and behind a busy GPU it
  blocked for WebKit's `gpuProcessTimeoutDuration`, 15 s. The page stages its
  writes on a belt now, as a device does (`xpute/ts/gpu/src/belt.ts`).
- **Chrome restarts a GPU process that does not answer** (its GPU watchdog),
  and the page's device is lost and opened again. WebKit leaves the canvas
  standing instead. The user saw no stall in Chrome on the Mac.
- **What the page can see.** Only its own queue: `onSubmittedWorkDone` and
  timestamp queries time its own work. The WebGPU specification notes that a
  page timing its queue can tell something else has started using the GPU; no
  implementation names the other process.
- **What the Mac can see.** Each process's GPU time is in its Metal clients'
  usage, readable without admin: `ioreg -a -r -d 1 -c AGXDeviceUserClient`,
  each client's `AppUsage[].accumulatedGPUTime` (ns) by its
  `IOUserClientCreator` (pid and name), as Activity Monitor's GPU column reads
  it.
- **A pass's WebGPU label reaches Metal** as the encoder's and the command
  buffer's label. A published build numbers its file paths
  (`render ....#58:1437`), and the build's `ts/client/core.paths.json` turns the
  number back into the file (`render renderer.rs:1437`). A Metal System Trace
  names a job's process only when attached to WebKit's GPU process before the
  run.

## Decided, and open

- The app does not detect the GPU's other users, and does not take 321876's
  way round (the user, in the minutes
  `doc/gpu/minutes/20261112-the-gpus-other-users-are-not-the-apps.md`).
- The repository's own tools hold the device they drive, one user at a time: a
  run, the shader probe, the renderer's GPU tests and the frame harness
  (`script/perf/measure.py` `hold`, the same minutes).
- Open: whose job held the GPU in the stall; whether this Mac and Safari carry
  321876.

## References

- WebKit bug 321876, GPU hangs unrecoverably when a render pass issues repeated
  indirect draws through `clampIndirectBufferToValidValues`:
  https://bugs.webkit.org/show_bug.cgi?id=321876
- WebKit commit 319317@main: https://commits.webkit.org/319317@main
- WebGPU specification, workload identification (a page timing its queue):
  https://www.w3.org/TR/webgpu/#security-workload-identification
- MDN, `GPUQueue.onSubmittedWorkDone()`:
  https://developer.mozilla.org/docs/Web/API/GPUQueue/onSubmittedWorkDone
- Apple, `MTLCommandBufferError.Code` (`.timeout`):
  https://developer.apple.com/documentation/metal/mtlcommandbuffererror-swift.struct/code
- oMLX issue 4224, AGX timeouts ("progress timeout") and `recoveryCount`:
  https://github.com/jundot/omlx/issues/4224
