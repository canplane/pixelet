// cpp/conformance/guest.cpp

#include "guest.hpp"

#include <array>
#include <bit>
#include <cstddef>
#include <new>
#include <optional>
#include <variant>
#include <span>
#include <utility>

#include "xpute/kit/alloc/slab.hpp"
#include "xpute/kit/math/rng.hpp"
#include "xpute/kit/math/scalar.hpp"
#include "xpute/kit/status/bug.hpp"
#include "xpute/kit/status/errno.spec.hpp"
#include "xpute/kit/wire/xtp/cursor.hpp"
#include "xpute/guest/abi/cmd.spec.hpp"
#include "xpute/guest/abi/handle.hpp"
#include "xpute/guest/hostcall.hpp"
#include "xpute/guest/ipc/directory.hpp"
#include "xpute/guest/ipc/door.hpp"
#include "xpute/guest/ipc/file.hpp"
#include "xpute/guest/ipc/ring.hpp"
#include "xpute/guest/ipc/stream.hpp"
#include "xpute/guest/ipc/sys.hpp"
#include "xpute/guest/mem.hpp"
#include "xpute/guest/mem/section.hpp"
#include "xpute/guest/sched/wake.hpp"
#include "xpute/guest/wasm.hpp"
#include "guest.spec.hpp"

namespace xpute::conformance {
namespace {

constexpr std::uint32_t page_up(std::uint32_t bytes) {
    return align_up(bytes, PAGE_BYTES);
}

// What the guest shares with its host, each on pages of its own, in the order
// a layout draws (`layout`).
enum Part : std::size_t { DIRECTORY, SQ_DESC, CQ_DESC, SQ_SLOT, CQ_SLOT, STREAM, SYS, PARTS };

constexpr std::array<std::uint32_t, PARTS> PART_BYTES{
    directory_words(4) * 4,
    ring_bytes(SQ_CAP),
    ring_bytes(CQ_CAP),
    slots_bytes(SQ_CAP, SLOT_BYTES),
    slots_bytes(CQ_CAP, SLOT_BYTES),
    STREAM_WORDS * 4,
    SYS_WORDS * 4,
};

// The most a layout spans: the page it starts past, every part, and the most
// pages before each. The guest's own state follows, where no layout moves it.
constexpr std::uint32_t STATE_AT = [] {
    std::uint32_t end = PAGE_BYTES + PARTS * GAP_PAGES * PAGE_BYTES;
    for (std::uint32_t bytes : PART_BYTES) end += page_up(bytes);
    return end;
}();

// Where each part lies under `seed`, from the range's start: the parts in the
// order the seed draws, each past the pages it draws to leave before it. The
// first page is left out (the contract's rule 1).
std::array<std::uint32_t, PARTS> layout(std::uint32_t seed) {
    SplitMix32 rng(seed);
    std::array<std::size_t, PARTS> order{};
    for (std::size_t p = 0; p < PARTS; p++) order[p] = p;
    for (std::size_t i = PARTS - 1; i >= 1; i--) std::swap(order[i], order[rng.next_u32() % (i + 1)]);
    std::array<std::uint32_t, PARTS> at{};
    std::uint32_t end = PAGE_BYTES;
    for (std::size_t p : order) {
        end += rng.next_u32() % (GAP_PAGES + 1) * PAGE_BYTES;
        at[p] = end;
        end += page_up(PART_BYTES[p]);
    }
    return at;
}

struct State {
    Door door;
    Stream stream;
    Sys sys;
    // Each for the owner its packet named.
    Files<std::uint32_t, HANDLES> files;
    abi::Table<std::monostate, HANDLES> handles;
    xpute_conformance_now now;
    double mark = 0;
    // What HOSTCALL's host writes.
    std::uint32_t word = 0;
};

// The memory never grows, so a module's allocator sits over a fixed range.
constexpr std::uint32_t HEAP_AT = page_up(STATE_AT + sizeof(State));

static_assert(HEAP_AT + HEAP_BYTES <= MEM_BYTES, "the ranges run past the memory");
static_assert(STATE_AT % alignof(State) == 0, "the state lies on its own alignment");

State &state_of(Memory m, std::uint32_t range) {
    return *std::launder(reinterpret_cast<State *>(m.at(range + STATE_AT)));
}

std::optional<std::uint32_t> first_u32(std::span<const std::uint8_t> pkt) {
    Result<xtp::TreeReader> reader = xtp::TreeReader::make(pkt);
    if (!reader) return std::nullopt;
    Result<xtp::BranchCursor> root = reader->read_branch();
    if (!root) return std::nullopt;
    Result<xtp::Cursor> first = root->at(0);
    if (!first) return std::nullopt;
    Result<xtp::Value> v = first->get();
    const std::uint32_t *n = v ? std::get_if<std::uint32_t>(&*v) : nullptr;
    return n ? std::optional(*n) : std::nullopt;
}

Applied apply(State &s, std::uint32_t cmd, std::span<const std::uint8_t> pkt) {
    // A command that takes a number fails with EBADMSG when its packet does not read.
    std::uint32_t arg = 0;
    if (cmd == cmd::RAISE || cmd == cmd::RECORD || cmd == cmd::RELEASE || cmd == cmd::USE || cmd == cmd::OPEN || cmd == cmd::CLOSE || cmd == cmd::HOSTCALL) {
        std::optional<std::uint32_t> n = first_u32(pkt);
        if (!n) return {.err = Errno::ebadmsg};
        arg = *n;
    }
    switch (cmd) {
    case cmd::OK:
        return {};
    case cmd::FAIL:
        return {.err = Errno::einval};
    case cmd::RAISE:
        if (arg > MOST_RAISE) return {.err = Errno::einval};
        for (std::uint32_t k = 0; k < arg; k++) s.door.post(frame::tagged(SENDER, k), cmd::EVENT, 0, 0);
        return {.val = static_cast<std::int32_t>(arg)};
    case cmd::RECORD:
        s.stream.record(arg, {});
        return {};
    case cmd::ISSUE: {
        std::optional<std::uint32_t> h = s.handles.insert({});
        if (!h) return {.err = Errno::enfile};
        return {.val = static_cast<std::int32_t>(*h)};
    }
    case cmd::RELEASE:
        return s.handles.remove(arg) ? Applied{} : Applied{.err = Errno::ebadf};
    case cmd::USE:
        return s.handles.live(arg) ? Applied{} : Applied{.err = Errno::ebadf};
    case cmd::MARK:
        s.mark = s.now();
        return {};
    case cmd::OPEN:
        return {.val = static_cast<std::int32_t>(s.files.open(s.sys, 1, "f", arg))};
    case cmd::CLOSE:
        return s.files.close(s.sys, arg) ? Applied{} : Applied{.err = Errno::ebadf};
    case cmd::SINCE: {
        double since = s.now() - s.mark;
        return {.val = static_cast<std::int32_t>(round(since))};
    }
    case cmd::HOSTCALL: {
        s.word = 0;
        const std::array<std::uint32_t, 2> args{arg, s.stream.mem().off_of(&s.word)};
        Result<std::uint32_t> res = s.sys.call_now(hostcall::ANSWER, args, {});
        if (!res) return {.err = res.error().errnum};
        // The word the host wrote before the hostcall returned.
        return {.val = static_cast<std::int32_t>(*static_cast<volatile std::uint32_t *>(&s.word))};
    }
    default:
        return {.err = Errno::enosys};
    }
}

// Every offset written is the memory's, so the range's start is added to each.
std::uint32_t boot(Memory m, std::uint32_t range, xpute_conformance_now now, std::uint32_t seed) {
    std::array<std::uint32_t, PARTS> at = layout(seed);
    for (std::uint32_t &off : at) off += range;
    Door door(Ring::init(m, at[SQ_DESC], SQ_CAP, at[SQ_SLOT], SLOT_BYTES), Ring::init(m, at[CQ_DESC], CQ_CAP, at[CQ_SLOT], SLOT_BYTES), MOST_RAISE);
    new (m.at(range + STATE_AT)) State{
        .door = door, .stream = Stream(m, at[STREAM], STREAM_WORDS), .sys = Sys(m, at[SYS], SYS_WORDS), .files = {}, .handles = {}, .now = now};
    const std::array<Entry, 4> entries{{
        {directory::key::SQ, at[SQ_DESC]},
        {directory::key::CQ, at[CQ_DESC]},
        {key::STREAM, at[STREAM]},
        {directory::key::SYS, at[SYS]},
    }};
    write_directory(m, at[DIRECTORY], directory_words(entries.size()), entries);
    return at[DIRECTORY];
}

double turn(Memory m, std::uint32_t range) {
    State &s = state_of(m, range);
    s.sys.rise();
    s.door.drain(
        [&s](const abi::SysOpened &opened) {
            if (std::optional<std::uint32_t> owner = s.files.opened(s.sys, opened)) s.door.post(frame::tagged(SENDER, *owner), cmd::OPENED, 0, opened.res);
            return Applied{};
        },
        [&s](std::uint32_t cmd, std::span<const std::uint8_t> pkt) { return apply(s, cmd, pkt); });
    s.stream.publish();
    s.sys.fall();
    return s.door.pending() ? 0.0 : NO_WAKE;
}

#ifdef __wasm__

// The range starts past where the linker stopped, so how large the stack or
// data is decides nothing a host reads.
std::uint32_t range_at() {
    return page_up(wasm::data_end());
}

double host_now() {
    return wasm::now();
}

std::int32_t host_hostcall(std::uint32_t at) {
    return wasm::hostcall(at);
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
    static constexpr std::size_t MIN_LOG2 = std::countr_zero(PAGE_BYTES);
    static constexpr std::size_t MAX_LOG2 = std::countr_zero(HEAP_BYTES);
    static std::uintptr_t base() noexcept { return range_at() + HEAP_AT; }
    static std::uintptr_t end() noexcept { return base() + HEAP_BYTES; }
};

SlabMalloc<HeapRange> slab;

void *operator new(std::size_t n) {
    void *p = slab.alloc(n, alignof(std::max_align_t));
    if (p == nullptr) bug(Errno::enomem, n);
    return p;
}
void *operator new(std::size_t n, std::align_val_t a) {
    void *p = slab.alloc(n, static_cast<std::size_t>(a));
    if (p == nullptr) bug(Errno::enomem, n);
    return p;
}
void *operator new(std::size_t n, const std::nothrow_t &) noexcept {
    return slab.alloc(n, alignof(std::max_align_t));
}
void *operator new(std::size_t n, std::align_val_t a, const std::nothrow_t &) noexcept {
    return slab.alloc(n, static_cast<std::size_t>(a));
}
void operator delete(void *p, std::size_t n) noexcept {
    if (p != nullptr) slab.dealloc(p, n, alignof(std::max_align_t));
}
void operator delete(void *p, std::size_t n, std::align_val_t a) noexcept {
    if (p != nullptr) slab.dealloc(p, n, static_cast<std::size_t>(a));
}
// Without its size a block cannot be given back to a slab.
void operator delete(void *p) noexcept {
    if (p != nullptr) bug(Errno::enotrecoverable);
}
void operator delete(void *p, std::align_val_t) noexcept {
    if (p != nullptr) bug(Errno::enotrecoverable);
}

extern "C" std::uint32_t xpute_conformance_boot(std::uint32_t seed) {
    // The stack the module was linked with is the one the guest declares.
    ensure(wasm::stack_high() == STACK_BYTES, Errno::enotrecoverable, wasm::stack_high());
    std::uint32_t range = range_at();
    // A range past the memory's end is a host that sized it for another guest.
    ensure(std::uint64_t{range} + MEM_BYTES <= wasm::given(), Errno::enomem, range);
    set_hostcall(host_hostcall);
    return boot(mem::core(), range, host_now, seed);
}

extern "C" double run(double) {
    return turn(mem::core(), range_at());
}

#else

// The memory a native host hands over, MEM_BYTES from `mem`; the guest
// allocates from the process, so nothing reads the ambient one.
Memory given(std::uint8_t *mem) noexcept { return Memory(reinterpret_cast<std::uintptr_t>(mem), MEM_BYTES); }

extern "C" std::uint32_t xpute_conformance_boot(std::uint8_t *mem, xpute_conformance_now now, xpute_conformance_hostcall hostcall, std::uint32_t seed) {
    set_hostcall(hostcall);
    return boot(given(mem), 0, now, seed);
}

extern "C" double xpute_conformance_run(std::uint8_t *mem, double) { return turn(given(mem), 0); }

#endif
