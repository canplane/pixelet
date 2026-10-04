# A device's kernel on a thread of its own

**2026-09-26** — found while giving the native kernel its images, decided in
the conversation. It settles which thread a device's kernel runs on, and why
that is not the worker question
[20260911-single-thread-core-and-when-to-spawn-a-worker.md](./closed/20260911-single-thread-core-and-when-to-spawn-a-worker.md)
parked.

## What happened

On a device the kernel is linked into the app and draws through wgpu itself
(crates/pxl/web/render/src/gpu/api/device.rs). The shell rang it from React
Native's JavaScript thread, so every turn ran on that thread's stack. Once
PNGs decoded, the first skin uploaded killed the app with a stack overflow,
on every launch: `pxl_render::gpu::renderer::skin_group` alone reserves
0x38000 bytes (224 KiB) of stack, because wgpu's calls inlined into it bring
their large structures with them, and the JavaScript thread has 1 MiB —
NSThread's 512 KiB default, doubled in React Native's
`RCTJSThreadManager.mm`.

The page never meets this. Its renderer only records its calls for the host
to run, so no wgpu frame is ever on the kernel's stack, and a WebAssembly
module's stack is its own, sized when it is linked.

## The choice

**A thread of the kernel's own (taken).** `pxl-native` owns one thread with a
stack it sizes, and every C ABI entry — the doorbell, opening the device,
binding a surface — hands its work to that thread and waits for it. The
kernel's stack then answers to nothing React Native configures, and Android
and Quest take the same code.

**Shrinking the frames (not taken).** `#[inline(never)]` on the device's calls
and boxing their largest structures would push the peak down, but whether it
then fits in 1 MiB rests on wgpu's internal sizes, which nothing here bounds
and any wgpu release can change. That is a fit by luck, not an invariant.

**The stack is 8 MiB, said aloud.** It is the main thread's default on Linux
and macOS, not a number derived from anything: how deep wgpu's internals go
has no bound this tree can state.

## Why this is not a worker

The worker question was about running work alongside the core, and it was
closed on the web for reasons a device does not have:

- A worker on the web shares memory only through SharedArrayBuffer, and with
  it the COOP/COEP headers this project avoids; otherwise it copies.
- The page's main thread may not block: `Atomics.wait` is refused there, so a
  call cannot hand work to another thread and wait for the answer.

On a device, threads share one address space, so the memory the shell holds —
the buffer over the core's region, no copy — is the kernel's thread's too,
and a thread may block on another. The handoff is synchronous: the
JavaScript thread calls, the kernel's thread runs the turn, the JavaScript
thread waits until it is done. Only one of them runs at a time, so the core
is as single-threaded as 20260911 says, and "one thread, one call at a time"
still holds, of the kernel's thread. What changed is which thread that is.

The cost is a handoff per call, some microseconds, a few times a frame.

## Where it stands

Built with the device's images (crates/pxl/web/native/src/thread.rs): with
the texture pack, the skins and the moon decoded and uploaded on the
simulator, the turns that overflowed the JavaScript thread's stack run.

## 2026-10-05: a layer is configured on the main thread

iOS 27 logs every change to a view's layer made off the main thread as not
allowed, and configuring a wgpu surface sets its `CAMetalLayer`'s size and
format. The kernel's thread made every configure — at a bind, at a resize the
kernel decided, and when a drawable came back suboptimal or outdated.

What a layer is set to is now the main thread's, and the kernel's thread only
takes drawables and shows them. A surface whose wanted configuration is not
yet applied is stale and draws no frame. The host says at `open_device` where
a stale one is configured (pxl-render `gpu/api/device.rs` `ConfigureAt`):

- An iPhone sends it to the main queue, which configures it while the kernel
  is idle, holding the handoff (pxl-native `surface.rs`, `thread.rs`).
- Android, which has no such rule, configures it where it was found stale.

The cost is a frame not drawn while a surface waits for the main thread.

