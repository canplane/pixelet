// cpp/xpute/guest/ipc/door.hpp

// The guest's end of its two rings. A command is started only while the
// completion ring has room for all it may leave (`reserve`), as a credit-based
// link bounds a sender, so one started never finds the ring full; what is not
// started waits in order. Overrunning the ring is a sizing fault.

#pragma once

#include <algorithm>
#include <cstdint>
#include <span>
#include <utility>

#include "xpute/kit/status/bug.hpp"
#include "xpute/kit/status/errno.spec.hpp"
#include "xpute/kit/wire/xtp/encode.hpp"
#include "xpute/guest/abi/cmd.spec.hpp"
#include "frame.hpp"
#include "ring.hpp"

namespace xpute {

// Errno::enosys for a command the guest does not number.
struct Applied {
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

class Door {
  public:
    // `most_events` is the most one command raises, or the turn once its drain
    // is done: the program's bound.
    Door(Ring sq, Ring cq, std::uint32_t most_events) noexcept : sq_(sq), cq_(cq), reserve_(most_events + 1) {}

    const Ring &sq() const noexcept { return sq_; }
    const Ring &cq() const noexcept { return cq_; }

    DoorStat take_stat() const noexcept { return std::exchange(stat_, {}); }

    void post(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result) const noexcept {
        posted(cmd, cq_.push(tag, cmd, flags, result));
    }

    template <class Write> void post_in_place(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, Write &&write) const noexcept {
        posted(cmd, cq_.push_in_place(tag, cmd, flags, result, write));
    }

    template <class Fill> void raise(std::uint32_t cmd, std::uint32_t cnt, Fill &&fill) const noexcept {
        post_in_place(0, cmd, 0, 0, [&](std::span<std::uint8_t> out) -> std::optional<std::uint32_t> {
            xtp::PacketWriter w(out);
            w.branch(cnt, fill);
            Result<std::uint32_t> n = w.finish();
            return n ? std::optional(*n) : std::nullopt;
        });
    }

    // Applies waiting commands in order while there is room for what one more may
    // leave; replies to ACKREQ and to failures. Answers how many completions it made.
    // xpute's own frames are applied here: NOP answers its value, and an open the
    // host completed goes to `opened`; the rest go to `apply`. A drain runs before
    // any phase's budget, so the program records what it is handed and returns.
    template <class Opened, class Apply> std::uint32_t drain(Opened &&opened, Apply &&apply) const noexcept {
        std::uint32_t tail = cq_.tail();
        stat_.sq_peak = std::max(stat_.sq_peak, sq_.len());
        while (has_room()) {
            std::optional<Frame> f = sq_.peek();
            if (!f) break;
            Result<std::span<const std::uint8_t>> pkt = sq_.pkt(*f);
            Applied a = pkt ? applied(f->cmd, *pkt, opened, apply) : Applied{.err = pkt.error().errnum};
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

    template <class Opened, class Apply> static Applied applied(std::uint32_t cmd, std::span<const std::uint8_t> pkt, Opened &opened, Apply &apply) noexcept {
        if (cmd == static_cast<std::uint32_t>(abi::Command::NOP)) {
            if (pkt.empty()) return {};
            Result<abi::Nop> nop = abi::Nop::read(pkt);
            if (!nop) return {.err = nop.error().errnum};
            return {.val = static_cast<std::int32_t>(nop->val)};
        }
        if (cmd == static_cast<std::uint32_t>(abi::Command::SYS_OPENED)) {
            Result<abi::SysOpened> o = abi::SysOpened::read(pkt);
            if (!o) return {.err = o.error().errnum};
            return opened(*o);
        }
        return apply(cmd, pkt);
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
};

} // namespace xpute
