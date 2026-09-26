// pxl-native/include/pxl_native.h

// What the device app calls in the kernel (src/lib.rs), declared for its
// Swift: the module map beside this makes it `import PxlNative`.

#pragma once

#include <stdint.h>

// The doorbell: a turn of quota_ms, and when the kernel wants the next, in ms
// (xpute_runtime::sched::quantum). The first lays the memory out.
double pxl_interrupt(double quota_ms);

// The memory: its first byte, with its bytes written through `bytes` (which
// may be null) — what the shell reads the boot block and the rings through.
uint8_t *pxl_memory(uint32_t *bytes);

// Opens the device, writing its uniform offset alignment and whether it
// times its passes: 0 when it is open, -1 when the system has none.
int32_t pxl_gpu_open(uint32_t *align, uint32_t *timestamps);

// The view's CAMetalLayer is there to draw into (main thread); the view keeps
// it until it detaches.
void pxl_surface_attach(void *layer);
void pxl_surface_detach(void);

// Binds the kernel's surface `handle` to the view's layer: 0, or -1 when no
// view has left one, -2 when no device is open, -3 when the system would not
// make a surface of the layer. Called on the kernel's thread, as unbind is.
int32_t pxl_surface_bind(uint32_t handle);
void pxl_surface_unbind(uint32_t handle);
