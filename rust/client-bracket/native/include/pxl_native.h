// pxl-native/include/pxl_native.h

// What the device app calls in the guest linked into it (src/lib.rs),
// declared for its Swift: the module map beside this makes it
// `import PxlNative`. Whatever reaches the guest runs on the guest's own
// thread while the caller waits (xpute-host native `Guest::run`), so the
// caller may be any thread, one call at a time. The entry and the memory,
// and the GPU's host, are xpute's, under the names the floor gives them
// (xpute-host native `native_abi!`, xpute-gpu host native `native_abi!`);
// the rest are the app's.

#pragma once

#include <stddef.h>
#include <stdint.h>

// The entry, the doorbell: a turn of quota_ms, and when the kernel wants the
// next, in ms (xpute_guest::sched::quantum). The first lays the memory out.
double xpute_intr(double quota_ms);

// The memory: its first byte, with its bytes written through `bytes` (which
// may be null) — what the shell reads the directory and the rings through.
uint8_t *xpute_mem(uint32_t *bytes);

// `answer`, or none for null, answers the kernel's hostcalls: each a record
// at an offset of the memory, handed back to the thread waiting in
// `xpute_intr` and answered there before the turn goes on, with a value or
// -errno. It must not call into the guest.
void xpute_hostcall_on(int32_t (*answer)(uint32_t at));

// Opens the device: 0 when it is open, -1 when the system has none. What it
// is, the kernel reads from the block its first call asks the host for.
int32_t xpute_gpu_open(void);

// `f` is what the device's host calls to ask a turn, from whichever thread
// learned of a completion (an image decoded): it sends the app an event, and
// JavaScript asks for the turn that reads it.
void xpute_gpu_wake_on(void (*f)(void));

// Binds the kernel's surface `handle` to `window`, on an iPhone a view's
// CAMetalLayer (src/surface.rs `target`), which the view keeps until it has
// unbound it: 0, or -2 when no device is open, -3 when the system would not
// make a surface of it, -4 when a newer handle has bound its slot since.
int32_t xpute_gpu_surface_bind(uint32_t handle, void *window);
// Lets `handle`'s window go, if it is still the one bound under its slot.
void xpute_gpu_surface_unbind(uint32_t handle);

// A body the app's fetch hands over still zstd (src/unpack.rs), which never
// reaches the kernel and so runs on the caller's thread, off the JavaScript
// thread: 0 with the decoded bytes at *out, *out_len long, the library's
// until pxl_unpacked_free; -1 where it does not decode.
int32_t pxl_unpack(const uint8_t *src, size_t len, uint8_t **out, size_t *out_len);
void pxl_unpacked_free(uint8_t *out, size_t len);
