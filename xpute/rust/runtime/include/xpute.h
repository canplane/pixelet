// xpute-runtime/include/xpute.h

// This is the C ABI of a core linked into its runtime's own process, as
// xpute-runtime's `native_core!` and `native_abi!` export it, for an app in C,
// Objective-C or Swift. The core runs on its own thread, which begins a wanted
// turn at a display frame (`xpute_vsync`; xpute-runtime native/thread.rs). The
// app's thread waits on the core only for a turn it rings, and it answers the
// core's calls.

#pragma once

#include <stdbool.h>
#include <stdint.h>

// Rings a turn that ends no sooner than deadline_ms after its rise, and waits
// until a turn begun after this call has ended, answering the core's calls
// meanwhile. Returns when the core wants the next turn, in ms: 0 for at once,
// -1 for never. It returns early if no turn may begin until the app reads the
// completion ring. The first call boots the core, and its first turn checks
// the front the runtime laid against the core's machine. Returns NaN once the
// core has stopped: its thread is gone, and no later turn runs.
double xpute_run(double deadline_ms);

// Asks for a turn at the next display frame, after the app has written the
// submission ring or read the completion ring.
void xpute_wake(void);

// Says that an input came now, so the turns are interactive for a while.
void xpute_input(void);

// The platform's display link calls this at each display frame, age_ms after
// that frame's time. A frame begins a turn when one is wanted and the pacing
// admits it.
void xpute_vsync(double age_ms);

// From now on, only a turn `xpute_run` rings may begin. An app calls this
// while it is hidden.
void xpute_pause(void);

// Turns begin again at the display's frames.
void xpute_resume(void);

// Returns the first byte of the core's memory, and writes its size through
// `bytes`, which may be null. The runtime lays the front's header and the
// rings in this memory and reads the rings through it.
uint8_t *xpute_mem(uint32_t *bytes);

// Load the word at byte `at` of the memory with acquire, and store `word`
// there with release. They are for an app whose language has no atomics of its
// own, to read and write words the core's thread touches at the same time,
// such as a ring's positions.
uint32_t xpute_load(uint32_t at);
void xpute_store(uint32_t at, uint32_t word);

// Makes `answer` answer the core's calls; null sets none. Each call is a frame
// laid at an offset of the memory, and it is answered with a value or -errno on
// the thread waiting in `xpute_run` or on the thread calling `xpute_answer`.
// `answer` must not call into the core.
void xpute_call_on(int32_t (*answer)(uint32_t at));

// Makes `f` be called from the core's thread with news; null sets none. The
// news is that a turn has ended since the app last
// called `xpute_heard`, that a call waits for `xpute_answer`, or that the core
// has stopped. `f` must not call into the core: it hands the news to the app's
// thread, which then calls `xpute_answer` and `xpute_heard` and reads the
// completion ring.
void xpute_notify_on(void (*f)(void));

// Answers, on this thread, the call that waits for its answer, if one does.
void xpute_answer(void);

// Says the news of `xpute_notify_on` was heard, so that the next is told, and
// returns whether the core has stopped.
bool xpute_heard(void);
