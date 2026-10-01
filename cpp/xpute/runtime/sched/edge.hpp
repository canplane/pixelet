// cpp/xpute/runtime/sched/edge.hpp

// A guest's side of the turn the host grants it (quantum.hpp): the rising
// edge's quota as a deadline on the host clock, the tasks parked until the
// next rising edge, and what the falling edge asks for.
//
// This is the interface a guest's work is written against, whichever
// executor resumes it (task.hpp). A task yields with `yield_if_spent` or
// `next_turn`, and nothing here names an executor; the program that
// assembles the guest polls one between `rise` and `fall`.
//
// A parked task is woken on the next rising edge, not by itself. A task that
// woke itself would be resumed again by whichever executor re-reads its
// queue within one poll, past the quota; one woken at `rise` runs only in the
// turn after. The table is fixed at N, the guest's number of tasks: a task
// awaits one thing at a time and so is parked at most once, and a table that
// fills is a guest that spawned more than it declared.
//
// What the falling edge carries: 0 when a turn asked for the next frame, the
// delay to the earliest `wake_at` otherwise, NO_WAKE for neither. Parked
// tasks ask for the next frame by being parked.

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
    // The rising edge: the turn's quota from now, and every task parked for
    // it woken.
    void rise(double quota_ms) noexcept {
        edge_at_ = now();
        quota_ms_ = quota_ms;
        std::size_t n = std::exchange(parked_len_, 0);
        for (std::size_t i = 0; i < n; i++) parked_[i].wake();
    }

    // The falling edge: when the next turn is wanted, in ms from now. Reading
    // clears it.
    double fall() noexcept {
        bool asked = std::exchange(asked_, false) || parked_len_ > 0;
        double at = std::exchange(wake_at_, std::numeric_limits<double>::infinity());
        if (asked) return 0.0;
        if (at != std::numeric_limits<double>::infinity()) return std::max(at - now(), 0.0);
        return NO_WAKE;
    }

    // When the turn rose.
    double edge_at() const noexcept { return edge_at_; }
    double quota_ms() const noexcept { return quota_ms_; }

    // What is left of the quota, never below zero.
    double remaining() const noexcept { return std::max(edge_at_ + quota_ms_ - now(), 0.0); }
    bool spent() const noexcept { return now() >= edge_at_ + quota_ms_; }

    // Asks for the next frame's turn.
    void ask_next() noexcept { asked_ = true; }

    // Asks for a turn once the host clock reaches `at`: a wait only time
    // ends. The earliest asked wins.
    void wake_at(double at) noexcept { wake_at_ = std::min(wake_at_, at); }

    // Wakes `waker` on the next rising edge.
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
    // Whether the task goes on without waiting.
    bool ready;
    bool await_ready() const noexcept { return ready; }
    template <class P> void await_suspend(std::coroutine_handle<P> h) const noexcept { edge.park(waker_of(h)); }
    void await_resume() const noexcept {}
};

// Ready at once while the quota lasts; once it is spent, the task waits
// until the next turn.
template <std::size_t N> ParkAwaiter<N> yield_if_spent(Edge<N> &edge) noexcept {
    return ParkAwaiter<N>{edge, !edge.spent()};
}

// The task waits until the next turn, whatever is left of this one.
template <std::size_t N> ParkAwaiter<N> next_turn(Edge<N> &edge) noexcept {
    return ParkAwaiter<N>{edge, false};
}

// Where a task with nothing to do waits for something to arrive, one task a
// notify. It asks for no turn while it waits: a task parked for the next
// turn instead would ask for every frame, and an idle guest would never be
// left alone. A notice that comes before the task waits is kept for it.
class Notify {
  public:
    // Something arrived: the waiting task is woken, or the next to wait goes
    // on at once.
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

// The task waits until `notify` is notified.
inline Notify::Awaiter notified(Notify &notify) noexcept {
    return Notify::Awaiter{notify};
}

} // namespace xpute
