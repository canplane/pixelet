// cpp/xpute/core/sched/edge.hpp

// A core's side of the turn: the deadline it keeps, the tasks parked until
// the next rising edge, and what the falling edge asks for.

#pragma once

#include <algorithm>
#include <array>
#include <cmath>
#include <coroutine>
#include <cstddef>
#include <limits>
#include <optional>
#include <utility>

#include "xpute/kit/status/bug.hpp"
#include "xpute/core/clock.hpp"
#include "wake.hpp"
#include "task.hpp"

namespace xpute {

template <std::size_t N> class Edge {
  public:
    void rise(double deadline_ms) noexcept {
        edge_at_ = now();
        deadline_ms_ = deadline_ms;
        until_ = edge_at_ + deadline_ms;
        std::size_t n = std::exchange(parked_len_, 0);
        for (std::size_t i = 0; i < n; i++) parked_[i].wake();
        n = std::exchange(runtime_parked_len_, 0);
        for (std::size_t i = 0; i < n; i++) runtime_parked_[i].wake();
    }

    // Reading clears it.
    double fall() noexcept {
        bool resched = std::exchange(resched_, false) || parked_len_ > 0;
        double at = std::exchange(wake_at_, std::numeric_limits<double>::infinity());
        if (resched) return 0.0;
        if (std::isfinite(at)) return std::max(at - now(), 0.0);
        return NO_WAKE;
    }

    double edge_at() const noexcept { return edge_at_; }
    double deadline_ms() const noexcept { return deadline_ms_; }

    bool spent() const noexcept { return now() >= until_; }

    // The tasks yield no sooner than `at`, whatever the deadline, so what overran
    // it does not take the tasks' turn too.
    void extend_to(double at) noexcept { until_ = std::max(until_, at); }

    void ask_next() noexcept { resched_ = true; }

    // The earliest asked wins.
    void wake_at(double at) noexcept { wake_at_ = std::min(wake_at_, at); }

    void park(Waker waker) noexcept {
        ensure(parked_len_ < N, Errno::enospc, N);
        parked_[parked_len_++] = waker;
    }

    // Parked until a turn the runtime begins: `fall` asks for none for it.
    void park_runtime(Waker waker) noexcept {
        ensure(runtime_parked_len_ < N, Errno::enospc, N);
        runtime_parked_[runtime_parked_len_++] = waker;
    }

  private:
    double edge_at_ = 0;
    double deadline_ms_ = 0;
    // When the tasks yield: the deadline's end, or later (`extend_to`).
    double until_ = 0;
    std::array<Waker, N> parked_{};
    std::size_t parked_len_ = 0;
    std::array<Waker, N> runtime_parked_{};
    std::size_t runtime_parked_len_ = 0;
    bool resched_ = false;
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

template <std::size_t N> struct RuntimeParkAwaiter {
    Edge<N> &edge;
    bool await_ready() const noexcept { return false; }
    template <class P> void await_suspend(std::coroutine_handle<P> h) const noexcept { edge.park_runtime(waker_of(h)); }
    void await_resume() const noexcept {}
};

// Parked until the next turn the runtime begins, asking for none: the runtime begins
// it once it has done what the task waits for, as its read of the completion
// ring kicks one.
template <std::size_t N> RuntimeParkAwaiter<N> runtime_turn(Edge<N> &edge) noexcept {
    return RuntimeParkAwaiter<N>{edge};
}

// A wait for something to arrive that asks for no turn meanwhile, so an idle
// core is left alone. A notice that comes before the wait is kept for it.
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
