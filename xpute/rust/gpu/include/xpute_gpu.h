// xpute-gpu/include/xpute_gpu.h

// The C ABI of the GPU's runtime, for a core linked into its runtime's own
// process (xpute.h); xpute-gpu's `native_abi!` exports it. Each call runs on
// the core's thread while the caller waits. The runtime runs the core's
// stream as the core publishes it, and kicks the core when a request it
// holds completes.

#pragma once

#include <stdint.h>

// Opens the device. Returns 0 when it is open, and -1 when no device opens as
// the program asks. The core learns what the device is from the device's
// block, which it asks the runtime for.
int32_t xpute_gpu_open(void);

// Binds the core's surface `handle` to `window`: on an Apple platform a
// view's CAMetalLayer, on Android a view's ANativeWindow. The caller keeps
// the window alive until it has unbound it. Returns 0, or -2 when no device is
// open, -3 when the system makes no surface of the window, and -4 when a
// newer handle has bound the slot since.
int32_t xpute_gpu_surface_bind(uint32_t handle, void *window);

// Lets the window of `handle` go, if `handle` is still the one bound in its
// slot.
void xpute_gpu_surface_unbind(uint32_t handle);
