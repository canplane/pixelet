# Journal

Whether the page can run the guest, its host and its devices in a worker, the
shell alone on the main thread, as the device app runs the guest on its own
thread: what the shell asks the guest and waits for, and what the browsers give
a worker. Main at c01333000e, whose change main holds now in `9f116e5f34`. A
proposal under study; 20260911 (one thread) stands until the user decides.
Chrome 156 and Safari 27.0.1 on the Mac.

Closed: the user decided on 2026-11-06 that the core runs apart from the UI, a
page's in a worker ([the core apart from the UI](../../minutes/20261106-the-runtime-is-glue.md)),
and the page's kernel runs in a worker on main since `2651534165`. The S23 and
the iPhone stay open (below). The client's `services/` is `service/` since that
commit.

## What the shell waits for

Seventeen of the shell's commands are calls (`"call": true` in
`rust/client/kernel/shell.json`): `host.call` runs a turn and returns the
guest's answer before it returns. Read by what their callers do with the answer:

| commands | caller | the answer | with the guest in a worker |
|---|---|---|---|
| `edit_marker_cancel`, `edit_marker_commit`, `surface_close`, `scene_draw`, `setting_set`, `scene_chat`, `camera_north_up`, `sensor_locate`, `camera_goto`, `perf_start`, `perf_stop` | `port/` | dropped | a send: `"call"` off |
| `fs_mount` | `sys/fs/source.ts` | refused or taken | stays a call: `sys/` goes into the worker with the guest |
| `surface_open` | `services/surface.tsx`, `surface.native.tsx` | the handle, before the canvas is bound | a request and its completion; on the page the request carries the canvas, transferred |
| `edit_tool` | `shell/world/dock/store.ts` | the tool is kept only where taken | a completion, or the held tool in the snapshot |
| `edit_marker_text` | `shell/world/dock/message.tsx` | whether the text fits, each keystroke | a completion, or the fit in the snapshot |
| `perf_stats`, `perf_rings` | `port/perf.ts` | an address in the guest's memory, read there | the reply carries the bytes |

The shell reads the guest's memory outside the calls too, which a worker
takes away:

- `port/snapshot.ts:54`: SHELL_SNAPSHOT names where the snapshot lies, and the
  shell reads it in place. The worker's host slices it and posts it.
- `port/perf.ts`: the summary, the rings and the trace read the guest's memory.
- `port/text.ts`: TEXT_LAYOUT and TEXT_RASTER read and write it; they go into
  the worker with the guest.

## What the browsers give a worker

`probe/`: the page hands its canvas to a worker, which draws it with WebGPU on
its own animation frames, measures and draws the kernel's text as
`services/text.ts` does, and answers the page's messages (`probe/run.js`).

| | Chrome 156, Mac | Safari 27.0.1, Mac |
|---|---|---|
| `navigator.gpu`, adapter, device in the worker | yes | yes |
| the canvas transferred, a `webgpu` context, the clear read back | yes, bgra8unorm | yes, bgra8unorm |
| `requestAnimationFrame` in the worker | yes, 8.3 ms, as the page's | yes, 17–18 ms, as the page's |
| a worker frame against the page's nearest, median | 3.6 ms | 4 ms |
| the text, 36 samples (6 scripts with emoji, 2 weights, 3 sizes), width and pixels | the same as the page's | the same as the page's |
| a message out and its answer, median / most | <0.1 / 0.2 ms | <1 / 1 ms (1 ms timer) |
| 1 MiB sliced and transferred back | 0.2 ms | ≤1 ms |

- The worker's frames run at the display's rate but their timestamps are not
  the page's: a frame the shell's DOM changes and the world's canvas are not
  yet shown to land on the same vsync.

## Open

- The S23's Chrome (not attached to adb today) and the iPhone's Safari.
- Whether the shell's DOM and the world's frame land together, in a page that
  changes both each frame.

## The page, its kernel in a worker

The worktree's page (`runtime/worker.ts`, `services/runtime.ts`; on main
`ts/client/src/runtime/worker.ts` and `service/runtime.ts`), exported
(`pnpm build:web`) and served (`web.ts serve`): Metro bundles the worker apart
(`worker-….js`, 199 KB, beside the 6.8 MB page).

- **Chrome 156, Mac**: the world drawn from the worker, WebGPU on the canvas
  the shell lent, the shell's dock and search over it from the snapshot's
  bytes. The kernel stopped at 25 to 70 s, `floor::fits` refusing the eye's
  beams (pxl-render gpu/ray/cast.rs, 36 247 802 words) at the window's full
  size: the renderer's own bound.
- **Safari 27.0.1, Mac**: the meshes drawn (the companion, decor, a car), the
  world's rays and the sky black, the shell whole: the renderer's, the
  wiring drawing what it is given or nothing.

Open: the S23's Chrome and the iPhone's Safari.
