// pxl-native/include/pxl_native.h

// What the device app calls in the kernel (src/lib.rs), declared for its
// Swift: the module map beside this makes it `import PxlNative`. Whatever
// reaches the kernel runs on the kernel's own thread while the caller waits
// (src/thread.rs), so the caller may be any thread, one call at a time.

#pragma once

#include <stdint.h>

// The doorbell: a turn of quota_ms, and when the kernel wants the next, in ms
// (xpute_guest::sched::quantum). The first lays the memory out.
double pxl_interrupt(double quota_ms);

// The memory: its first byte, with its bytes written through `bytes` (which
// may be null) — what the shell reads the directory and the rings through.
uint8_t *pxl_memory(uint32_t *bytes);

// Opens the device, writing its uniform offset alignment and whether it
// times its passes: 0 when it is open, -1 when the system has none.
int32_t pxl_gpu_open(uint32_t *align, uint32_t *timestamps);

// Decodes the PNG or WebP of `bytes` bytes at `file` and holds it for the
// renderer as `handle`, which the kernel issued: 1, or 0 when it does not
// decode. Any thread.
uint32_t pxl_image_hold(uint32_t handle, const uint8_t *file, uint32_t bytes);

// Binds the kernel's surface `handle` to `layer`, a view's CAMetalLayer,
// which the view keeps until it has unbound it: 0, or -2 when no device is
// open, -3 when the system would not make a surface of the layer.
int32_t pxl_surface_bind(uint32_t handle, void *layer);
// Lets `handle`'s layer go, if it is still the one bound under its slot.
void pxl_surface_unbind(uint32_t handle);
