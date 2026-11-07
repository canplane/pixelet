// cpp/xpute/core/sched/edge.hpp

// A core's side of the turn: the deadline it keeps, the tasks parked until
// the next rising edge, and what the falling edge asks for. A task parks at
// most once a turn, so the edge holds no more wakers than the core has tasks:
// its lists grow to the most parked at once and keep that room.

#pragma once

#include <algorithm>
#include <cmath>
#include <coroutine>
#include <limits>
#include <optional>
#include <utility>
#include <vector>

#include "xpute/core/clock.hpp"
#include "wake.hpp"
#include "task.hpp"

namespace xpute {

class Edge {
  public:
    void rise(double deadline_ms) noexcept {
        edge_at_ = now();
        deadline_ms_ = deadline_ms;
        until_ = edge_at_ + deadline_ms;
        for (Waker w : parked_) w.wake();
        parked_.clear();
        for (Waker w : runtime_parked_) w.wake();
        runtime_parked_.clear();
    }

    // Reading clears it.
    double fall() noexcept {
        bool resched = std::exchange(resched_, false) || !parked_.empty();
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

    void park(Waker waker) noexcept { parked_.push_back(waker); }

    // Parked until a turn the runtime begins: `fall` asks for none for it.
    void park_runtime(Waker waker) noexcept { runtime_parked_.push_back(waker); }

  private:
    double edge_at_ = 0;
    double deadline_ms_ = 0;
    // When the tasks yield: the deadline's end, or later (`extend_to`).
    double until_ = 0;
    std::vector<Waker> parked_;
    std::vector<Waker> runtime_parked_;
    bool resched_ = false;
    double wake_at_ = std::numeric_limits<double>::infinity();
};

struct ParkAwaiter {
    Edge &edge;
    bool ready;
    bool await_ready() const noexcept { return ready; }
    template <class P> void await_suspend(std::coroutine_handle<P> h) const noexcept { edge.park(waker_of(h)); }
    void await_resume() const noexcept {}
};

inline ParkAwaiter yield_if_spent(Edge &edge) noexcept {
    return ParkAwaiter{edge, !edge.spent()};
}

inline ParkAwaiter next_turn(Edge &edge) noexcept {
    return ParkAwaiter{edge, false};
}

struct RuntimeParkAwaiter {
    Edge &edge;
    bool await_ready() const noexcept { return false; }
    template <class P> void await_suspend(std::coroutine_handle<P> h) const noexcept { edge.park_runtime(waker_of(h)); }
    void await_resume() const noexcept {}
};

// Parked until the next turn the runtime begins, asking for none: the runtime begins
// it once it has done what the task waits for, as its read of the completion
// ring kicks one.
inline RuntimeParkAwaiter runtime_turn(Edge &edge) noexcept {
    return RuntimeParkAwaiter{edge};
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
