// xpute-host/include/xpute.h

// The C ABI of a guest linked into its host's own process, as xpute-host's
// `native_guest!` and `native_abi!` export it, for an app in C, Objective-C
// or Swift. The guest runs on its own thread while it has work and sleeps
// until it is kicked or the time it asked for comes (xpute-host
// native/thread.rs); the app's thread waits on it only for a turn it rings,
// and answers the guest's hostcalls.

#pragma once

#include <stdint.h>

// A turn given at least quota_ms, the caller waiting until one begun after
// this call has ended, answering the guest's hostcalls meanwhile: when the
// guest wants the next, in ms (0 at once, -1 for none). Back early where no
// turn may begin until the app reads the completion ring. The first boots
// the guest and holds the front the host laid to its machine. NaN where the
// guest has stopped: its thread is gone, and so is every later turn.
double xpute_run(double quota_ms);

// A turn wanted at once, after the app has written the submission ring, the
// next given at least quota_ms, none of its own where negative; it returns at
// once. The calling thread answers the guest's hostcalls from now on.
void xpute_kick(double quota_ms);

// The memory: its first byte, with its bytes written through `bytes` (which
// may be null), what the host lays the front's header and the rings in and
// reads the rings through.
uint8_t *xpute_mem(uint32_t *bytes);

// `answer`, or none for null, answers the guest's hostcalls: each a frame
// laid at an offset of the memory, answered on the thread waiting in
// `xpute_run` or the one calling `xpute_answer`, with a value or -errno. It
// must not call into the guest.
void xpute_hostcall_on(int32_t (*answer)(uint32_t at));

// `f`, or none for null, is called from the guest's thread when the guest has
// posted or asked since the app last called `xpute_heard`, when a hostcall
// waits for `xpute_answer`, and when the guest has stopped. It must not call
// into the guest: it hands the news to the app's thread, which then calls
// `xpute_answer` and `xpute_heard` and reads the completion ring.
void xpute_notify_on(void (*f)(void));

// The hostcall waiting for its answer, if one does, answered on this thread.
void xpute_answer(void);

// What the turns since the last call asked: when the guest wants the next, in
// ms (0 at once, -1 for none, NaN once it has stopped), the longest of them,
// in ms, written through `turn_ms` (which may be null).
double xpute_heard(double *turn_ms);
