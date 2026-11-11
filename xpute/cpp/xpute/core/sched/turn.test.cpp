// cpp/xpute/core/sched/turn.test.cpp
//
// The tasks run on the test's own scheduler, a queue resumed in order, which
// is all a turn needs of one. The test owns the clock.

#include "turn.hpp"

#include <coroutine>
#include <vector>

#include "xpute/kit/test.hpp"

using namespace xpute;

namespace {

double clock_ms = 0;

double test_clock() {
    return clock_ms;
}

// The test's scheduler: the tasks to resume, in order, and how many times a
// task was woken.
std::vector<std::coroutine_handle<>> queued;
std::uint32_t wakes = 0;

void queue(std::coroutine_handle<> task) noexcept {
    wakes++;
    queued.push_back(task);
}

bool spawn(Task<> task) noexcept {
    if (!task) return false;
    Task<>::Handle h = task.release();
    h.promise().queue = queue;
    queued.push_back(h);
    return true;
}

void poll() noexcept {
    std::vector<std::coroutine_handle<>> now = std::move(queued);
    queued.clear();
    for (std::coroutine_handle<> h : now) h.resume();
}

bool pending() noexcept { return !queued.empty(); }

Turn turn;
Notify notice;
std::vector<int> seen;

void reset() {
    set_clock(test_clock);
    clock_ms = 0;
    poll();
    wakes = 0;
    turn = Turn();
    notice = Notify();
    seen.clear();
}

Task<> spends_then_goes_on() {
    seen.push_back(1);
    clock_ms += 5;
    co_await yield_if_spent(turn);
    seen.push_back(2);
}

Task<int> twice(int n) {
    co_await next_turn(turn);
    co_return 2 * n;
}

Task<> waits_on_the_runtime() {
    co_await runtime_turn(turn);
    seen.push_back(3);
}

Task<> awaits_a_task() {
    seen.push_back(co_await twice(21));
}

Task<> waits_for_notice() {
    for (;;) {
        co_await notified(notice);
        seen.push_back(7);
    }
}

} // namespace

XPUTE_TEST(turn_past_deadline_parks) {
    reset();
    turn.start(4);
    XPUTE_CHECK(spawn(spends_then_goes_on()));
    poll();
    XPUTE_CHECK((seen == std::vector{1}));
    XPUTE_CHECK(wakes == 0);
    XPUTE_CHECK_EQ(turn.end(), 0.0, "a parked task asks for the next turn");
    XPUTE_CHECK(!pending());

    turn.start(4);
    XPUTE_CHECK(wakes == 1);
    poll();
    XPUTE_CHECK((seen == std::vector{1, 2}));
}

XPUTE_TEST(turn_end_earliest_wait) {
    reset();
    turn.start(4);
    turn.wake_at(test_clock() + 300);
    turn.wake_at(test_clock() + 100);
    XPUTE_CHECK(turn.end() == 100);
    XPUTE_CHECK(turn.end() == NO_WAKE);
    turn.ask_next();
    turn.wake_at(test_clock() + 100);
    XPUTE_CHECK(turn.end() == 0);
}

XPUTE_TEST(turn_runtime_turn_asks_none) {
    reset();
    turn.start(4);
    XPUTE_CHECK(spawn(waits_on_the_runtime()));
    poll();
    XPUTE_CHECK(seen.empty());
    XPUTE_CHECK_EQ(turn.end(), NO_WAKE, "a task waiting on its runtime asks for no turn");

    turn.start(4);
    XPUTE_CHECK(wakes == 1);
    poll();
    XPUTE_CHECK((seen == std::vector{3}));
}

XPUTE_TEST(turn_notice_wakes_task) {
    reset();
    XPUTE_CHECK(spawn(waits_for_notice()));
    poll();
    XPUTE_CHECK(seen.empty());
    XPUTE_CHECK(turn.end() == NO_WAKE);
    notice.notify();
    XPUTE_CHECK(wakes == 1);
    poll();
    XPUTE_CHECK((seen == std::vector{7}));

    // A notice before the wait is kept for it.
    notice.notify();
    notice.notify();
    poll();
    poll();
    XPUTE_CHECK((seen == std::vector{7, 7}));
}

XPUTE_TEST(task_awaited_runs_in_place) {
    reset();
    XPUTE_CHECK(spawn(awaits_a_task()));
    poll();
    XPUTE_CHECK(seen.empty());
    turn.start(4);
    poll();
    XPUTE_CHECK((seen == std::vector{42}));
}
