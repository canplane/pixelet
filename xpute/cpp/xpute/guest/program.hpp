// cpp/xpute/guest/program.hpp

// A guest in one declaration (`XPUTE_PROGRAM`): its range laid out past where
// the linker stopped, the toc's page, the two rings and their slots and the
// hostcalls' stream, then the heap; its turns run; and each command handed to
// the function the program names for it, whose result is the reply and whose
// errno is the failure. Work that waits on the host runs as a task
// (`Turn::spawn`): a file read whole (`read`) is awaited, and each await ends
// at a later turn. A bug is published to the host (stop.hpp). What lies under
// it, ipc/door.hpp, ipc/sys.hpp, ipc/toc.hpp, sched/task.hpp and stop.hpp, a
// guest that lays its memory out itself uses as it is.

#pragma once

#include <bit>
#include <concepts>
#include <coroutine>
#include <cstddef>
#include <cstdint>
#include <new>
#include <optional>
#include <span>
#include <string_view>
#include <type_traits>
#include <utility>

#include "xpute/kit/alloc/slab.hpp"
#include "xpute/kit/status/bug.hpp"
#include "xpute/kit/status/errno.spec.hpp"
#include "xpute/kit/status/error.hpp"
#include "xpute/kit/wire/xtp/cursor.hpp"
#include "xpute/kit/wire/xtp/encode.hpp"
#include "xpute/guest/ipc/door.hpp"
#include "xpute/guest/ipc/file.hpp"
#include "xpute/guest/ipc/ring.hpp"
#include "xpute/guest/ipc/sys.hpp"
#include "xpute/guest/mem/section.hpp"
#include "xpute/guest/sched/task.hpp"
#include "xpute/guest/wasm.hpp"
#include "program.spec.hpp"

namespace xpute {

// The port a program's host speaks as unless it says otherwise, and the one a
// handler's events are raised for (@xpute/host port.ts `PROGRAM_PORT`).
inline constexpr std::uint32_t PROGRAM_PORT = program::PORT;

// What the range holds, by count; the offsets follow from it.
struct Layout {
    // Each ring's slots: how many commands can wait, and replies and events be
    // left, between two turns.
    std::uint32_t cap;
    // A slot's bytes: the most a command's packet, or an event's, may be.
    std::uint32_t slot_bytes;
    // The hostcalls' stream, in words: what one turn may ask the host to run
    // after it.
    std::uint32_t sys_words;
    // The events one command may raise beside its reply.
    std::uint32_t most_events;

    static const Layout DEFAULT;

    constexpr std::uint32_t sq_at() const noexcept { return PAGE_BYTES; }
    constexpr std::uint32_t cq_at() const noexcept { return sq_at() + ring_bytes(cap); }
    constexpr std::uint32_t sq_slots_at() const noexcept { return cq_at() + ring_bytes(cap); }
    constexpr std::uint32_t cq_slots_at() const noexcept { return sq_slots_at() + slots_bytes(cap, slot_bytes); }
    constexpr std::uint32_t sys_at() const noexcept { return cq_slots_at() + slots_bytes(cap, slot_bytes); }

    // Where the heap begins, from the range's start.
    constexpr std::uint32_t heap_at() const noexcept { return page_up(sys_at() + 4 * sys_words); }
};

inline constexpr Layout Layout::DEFAULT{
    .cap = program::CAP,
    .slot_bytes = program::SLOT_BYTES,
    .sys_words = program::SYS_WORDS,
    .most_events = program::MOST_EVENTS,
};

// The range starts on the page past where the linker stopped.
constexpr std::uint32_t range_at(std::uint32_t data_end) noexcept {
    return page_up(data_end);
}

// Where the heap begins in the memory.
constexpr std::uint32_t heap_at(const Layout &layout, std::uint32_t data_end) noexcept {
    return range_at(data_end) + layout.heap_at();
}

// Lays the range out and writes the toc, before the first turn: the toc's
// offset, which the module's `boot` answers.
std::uint32_t boot(const Layout &layout, std::uint32_t data_end) noexcept;

// A command's packet as its handler reads it: a branch of fields, or none.
class Packet {
  public:
    // An empty `pkt` is none: a packet holds its header at least.
    explicit Packet(std::span<const std::uint8_t> pkt) noexcept : pkt_(pkt) {}

    // Its fields; EBADMSG where the command carried none, or why they do not parse.
    Result<xtp::BranchCursor> fields() const noexcept;

    // Field `i` as a number, whatever width the sender lowered it as.
    Result<double> number(std::uint32_t i) const noexcept;

    // Field `i` as text.
    Result<std::string_view> str(std::uint32_t i) const noexcept;

    // The packet's bytes, for a reader of the program's own.
    std::optional<std::span<const std::uint8_t>> bytes() const noexcept {
        return pkt_.empty() ? std::nullopt : std::optional(pkt_);
    }

  private:
    std::span<const std::uint8_t> pkt_;
};

// What a handler takes a command's packet as: the packet itself, or the struct
// its table generates for it (xpute-tool `spec/cmd`), read by its
// `from_packet`, EBADMSG where it does not read.
template <class T>
concept FromPacket = std::same_as<T, Packet> || requires(Packet pkt) {
    { T::from_packet(pkt) } -> std::same_as<Result<T>>;
};

// A turn as a handler sees it: the host to call, and events to raise.
class Turn {
  public:
    Turn(Sys &sys, const Door &door) noexcept : sys_(sys), door_(door) {}

    // A hostcall of the program's own, `code` its number, answered by the host
    // before it returns from what the host already holds.
    Result<std::uint32_t> call_now(std::uint32_t code, std::span<const std::uint32_t> args = {}) noexcept {
        return sys_.call_now(code | sys::op::PROGRAM, args, {});
    }

    // A hostcall of the program's own, run by the host after the turn: what it
    // writes back is read on a later turn.
    void call(std::uint32_t code, std::span<const std::uint32_t> args, std::span<const std::uint8_t> bytes = {}) noexcept {
        sys_.call(code | sys::op::PROGRAM, args, bytes);
    }

    // An event for the program's port, heard after the turn.
    template <Event E> void raise(const E &event) const noexcept { door_.raise_to(PROGRAM_PORT, event); }

    // An event of no table's: `cmd`, and `cnt` fields, which `fill` writes.
    template <class Fill> void raise_packet(std::uint32_t cmd, std::uint32_t cnt, Fill &&fill) const noexcept {
        door_.raise(PROGRAM_PORT, cmd, cnt, fill);
    }

    // Work that waits on the host, first resumed after this turn's commands
    // and then whenever what it awaits is ready; EAGAIN where the heap or the
    // executor has no room for it.
    Result<void> spawn(Task<> work) const noexcept;

  private:
    Sys &sys_;
    const Door &door_;
};

namespace program_detail {

struct Program;

// The booted program's door, none before `boot`.
const Door *door() noexcept;

} // namespace program_detail

// An event for the program's port from a task, heard after the turn.
template <Event E> void raise(const E &event) noexcept {
    if (const Door *door = program_detail::door()) door->raise_to(PROGRAM_PORT, event);
}

// A read awaited (`read`), holding its own copy of the path, so it may be made
// in a handler and handed to a task; moved only before it is awaited, and
// destroyed before it is ready, its open is cancelled.
class [[nodiscard]] Read {
  public:
    Read(std::uint32_t root, std::string_view path) noexcept;
    Read(Read &&o) noexcept;
    Read(const Read &) = delete;
    Read &operator=(const Read &) = delete;
    ~Read();

    bool await_ready() const noexcept { return got_ == Got::done; }
    template <class P> bool await_suspend(std::coroutine_handle<P> h) noexcept { return open(waker_of(h)); }
    Result<Bytes> await_resume() noexcept;

  private:
    friend struct program_detail::Program;

    enum class Got : std::uint8_t {
        idle,
        opening,
        // Filled by the host after the turn `asked_` ends.
        reading,
        done,
    };

    // False where it is done at once, without waiting.
    bool open(Waker waker) noexcept;
    void ready(Errno err) noexcept;

    std::uint32_t root_;
    Bytes path_;
    Fd fd_ = 0;
    Got got_ = Got::idle;
    std::uint32_t asked_ = 0;
    Errno err_ = Errno::ok;
    Bytes bytes_;
    Waker waker_;
};

// The file at `path` below `root`, whole: its bytes, or the errno its open was
// refused with (ENOENT where there is none; what a root means is the host's
// `Fs`), EMFILE where as many are open as a program may have, ENOMEM where the
// heap has no room for it or its path. Opened on the turn it is first awaited,
// read on the turn its open is answered, and ready on the turn after.
inline Read read(std::uint32_t root, std::string_view path) noexcept {
    return Read(root, path);
}

// What a command is handed to: its reply, or its errno.
using Handler = Result<std::int32_t> (*)(Turn &, Packet);

// A command's number and its handler (`on`).
struct On {
    std::uint32_t cmd;
    Handler handler;
};

namespace program_detail {

template <class A> A arg_of(Result<std::int32_t> (*)(Turn &, A));
template <class A> A arg_of(Result<std::int32_t> (*)(Turn &, A) noexcept);

template <auto F> Result<std::int32_t> handle(Turn &cx, Packet pkt) {
    using A = std::remove_cvref_t<decltype(arg_of(F))>;
    static_assert(FromPacket<A>, "a handler takes its packet as Packet or as the struct its table generates");
    if constexpr (std::same_as<A, Packet>) {
        return F(cx, pkt);
    } else {
        Result<A> a = A::from_packet(pkt);
        if (!a) return a.error();
        return F(cx, std::move(*a));
    }
}

} // namespace program_detail

// `F` for the command `Cmd`: F takes the turn and the packet, as `Packet` or as
// the struct its table generates (`FromPacket`), and answers its reply or its
// errno.
template <auto Cmd, auto F> inline constexpr On on{static_cast<std::uint32_t>(Cmd), &program_detail::handle<F>};

// A turn: what waits, each command to its handler in order, ENOSYS for one
// with none, then the tasks; the next turn asked for while any is left.
double turn(std::span<const On> handlers) noexcept;

#ifdef __wasm__

// A module program's heap: from past its range to the end of the memory
// given, which never grows.
template <Layout L> struct ProgramHeap {
    static constexpr std::size_t MIN_LOG2 = std::countr_zero(PAGE_BYTES);
    // The largest power of two short of the whole address space: the tree grows
    // only as requests need, and `end` says what is real.
    static constexpr std::size_t MAX_LOG2 = ADDR_BITS - 1;
    static std::uintptr_t base() noexcept { return heap_at(L, wasm::data_end()); }
    static std::uintptr_t end() noexcept { return static_cast<std::uintptr_t>(wasm::given()); }
};

// `boot` checked against the stack the module was linked with, the host's
// clock and hostcall installed.
std::uint32_t boot_module(const Layout &layout, std::uint32_t stack_bytes) noexcept;

#endif

} // namespace xpute

#ifdef __wasm__

// A guest built as a WebAssembly module, in one declaration: its layout
// (`Layout::DEFAULT` in `XPUTE_PROGRAM`), the stack it was linked with (the
// spec's `program::STACK_BYTES` in `XPUTE_PROGRAM`, as cpp/Makefile links the
// example), and a handler for each command (`on`). It defines the module's
// allocator over its heap, and exports `run`, the turn, and `boot`, which lays
// the range out and answers where the toc lies (@xpute/host port.ts `open`).
#define XPUTE_PROGRAM_WITH(layout, stack_bytes, ...)                                                                      \
    namespace {                                                                                                           \
    constexpr ::xpute::Layout xpute_layout = (layout);                                                                    \
    ::xpute::SlabMalloc<::xpute::ProgramHeap<xpute_layout>> xpute_heap;                                                   \
    void *xpute_alloc(std::size_t n, std::size_t align) noexcept {                                                        \
        void *p = xpute_heap.alloc(n, align);                                                                             \
        if (p == nullptr) ::xpute::bug(::xpute::Errno::enomem, n);                                                        \
        return p;                                                                                                         \
    }                                                                                                                     \
    }                                                                                                                     \
    void *operator new(std::size_t n) { return xpute_alloc(n, alignof(std::max_align_t)); }                               \
    void *operator new(std::size_t n, std::align_val_t a) { return xpute_alloc(n, static_cast<std::size_t>(a)); }         \
    void *operator new(std::size_t n, const std::nothrow_t &) noexcept { return xpute_heap.alloc(n, alignof(std::max_align_t)); } \
    void *operator new(std::size_t n, std::align_val_t a, const std::nothrow_t &) noexcept {                              \
        return xpute_heap.alloc(n, static_cast<std::size_t>(a));                                                          \
    }                                                                                                                     \
    void operator delete(void *p, std::size_t n) noexcept {                                                               \
        if (p != nullptr) xpute_heap.dealloc(p, n, alignof(std::max_align_t));                                            \
    }                                                                                                                     \
    void operator delete(void *p, std::size_t n, std::align_val_t a) noexcept {                                           \
        if (p != nullptr) xpute_heap.dealloc(p, n, static_cast<std::size_t>(a));                                          \
    }                                                                                                                     \
    /* Without its size a block cannot be given back to a slab. */                                                        \
    void operator delete(void *p) noexcept {                                                                              \
        if (p != nullptr) ::xpute::bug(::xpute::Errno::enotrecoverable);                                                  \
    }                                                                                                                     \
    void operator delete(void *p, std::align_val_t) noexcept {                                                            \
        if (p != nullptr) ::xpute::bug(::xpute::Errno::enotrecoverable);                                                  \
    }                                                                                                                     \
    extern "C" __attribute__((export_name("boot"))) std::uint32_t boot() {                                                \
        return ::xpute::boot_module(xpute_layout, (stack_bytes));                                                         \
    }                                                                                                                     \
    extern "C" __attribute__((export_name("run"))) double run(double) {                                                   \
        static constexpr ::xpute::On xpute_handlers[]{__VA_ARGS__};                                                       \
        return ::xpute::turn(xpute_handlers);                                                                             \
    }

// `XPUTE_PROGRAM_WITH` with the default layout and stack.
#define XPUTE_PROGRAM(...) XPUTE_PROGRAM_WITH(::xpute::Layout::DEFAULT, ::xpute::program::STACK_BYTES, __VA_ARGS__)

#endif
