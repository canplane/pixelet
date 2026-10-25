// pxl-native/include/pxl_native.h

// What the device app calls in the guest linked into it (src/lib.rs),
// declared for its Swift: the module map beside this makes it
// `import PxlNative`. Whatever reaches the guest runs on the guest's own
// thread while the caller waits (xpute-host native `Guest::run`), so the
// caller may be any thread, one call at a time.

#pragma once

#include <stddef.h>
#include <stdint.h>

// The doorbell: a turn of quota_ms, and when the kernel wants the next, in ms
// (xpute_guest::sched::quantum). The first lays the memory out.
double pxl_interrupt(double quota_ms);

// The memory: its first byte, with its bytes written through `bytes` (which
// may be null) — what the shell reads the directory and the rings through.
uint8_t *pxl_memory(uint32_t *bytes);

// Opens the device: 0 when it is open, -1 when the system has none. What it
// is, the kernel reads from the block its first call asks the host for.
int32_t pxl_gpu_open(void);

// `f` is what the device's host calls to ask a turn, from whichever thread
// learned of a completion (an image decoded): it sends the app an event, and
// JavaScript asks for the turn that reads it.
void pxl_wake_on(void (*f)(void));

// Binds the kernel's surface `handle` to `layer`, a view's CAMetalLayer,
// which the view keeps until it has unbound it: 0, or -2 when no device is
// open, -3 when the system would not make a surface of the layer.
int32_t pxl_surface_bind(uint32_t handle, void *layer);
// Lets `handle`'s layer go, if it is still the one bound under its slot.
void pxl_surface_unbind(uint32_t handle);

// A body the app's fetch hands over still zstd (src/unpack.rs), which never
// reaches the kernel and so runs on the caller's thread, off the JavaScript
// thread: 0 with the decoded bytes at *out, *out_len long, the library's
// until pxl_unpacked_free; -1 where it does not decode.
int32_t pxl_unpack(const uint8_t *src, size_t len, uint8_t **out, size_t *out_len);
void pxl_unpacked_free(uint8_t *out, size_t len);
