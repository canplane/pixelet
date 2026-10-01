// cpp/xpute/runtime/ipc/door.hpp

// The guest's end of its two rings: what the host sends applied in the order
// sent, each once, and what the guest raises and answers posted into the
// completion ring, which the host reads dry before it rings again
// (ARCHITECTURE.xpute.md, the contract's rules 3 to 5). Each message is
// written once, in place, and nothing is held on a heap.
//
// A command is started only while the completion ring has room for all it
// may leave — `reserve` messages, its signals and its reply — so one started
// never finds the ring full, as a credit-based link bounds what a sender may
// start by the receiver's room. What is not started waits in the submission
// ring, in order, and the guest asks for the next turn while any does. One
// that would overrun the ring has left more than it was sized for — a sizing
// fault, as a full pool is, and not a state to wait out.

#pragma once

#include <cstdint>
#include <span>

#include "../../core/status/bug.hpp"
#include "../../core/status/errno.spec.hpp"
#include "../../core/wire/xtp/encode.hpp"
#include "frame.hpp"
#include "ring.hpp"

namespace xpute {

// What applying a command came to: its reply's value, or the errno it failed
// with — Errno::enosys for one the guest does not number, and a packet's own
// for one that does not read.
struct Applied {
    Errno error = Errno::ok;
    std::int32_t value = 0;
};

// Told the completion ring's length at every post, and how many submissions
// a drain left unapplied, for whoever measures how full the rings run.
using DoorNote = void (*)(std::uint32_t completions, std::uint32_t left);

class Door {
  public:
    Door(Ring submission, Ring completion, std::uint32_t reserve, DoorNote note = nullptr) noexcept
        : sq_(submission), cq_(completion), reserve_(reserve), note_(note) {}

    // The submission ring, which the host pushes into.
    const Ring &submission() const noexcept { return sq_; }
    // The completion ring, which the host reads dry.
    const Ring &completion() const noexcept { return cq_; }

    // Posts a message with no packet into the completion ring.
    void post(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result) const noexcept {
        posted(cmd, cq_.push(tag, cmd, flags, result));
    }

    // Posts a message whose packet `write` writes in place into its slot.
    template <class Write> void post_in_place(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, Write &&write) const noexcept {
        posted(cmd, cq_.push_in_place(tag, cmd, flags, result, write));
    }

    // A signal for the host with a packet whose root is a branch of `count`
    // children, written by `fill`.
    template <class Fill> void raise(std::uint32_t cmd, std::uint32_t count, Fill &&fill) const noexcept {
        post_in_place(0, cmd, 0, 0, [&](std::span<std::uint8_t> out) -> std::optional<std::uint32_t> {
            xtp::PacketWriter w(out);
            w.branch(count, fill);
            Result<std::uint32_t> n = w.finish();
            return n ? std::optional(*n) : std::nullopt;
        });
    }

    // Applies what waits, each by `apply(cmd, packet)` — an empty packet for
    // a message with none — in order, while there is room for what one more
    // command may leave; the rest waits for the next turn. Replies to what
    // asks (ACKREQ), and to what failed. How many completions the call made.
    template <class Apply> std::uint32_t drain(Apply &&apply) const noexcept {
        std::uint32_t tail = cq_.tail();
        while (has_room()) {
            std::optional<Ring::Message> m = sq_.peek();
            if (!m) break;
            Result<std::span<const std::uint8_t>> pkt = sq_.packet(*m);
            Applied a = pkt ? apply(m->cmd, *pkt) : Applied{.error = pkt.error().code};
            sq_.advance();
            if (a.error != Errno::ok) {
                post(m->tag, m->cmd, frame::RES, static_cast<std::int32_t>(a.error));
            } else if (m->flags & frame::ACKREQ) {
                post(m->tag, m->cmd, frame::RES, a.value);
            }
        }
        if (note_ != nullptr) note_(cq_.len(), sq_.len());
        return cq_.tail() - tail;
    }

    // Whether a submission waits that a drain left for a later turn.
    bool pending() const noexcept { return !sq_.empty(); }

  private:
    bool has_room() const noexcept { return cq_.len() + reserve_ <= cq_.capacity(); }

    void posted(std::uint32_t cmd, Errno e) const noexcept {
        if (note_ != nullptr) note_(cq_.len(), 0);
        ensure(e == Errno::ok, Errno::enospc, cmd, static_cast<std::uint64_t>(static_cast<std::int64_t>(e)));
    }

    Ring sq_;
    Ring cq_;
    std::uint32_t reserve_;
    DoorNote note_;
};

} // namespace xpute
