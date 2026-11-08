# Pipelines are made by ticket before the first frame

**2026-11-02** — recorded with the user. Main was at `315e74e573`, the trace
below taken at `661d37bf9d`; the commit that carries this record lands it. The
commits are named as `original` keeps them.

## What was asked

The first load must never hold the browser's main thread through the GPU's
objects being made, as the standard way, whatever thread xpute later runs on.
The user decided this.

## What the code did

- The first frame with a device made the renderer (`Renderer::new`) and the
  ground's pipelines (`RaysPass::hand`); the first frame that cast made the
  cast's (`RaysPass::make`, `Raster::new`, `Generator::new`), most of the WGSL
  (4.1 of 4.8 MB at `9cd5f6ea83`; 2.83 MB in all at `661d37bf9d`), every module
  and pipeline with WebGPU's sync calls.
- The web GPU host ran each as `createShaderModule` and the sync
  `createComputePipeline`/`createRenderPipeline` (xpute ts/gpu `GpuHost`).
- The WGSL went into the 4 MiB stream (`stream::BYTES`) inline, outside the
  frame's transaction, where a record that does not fit stops the guest
  (`ENOSPC`). At `9cd5f6ea83`, where the first frame with a device also cast,
  both sets landed in one turn's stream, 4.8 MB, and the guest stopped:
  confirmed by reading, never seen. At `661d37bf9d` all of it is 2.83 MB and
  fits.

## What holds which thread

Measured on main `661d37bf9d`: Chrome 156, Apple Metal, a fresh profile a run
(cold caches), one load traced (Chrome's trace, every process) and one under
the CDP sampling profiler with the Long Animation Frames.

- **The page's main thread is not held by them.** Its longest task in the
  load was 83 ms, its three over 50 ms (191 ms in all) the start's script
  evaluation. The JS inside `createShaderModule` and the pipeline calls is
  below the profiler's 250 µs samples (another session: about 5 ms in all).
- **The GPU process's main thread is.** The objects' making took 878 ms there
  (`APICreateShaderModule` 520 ms over 134 calls, `APICreateComputePipeline`
  240 ms over 37, `APICreateRenderPipeline` 118 ms over 46), 844 ms of it in
  tasks over 50 ms, all between 8.7 and 9.8 s after navigation: WebGPU flushes
  of 64, 202, 395 and 187 ms. Right after, the compositor's swaps waited on
  Dawn for 208, 232 and 188 ms, and the display drew no frame for up to 432,
  421 and 380 ms. The world showed at about 10.1 s. A pipeline's MSL (Tint)
  and Metal function are made inside the sync calls on that thread; Chrome's
  own async pipelines make them on its thread pool
  (`CreatePipelineAsyncEvent`).
- **Not explained:** a 5.8 s gap in the display's frames at about 3.3 s, with
  no object made in it; and, in the profiled run (shown at 20.2 s), a long
  frame of 8.1 s with no blocking time, its render delayed, and a 2.4 s run of
  native samples on the page's main thread at about 9.1 s, neither in the
  traced run.

## Premises

A design that breaks one stops for a redesign; it is not patched.

0. What must not be held is the GPU process's main thread: every frame the
   browser presents waits on it, so while it is held the page's frames, a
   loading bar among them, stand still (rewritten from "the page's main
   thread", which the trace above refuted).
1. The async creation is in xpute's GPU spec in WebGPU's words (`spec/gpu/`),
   an op whose completion comes back by ticket, as `COPY_EXTERNAL_IMAGE` and
   the buffer read back already do. The guest knows a pipeline is pending.
2. The guest never records a use of a pipeline that has not completed. The
   renderer draws its first frame, and so casts and runs any pass that needs a
   pipeline, only once its pipelines are made. No record is copied out of the
   core memory to be replayed.
3. The native GPU host (wgpu, no async pipeline) answers the same op at once,
   by the same ticket path. The guest's code is one path for both.
4. Shader modules are made across turns by a byte budget that also keeps each
   turn's stream within its room, so the `ENOSPC` case cannot occur.
5. The share the shell's loading bar shows comes through the snapshot the
   shell already reads. The host gives no number a meaning; the shell
   computes nothing.
6. xpute holds no pixelet word. One owner per fact; spec changes go through
   `make spec` and the generated code.

## Design

- **xpute.** Two ops, `RENDER_PIPELINE_ASYNC` and `COMPUTE_PIPELINE_ASYNC`
  (`spec/gpu/api.json`): the sync op's words, then a word and a ticket. The web
  host calls `createRenderPipelineAsync` or `createComputePipelineAsync`, keeps
  the pipeline at its handle once the promise resolves, where the handle still
  names it, and writes `ticket | DONE` at the word where the ticket still
  stands. One the device refuses is made with the sync call, so its error is
  reported as the sync op's is, and its word written all the same. The native
  host makes it with wgpu's one call and answers by the same pending path as
  a read back, at the next poll. The encoders (Rust, C++) write both; an async
  pipeline counts among the objects as its sync op's. A golden program
  (`pipelines_async`) holds the encoders' words and both hosts to them.
- **The renderer is made in three steps** before its first frame
  (`gpu/renderer.rs` `render_frame`, `gpu/pipeline.rs` `Pipelines`):
  1. its shader modules, every shader of the record, a few each turn: at
     least one, then more while their sources stay within `stream::MODULE_BYTES`
     and the stream keeps `stream::RESERVE` past them (`room`), as an upload
     waits; the stream's records outside a frame can no longer outgrow it;
  2. the renderer, with every pipeline it draws with made by the async ops,
     the cast's and the ground's (`RaysPass::make`, `GroundPipelines`) among
     them, each with its ticket's word in `Pipelines`;
  3. no frame until every word holds `DONE`; the cloud field, the one thing
     made at construction that draws, is baked then (`CloudPass::bake`).
  A frame turn before that draws nothing, reports `unready`, and asks for the
  next turn. A pipeline made after the renderer began drawing is a bug
  (`Pipelines::build_*` refuse it with `EBUSY`); a preview's pipelines are
  the renderer's own, found made. Compute pipelines are kept by their shader
  and group, as render pipelines are by their record and targets, so two
  owners asking the same one share it.
- **The bar.** The kernel's snapshot carries `renderer_made`, the share of
  that making done: half the modules by their bytes, half the pipelines made
  (`Pipelines::share`), 1 once the renderer draws. The world's loading toast
  shows it as a bar while it is short of 1. On 2026-11-03 the user moved the bar
  to the white screen ([what stands over the app](./20261103-what-stands-over-the-app.md)).

## Rejected

- **`a470bdfb88`, reverted by `6c6628bac5`.** Async hidden inside the web GPU
  host: a pipeline's handle named at once, and every record after a
  SET_PIPELINE naming one still being made copied out with the bytes of the
  guest's memory it read, then replayed. It breaks "a payload is read where it
  lies", the guest does not know, and the native host differs.
- **Lazy pipelines.** Each pipeline made the first time a pass sets
  it, mid-frame: it broke the frame's contract, a frame recording what it
  cannot yet use.
- **The whole core in a worker** (the xpute host, the guest and the GPU host
  on an OffscreenCanvas): rejected by the user, 2026-11-02. Unity Web, Figma
  and VS Code keep their core and renderer on the main thread; VS Code puts
  only its extension host in a worker. The core stays on the main thread, as
  doc/minutes/archive/20260911-single-thread-core-and-when-to-spawn-a-worker.md
  has it. Replaced on 2026-11-06: the core runs apart from the UI, on a page in a
  worker ([the runtime is glue](./20261106-the-runtime-is-glue.md)).
