// @xpute/native/ios/CXpute/xpute.h

// This is the C ABI of a core linked into its runtime's own process, as
// xpute-runtime's `native_core!` and `native_abi!` export it, for an app in C,
// Objective-C or Swift. The core runs on its own thread, which begins a wanted
// turn at a display frame (`xpute_vsync`; xpute-runtime native/thread.rs). The
// app's thread waits on the core only for a turn it runs, and it answers the
// core's calls.

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

// Runs a turn that ends no sooner than deadline_ms after it starts, and waits
// until a turn begun after this call has ended, answering the core's calls
// meanwhile. Returns when the core wants the next turn, in ms: 0 for at once,
// -1 for never. The first call boots the core, and its first turn checks
// the front the runtime laid against the core's manifest. Returns NaN once the
// core has stopped: its thread is gone, and no later turn runs.
double xpute_run(double deadline_ms);

// Asks for a turn at the next display frame, after something came that the
// core has to take.
void xpute_wake(void);

// Says that an input came now, so the turns are interactive for a while.
void xpute_input(void);

// The platform's display link calls this at each display frame, age_ms after
// that frame's time. A frame begins a turn when one is wanted and the pacing
// admits it.
void xpute_vsync(double age_ms);

// From now on, only a turn `xpute_run` runs may begin. An app calls this
// while it is hidden.
void xpute_pause(void);

// Turns begin again at the display's frames.
void xpute_resume(void);

// Returns the first byte of the core's memory, and writes its size through
// `bytes`, which may be null. The runtime lays the front's header in this
// memory, and a call's frame lies in it.
uint8_t *xpute_mem(uint32_t *bytes);

// Makes `answer` answer the core's calls; null sets none. Each call is a frame
// laid at an offset of the memory, and it is answered with a value or -errno on
// the thread waiting in `xpute_run` or on the thread calling `xpute_answer`.
// `answer` must not call into the core.
void xpute_call_on(int32_t (*answer)(uint32_t at));

// Makes `f` be called from the core's thread with news; null sets none. The
// news is that frames wait for `xpute_take`, that a call waits for `xpute_answer`, or that the core
// has stopped. `f` must not call into the core: it hands the news to the app's
// thread, which then calls `xpute_answer` and `xpute_take`.
void xpute_notify_on(void (*f)(void));

// Answers, on this thread, the call that waits for its answer, if one does.
void xpute_answer(void);

// Starts the runtime's own thread, the app having laid the front's header, and
// forgets what an earlier app attached left; what the core sent is still
// served. `fs` says whether the manifest asks for the file device.
void xpute_attach(bool fs);

// The program's numbers for frame pacing (@xpute/runtime sched/frame_pacing.ts
// `FramePacingConfig`), given before the turns run.
void xpute_pace(double pace_hz, double margin_share, double batch_frames, double settle_ms);

// Queues a frame for the core.
void xpute_post(uint32_t tag, uint32_t cmd, uint32_t flags, int32_t result, const uint8_t *pkt,
                size_t len);

// Hands over what the app has not heard from the core, a batch of
// frames: six little-endian u32s (tag, command, flags, result, the packet's
// length, the named memory's length), then each of those bytes padded to 8.
// The bytes are the app's until `xpute_taken_free`.
uint8_t *xpute_take(size_t *len);
void xpute_taken_free(uint8_t *at, size_t len);

// Completes the open of `fd` with the file's bytes, or with a negative result
// and the number the store refused with.
void xpute_fs_complete_open(uint32_t fd, int32_t result, uint32_t detail, const uint8_t *bytes,
                     size_t len);
