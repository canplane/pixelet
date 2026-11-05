// pxl-kernel/include/pxl_native.h

// What the device app calls in the kernel linked into it besides xpute's own
// (xpute.h, xpute_gpu.h), declared for its Swift: the module map beside this
// makes it `import PxlNative`.

#pragma once

#include <stddef.h>
#include <stdint.h>

// A body the app's fetch hands over still zstd (src/unpack.rs), which never
// reaches the kernel and so runs on the caller's thread, off the JavaScript
// thread: 0 with the decoded bytes at *out, *out_len long, the library's
// until pxl_unpacked_free; -1 where it does not decode.
int32_t pxl_unpack(const uint8_t *src, size_t len, uint8_t **out, size_t *out_len);
void pxl_unpacked_free(uint8_t *out, size_t len);
