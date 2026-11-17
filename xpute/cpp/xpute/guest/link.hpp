// cpp/xpute/guest/link.hpp

// The guest's end of the link (`xpute/link/link.hpp`), as `xpute-guest`'s
// `link.rs` is: its one side, which lays in its outbox and crosses by
// `link_out`, its three exports (`XPUTE_LINK`), and the entry they run each
// call in. The machine's names are read here as the guest's callers read them.
//
// A program says nothing of its memory here: what the link lays and lends,
// and its layout, it takes from the memory resource
// the program set as its default (`std::pmr::get_default_resource`), and the
// program picks it and its global `operator new`.

#pragma once

#include <cstdint>
#include <functional>
#include <memory>
#include <optional>
#include <span>
#include <utility>
#include <vector>

#include "xpute/kit/status/bug.hpp"
#include "xpute/kit/status/error.hpp"
#include "xpute/link/link.hpp"
#include "stop.hpp"

namespace xpute::guest {

// Lays the link over frames of `cap` bytes, a multiple of 8 that holds a
// frame's words, and runs nothing of the program; `enter` and `end` are the
// program's start and end of every entry (`boot`, `recv`), `end` answering
// whether it left work. It answers where the link's layout lies: the inbox's
// address and a frame's cap (frame.json5 `LAYOUT`), then the inbox and the
// outbox.
std::uint64_t init(std::uint32_t cap, void (*enter)(), bool (*end)()) noexcept;

// The guest's first entry, once the host has read the layout. Unlike every
// later entry, it starts by `boot`, which opens what the program's `enter`
// may read, and only then by `enter`; it ends as every entry does.
void boot(void (*boot)());

// The frame the host laid in the inbox: a later answer, to its request, or a
// method, to its object, in an entry of its own.
std::int32_t recv();

// Puts `open` as the opener of the URIs that start with `prefix`, whose
// objects are the resources of the interface of the named handle `h`.
void mount(link::Handle h, std::vector<link::Token> prefix, link::Opener open);

// Mounts `o` under its interface's named handle `h`.
void mount_named(link::Handle h, std::unique_ptr<link::Object> o);

// Adds `o` under a handle issued for it, for a method to answer.
link::Handle add(std::unique_ptr<link::Object> o);

// Opens the object `uri` names on the host, and answers its handle.
Result<link::Handle> open(std::span<const link::Token> uri);

// Lets the object `h` names on the host go.
void drop(link::Handle h);

// Calls `m` of `h`, which answers at once: its result, and its return data,
// readable until the next crossing.
std::pair<std::int32_t, std::optional<std::span<const std::uint8_t>>> call(link::Handle h, link::Method m, const link::Args &args);

// Sends `m` of `h`, its answer pending from before the crossing, as the
// host's is, and answers the request's tag.
link::Tag send(link::Handle h, link::Method m, const link::Args &args, link::Answer answer);

// Cancels the request `t`, while its answer is pending
// (`frame.json5` `CANCEL`).
void cancel(link::Tag t);

// Puts `stop` as how the request `t` the guest answers stops if it is
// canceled.
void on_cancel(link::Tag t, std::function<void()> stop);

// Answers the request `t`, whose method answered EINPROGRESS, with `result`
// and its return data. A later answer is found by its tag alone.
void task_return(link::Tag t, std::int32_t result, const link::Data &data);

// Whether control is in the guest's own code: inside an entry, and not out
// on the host through `link_out`. A program's report of its failures takes
// only these.
bool in_guest() noexcept;

} // namespace xpute::guest

// Defines the guest's three exports (`link/wit/types.wit`), over frames of
// `cap_` bytes, with `boot_` mounting what the guest implements and opening
// what it calls, and `enter_` and `end_` starting and ending each entry; from
// its init on, the guest's failures are its own (`stop.hpp` `hook`). A module
// exports them by name; a native guest as C symbols, which catch a bug (kit
// `caught`) and answer a trap (`TRAP`, or `TRAP_AT` for init's layout), as a
// Rust guest's catch its panic. A failed check of libc++'s own still aborts a
// native guest's process, where a module traps.
#ifdef __wasm__
// The module's static initializers, which wasm-ld gathers into one function.
// The guest's first export runs them, before anything of the program, as the
// native loader runs a library's; a module that calls it is a reactor to
// wasm-ld, whose exports it leaves as they are.
extern "C" void __wasm_call_ctors();
#define XPUTE_LINK(cap_, boot_, enter_, end_)                                                                                                      \
    extern "C" __attribute__((export_name("link_init"))) std::uint64_t link_init() {                                                              \
        __wasm_call_ctors();                                                                                                                        \
        ::xpute::guest::stop::hook();                                                                                                               \
        return ::xpute::guest::init((cap_), (enter_), (end_));                                                                                     \
    }                                                                                                                                               \
    extern "C" __attribute__((export_name("link_boot"))) void link_boot() { ::xpute::guest::boot(boot_); }                                        \
    extern "C" __attribute__((export_name("link_in"))) std::int32_t link_in() { return ::xpute::guest::recv(); }
#else
#define XPUTE_LINK(cap_, boot_, enter_, end_)                                                                                                      \
    extern "C" __attribute__((visibility("default"))) std::uint64_t link_init() {                                                                 \
        return ::xpute::caught(::xpute::link::TRAP_AT, [] {                                                                                       \
            ::xpute::guest::stop::hook();                                                                                                           \
            return ::xpute::guest::init((cap_), (enter_), (end_));                                                                                 \
        });                                                                                                                                         \
    }                                                                                                                                               \
    extern "C" __attribute__((visibility("default"))) std::int32_t link_boot() {                                                                  \
        return ::xpute::caught(::xpute::link::TRAP, [] {                                                                                            \
            ::xpute::guest::boot(boot_);                                                                                                            \
            return 0;                                                                                                                               \
        });                                                                                                                                         \
    }                                                                                                                                               \
    extern "C" __attribute__((visibility("default"))) std::int32_t link_in() { return ::xpute::caught(::xpute::link::TRAP, ::xpute::guest::recv); }
#endif
