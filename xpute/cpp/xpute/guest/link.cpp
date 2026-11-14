// cpp/xpute/guest/link.cpp

#include "link.hpp"

#include "xpute/kit/status/bug.hpp"
#include "xpute/link/frame.hpp"
#include "xpute/link/mem.hpp"

// A module takes the host's `link_out` under `env` (TS `link.ts` `imports`);
// a native guest, from the host linked into its process.
#ifdef __wasm__
extern "C" __attribute__((import_module("env"), import_name("link_out"))) std::int32_t link_out(std::uint32_t at);
#else
extern "C" std::int32_t link_out(std::uint32_t at);
#endif

namespace xpute::guest {

namespace {

// The guest's own side: it lays in its outbox, past the inbox, and crosses
// by `link_out`. It is made by the first `init` and lasts as long as the
// guest, so no constructor, guard or destructor of a static runs for it: a
// module's would link the libc's, which reach WASI.
link::Side *the_side = nullptr;

link::Side &side() noexcept {
    ensure(the_side != nullptr, Errno::enotrecoverable);
    return *the_side;
}

// Whether the guest is in an entry.
bool entered = false;
// The program's start and end of an entry (`init`).
void (*enter_fn)() = [] {};
bool (*end_fn)() = [] { return false; };

// Starts an entry by the program's `enter`. Every entry ends by `leave`.
void enter() {
    ensure(!std::exchange(entered, true), Errno::ebusy);
    enter_fn();
}

// Ends an entry: runs the answers due and the program's `end`, which answers
// whether it left work, again until neither has work left, and ends the
// entry.
void leave() {
    for (;;) {
        side().run_due();
        bool again = end_fn();
        if (!again && !side().has_due()) break;
    }
    entered = false;
}

} // namespace

std::uint32_t init(std::uint32_t room, void (*enter)(), bool (*end)()) noexcept {
    ensure(room % 8 == 0 && room >= link::HDR_SZ, Errno::einval, room);
    if (the_side == nullptr) the_side = new link::Side([](link::Layout l) { return l.inbox + l.room; }, link_out);
    // The layout's two words, then the inbox and the outbox, for as long as
    // the guest runs.
    std::uint64_t *words = new std::uint64_t[1 + 2 * room / 8]();
    std::uint32_t at = link::mem::off(words);
    std::uint32_t inbox = at + 8;
    link::mem::set_word(at, inbox);
    link::mem::set_word(at + 4, room);
    side().set_layout(link::Layout{inbox, room});
    enter_fn = enter;
    end_fn = end;
    return at;
}

void boot(void (*boot)()) {
    ensure(!std::exchange(entered, true), Errno::ebusy);
    boot();
    enter_fn();
    leave();
}

std::int32_t recv() {
    enter();
    std::int32_t result = side().recv(side().layout().inbox);
    leave();
    return result;
}

void mount(link::Handle h, std::vector<link::Token> prefix, link::Opener open) {
    side().mount(h, std::move(prefix), std::move(open));
}

void mount_named(link::Handle h, std::unique_ptr<link::Object> o) {
    side().mount_named(h, std::move(o));
}

link::Handle add(std::unique_ptr<link::Object> o) {
    return side().add(std::move(o));
}

Result<link::Handle> open(std::span<const link::Token> uri) {
    return side().open(uri);
}

void drop(link::Handle h) {
    side().drop(h);
}

std::pair<std::int32_t, std::optional<std::span<const std::uint8_t>>> call(link::Handle h, link::Method m, const link::Args &args) {
    return side().call(h, m, args);
}

link::Tag send(link::Handle h, link::Method m, const link::Args &args, link::Answer answer) {
    return side().send(h, m, args, std::move(answer));
}

void cancel(link::Tag t) {
    side().cancel(t);
}

void on_cancel(link::Tag t, std::function<void()> stop) {
    side().on_cancel(t, std::move(stop));
}

void task_return(link::Tag t, std::int32_t result, const link::Data &data) {
    side().task_return(t, result, data);
}

bool in_guest() noexcept {
    return entered && !side().crossing();
}

} // namespace xpute::guest
