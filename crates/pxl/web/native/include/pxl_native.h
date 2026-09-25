// pxl-native/include/pxl_native.h

// What the device app calls in the kernel (src/lib.rs), declared for its
// Swift: the module map beside this makes it `import PxlNative`.

#pragma once

#include <stdint.h>

// A surface on `layer`, a CAMetalLayer, width x height pixels: 0 when it is
// drawn into, -1 when no device would take it. The layer outlives it.
int32_t pxl_surface_open(void *layer, uint32_t width, uint32_t height);

// The layer is now width x height pixels.
void pxl_surface_resize(uint32_t width, uint32_t height);

// One frame.
void pxl_surface_draw(void);

// The view is going: the surface and its device with it.
void pxl_surface_close(void);
