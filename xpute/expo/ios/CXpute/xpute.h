// @xpute/expo/ios/CXpute/xpute.h

// This is the C ABI of xpute's runtime linked into the app beside its guest,
// for an app in C, Objective-C or Swift; the app's native library exports it
// (xpute-runtime-native `native_abi!`). The runtime runs on the guest's
// thread, its event loop serving the guest's link, and carries the UI's calls
// to it: the UI's methods on the guest's objects (`xpute_post`), its answers
// to the methods of the objects it implements (`xpute_answer`), and a batch of
// what the runtime sends the UI, handed over on a notify (`xpute_take`). Every
// call that reaches the runtime posts a task onto the guest's thread's queue
// and returns.

#pragma once

#include <stddef.h>
#include <stdint.h>

// Starts the runtime on the guest's thread, once a process. `named`,
// `named_len` are the named handles of the interfaces the UI implements;
// `prefixes`, `prefixes_len` the prefixes the UI mounts openers at, in their
// order, each with the named handle of the interface whose resources they
// open, as a lowered `list<tuple<u32, list<token>>>` packet (the Canonical
// ABI, xpute:link/types `token`); `cache_dir`, `cache_dir_len` the directory
// the app keeps its caches in, its UTF-8 bytes and their count. Each call, the
// first too, attaches a new UI to the runtime, as a reload's, which implements
// what the first did, and returns the new UI's handle, which each of its
// `xpute_post`, `xpute_answer` and `xpute_take` names. One UI is attached at a
// time: a new one drops the last UI's handle, and with it what the last owned
// under it: the requests it owed are answered ECANCELED, as is each method of
// the objects it implements for the guest, the guest's objects it opened are
// dropped on the guest, the answer to a request it sent reaches no new UI, and
// the batch it did not take goes; the guest is then told the new UI attached
// (xpute:life `attached`). Nothing an old UI sent that has not reached the
// guest when a new one attaches reaches it: a method is dropped, and an answer
// refused, its request answered ECANCELED. Every method the UI implements is
// an `async func`, answered by `xpute_answer`: the runtime refuses to start
// the guest, a trap, where one is not.
uint32_t xpute_start(const uint32_t *named, size_t named_len, const uint8_t *prefixes,
                     size_t prefixes_len, const char *cache_dir, size_t cache_dir_len);

// Makes `notify` be told that a batch waits for `xpute_take`; null sets none.
// It is called on the guest's thread alone, once for a batch: the next is
// told only after `xpute_take` or `xpute_start`. It hands the news to the
// JavaScript thread. Set before `xpute_start`.
void xpute_notify_on(void (*notify)(void));

// Makes `stopped` be told that the guest stopped, with its report: the errno,
// the file of `file_len` UTF-8 bytes, not NUL-terminated, the line, and two
// numbers of the guest's. Null sets none. It is called on the guest's thread,
// a failure of the host's own on another thread being posted there, and hands
// the report to the JavaScript thread. Set before `xpute_start`.
void xpute_stopped_on(void (*stopped)(int32_t errnum, const char *file, size_t file_len,
                                      uint32_t line, uint64_t n1, uint64_t n2));

// Makes `trapped` be told that the guest trapped, with what stopped it, `err`
// of `err_len` UTF-8 bytes, not NUL-terminated, the imports with no
// implementation among them where the runtime refused to start the guest. Null
// sets none. It is called on the guest's thread, and hands the news to the
// JavaScript thread. Set before `xpute_start`.
void xpute_trap_on(void (*trapped)(const char *err, size_t err_len));

// Answers the guest's last stop, as `xpute_stopped_on`'s function was last
// told it: its file's UTF-8 bytes, not NUL-terminated, their count put at
// `file_len`, and the errno, the line and the two numbers put at theirs; null
// before the guest stopped. A UI that starts after the stop reads it so. The
// bytes live as long as the process.
const char *xpute_stop(int32_t *errnum, size_t *file_len, uint32_t *line, uint64_t *n1,
                       uint64_t *n2);

// Answers what trapped the guest, as `xpute_trap_on`'s function was told it:
// its UTF-8 bytes, not NUL-terminated, their count put at `len`; null before
// the guest trapped. A UI that starts after the trap reads it so. The bytes
// live as long as the process.
const char *xpute_trap(size_t *len);

// Sends the method `m` of the guest's object `h` of the UI of the handle
// `ui`, its request `tag` the UI's (@xpute/link `Side.send`), with the packet
// `pkt` of `len` bytes, or with none where `pkt` is null and `len` 0. Its
// answer comes in a batch.
void xpute_post(uint32_t ui, uint32_t tag, uint32_t h, uint32_t m, const uint8_t *pkt,
                size_t len);

// Answers the request `tag` the runtime sent the UI of the handle `ui`, a
// method of an object the UI implements, with `result` and the data `data` of
// `len` bytes, or none where `data` is null and `len` 0.
void xpute_answer(uint32_t ui, uint32_t tag, int32_t result, const uint8_t *data,
                  size_t len);

// Hands over the batch the UI of the handle `ui` has not taken, none where it
// is not the UI attached, and writes its length through `len`: each message a
// frame of the link's (link/spec/frame.json5), its four words and then its
// packet, padded to 8 bytes. The bytes are the caller's until
// `xpute_taken_free(at, len)`.
uint8_t *xpute_take(uint32_t ui, size_t *len);
void xpute_taken_free(uint8_t *at, size_t len);

// Tells the runtime that the app was hidden (0) or shown (1).
void xpute_life(int32_t shown);

// Writes a log record of the UI's by the runtime's writer, as the host's are:
// `msg` of `msg_len` UTF-8 bytes at `level` (xpute:log `level`, its
// number), about `context` of `context_len` UTF-8 bytes, neither
// NUL-terminated.
void xpute_log(uint32_t level, const char *context, size_t context_len, const char *msg,
               size_t msg_len);
