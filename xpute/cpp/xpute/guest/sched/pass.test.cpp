// cpp/xpute/guest/sched/pass.test.cpp
//
// A clock the test moves, so a step costs exactly what it says.

#include "pass.hpp"

#include <array>
#include <limits>
#include <vector>

#include "xpute/kit/test.hpp"

using namespace xpute;

namespace {

double clock_ms = 0;

double test_clock() {
    return clock_ms;
}

void burn(double ms) {
    clock_ms += ms;
}

// `budget_ms` left from now, and `hard_budget_ms` never waived.
Pass pass(double budget_ms, std::optional<double> hard_budget_ms = std::nullopt) {
    set_clock(test_clock);
    clock_ms = 0;
    return Pass(budget_ms, 0, hard_budget_ms ? *hard_budget_ms - budget_ms : std::numeric_limits<double>::infinity());
}

std::vector<int> range(int n) {
    std::vector<int> v;
    for (int i = 0; i < n; i++) v.push_back(i);
    return v;
}

} // namespace

XPUTE_TEST(pass_stops_when_spent) {
    std::vector<int> seen;
    PassStat s = pass(10).run(std::array{1, 2, 3, 4, 5}, [&](int n) { return seen.push_back(n), burn(6), true; });
    // The third is refused at 12 ms.
    XPUTE_CHECK((seen == std::vector{1, 2}));
    XPUTE_CHECK(s.stopped_early && s.ran == seen.size());
}

XPUTE_TEST(pass_first_step_always_runs) {
    std::vector<int> seen;
    PassStat s = pass(0).run(std::array{1, 2, 3}, [&](int n) { return seen.push_back(n), burn(5), true; });
    XPUTE_CHECK((seen == std::vector{1}));
    XPUTE_CHECK(s.ran == 1 && s.stopped_early);
}

XPUTE_TEST(pass_max_steps_cap) {
    std::vector<int> seen;
    PassStat s = pass(1000).run(std::array{1, 2, 3, 4, 5}, [&](int n) { return seen.push_back(n), true; }, 2);
    XPUTE_CHECK((seen == std::vector{1, 2}));
    XPUTE_CHECK(s.ran == 2 && s.stopped_early);
}

XPUTE_TEST(pass_no_op_uncounted) {
    std::vector<int> seen;
    PassStat s = pass(0).run(std::array{1, 2, 3, 4}, [&](int n) { return seen.push_back(n), n == 4; }, 1);
    XPUTE_CHECK((seen == std::vector{1, 2, 3, 4}));
    XPUTE_CHECK(s.ran == 1 && s.visited == 4 && !s.stopped_early);
}

XPUTE_TEST(pass_hard_budget_bounds_no_ops) {
    std::vector<int> seen;
    PassStat s = pass(4, 12).run(range(40), [&](int n) { return seen.push_back(n), burn(3), false; });
    XPUTE_CHECK(s.ran == 0 && s.stopped_early);
    // Nothing reports work, so only the ceiling stops it, at the fifth.
    XPUTE_CHECK(seen.size() == 4);
}

XPUTE_TEST(pass_no_ceiling_walks_all) {
    std::vector<int> seen;
    PassStat s = pass(1).run(range(6), [&](int n) { return seen.push_back(n), burn(3), false; });
    XPUTE_CHECK(seen.size() == 6 && !s.stopped_early);
}

XPUTE_TEST(pass_zero_ceiling_first_item) {
    std::vector<int> seen;
    PassStat s = pass(0, 0).run(std::array{1, 2, 3}, [&](int n) { return seen.push_back(n), burn(2), true; });
    XPUTE_CHECK((seen == std::vector{1}));
    XPUTE_CHECK(s.ran == 1);
}

XPUTE_TEST(pass_empty_list) {
    PassStat s = pass(10).run(std::array<int, 0>{}, [](int) { return true; });
    XPUTE_CHECK(s.ran == 0 && s.visited == 0 && !s.stopped_early);
}
