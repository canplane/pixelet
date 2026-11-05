// xpute-host/include/xpute.h

// The C ABI of a guest linked into its host's own process, as xpute-host's
// `native_guest!` and `native_abi!` export it, for an app in C, Objective-C
// or Swift. Whatever reaches the guest runs on the guest's own thread while
// the caller waits (xpute-host native `Guest::run`), so the caller may be any
// thread, one call at a time.

#pragma once

#include <stdint.h>

// The entry, the doorbell: a turn of quota_ms, and when the guest wants the
// next, in ms. The first holds the front the host laid to the guest's
// machine, and stops the guest where it differs. NaN where the guest has
// stopped: its thread is gone, and so is every later turn.
double xpute_run(double quota_ms);

// The memory: its first byte, with its bytes written through `bytes` (which
// may be null), what the host lays the front's header and the rings in and
// reads the rings through.
uint8_t *xpute_mem(uint32_t *bytes);

// `answer`, or none for null, answers the guest's hostcalls: each a frame
// laid at an offset of the memory, handed back to the thread waiting in
// `xpute_run` and answered there before the turn goes on, with a value or
// -errno. It must not call into the guest.
void xpute_hostcall_on(int32_t (*answer)(uint32_t at));
