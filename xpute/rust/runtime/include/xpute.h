// xpute-runtime/include/xpute.h

// The C ABI of a core linked into its runtime's own process, as xpute-runtime's
// `native_core!` and `native_abi!` export it, for an app in C, Objective-C
// or Swift. The core runs on its own thread while it has work and sleeps
// until it is kicked, which the app does at the moment its policy gives for
// the time the core asked for (`xpute_ack`; xpute-runtime native/thread.rs);
// the app's thread waits on it only for a turn it rings, and answers the
// core's calls.

#pragma once

#include <stdint.h>

// A turn ending no sooner than deadline_ms from its rise, the caller waiting until one begun after
// this call has ended, answering the core's calls meanwhile: when the
// core wants the next, in ms (0 at once, -1 for none). Back early where no
// turn may begin until the app reads the completion ring. The first boots
// the core and holds the front the runtime laid to its machine. NaN where the
// core has stopped: its thread is gone, and so is every later turn.
double xpute_run(double deadline_ms);

// A turn wanted at once, after the app has written the submission ring, the
// next ending no sooner than deadline_ms from its rise, none of its own
// where negative; it returns at
// once. The calling thread answers the core's calls from now on.
void xpute_kick(double deadline_ms);

// No turn begins from now on but one `xpute_run` rings; a kick waits for
// `xpute_resume`. For an app that is hidden.
void xpute_pause(void);

// Turns begin again as kicks ask, one that came while paused at once.
void xpute_resume(void);

// The memory: its first byte, with its bytes written through `bytes` (which
// may be null), what the runtime lays the front's header and the rings in and
// reads the rings through.
uint8_t *xpute_mem(uint32_t *bytes);

// `answer`, or none for null, answers the core's calls: each a frame
// laid at an offset of the memory, answered on the thread waiting in
// `xpute_run` or the one calling `xpute_answer`, with a value or -errno. It
// must not call into the core.
void xpute_call_on(int32_t (*answer)(uint32_t at));

// `f`, or none for null, is called from the core's thread when the core has
// posted or asked since the app last called `xpute_ack`, when a call
// waits for `xpute_answer`, and when the core has stopped. It must not call
// into the core: it hands the news to the app's thread, which then calls
// `xpute_answer` and `xpute_ack` and reads the completion ring.
void xpute_notify_on(void (*f)(void));

// The call waiting for its answer, if one does, answered on this thread.
void xpute_answer(void);

// What the turns since the last call asked: when the core wants the next, in
// ms (0 at once, -1 for none, NaN once it has stopped), the longest of them,
// in ms, written through `turn_ms` (which may be null). The core's thread
// keeps no timer: the app kicks at the moment its policy gives for that
// wake; a turn that answered 0 is told as 0.
double xpute_ack(double *turn_ms);
