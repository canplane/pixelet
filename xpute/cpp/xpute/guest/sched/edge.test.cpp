// cpp/xpute/guest/sched/edge.test.cpp
//
// Tasks on an executor of the test's own: a queue resumed in order, which is
// all an executor owes the edge. The clock is the test's.

#include "edge.hpp"

#include <coroutine>
#include <vector>

#include "xpute/kit/test.hpp"

using namespace xpute;

namespace {

double clock_ms = 0;

double test_clock() {
    return clock_ms;
}

class Fifo final : public Executor {
  public:
    bool spawn(std::coroutine_handle<> task, Priority) noexcept override {
        queue_.push_back(task);
        return true;
    }
    void wake(std::coroutine_handle<> task) noexcept override {
        woken++;
        queue_.push_back(task);
    }
    void poll() noexcept override {
        std::vector<std::coroutine_handle<>> now = std::move(queue_);
        queue_.clear();
        for (std::coroutine_handle<> h : now) h.resume();
    }
    bool pending() const noexcept override { return !queue_.empty(); }

    std::uint32_t woken = 0;

  private:
    std::vector<std::coroutine_handle<>> queue_;
};

Fifo fifo;
Edge<2> edge;
Notify notice;
std::vector<int> seen;

void reset() {
    set_clock(test_clock);
    clock_ms = 0;
    set_executor(fifo);
    fifo.poll();
    fifo.woken = 0;
    edge = Edge<2>();
    notice = Notify();
    seen.clear();
}

Task<> spends_then_goes_on() {
    seen.push_back(1);
    clock_ms += 5;
    co_await yield_if_spent(edge);
    seen.push_back(2);
}

Task<int> twice(int n) {
    co_await next_turn(edge);
    co_return 2 * n;
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

XPUTE_TEST(edge_spent_quota_parks) {
    reset();
    edge.rise(4);
    XPUTE_CHECK(spawn(spends_then_goes_on(), 0));
    poll();
    XPUTE_CHECK((seen == std::vector{1}));
    XPUTE_CHECK(fifo.woken == 0);
    XPUTE_CHECK_EQ(edge.fall(), 0.0, "a parked task asks for the next turn");
    XPUTE_CHECK(!pending());

    edge.rise(4);
    XPUTE_CHECK(fifo.woken == 1);
    poll();
    XPUTE_CHECK((seen == std::vector{1, 2}));
}

XPUTE_TEST(edge_falling_edge_earliest_wait) {
    reset();
    edge.rise(4);
    edge.wake_at(test_clock() + 300);
    edge.wake_at(test_clock() + 100);
    XPUTE_CHECK(edge.fall() == 100);
    XPUTE_CHECK(edge.fall() == NO_WAKE);
    edge.ask_next();
    edge.wake_at(test_clock() + 100);
    XPUTE_CHECK(edge.fall() == 0);
}

XPUTE_TEST(edge_notice_wakes_task) {
    reset();
    XPUTE_CHECK(spawn(waits_for_notice(), 0));
    poll();
    XPUTE_CHECK(seen.empty());
    XPUTE_CHECK(edge.fall() == NO_WAKE);
    notice.notify();
    XPUTE_CHECK(fifo.woken == 1);
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
    XPUTE_CHECK(spawn(awaits_a_task(), 0));
    poll();
    XPUTE_CHECK(seen.empty());
    edge.rise(4);
    poll();
    XPUTE_CHECK((seen == std::vector{42}));
}
