// cpp/xpute/runtime/sched/frame_budget.test.cpp
//
// A clock the test moves, so a step costs exactly what it says and a pass is
// not a race with the machine.

#include "frame_budget.hpp"

#include <array>
#include <vector>

#include "../../core/test.hpp"

using namespace xpute;

namespace {

double clock_ms = 0;

double test_clock() {
    return clock_ms;
}

void burn(double ms) {
    clock_ms += ms;
}

BudgetOpts opts(double budget_ms) {
    set_clock(test_clock);
    clock_ms = 0;
    return BudgetOpts{.budget_ms = budget_ms};
}

std::vector<int> range(int n) {
    std::vector<int> v;
    for (int i = 0; i < n; i++) v.push_back(i);
    return v;
}

} // namespace

XPUTE_TEST(run_under_budget_runs_everything_when_the_budget_is_ample) {
    std::vector<int> seen;
    PassStats s = run_under_budget(std::array{1, 2, 3}, [&](int n) { return seen.push_back(n), true; }, opts(1000));
    XPUTE_CHECK((seen == std::vector{1, 2, 3}));
    XPUTE_CHECK(s.ran == 3 && !s.stopped_early);
}

XPUTE_TEST(run_under_budget_stops_once_the_time_budget_is_spent) {
    std::vector<int> seen;
    PassStats s = run_under_budget(std::array{1, 2, 3, 4, 5}, [&](int n) { return seen.push_back(n), burn(6), true; }, opts(10));
    // The third is refused at 12 ms.
    XPUTE_CHECK((seen == std::vector{1, 2}));
    XPUTE_CHECK(s.stopped_early && s.ran == seen.size());
}

XPUTE_TEST(run_under_budget_the_first_working_step_always_runs_even_past_budget) {
    std::vector<int> seen;
    PassStats s = run_under_budget(std::array{1, 2, 3}, [&](int n) { return seen.push_back(n), burn(5), true; }, opts(0));
    XPUTE_CHECK((seen == std::vector{1}));
    XPUTE_CHECK(s.ran == 1 && s.stopped_early);
}

XPUTE_TEST(run_under_budget_max_steps_caps_completed_work) {
    std::vector<int> seen;
    BudgetOpts o = opts(1000);
    o.max_steps = 2;
    PassStats s = run_under_budget(std::array{1, 2, 3, 4, 5}, [&](int n) { return seen.push_back(n), true; }, o);
    XPUTE_CHECK((seen == std::vector{1, 2}));
    XPUTE_CHECK(s.ran == 2 && s.stopped_early);
}

XPUTE_TEST(run_under_budget_a_no_op_step_neither_counts_nor_arms_the_guard) {
    std::vector<int> seen;
    BudgetOpts o = opts(0);
    o.max_steps = 1;
    PassStats s = run_under_budget(std::array{1, 2, 3, 4}, [&](int n) { return seen.push_back(n), n == 4; }, o);
    XPUTE_CHECK((seen == std::vector{1, 2, 3, 4}));
    XPUTE_CHECK(s.ran == 1 && s.visited == 4 && !s.stopped_early);
}

XPUTE_TEST(run_under_budget_hard_budget_ms_bounds_a_pass_of_expensive_no_ops) {
    std::vector<int> seen;
    BudgetOpts o = opts(4);
    o.hard_budget_ms = 12;
    PassStats s = run_under_budget(range(40), [&](int n) { return seen.push_back(n), burn(3), false; }, o);
    XPUTE_CHECK(s.ran == 0 && s.stopped_early);
    // Nothing reports work, so only the ceiling stops it, at the fifth.
    XPUTE_CHECK(seen.size() == 4);
}

XPUTE_TEST(run_under_budget_without_a_ceiling_expensive_no_ops_still_walk_the_whole_list) {
    std::vector<int> seen;
    PassStats s = run_under_budget(range(6), [&](int n) { return seen.push_back(n), burn(3), false; }, opts(1));
    XPUTE_CHECK(seen.size() == 6 && !s.stopped_early);
}

XPUTE_TEST(run_under_budget_the_first_item_is_attempted_even_under_a_zero_ceiling) {
    std::vector<int> seen;
    BudgetOpts o = opts(0);
    o.hard_budget_ms = 0;
    PassStats s = run_under_budget(std::array{1, 2, 3}, [&](int n) { return seen.push_back(n), burn(2), true; }, o);
    XPUTE_CHECK((seen == std::vector{1}));
    XPUTE_CHECK(s.ran == 1);
}

XPUTE_TEST(run_under_budget_an_empty_list_is_a_clean_no_op) {
    PassStats s = run_under_budget(std::array<int, 0>{}, [](int) { return true; }, opts(10));
    XPUTE_CHECK(s.ran == 0 && s.visited == 0 && !s.stopped_early);
}
