// pxl-core/include/pxl_native.h

// What the device app calls in the core linked into it besides xpute's own
// (xpute.h, xpute_gpu.h), declared for its Swift: the module map beside this
// makes it `import PxlNative`.

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

// A body the app's fetch hands over still zstd (src/sys/unpack.rs), which never
// reaches the core and so runs on the caller's thread, off the JavaScript
// thread: 0 with the decoded bytes at *out, *out_len long, the library's
// until pxl_unpacked_free; -1 where it does not decode.
int32_t pxl_unpack(const uint8_t *src, size_t len, uint8_t **out, size_t *out_len);
void pxl_unpacked_free(uint8_t *out, size_t len);

// The body of the file the core opened as fd, fetched off the JavaScript thread;
// zstd says whether it still needs decoding. Returns false where it does not
// decode, and the runtime has heard nothing.
bool pxl_file_body(uint32_t fd, const uint8_t *body, size_t len, bool zstd);

// The system's text, called on the core's own thread: how wide a line of UTF-8
// runs at a CSS weight and a size, and a line drawn from x into width by height
// pixels of RGBA, top row first, alpha not premultiplied.
void pxl_text_on(
    double (*width)(double weight, double px, const uint8_t *text, size_t len),
    void (*raster)(double weight, double px, uint32_t rgba, const uint8_t *text, size_t len,
                   double x, uint32_t width, uint32_t height, uint8_t *out));
