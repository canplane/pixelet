// cpp/xpute/runtime/sched/edge.hpp

// A guest's side of the turn: the quota as a deadline, the tasks parked until
// the next rising edge, and what the falling edge asks for.

#pragma once

#include <algorithm>
#include <array>
#include <coroutine>
#include <cstddef>
#include <limits>
#include <optional>
#include <utility>

#include "../../core/status/bug.hpp"
#include "../clock.hpp"
#include "quantum.hpp"
#include "task.hpp"

namespace xpute {

template <std::size_t N> class Edge {
  public:
    void rise(double quota_ms) noexcept {
        edge_at_ = now();
        quota_ms_ = quota_ms;
        std::size_t n = std::exchange(parked_len_, 0);
        for (std::size_t i = 0; i < n; i++) parked_[i].wake();
    }

    // Reading clears it.
    double fall() noexcept {
        bool asked = std::exchange(asked_, false) || parked_len_ > 0;
        double at = std::exchange(wake_at_, std::numeric_limits<double>::infinity());
        if (asked) return 0.0;
        if (at != std::numeric_limits<double>::infinity()) return std::max(at - now(), 0.0);
        return NO_WAKE;
    }

    double edge_at() const noexcept { return edge_at_; }
    double quota_ms() const noexcept { return quota_ms_; }

    double remaining() const noexcept { return std::max(edge_at_ + quota_ms_ - now(), 0.0); }
    bool spent() const noexcept { return now() >= edge_at_ + quota_ms_; }

    void ask_next() noexcept { asked_ = true; }

    // The earliest asked wins.
    void wake_at(double at) noexcept { wake_at_ = std::min(wake_at_, at); }

    void park(Waker waker) noexcept {
        ensure(parked_len_ < N, Errno::enospc, N);
        parked_[parked_len_++] = waker;
    }

  private:
    double edge_at_ = 0;
    double quota_ms_ = 0;
    std::array<Waker, N> parked_{};
    std::size_t parked_len_ = 0;
    bool asked_ = false;
    double wake_at_ = std::numeric_limits<double>::infinity();
};

template <std::size_t N> struct ParkAwaiter {
    Edge<N> &edge;
    bool ready;
    bool await_ready() const noexcept { return ready; }
    template <class P> void await_suspend(std::coroutine_handle<P> h) const noexcept { edge.park(waker_of(h)); }
    void await_resume() const noexcept {}
};

template <std::size_t N> ParkAwaiter<N> yield_if_spent(Edge<N> &edge) noexcept {
    return ParkAwaiter<N>{edge, !edge.spent()};
}

template <std::size_t N> ParkAwaiter<N> next_turn(Edge<N> &edge) noexcept {
    return ParkAwaiter<N>{edge, false};
}

// A wait for something to arrive that asks for no turn meanwhile, so an idle
// guest is left alone. A notice that comes before the wait is kept for it.
class Notify {
  public:
    void notify() noexcept {
        notified_ = true;
        if (std::optional<Waker> w = std::exchange(waker_, std::nullopt)) w->wake();
    }

    struct Awaiter {
        Notify &n;
        bool await_ready() const noexcept { return std::exchange(n.notified_, false); }
        template <class P> void await_suspend(std::coroutine_handle<P> h) const noexcept { n.waker_ = waker_of(h); }
        void await_resume() const noexcept { n.notified_ = false; }
    };

  private:
    std::optional<Waker> waker_;
    bool notified_ = false;
};

inline Notify::Awaiter notified(Notify &notify) noexcept {
    return Notify::Awaiter{notify};
}

} // namespace xpute
