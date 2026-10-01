// cpp/xpute/conformance/guest.cpp

#include "guest.hpp"

#include <array>
#include <new>
#include <optional>
#include <span>

#include "../core/math/scalar.hpp"
#include "../core/status/bug.hpp"
#include "../core/status/errno.spec.hpp"
#include "../core/wire/xtp/cursor.hpp"
#include "../runtime/abi/handle.hpp"
#include "../runtime/ipc/directory.hpp"
#include "../runtime/ipc/door.hpp"
#include "../runtime/ipc/ring.hpp"
#include "../runtime/ipc/stream.hpp"
#include "guest.spec.hpp"

#ifdef __wasm__
extern "C" {
// Where the linker stopped placing the module's stack and data. wasm-ld
// defines it.
extern const std::uint8_t __heap_base;

// The host's clock, the module's one import besides its memory.
__attribute__((import_module("env"), import_name("now"))) double xpute_conformance_host_now(void);
}
#endif

namespace xpute::conformance {
namespace {

constexpr std::uint32_t PAGE = 4096;

constexpr std::uint32_t page_up(std::uint32_t bytes) {
    return (bytes + PAGE - 1) / PAGE * PAGE;
}

// Where this guest's ranges lie, from the start of its range. Not where
// another guest puts them: the stream after the directory and the completion
// ring before the submission ring, so a host that finds them at all has found
// them through the directory.
constexpr std::uint32_t DIRECTORY_AT = 0;
constexpr std::uint32_t STREAM_AT = PAGE;
constexpr std::uint32_t COMPLETION_DESC_AT = STREAM_AT + page_up(STREAM_WORDS * 4);
constexpr std::uint32_t SUBMISSION_DESC_AT = COMPLETION_DESC_AT + page_up(Ring::bytes(COMPLETION_CAPACITY));
constexpr std::uint32_t COMPLETION_SLOT_AT = SUBMISSION_DESC_AT + page_up(Ring::bytes(SUBMISSION_CAPACITY));
constexpr std::uint32_t SUBMISSION_SLOT_AT = COMPLETION_SLOT_AT + COMPLETION_CAPACITY * SLOT_BYTES;
constexpr std::uint32_t STATE_AT = SUBMISSION_SLOT_AT + SUBMISSION_CAPACITY * SLOT_BYTES;

// What the guest keeps, made in its own memory.
struct State {
    Door door;
    Stream stream;
    HandleTable<HANDLES> handles;
    xpute_conformance_now now;
    // The clock at the last MARK.
    double mark = 0;
};

static_assert(STATE_AT + sizeof(State) <= MEMORY_BYTES, "the ranges run past the memory");
static_assert(STATE_AT % alignof(State) == 0, "the state lies on its own alignment");
static_assert(DIRECTORY_AT + directory_words(3) * 4 <= STREAM_AT, "the directory runs into the stream");

State &state_of(Memory m, std::uint32_t range) {
    return *std::launder(reinterpret_cast<State *>(m.at(range + STATE_AT)));
}

// The u32 a command's packet carries as its root branch's first child.
std::optional<std::uint32_t> first_u32(std::span<const std::uint8_t> packet) {
    Result<xtp::TreeReader> reader = xtp::TreeReader::make(packet);
    if (!reader) return std::nullopt;
    Result<xtp::BranchCursor> root = reader->read_branch();
    if (!root) return std::nullopt;
    Result<xtp::Cursor> first = root->at(0);
    if (!first) return std::nullopt;
    Result<xtp::Value> v = first->get();
    const std::uint32_t *n = v ? std::get_if<std::uint32_t>(&*v) : nullptr;
    return n ? std::optional(*n) : std::nullopt;
}

Applied apply(State &s, std::uint32_t cmd, std::span<const std::uint8_t> packet) {
    // The commands that take a number read it first; one without a packet,
    // or with one that does not read, fails as a bad message.
    std::uint32_t arg = 0;
    if (cmd == cmd::RAISE || cmd == cmd::RECORD || cmd == cmd::RELEASE || cmd == cmd::USE) {
        std::optional<std::uint32_t> n = first_u32(packet);
        if (!n) return {.error = Errno::ebadmsg};
        arg = *n;
    }
    switch (cmd) {
    case cmd::OK:
        return {};
    case cmd::FAIL:
        return {.error = Errno::einval};
    case cmd::RAISE:
        if (arg > MOST_RAISE) return {.error = Errno::einval};
        for (std::uint32_t k = 0; k < arg; k++) s.door.post(k, cmd::SIGNAL, 0, 0);
        return {.value = static_cast<std::int32_t>(arg)};
    case cmd::RECORD:
        s.stream.record(arg, {});
        return {};
    case cmd::ISSUE: {
        std::optional<std::uint32_t> h = s.handles.acquire();
        if (!h) return {.error = Errno::enfile};
        return {.value = static_cast<std::int32_t>(*h)};
    }
    case cmd::RELEASE:
        return s.handles.release(arg) ? Applied{} : Applied{.error = Errno::ebadf};
    case cmd::USE:
        return s.handles.live(arg) ? Applied{} : Applied{.error = Errno::ebadf};
    case cmd::MARK:
        s.mark = s.now();
        return {};
    case cmd::SINCE: {
        double since = s.now() - s.mark;
        return {.value = static_cast<std::int32_t>(round(since))};
    }
    default:
        return {.error = Errno::enosys};
    }
}

// Lays the guest out in the range that begins at `range`, and answers where
// its directory is. Every offset written anywhere is the memory's, so the
// range's start is added to each.
std::uint32_t boot(Memory m, std::uint32_t range, xpute_conformance_now now) {
    // A command leaves at most its signals and its reply.
    Door door(Ring::init(m, range + SUBMISSION_DESC_AT, SUBMISSION_CAPACITY, range + SUBMISSION_SLOT_AT, SLOT_BYTES),
              Ring::init(m, range + COMPLETION_DESC_AT, COMPLETION_CAPACITY, range + COMPLETION_SLOT_AT, SLOT_BYTES), MOST_RAISE + 1);
    new (m.at(range + STATE_AT)) State{.door = door, .stream = Stream(m, range + STREAM_AT, STREAM_WORDS), .handles = {}, .now = now};
    const std::array<Entry, 3> entries{{
        {directory::key::SUBMISSION, range + SUBMISSION_DESC_AT},
        {directory::key::COMPLETION, range + COMPLETION_DESC_AT},
        {key::STREAM, range + STREAM_AT},
    }};
    write_directory(m, range + DIRECTORY_AT, entries);
    return range + DIRECTORY_AT;
}

double interrupt(Memory m, std::uint32_t range) {
    State &s = state_of(m, range);
    s.door.drain([&s](std::uint32_t cmd, std::span<const std::uint8_t> packet) { return apply(s, cmd, packet); });
    s.stream.publish();
    return s.door.pending() ? 0.0 : -1.0;
}

#ifdef __wasm__

// The module's memory, from address 0: the linker's stack and data first,
// and the range from the first page past where the linker stopped, so how
// large the stack or the data is decides nothing a host reads.
std::uint32_t range_at() {
    return page_up(static_cast<std::uint32_t>(reinterpret_cast<std::uintptr_t>(&__heap_base)));
}

double now() {
    return xpute_conformance_host_now();
}

#endif

} // namespace
} // namespace xpute::conformance

using namespace xpute;
using namespace xpute::conformance;

#ifdef __wasm__

// This guest keeps nothing on a heap, so it has none: an allocation is a
// bug, and the library's own allocator — whose failure path prints through
// WASI — is never linked. A guest with a heap replaces these over it
// (runtime/mem/heap.hpp). Natively the operators stay the process's, which
// the host's own code shares.
void *operator new(std::size_t n) {
    bug(Errno::enomem, n);
}
void *operator new(std::size_t n, std::align_val_t) {
    bug(Errno::enomem, n);
}
void *operator new(std::size_t, const std::nothrow_t &) noexcept {
    return nullptr;
}
void *operator new(std::size_t, std::align_val_t, const std::nothrow_t &) noexcept {
    return nullptr;
}
void operator delete(void *) noexcept {}
void operator delete(void *, std::size_t) noexcept {}
void operator delete(void *, std::align_val_t) noexcept {}
void operator delete(void *, std::size_t, std::align_val_t) noexcept {}

extern "C" std::uint32_t xpute_conformance_boot(void) {
    std::uint32_t range = range_at();
    // The host sized the memory; a range past its end is a host that sized
    // it for another guest.
    ensure(std::uint64_t{range} + MEMORY_BYTES <= std::uint64_t{__builtin_wasm_memory_size(0)} * 65536, Errno::enomem, range);
    return boot(Memory(0), range, now);
}

extern "C" double xpute_conformance_interrupt(double) {
    return interrupt(Memory(0), range_at());
}

#else

extern "C" std::uint32_t xpute_conformance_boot(std::uint8_t *mem, xpute_conformance_now now) {
    return boot(Memory(reinterpret_cast<std::uintptr_t>(mem), MEMORY_BYTES), 0, now);
}

extern "C" double xpute_conformance_interrupt(std::uint8_t *mem, double) {
    return interrupt(Memory(reinterpret_cast<std::uintptr_t>(mem), MEMORY_BYTES), 0);
}

#endif
