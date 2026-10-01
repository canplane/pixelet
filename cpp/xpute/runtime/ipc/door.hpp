// cpp/xpute/runtime/ipc/door.hpp

// The guest's end of its two rings: what the host sends applied in the order
// sent, each once, and what the guest raises and answers posted into the
// completion ring, which the host reads dry before it rings again
// (ARCHITECTURE.xpute.md, the contract's rules 3 to 5).
//
// A command is started only while the completion ring has room for all it
// may leave — `reserve` messages, its signals and its reply — so one started
// never finds the ring full. What is not started waits in the submission
// ring, in order, and the guest asks for the next turn while any does.

#pragma once

#include <cstdint>
#include <span>

#include "../../core/status/errno.spec.hpp"
#include "frame.hpp"
#include "ring.hpp"

namespace xpute {

// What applying a command came to: its reply's value, or the errno it failed
// with — Errno::enosys for one the guest does not number.
struct Applied {
    Errno error = Errno::ok;
    std::int32_t value = 0;
};

class Door {
  public:
    Door(Ring submission, Ring completion, std::uint32_t reserve) noexcept : sq_(submission), cq_(completion), reserve_(reserve) {}

    // Posts a message into the completion ring. One that finds it full is a
    // guest that left more than it sized the ring for, and stops.
    void post(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result) const noexcept;

    // Applies what waits, each by `apply(cmd, packet)`, while there is room
    // for what one more command may leave.
    template <class Apply> void drain(Apply &&apply) const noexcept {
        while (has_room()) {
            auto m = sq_.peek();
            if (!m) return;
            Applied a = apply(m->cmd, m->packet);
            sq_.advance();
            if (a.error != Errno::ok) {
                post(m->tag, m->cmd, frame::RES, static_cast<std::int32_t>(a.error));
            } else if (m->flags & frame::ACKREQ) {
                post(m->tag, m->cmd, frame::RES, a.value);
            }
        }
    }

    // Whether a submission waits that a drain left for a later turn.
    bool pending() const noexcept { return sq_.len() > 0; }

  private:
    bool has_room() const noexcept { return cq_.len() + reserve_ <= cq_.capacity(); }

    Ring sq_;
    Ring cq_;
    std::uint32_t reserve_;
};

} // namespace xpute
