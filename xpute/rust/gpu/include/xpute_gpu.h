// xpute-gpu/include/xpute_gpu.h

// The C ABI of the GPU's runtime beside a core linked into its runtime's own
// process (xpute.h), as xpute-gpu's `native_abi!` exports it. Each call
// reaches the runtime on the core's thread while the caller waits. The runtime
// runs the core's stream as the core publishes it, and a completion kicks
// the core.

#pragma once

#include <stdint.h>

// Opens the device: 0 when it is open, -1 when none opened as the program
// asks. What it is, the core reads from the block its first call asks the
// runtime for.
int32_t xpute_gpu_open(void);

// Binds the core's surface `handle` to `window`, on an Apple platform a
// view's CAMetalLayer, which the caller keeps until it has unbound it: 0, or
// -2 when no device is open, -3 when the system would not make a surface of
// it, -4 when a newer handle has bound its slot since.
int32_t xpute_gpu_surface_bind(uint32_t handle, void *window);

// Lets `handle`'s window go, if it is still the one bound under its slot.
void xpute_gpu_surface_unbind(uint32_t handle);
