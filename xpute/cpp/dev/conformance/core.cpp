// cpp/dev/conformance/core.cpp

#include "core.hpp"

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
#include "xpute/core/abi/sys.spec.hpp"
#include "xpute/core/abi/handle.hpp"
#include "xpute/core/call.hpp"
#include "xpute/core/ipc/door.hpp"
#include "xpute/core/ipc/file.hpp"
#include "xpute/core/ipc/head.hpp"
#include "xpute/core/ipc/ring.hpp"
#include "xpute/core/ipc/stream.hpp"
#include "xpute/core/mem.hpp"
#include "xpute/core/mem/section.hpp"
#include "xpute/core/sched/wake.hpp"
#include "xpute/core/wasm.hpp"
#include "core.spec.hpp"

namespace xpute::conformance {
namespace {

constexpr std::uint32_t page_up(std::uint32_t bytes) {
    return align_up(bytes, PAGE_BYTES);
}

// The parts the core shares with its runtime lie each on pages of its own, and
// `layout` places them in an order the seed draws.
enum Part : std::size_t { HEAD, SQ_DESC, CQ_DESC, SQ_SLOT, CQ_SLOT, STREAM, PARTS };

constexpr std::array<std::uint32_t, PARTS> PART_BYTES{
    head::WORDS * 4,
    ring_bytes(SQ_CAP),
    ring_bytes(CQ_CAP),
    slots_bytes(SQ_CAP, SLOT_BYTES),
    slots_bytes(CQ_CAP, SLOT_BYTES),
    STREAM_WORDS * 4,
};

// The most any layout spans: the first page, which it starts past, every
// part, and the most gap pages before each part. The core's own state lies
// here, past every layout, so that no layout moves it.
constexpr std::uint32_t STATE_AT = [] {
    std::uint32_t end = PAGE_BYTES + PARTS * GAP_PAGES * PAGE_BYTES;
    for (std::uint32_t bytes : PART_BYTES) end += page_up(bytes);
    return end;
}();

// `layout` answers where each part lies under `seed`, counted from the range's
// start. The seed draws the order of the parts and, for each part, how many
// pages to leave before it. The first page is never used, since offset 0 names
// nothing (ARCHITECTURE.md, the contract's rule 1).
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

// CALL makes this import: a number, and the offset of the word the runtime
// writes in answer.
struct Inc {
    std::uint32_t n = 0;
    std::uint32_t at = 0;

    static constexpr std::uint32_t NUMBER = cmd::INC;
    static constexpr std::uint32_t FIELDS = 2;

    void write(xtp::PacketWriter &w) const noexcept {
        w.u32(n);
        w.u32(at);
    }
};

struct State {
    Door door;
    Stream stream;
    // Each open file's owner is the number its OPEN packet named.
    FileTable<std::uint32_t, HANDLES> files;
    abi::Table<std::monostate, HANDLES> handles;
    xpute_conformance_now now;
    double mark = 0;
    // What the runtime writes as it answers CALL's INC.
    std::uint32_t word = 0;
};

// The heap begins here. The memory never grows, so a module's allocator sits
// over a fixed range.
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

Outcome apply(State &s, std::uint32_t cmd, std::span<const std::uint8_t> pkt) {
    // A command that takes a number fails with EBADMSG when its packet does
    // not read as one.
    std::uint32_t arg = 0;
    if (cmd == cmd::RAISE || cmd == cmd::RECORD || cmd == cmd::RELEASE || cmd == cmd::USE || cmd == cmd::OPEN || cmd == cmd::CLOSE || cmd == cmd::CALL) {
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
        for (std::uint32_t k = 0; k < arg; k++) s.door.post(k, cmd::NOTE, 0, 0);
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
        return s.handles.remove(arg) ? Outcome{} : Outcome{.err = Errno::ebadf};
    case cmd::USE:
        return s.handles.live(arg) ? Outcome{} : Outcome{.err = Errno::ebadf};
    case cmd::MARK:
        s.mark = s.now();
        return {};
    case cmd::OPEN:
        return {.val = static_cast<std::int32_t>(s.files.open(s.door, 1, "f", arg))};
    case cmd::CLOSE:
        return s.files.close(s.door, arg) ? Outcome{} : Outcome{.err = Errno::ebadf};
    case cmd::SINCE: {
        double since = s.now() - s.mark;
        return {.val = static_cast<std::int32_t>(round(since))};
    }
    case cmd::CALL: {
        s.word = 0;
        Result<std::uint32_t> res = s.door.call(Inc{.n = arg, .at = s.stream.mem().off_of(&s.word)});
        if (!res) return {.err = res.error().errnum};
        // The word the runtime wrote before the call returned.
        return {.val = static_cast<std::int32_t>(*static_cast<volatile std::uint32_t *>(&s.word))};
    }
    default:
        return {.err = Errno::enosys};
    }
}

// Every offset the core writes is counted from the memory's start, so the
// start of the core's range is added to each.
std::uint32_t boot(Memory m, std::uint32_t range, xpute_conformance_now now, std::uint32_t seed) {
    std::array<std::uint32_t, PARTS> at = layout(seed);
    for (std::uint32_t &off : at) off += range;
    Door door(Ring::init(m, at[SQ_DESC], SQ_CAP, at[SQ_SLOT], SLOT_BYTES), Ring::init(m, at[CQ_DESC], CQ_CAP, at[CQ_SLOT], SLOT_BYTES), MOST_RAISE);
    new (m.at(range + STATE_AT)) State{
        .door = std::move(door), .stream = Stream(m, at[STREAM], STREAM_WORDS), .files = {}, .handles = {}, .now = now};
    // The stream lies where a GPU stream would, so the runtime runs it after
    // every turn.
    write_head(m, at[HEAD], Head{.sq_cap = SQ_CAP, .cq_cap = CQ_CAP, .slot_bytes = SLOT_BYTES, .sq_at = at[SQ_DESC], .cq_at = at[CQ_DESC], .stream_at = at[STREAM]});
    return at[HEAD];
}

double turn(Memory m, std::uint32_t range) {
    State &s = state_of(m, range);
    // The conformance core keeps nothing across its life, reads no file
    // again, and draws nothing, so it answers 0 to every other submission,
    // and a surface it is lent gets the handle 0, none.
    double wake = s.door.turn([&s](const Submission &sub) {
        if (const FsOpened *opened = std::get_if<FsOpened>(&sub)) {
            if (std::optional<std::uint32_t> owner = s.files.complete_open(*opened)) s.door.post(*owner, cmd::OPENED, 0, opened->res);
            return Outcome{};
        }
        if (const Program *p = std::get_if<Program>(&sub)) return apply(s, p->cmd, p->pkt);
        return Outcome{};
    });
    s.stream.publish();
    return wake;
}

#ifdef __wasm__

// The range starts past where the linker's data ends, so the size of the
// stack or the data changes nothing a runtime reads.
std::uint32_t range_at() {
    return page_up(wasm::data_end());
}

double runtime_now() {
    return wasm::now();
}

std::int32_t runtime_call(std::uint32_t at) {
    return wasm::call(at);
}

#endif

} // namespace
} // namespace xpute::conformance

using namespace xpute;
using namespace xpute::conformance;

#ifdef __wasm__

// The C library's allocator is never linked: it would grow a memory that
// never grows, and its failure path prints through WASI.
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
    // The stack the module was linked with is the one the core declares.
    ensure(wasm::stack_high() == STACK_BYTES, Errno::enotrecoverable, wasm::stack_high());
    std::uint32_t range = range_at();
    // A range past the memory's end means a runtime sized the memory for
    // another core.
    ensure(std::uint64_t{range} + MEM_BYTES <= wasm::mem_sz(), Errno::enomem, range);
    set_call(runtime_call);
    return boot(mem::core(), range, runtime_now, seed);
}

extern "C" double run(double) {
    return turn(mem::core(), range_at());
}

#else

// A native runtime hands over MEM_BYTES from `mem`. The core allocates from
// the process, so nothing reads the process-wide core memory (`mem::core`),
// which stays unset.
Memory memory_of(std::uint8_t *mem) noexcept { return Memory(reinterpret_cast<std::uintptr_t>(mem), MEM_BYTES); }

extern "C" std::uint32_t xpute_conformance_boot(std::uint8_t *mem, xpute_conformance_now now, xpute_conformance_call call, std::uint32_t seed) {
    set_call(call);
    return boot(memory_of(mem), 0, now, seed);
}

extern "C" double xpute_conformance_run(std::uint8_t *mem, double) { return turn(memory_of(mem), 0); }

#endif
