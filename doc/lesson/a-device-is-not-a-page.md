# A device is not a page

**2026-11-09.** What the native runtime has to do itself that a browser does for
a page, learned on a Galaxy S23 and an iPhone 13 mini. The numbers are
[the app that did not move](../experiment/20261109-the-app-that-did-not-move/README.md).
Keep the two runtimes' contracts the same shape: the difference below is where
the work lands, and each fault here came of a native path that had drifted from
the page's.

## Keep clear of

- **A command that only reaches the core.** A core on its own thread is kicked
  as a command is sent, and that turn has no grant and draws nothing. A frame
  comes of a granted turn, and a pointer's move asks for none itself
  (`camera_pointer_move`). Every send asks for a turn on both cores
  (`Scheduler.wake`), so a move is drawn. Without it the touches arrive and the
  screen stands.
- **The stream held across a present.** On Android the Vulkan swapchain connects
  as EGL's API, so `queueBuffer` waits for the previous frame's fence
  (`BufferQueueProducer`, "Throttling EGL Production"). A stream that is handed
  back after the present is held for the GPU's time, the core records nothing,
  and frames are put off. `GpuRuntime::run` is the page's `run`: the records, the
  frame counted if the stream took a surface's texture, END back. The present
  comes after, and waits behind the stream.
- **A turn started from the UI's thread.** React Native's JavaScript thread
  holds the runtime and React at once, and its animation frame gives the time
  the callback arrived. The scheduler offers the next display frame ahead of it
  (`next_frame`, from the platform's display link), and the core's thread waits
  for that frame. A frame offered and not taken is offered no more, a later kick
  does not put off a turn already due, and no turn is granted while one has not
  begun, since it takes all that was sent before it.
- **A write the queue has not heard of.** The page's `writeBuffer` is on the
  queue when it returns. The native runtime stages writes for the next submit, so
  it submits them before it destroys a buffer or a texture.
- **A second concept for what a frame is.** The count of unfinished frames goes
  up where the stream drew, as on the page. An admission flag kept beside it
  can be overwritten by the next turn's question before the stream it belongs
  to has run.
- **One command buffer a pass.** wgpu-core 30.0.1 encodes each pass into a hal
  command buffer of its own, with another before it for the pass's barriers. On
  Metal the driver holds each `MTLCommandBuffer` until it completes, about 2.3
  MiB (femtovg#367, gfx-rs/wgpu#10494, #10506), and a frame of many passes reached the
  iPhone's memory limit at boot. On Vulkan each pool, reset without
  `RELEASE_RESOURCES` and reused, keeps the memory of its heaviest frame
  (gfx-rs/wgpu#5738, an Adreno at 2.1 GB). The fork of wgpu (`canplane/fork.wgpu`) records a
  pass, encodes its barriers, then replays it into the encoder's open buffer, so an
  encoder is one hal command buffer on every backend: the prototype attached to
  #5738, whose recorder extends borrows that wgpu-core's trackers keep alive. The
  user allowed a patch where the fault is wgpu's on 2026-11-09.
- **Taking the simulator's word.** It reports the Apple2 family and runs indirect
  calls on the Mac's GPU, which `supportsFamily` cannot say. wgpu decides this
  by `target_abi = "sim"` (gfx-rs/wgpu#10189, carried in the vendored crate),
  with 256-byte constant buffer offsets. Metal API validation (`MTL_DEBUG_LAYER`,
  Xcode's Run) aborts on the first indirect call.

## Check

How a touch, a stall on the main thread and the iPhone's memory are measured is
the measure skill's (`.agents/skills/measure/SKILL.md`). On the S23, three runs
in a row read GPU p50 69, 93 and 139 ms as the phone heated and its GPU's clock
fell: a slow run follows the clock (kgsl `cur_freq`) more closely than the AP's
temperature
([the lights' tail](../experiment/20261107-the-lights-tail/README.md)).
