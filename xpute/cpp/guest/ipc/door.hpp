// cpp/guest/ipc/door.hpp

// The guest's end of its two rings. A command is started only while the
// completion ring has room for all it may leave (`reserve`), as a credit-based
// link bounds a sender, so one started never finds the ring full; what is not
// started waits in order. Overrunning the ring is a sizing fault.

#pragma once

#include <cstdint>
#include <span>

#include "../../kit/status/bug.hpp"
#include "../../kit/status/errno.spec.hpp"
#include "../../kit/wire/xtp/encode.hpp"
#include "frame.hpp"
#include "ring.hpp"

namespace xpute {

// Errno::enosys for a command the guest does not number.
struct Applied {
    Errno error = Errno::ok;
    std::int32_t value = 0;
};

// For whoever measures how full the rings run.
using DoorNote = void (*)(std::uint32_t completions, std::uint32_t left);

class Door {
  public:
    // `most_signals` is what one command may raise, the program's bound.
    Door(Ring submission, Ring completion, std::uint32_t most_signals, DoorNote note = nullptr) noexcept
        : sq_(submission), cq_(completion), reserve_(most_signals + 1), note_(note) {}

    const Ring &submission() const noexcept { return sq_; }
    const Ring &completion() const noexcept { return cq_; }

    void post(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result) const noexcept {
        posted(cmd, cq_.push(tag, cmd, flags, result));
    }

    template <class Write> void post_in_place(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, Write &&write) const noexcept {
        posted(cmd, cq_.push_in_place(tag, cmd, flags, result, write));
    }

    template <class Fill> void raise(std::uint32_t cmd, std::uint32_t count, Fill &&fill) const noexcept {
        post_in_place(0, cmd, 0, 0, [&](std::span<std::uint8_t> out) -> std::optional<std::uint32_t> {
            xtp::PacketWriter w(out);
            w.branch(count, fill);
            Result<std::uint32_t> n = w.finish();
            return n ? std::optional(*n) : std::nullopt;
        });
    }

    // Applies waiting commands in order while there is room for what one more may
    // leave; replies to ACKREQ and to failures. Answers how many completions it made.
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

    bool pending() const noexcept { return !sq_.empty(); }

  private:
    bool has_room() const noexcept { return cq_.len() + reserve_ <= cq_.capacity(); }

    void posted(std::uint32_t cmd, Errno e) const noexcept {
        if (note_ != nullptr) note_(cq_.len(), 0);
        ensure(e == Errno::ok, Errno::enospc, cmd, static_cast<std::uint64_t>(static_cast<std::int64_t>(e)));
    }

    Ring sq_;
    Ring cq_;
    // What one command may leave: its signals and its reply.
    std::uint32_t reserve_;
    DoorNote note_;
};

} // namespace xpute
