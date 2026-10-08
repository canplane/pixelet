// cpp/conformance/guest.cpp

#include "guest.hpp"

#include <array>
#include <bit>
#include <cstddef>
#include <new>
#include <optional>
#include <span>

#include "../kit/alloc/slab.hpp"
#include "../kit/math/scalar.hpp"
#include "../kit/status/bug.hpp"
#include "../kit/status/errno.spec.hpp"
#include "../kit/wire/xtp/cursor.hpp"
#include "../guest/abi/cmd.spec.hpp"
#include "../guest/abi/handle.hpp"
#include "../guest/ipc/directory.hpp"
#include "../guest/ipc/door.hpp"
#include "../guest/ipc/file.hpp"
#include "../guest/ipc/ring.hpp"
#include "../guest/ipc/stream.hpp"
#include "../guest/ipc/sys.hpp"
#include "guest.spec.hpp"

#ifdef __wasm__
extern "C" {
// Defined by wasm-ld: where the linker stopped placing stack and data.
extern const std::uint8_t __heap_base;

__attribute__((import_module("env"), import_name("now"))) double xpute_conformance_host_now(void);
}
#endif

namespace xpute::conformance {
namespace {

constexpr std::uint32_t PAGE = 4096;

constexpr std::uint32_t page_up(std::uint32_t bytes) {
    return (bytes + PAGE - 1) / PAGE * PAGE;
}

// Deliberately not where another guest puts them, so a host that finds them
// at all has found them through the directory.
constexpr std::uint32_t DIRECTORY_AT = 0;
constexpr std::uint32_t STREAM_AT = PAGE;
constexpr std::uint32_t COMPLETION_DESC_AT = STREAM_AT + page_up(STREAM_WORDS * 4);
constexpr std::uint32_t SUBMISSION_DESC_AT = COMPLETION_DESC_AT + page_up(Ring::bytes(COMPLETION_CAPACITY));
constexpr std::uint32_t COMPLETION_SLOT_AT = SUBMISSION_DESC_AT + page_up(Ring::bytes(SUBMISSION_CAPACITY));
constexpr std::uint32_t SUBMISSION_SLOT_AT = COMPLETION_SLOT_AT + COMPLETION_CAPACITY * SLOT_BYTES;
constexpr std::uint32_t SYS_AT = SUBMISSION_SLOT_AT + SUBMISSION_CAPACITY * SLOT_BYTES;
constexpr std::uint32_t STATE_AT = page_up(SYS_AT + SYS_WORDS * 4);

struct State {
    Door door;
    Stream stream;
    Sys sys;
    // Each for the owner its packet named.
    Files<std::uint32_t> files;
    HandleTable<HANDLES> handles;
    xpute_conformance_now now;
    double mark = 0;
};

// The memory never grows, so a module's allocator sits over a fixed range.
constexpr std::uint32_t HEAP_AT = page_up(STATE_AT + sizeof(State));

static_assert(HEAP_AT + HEAP_BYTES <= MEMORY_BYTES, "the ranges run past the memory");
static_assert(STATE_AT % alignof(State) == 0, "the state lies on its own alignment");
static_assert(DIRECTORY_AT + directory_words(4) * 4 <= STREAM_AT, "the directory runs into the stream");

State &state_of(Memory m, std::uint32_t range) {
    return *std::launder(reinterpret_cast<State *>(m.at(range + STATE_AT)));
}

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
    // A command that takes a number fails with EBADMSG when its packet does not read.
    std::uint32_t arg = 0;
    if (cmd == static_cast<std::uint32_t>(abi::Command::SYS_OPENED)) {
        Result<abi::SysOpened> answer = abi::SysOpened::read(packet);
        if (!answer) return {.error = answer.error().code};
        if (const std::uint32_t *owner = s.files.owner(answer->fd)) s.door.post(*owner, cmd::OPENED, 0, answer->res);
        return {};
    }
    if (cmd == cmd::RAISE || cmd == cmd::RECORD || cmd == cmd::RELEASE || cmd == cmd::USE || cmd == cmd::OPEN || cmd == cmd::CLOSE) {
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
    case cmd::OPEN:
        return {.value = static_cast<std::int32_t>(s.files.open(s.sys, 1, "f", arg))};
    case cmd::CLOSE:
        return s.files.close(s.sys, arg) ? Applied{} : Applied{.error = Errno::ebadf};
    case cmd::SINCE: {
        double since = s.now() - s.mark;
        return {.value = static_cast<std::int32_t>(round(since))};
    }
    default:
        return {.error = Errno::enosys};
    }
}

// Every offset written is the memory's, so the range's start is added to each.
std::uint32_t boot(Memory m, std::uint32_t range, xpute_conformance_now now) {
    Door door(Ring::init(m, range + SUBMISSION_DESC_AT, SUBMISSION_CAPACITY, range + SUBMISSION_SLOT_AT, SLOT_BYTES),
              Ring::init(m, range + COMPLETION_DESC_AT, COMPLETION_CAPACITY, range + COMPLETION_SLOT_AT, SLOT_BYTES), MOST_RAISE);
    new (m.at(range + STATE_AT)) State{
        .door = door, .stream = Stream(m, range + STREAM_AT, STREAM_WORDS), .sys = Sys(m, range + SYS_AT, SYS_WORDS), .files = {}, .handles = {}, .now = now};
    const std::array<Entry, 4> entries{{
        {directory::key::SUBMISSION, range + SUBMISSION_DESC_AT},
        {directory::key::COMPLETION, range + COMPLETION_DESC_AT},
        {key::STREAM, range + STREAM_AT},
        {directory::key::SYS, range + SYS_AT},
    }};
    write_directory(m, range + DIRECTORY_AT, directory_words(entries.size()), entries);
    return range + DIRECTORY_AT;
}

double interrupt(Memory m, std::uint32_t range) {
    State &s = state_of(m, range);
    s.sys.rise();
    s.door.drain([&s](std::uint32_t cmd, std::span<const std::uint8_t> packet) { return apply(s, cmd, packet); });
    s.stream.publish();
    s.sys.fall();
    return s.door.pending() ? 0.0 : -1.0;
}

#ifdef __wasm__

// The range starts past where the linker stopped, so how large the stack or
// data is decides nothing a host reads.
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

// The library's allocator is never linked: it grows a memory that never grows,
// and its failure path prints through WASI.
struct HeapRange {
    static constexpr std::size_t MIN_LOG2 = 12;
    static constexpr std::size_t MAX_LOG2 = std::countr_zero(HEAP_BYTES);
    static std::uintptr_t base() noexcept { return range_at() + HEAP_AT; }
    static std::uintptr_t end() noexcept { return base() + HEAP_BYTES; }
};

SlabMalloc<HeapRange> heap;

void *operator new(std::size_t n) {
    void *p = heap.alloc(n, alignof(std::max_align_t));
    if (p == nullptr) bug(Errno::enomem, n);
    return p;
}
void *operator new(std::size_t n, std::align_val_t a) {
    void *p = heap.alloc(n, static_cast<std::size_t>(a));
    if (p == nullptr) bug(Errno::enomem, n);
    return p;
}
void *operator new(std::size_t n, const std::nothrow_t &) noexcept {
    return heap.alloc(n, alignof(std::max_align_t));
}
void *operator new(std::size_t n, std::align_val_t a, const std::nothrow_t &) noexcept {
    return heap.alloc(n, static_cast<std::size_t>(a));
}
void operator delete(void *p, std::size_t n) noexcept {
    if (p != nullptr) heap.dealloc(p, n, alignof(std::max_align_t));
}
void operator delete(void *p, std::size_t n, std::align_val_t a) noexcept {
    if (p != nullptr) heap.dealloc(p, n, static_cast<std::size_t>(a));
}
// Without its size a block cannot be given back to a slab.
void operator delete(void *p) noexcept {
    if (p != nullptr) bug(Errno::enotrecoverable);
}
void operator delete(void *p, std::align_val_t) noexcept {
    if (p != nullptr) bug(Errno::enotrecoverable);
}

extern "C" std::uint32_t xpute_conformance_boot(void) {
    std::uint32_t range = range_at();
    // A range past the memory's end is a host that sized it for another guest.
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
