// @xpute/gpu-native-runtime/ios/CXputeGpu/xpute_gpu.h

// This is the C ABI of the GPU's runtime, for a core linked into its runtime's
// own process (xpute.h); xpute-gpu-guest's `native_abi!` exports it. The
// runtime runs the core's stream as each entry returns, and asks for an entry
// when a request it holds completes. A device the system loses is let go: the
// windows bound stay bound, the app is told (`xpute_gpu_lost_on`), and the
// next `xpute_gpu_open` opens a new device, which binds them again.

#pragma once

#include <stdint.h>

// Opens a device, keeping its compiled pipelines in `cache_dir`, the directory
// the system keeps the app's caches in, or nowhere when it is null. Returns 0
// when one is open, newly or already, and -ENODEV (-19) when no device opens
// as the program asks. After a loss it opens a new device. The core learns what the
// device is from the device's block, which it asks the runtime for.
int32_t xpute_gpu_open(const char *cache_dir);

// Makes `f` be told of each device lost, once a loss; null sets none. It is
// called on the thread that found the loss, after the runtime has written the
// loss into the core's device block and let the device go. `f` must not call
// into the GPU's runtime: it hands the news to the app's thread, which opens a
// device again with `xpute_gpu_open`.
void xpute_gpu_lost_on(void (*f)(void));

// Binds the core's surface `handle` to `window`: on an Apple platform a
// view's CAMetalLayer, on Android a view's ANativeWindow, live at the call. The
// runtime holds a reference of its own on the window, retained or acquired,
// until the window is unbound. It binds on the GPU's thread and returns at
// once: 0, or -EINVAL (-22) for a null window. Where the system makes no
// surface of the window, or a newer handle has bound the slot since, it is
// logged and the window let go. While no device is open the window is kept,
// and the next device binds it.
int32_t xpute_gpu_surface_bind(uint32_t handle, void *window);

// Lets the window `handle` bound go: its surface, if `handle` is still the one
// bound in its slot, and the runtime's reference on the window in any case.
void xpute_gpu_surface_unbind(uint32_t handle);
