// cpp/xpute/guest/ipc/door.hpp

// The guest's end of its two rings. A command is started only while the
// completion ring has room for all it may leave (`reserve`), so one started
// never finds the ring full; what is not started waits in order. Overrunning
// the ring is a sizing fault.
//
// The guest's hostcalls are frames too (ipc/sys.json): its files and its report
// go to the host's port, 0, posted as any event is; a call answered before it
// returns, to any port, is laid unpublished in the next slot and its place
// handed to the host's `hostcall`.

#pragma once

#include <algorithm>
#include <concepts>
#include <cstdint>
#include <source_location>
#include <span>
#include <string_view>
#include <utility>
#include <vector>

#include "xpute/kit/status/bug.hpp"
#include "xpute/kit/status/errno.spec.hpp"
#include "xpute/kit/wire/xtp/encode.hpp"
#include "xpute/guest/abi/cmd.spec.hpp"
#include "xpute/guest/hostcall.hpp"
#include "frame.hpp"
#include "ring.hpp"
#include "sys.hpp"

namespace xpute {

// An event a table generates (xpute-tool `spec/cmd`): its number, the
// children it is written as, and its writer.
template <class E>
concept Event = requires(const E &e, xtp::PacketWriter &w) {
    { E::NUMBER } -> std::convertible_to<std::uint32_t>;
    { E::FIELDS } -> std::convertible_to<std::uint32_t>;
    e.write(w);
};

// Errno::enosys for a command the guest does not number.
struct Outcome {
    Errno err = Errno::ok;
    std::int32_t val = 0;
};

// How full the rings ran since the stat was last taken (`Door::take_stat`):
// each figure the most it reached.
struct DoorStat {
    std::uint32_t cq_peak = 0;
    // The submission ring's length as a drain started: how close it came to full.
    std::uint32_t sq_peak = 0;
    // The submissions a drain left for lack of room.
    std::uint32_t left_peak = 0;

    bool operator==(const DoorStat &) const = default;
};

// How much of a report's path is kept where the whole would not fit a slot.
inline constexpr std::size_t ABORT_PATH_BYTES = 32;

class Door {
  public:
    // `most_events` is the most one command raises, or the turn once its drain
    // is done: the program's bound.
    Door(Ring sq, Ring cq, std::uint32_t most_events) noexcept : sq_(sq), cq_(cq), reserve_(most_events + 1) {}

    const Ring &sq() const noexcept { return sq_; }
    const Ring &cq() const noexcept { return cq_; }

    DoorStat take_stat() const noexcept { return std::exchange(stat_, {}); }

    // A turn begins: what the last one's writes named is the host's no more.
    void rise() const noexcept { writes_.clear(); }

    void post(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result) const noexcept {
        posted(cmd, cq_.push(tag, cmd, flags, result));
    }

    template <class Write> void post_in_place(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, Write &&write) const noexcept {
        posted(cmd, cq_.push_in_place(tag, cmd, flags, result, write));
    }

    // An event for `port`.
    template <class Fill> void send_with(std::uint32_t port, std::uint32_t cmd, std::uint32_t cnt, Fill &&fill) const noexcept {
        post_in_place(frame::tag_of(port, 0), cmd, 0, 0, [&](std::span<std::uint8_t> out) -> std::optional<std::uint32_t> {
            xtp::PacketWriter w(out);
            w.branch(cnt, fill);
            Result<std::uint32_t> n = w.finish();
            return n ? std::optional(*n) : std::nullopt;
        });
    }

    // `event` for `port`.
    template <Event E> void send(std::uint32_t port, const E &event) const noexcept {
        send_with(port, E::NUMBER, E::FIELDS, [&event](xtp::PacketWriter &w) { event.write(w); });
    }

    // `event` for `port`, answered before this returns: laid in the next slot
    // unpublished and handed to the host's `hostcall`, its value or its errno;
    // ENOSPC where no slot is free, EMSGSIZE where it does not fit one.
    template <Event E> Result<std::uint32_t> call(std::uint32_t port, const E &event) const noexcept {
        Result<std::uint32_t> at = cq_.stage_in_place(frame::tag_of(port, 0), E::NUMBER, frame::ACKREQ, [&](std::span<std::uint8_t> out) -> std::optional<std::uint32_t> {
            xtp::PacketWriter w(out);
            w.branch(E::FIELDS, [&event](xtp::PacketWriter &b) { event.write(b); });
            Result<std::uint32_t> n = w.finish();
            return n ? std::optional(*n) : std::nullopt;
        });
        if (!at) return make_error(ErrorClass::fault, at.error().errnum == Errno::eagain ? Errno::enospc : at.error().errnum);
        return answer_of(xpute::hostcall(*at));
    }

    // Answered by SYS_OPENED on a later turn.
    void openat(Fd fd, std::uint32_t root, std::uint32_t flags, std::string_view path) const noexcept {
        send(frame::HOST, abi::SysOpenatEvent{.fd = fd, .root = root, .flags = flags, .path = path});
    }

    // `dst` is filled once the host takes the turn's frames, and must be kept until then.
    void read(Fd fd, std::span<std::uint8_t> dst) const noexcept {
        send(frame::HOST, abi::SysReadEvent{.fd = fd, .at = cq_.mem().off_of(dst), .len = static_cast<std::uint32_t>(dst.size())});
    }

    void write(Fd fd, std::vector<std::uint8_t> bytes) const noexcept {
        send(frame::HOST, abi::SysWriteEvent{.fd = fd, .at = cq_.mem().off_of(std::span<const std::uint8_t>(bytes)), .len = static_cast<std::uint32_t>(bytes.size())});
        writes_.push_back(std::move(bytes));
    }

    // A write is committed, an open not yet answered cancelled.
    void close(Fd fd) const noexcept { send(frame::HOST, abi::SysCloseEvent{.fd = fd}); }

    // Drops what the turn posted and posts the report alone, as the guest stops:
    // its file's path cut to its end, or left out, where the whole of it would
    // not fit a slot.
    void abort(Errno errnum, const std::source_location &at, std::uint64_t a, std::uint64_t b) const noexcept {
        cq_.clear();
        std::string_view path = at.file_name();
        std::string_view tail = path.substr(path.size() > ABORT_PATH_BYTES ? path.size() - ABORT_PATH_BYTES : 0);
        for (std::string_view file : {path, tail, std::string_view{}}) {
            abi::SysAbortEvent report{
                .errnum = static_cast<std::int32_t>(errnum),
                .line = at.line(),
                .column = at.column(),
                .a_low = static_cast<std::uint32_t>(a),
                .a_high = static_cast<std::uint32_t>(a >> 32),
                .b_low = static_cast<std::uint32_t>(b),
                .b_high = static_cast<std::uint32_t>(b >> 32),
                .file = file,
            };
            Errno e = cq_.push_in_place(frame::tag_of(frame::HOST, 0), abi::SysAbortEvent::NUMBER, 0, 0, [&](std::span<std::uint8_t> out) -> std::optional<std::uint32_t> {
                xtp::PacketWriter w(out);
                w.branch(abi::SysAbortEvent::FIELDS, [&report](xtp::PacketWriter &b) { report.write(b); });
                Result<std::uint32_t> n = w.finish();
                return n ? std::optional(*n) : std::nullopt;
            });
            if (e != Errno::emsgsize || file.empty()) return posted(abi::SysAbortEvent::NUMBER, e);
        }
    }

    // Applies waiting commands in order while there is room for what one more may
    // leave; replies to ACKREQ and to failures. Answers how many completions it made.
    // What the host sent, port 0, is xpute's own and applied here: NOP answers
    // its value, and an open the host completed goes to `opened`; every other
    // sender's goes to `apply`. A drain runs before
    // any phase's budget, so the program records what it is handed and returns.
    template <class Opened, class Apply> std::uint32_t receive(Opened &&opened, Apply &&apply) const noexcept {
        std::uint32_t tail = cq_.tail();
        stat_.sq_peak = std::max(stat_.sq_peak, sq_.len());
        while (has_room()) {
            std::optional<Frame> f = sq_.peek();
            if (!f) break;
            Result<std::span<const std::uint8_t>> pkt = sq_.pkt(*f);
            Outcome a = pkt ? apply_frame(*f, *pkt, opened, apply) : Outcome{.err = pkt.error().errnum};
            sq_.advance();
            if (a.err != Errno::ok) {
                post(f->tag, f->cmd, frame::RES, static_cast<std::int32_t>(a.err));
            } else if (f->flags & frame::ACKREQ) {
                post(f->tag, f->cmd, frame::RES, a.val);
            }
        }
        stat_.left_peak = std::max(stat_.left_peak, sq_.len());
        return cq_.tail() - tail;
    }

    bool pending() const noexcept { return !sq_.empty(); }

  private:
    bool has_room() const noexcept { return cq_.len() + reserve_ <= cq_.cap(); }

    template <class Opened, class Apply> static Outcome apply_frame(const Frame &f, std::span<const std::uint8_t> pkt, Opened &opened, Apply &apply) noexcept {
        if (frame::port_of(f.tag) != frame::HOST) return apply(f.cmd, pkt);
        if (f.cmd == static_cast<std::uint32_t>(abi::Command::NOP)) {
            if (pkt.empty()) return {};
            Result<abi::Nop> nop = abi::Nop::read(pkt);
            if (!nop) return {.err = nop.error().errnum};
            return {.val = static_cast<std::int32_t>(nop->val)};
        }
        if (f.cmd == static_cast<std::uint32_t>(abi::Command::SYS_OPENED)) {
            Result<abi::SysOpened> o = abi::SysOpened::read(pkt);
            if (!o) return {.err = o.error().errnum};
            return opened(*o);
        }
        return {.err = Errno::enosys};
    }

    void posted(std::uint32_t cmd, Errno e) const noexcept {
        stat_.cq_peak = std::max(stat_.cq_peak, cq_.len());
        ensure(e == Errno::ok, Errno::enospc, cmd, static_cast<std::uint64_t>(static_cast<std::int64_t>(e)));
    }

    Ring sq_;
    Ring cq_;
    // What one command may leave: its events, or the turn's after the drain, and its reply.
    std::uint32_t reserve_;
    // Kept by the const calls that move the rings, as the rings' own words are.
    mutable DoorStat stat_;
    // Kept until the host has run the `write` that names it.
    mutable std::vector<std::vector<std::uint8_t>> writes_;
};

} // namespace xpute
