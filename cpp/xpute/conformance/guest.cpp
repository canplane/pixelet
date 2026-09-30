// cpp/xpute/conformance/guest.cpp

#include "guest.hpp"

#include <array>
#include <new>
#include <optional>
#include <span>

#include "../core/math/scalar.hpp"
#include "../core/status/errno.spec.hpp"
#include "../core/wire/xtp/cursor.hpp"
#include "../runtime/abi/handle.hpp"
#include "../runtime/ipc/directory.hpp"
#include "../runtime/ipc/door.hpp"
#include "../runtime/ipc/ring.hpp"
#include "../runtime/ipc/stream.hpp"
#include "guest.spec.hpp"

namespace xpute::conformance {
namespace {

constexpr std::uint32_t PAGE = 4096;

constexpr std::uint32_t page_up(std::uint32_t bytes) {
    return (bytes + PAGE - 1) / PAGE * PAGE;
}

// Where this guest's ranges lie. Not where another guest puts them: the
// stream first and the completion ring before the submission ring, so a host
// that finds them at all has found them through the directory.
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

State &state_of(std::uint8_t *mem) {
    return *std::launder(reinterpret_cast<State *>(mem + STATE_AT));
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
        std::uint32_t h = s.handles.acquire();
        if (h == 0) return {.error = Errno::enfile};
        return {.value = static_cast<std::int32_t>(h)};
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

} // namespace
} // namespace xpute::conformance

using namespace xpute;
using namespace xpute::conformance;

extern "C" void xpute_conformance_boot(std::uint8_t *mem, xpute_conformance_now now) {
    Memory m(mem);
    // A command leaves at most its signals and its reply.
    Door door(Ring::init(m, SUBMISSION_DESC_AT, SUBMISSION_CAPACITY, SUBMISSION_SLOT_AT, SLOT_BYTES),
              Ring::init(m, COMPLETION_DESC_AT, COMPLETION_CAPACITY, COMPLETION_SLOT_AT, SLOT_BYTES), MOST_RAISE + 1);
    new (mem + STATE_AT) State{.door = door, .stream = Stream(m, STREAM_AT, STREAM_WORDS), .handles = {}, .now = now};
    const std::array<Entry, 3> entries{{
        {directory::key::SUBMISSION, SUBMISSION_DESC_AT},
        {directory::key::COMPLETION, COMPLETION_DESC_AT},
        {key::STREAM, STREAM_AT},
    }};
    write_directory(m, DIRECTORY_AT, entries);
}

extern "C" double xpute_conformance_interrupt(std::uint8_t *mem, double) {
    State &s = state_of(mem);
    s.door.drain([&s](std::uint32_t cmd, std::span<const std::uint8_t> packet) { return apply(s, cmd, packet); });
    s.stream.publish();
    return s.door.pending() ? 0.0 : -1.0;
}
