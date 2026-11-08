// cpp/xpute/core/sched/edge.hpp

// The core's side of a turn. A turn begins at its rising edge (`rise`),
// which sets its deadline, and ends at its falling edge (`fall`), which
// answers when the core wants the next turn. A task that must wait for a
// later turn parks here in between.
//
// Only `rise` wakes a parked task; a task never wakes itself. An executor
// that re-reads its queue within one poll would otherwise run a self-woken
// task again, past the deadline. A task parks at most once a turn, so the
// edge never holds more wakers than the core has tasks; its lists grow to
// the most ever parked at once and keep that room.
//
// A task parked by `next_turn` asks for the next turn at once. A task parked
// by `runtime_turn` waits on the runtime and asks for no turn: the runtime
// starts the next turn itself once it has done what the task waits for,
// because its read of the completion ring kicks the core. A core on its own
// thread therefore does not spin while the runtime has not yet read.

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

    // Answers in how many ms the core wants the next turn: 0 when a task
    // asked or parked for it, else the time until the earliest `wake_at`,
    // else NO_WAKE. It clears what it answers, so every turn asks afresh.
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

    // Keeps the tasks running until at least `at`, past the deadline if need
    // be, so that work which overran the deadline does not also take the
    // tasks' share of the turn.
    void extend_to(double at) noexcept { until_ = std::max(until_, at); }

    void ask_next() noexcept { resched_ = true; }

    void wake_at(double at) noexcept { wake_at_ = std::min(wake_at_, at); }

    void park(Waker waker) noexcept { parked_.push_back(waker); }

    // Parks a task until the runtime begins a turn. `fall` asks for no turn
    // on its behalf.
    void park_runtime(Waker waker) noexcept { runtime_parked_.push_back(waker); }

  private:
    double edge_at_ = 0;
    double deadline_ms_ = 0;
    // The time at which the turn's tasks yield: the deadline's end, or later
    // where `extend_to` kept the tasks their share after earlier work overran.
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

// Waits for the next turn the runtime begins, without asking for one.
inline RuntimeParkAwaiter runtime_turn(Edge &edge) noexcept {
    return RuntimeParkAwaiter{edge};
}

// A wait for a notice that asks for no turn meanwhile, so an idle core is
// left alone; parking instead would ask for every turn. One task at a time
// waits on a `Notify`, and a notice that comes before the wait is kept for it.
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
