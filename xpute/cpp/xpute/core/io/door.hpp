// cpp/xpute/core/io/door.hpp

// The door is how the core takes the frames its runtime sends it, and sends
// its own (host.hpp). A turn receives every frame that waits, one at a time
// into the door's receive frame, and applies each in order. The core sends
// its replies and its own commands as it goes, each laid in the door's send
// frame, which the runtime takes before `send` returns. Both frames lie in
// the core's memory wherever the core put them, and each call names its frame
// by its offset.
//
// Each command is routed by its major to its protocol's generated `apply`:
// xpute's own (abi/sys.json, abi/fs.json) to the door's end of them, which
// answers NOP and hands the program the rest as what each means, and every
// other to the program, as its own (`Program`). The program is handed each
// completion of a request it made too (`Completion`), which its protocol's
// generated `completion` reads.
//
// The core's imports, the commands it sends the runtime, are frames too, each
// sent by its protocol's generated function. A send goes without ACKREQ and is
// answered only where it fails; a request goes with ACKREQ, and the runtime
// completes it once, on a later turn, with a frame of its tag flagged RES. A
// call, which is answered before it returns, is laid in the send frame and
// handed to the runtime's `call`. The runtime routes each import by its
// command's major.
//
// - R1. Memory an import names (`named`), or a reply (`Outcome::named`), is
//   taken with the frame, so it is the core's again once `send` returns: a
//   file's write is, and a reply's bytes. A file's read
//   names memory the runtime fills later: it is the runtime's until the
//   FS_CLOSE of its descriptor is completed, and the file table holds the
//   buffer until then (file.hpp). Memory a call names is the runtime's only
//   until the call returns.
// - R2. A completion finds its request by its tag, never by when it came.
// - R3. A call never nests: the runtime answers it without calling into the
//   core, and the core makes no call while another is unanswered.

#pragma once

#include <array>
#include <concepts>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <source_location>
#include <span>
#include <string_view>
#include <utility>
#include <variant>

#include "xpute/kit/status/bug.hpp"
#include "xpute/kit/status/errno.spec.hpp"
#include "xpute/kit/wire/xtp/encode.hpp"
#include "xpute/core/abi/sys.spec.hpp"
#include "xpute/core/abi/fs.spec.hpp"
#include "xpute/core/abi/handle.hpp"
#include "xpute/core/host.hpp"
#include "xpute/core/mem.hpp"
#include "frame.hpp"
#include "fs.hpp"

namespace xpute {

// An import is what xpute-link-gen generates from a protocol's table (`spec/cmd`):
// its command number, the number of children its packet is written as,
// whether it goes with ACKREQ, its writer, and the memory it names, where it
// is and how long (0 long for none).
template <class E>
concept Import = requires(const E &e, xtp::PacketWriter &w) {
    { E::NUMBER } -> std::convertible_to<std::uint32_t>;
    { E::FIELDS } -> std::convertible_to<std::uint32_t>;
    { E::ACKREQ } -> std::convertible_to<bool>;
    e.write(w);
    { e.named() } -> std::same_as<std::array<std::uint32_t, 2>>;
};

// The core's life is as its runtime tells it (abi/sys.json). What each event
// means the program keeps or resets is the program's to decide.
enum class Life { attach, suspend, resume };

struct Program {
    std::uint32_t cmd = 0;
    std::span<const std::uint8_t> pkt;
};

// The door hands the program a change in its life that its runtime tells it
// of, a turn the scheduling policy grants (SYS_TURN) and its frame
// (SYS_FRAME), a file changed under it (FS_CHANGED), a command of the
// program's own, for its protocol's generated `apply`, or the completion of a
// request it made, for its protocol's generated `completion`. Nothing answers
// a completion, so what the program returns for one is dropped. The receive
// runs before the turn's work, so the program should record what it is
// handed and return.
using Event = std::variant<Life, abi::sys::SysTurn, abi::sys::SysFrame, Completion, abi::fs::FsChanged, Program>;

// How many bytes from the end of a stop report's file path are kept when
// the whole path would not fit in a frame.
inline constexpr std::size_t ABORT_PATH_BYTES = 32;

// The bytes of a stop report with its file path left out, which every
// frame's packet must hold so that a stop is always reported (`abort`): XTP's
// header, the root's table of the report's children, its numbers of four
// bytes each, and an empty string, its length word and terminator padded to
// a word (xpute/kit/wire/xtp/encode.hpp).
inline constexpr std::uint32_t ABORT_PKT_BYTES = [] {
    constexpr std::uint32_t FIELDS = abi::sys::SysAbort::FIELDS;
    auto word_up = [](std::uint32_t n) { return (n + xtp::WORD_SZ - 1) / xtp::WORD_SZ * xtp::WORD_SZ; };
    std::uint32_t numbers = xtp::HDR_SZ + xtp::WORD_SZ * (1 + FIELDS) + 4 * (FIELDS - 1);
    return word_up(word_up(numbers) + xtp::WORD_SZ + 1);
}();

class Door {
  public:
    // A door over the receive frame at `receive_at` and the send frame at
    // `send_at`, each `frame_bytes` long and 8-aligned, which nothing else
    // holds for the door's life.
    Door(Memory mem, std::uint32_t receive_at, std::uint32_t send_at, std::uint32_t frame_bytes) noexcept
        : mem_(mem), receive_at_(receive_at), send_at_(send_at), frame_bytes_(frame_bytes) {}

    // The core's memory, in which an import names what it lends.
    const Memory &mem() const noexcept { return mem_; }
    Door(Door &&) noexcept = default;
    Door &operator=(Door &&) = delete;
    Door(const Door &) = delete;
    Door &operator=(const Door &) = delete;

    // Sends a frame tagged `tag` and flagged `flags`, with `result` and no
    // packet.
    void post(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result) const noexcept {
        send_frame(tag, cmd, flags, result, {0, 0}, std::nullopt);
    }

    template <Import E> void send(const E &e) const noexcept { import(0, e); }

    // Sends `e` tagged `tag`, with ACKREQ where it goes with it, for the
    // runtime to complete on a later turn.
    template <Import E> void import(std::uint32_t tag, const E &e) const noexcept {
        send_frame(tag, E::NUMBER, E::ACKREQ ? frame::ACKREQ : 0, 0, e.named(), packet_of(e));
    }

    // The next handle with no value, slot 0 under its next generation: the
    // handle of a request, or of a written file, which holds no slot of a table
    // (abi/handle.hpp). It differs from every handle that holds a value, and
    // from those it issued before, until its generation comes round.
    std::uint32_t tag() const noexcept {
        tags_ = static_cast<std::uint16_t>(tags_ + 1) == 0 ? 1 : static_cast<std::uint16_t>(tags_ + 1);
        return abi::tag_of(tags_);
    }

    // Sends `e` with ACKREQ under the next handle with no value, and answers
    // it: the runtime completes the request on a later turn under that handle
    // (`Completion`).
    template <Import E> std::uint32_t request(const E &e) const noexcept {
        std::uint32_t handle = tag();
        import(handle, e);
        return handle;
    }

    // `call` sends `e`, which the runtime answers before this returns. It
    // answers the runtime's value or errno, or EMSGSIZE when the frame does
    // not fit.
    template <Import E> Result<std::uint32_t> call(const E &e) const noexcept {
        if (Errno laid = lay(0, E::NUMBER, frame::ACKREQ, 0, packet_of(e)); laid != Errno::ok) return make_error(ErrorClass::fault, laid);
        return answer_of(host::call(send_at_));
    }

    // Sends the stop report as the core stops. Where the whole report would
    // not fit in a frame, its file path is cut to its last bytes, or left
    // out.
    void abort(Errno errnum, const std::source_location &at, std::uint64_t a, std::uint64_t b) const noexcept {
        std::string_view path = at.file_name();
        std::string_view tail = path.substr(path.size() > ABORT_PATH_BYTES ? path.size() - ABORT_PATH_BYTES : 0);
        for (std::string_view file : {path, tail, std::string_view{}}) {
            abi::sys::SysAbort report{
                .errnum = static_cast<std::int32_t>(errnum),
                .line = at.line(),
                .column = at.column(),
                .a_low = static_cast<std::uint32_t>(a),
                .a_high = static_cast<std::uint32_t>(a >> 32),
                .b_low = static_cast<std::uint32_t>(b),
                .b_high = static_cast<std::uint32_t>(b >> 32),
                .file = file,
            };
            if (lay(0, abi::sys::SysAbort::NUMBER, 0, 0, packet_of(report)) == Errno::ok) return host::send(send_at_, 0, 0);
        }
    }

    // Receives and applies every frame that waits, in order. It replies only
    // to frames flagged ACKREQ and to failures, and never to a completion.
    // NOP answers its value here; every other command goes to `handle` as
    // what it means (`Event`).
    template <class Handle> void receive(Handle &&handle) const noexcept {
        while (host::receive(receive_at_, frame_bytes_) > 0) {
            Frame f = Frame::at(mem_, receive_at_);
            Result<std::span<const std::uint8_t>> pkt = f.pkt(mem_, receive_at_, frame_bytes_);
            if (f.flags & frame::RES) {
                (void)handle(Event{Completion{.cmd = f.cmd, .tag = f.tag, .res = f.result, .pkt = pkt ? *pkt : std::span<const std::uint8_t>()}});
                continue;
            }
            Outcome a = pkt ? apply_frame(f, *pkt, handle) : Outcome{.err = pkt.error().errnum};
            if (a.err != Errno::ok) {
                post(f.tag, f.cmd, frame::RES, static_cast<std::int32_t>(a.err));
            } else if (f.flags & frame::ACKREQ) {
                send_frame(f.tag, f.cmd, frame::RES, a.val, {mem_.off_of(a.named), static_cast<std::uint32_t>(a.named.size())}, std::nullopt);
            }
        }
    }

  private:
    // Writes a packet into the room it is given, and answers its length, or
    // none when it does not fit.
    using Write = std::optional<std::uint32_t> (*)(const void *what, std::span<std::uint8_t> room);

    struct Packet {
        Write write;
        const void *what;
    };

    template <Import E> static Packet packet_of(const E &e) noexcept {
        return {[](const void *what, std::span<std::uint8_t> room) -> std::optional<std::uint32_t> {
                    xtp::PacketWriter w(room);
                    w.branch(E::FIELDS, [what](xtp::PacketWriter &b) { static_cast<const E *>(what)->write(b); });
                    Result<std::uint32_t> n = w.finish();
                    return n ? std::optional(*n) : std::nullopt;
                },
                &e};
    }

    // Lays a frame in the send frame, with the packet `pkt` writes, or none
    // without it. A packet that does not fit answers EMSGSIZE.
    Errno lay(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, std::optional<Packet> pkt) const noexcept {
        std::uint32_t pkt_len = 0;
        if (pkt) {
            std::optional<std::uint32_t> n = pkt->write(pkt->what, std::span<std::uint8_t>(mem_.at(send_at_ + frame::WORD_BYTES), frame_bytes_ - frame::WORD_BYTES));
            if (!n) return Errno::emsgsize;
            pkt_len = *n;
        }
        Frame{.tag = tag, .cmd = cmd, .flags = flags, .result = result, .pkt_len = pkt_len}.lay(mem_, send_at_);
        return Errno::ok;
    }

    // Sends a frame with the packet `pkt` writes, or none, and the memory it
    // names. A frame that does not fit is a bug.
    void send_frame(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, std::array<std::uint32_t, 2> named,
                    std::optional<Packet> pkt) const noexcept {
        Errno laid = lay(tag, cmd, flags, result, pkt);
        ensure(laid == Errno::ok, Errno::emsgsize, cmd);
        host::send(send_at_, named[0], named[1]);
    }

    // The door's end of xpute's own protocols: NOP answered here, and every
    // other export handed to the program as what it means.
    template <class Handle> struct Own {
        Handle &handle;

        Outcome nop(const abi::sys::Nop &c) noexcept { return {.val = static_cast<std::int32_t>(c.val)}; }
        Outcome sys_attach() noexcept { return handle(Event{Life::attach}); }
        Outcome sys_suspend() noexcept { return handle(Event{Life::suspend}); }
        Outcome sys_resume() noexcept { return handle(Event{Life::resume}); }
        Outcome sys_turn(const abi::sys::SysTurn &c) noexcept { return handle(Event{c}); }
        Outcome sys_frame(const abi::sys::SysFrame &c) noexcept { return handle(Event{c}); }
        Outcome fs_changed(const abi::fs::FsChanged &c) noexcept { return handle(Event{c}); }
    };

    // Applies one submitted command, by its major: xpute's own through their
    // protocols' generated `apply`, and every command past xpute's majors to
    // the program. A number xpute gives no protocol is answered with ENOSYS.
    template <class Handle> static Outcome apply_frame(const Frame &f, std::span<const std::uint8_t> pkt, Handle &handle) noexcept {
        const std::uint32_t major = frame::cmd_major(f.cmd);
        Own<Handle> own{handle};
        if (major == static_cast<std::uint32_t>(abi::sys::Major::NOP) || major == static_cast<std::uint32_t>(abi::sys::Major::SYS))
            return abi::sys::apply(own, f.cmd, pkt);
        if (major == static_cast<std::uint32_t>(abi::fs::Major::FS)) return abi::fs::apply(own, f.cmd, pkt);
        if (major < frame::PROGRAM_MAJOR) return {.err = Errno::enosys};
        return handle(Event{Program{.cmd = f.cmd, .pkt = pkt}});
    }

    Memory mem_;
    std::uint32_t receive_at_;
    std::uint32_t send_at_;
    // A frame's room: its words and the most its packet takes.
    std::uint32_t frame_bytes_;
    // The generation of the last handle with no value issued (`tag`).
    mutable std::uint16_t tags_ = 0;
};

namespace door {

inline std::optional<Door> &held() noexcept {
    static std::optional<Door> held;
    return held;
}

// Makes `door` the core's, as a core that lays its frames out does.
inline void set(Door door) noexcept { held().emplace(std::move(door)); }

// This is the door `set` opened. Asking for it before is a bug.
inline const Door &get() noexcept {
    ensure(held().has_value(), Errno::enotrecoverable);
    return *held();
}

} // namespace door

} // namespace xpute
