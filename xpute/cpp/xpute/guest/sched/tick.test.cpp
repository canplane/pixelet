// cpp/xpute/guest/sched/tick.test.cpp
//
// A step is a plain function, so what a step sees goes in `seen`; and the
// clock is the test's, so a step costs what it says.

#include "tick.hpp"

#include <string_view>
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

struct Seen {
    std::vector<std::string_view> order;
    double a = 0;
    double b = 0;
    std::uint32_t runs = 0;
    std::uint32_t id = 0;
};

Seen seen;

constexpr TickPolicy POLICY{.hard_slack_ms = 4, .content_share = 0.2};
constexpr double QUOTA_MS = 6;
constexpr double COST = 0.3;

Tick<8> tick(Seen start = {}) {
    set_clock(test_clock);
    clock_ms = 0;
    seen = start;
    return Tick<8>(POLICY);
}

std::vector<int> range(int n) {
    std::vector<int> v;
    for (int i = 0; i < n; i++) v.push_back(i);
    return v;
}

} // namespace

XPUTE_TEST(tick_phases_run_in_their_fixed_order_whatever_order_they_registered_in) {
    Tick<8> t = tick();
    t.on_tick(Phase::report, [](TickContext &) { seen.order.push_back("report"); }, 0);
    t.on_tick(Phase::cosmetic, [](TickContext &) { seen.order.push_back("cosmetic"); }, 0);
    t.on_tick(Phase::content, [](TickContext &) { seen.order.push_back("content"); }, 0);
    t.on_tick(Phase::visible, [](TickContext &) { seen.order.push_back("visible"); }, 0);
    t.on_tick(Phase::interaction, [](TickContext &) { seen.order.push_back("interaction"); }, 0);
    t.run(0.016, QUOTA_MS, now());
    XPUTE_CHECK((seen.order == std::vector<std::string_view>(PHASES.begin(), PHASES.end())));
}

XPUTE_TEST(tick_within_a_phase_steps_run_in_registration_order) {
    Tick<8> t = tick();
    t.on_tick(Phase::visible, [](TickContext &) { seen.order.push_back("1"); }, 0);
    t.on_tick(Phase::visible, [](TickContext &) { seen.order.push_back("2"); }, 0);
    t.on_tick(Phase::visible, [](TickContext &) { seen.order.push_back("3"); }, 0);
    t.run(0.016, QUOTA_MS, now());
    XPUTE_CHECK((seen.order == std::vector<std::string_view>{"1", "2", "3"}));
}

XPUTE_TEST(tick_one_budget_is_shared_what_an_earlier_phase_spends_a_later_one_does_not_get) {
    Tick<8> t = tick(Seen{.a = -1});
    t.on_tick(Phase::visible, [](TickContext &c) { burn(c.pass.total_ms * 0.75); }, 0);
    t.on_tick(Phase::content, [](TickContext &c) { seen.a = c.pass.remaining(); }, 0);
    TickReport r = t.run(0.016, QUOTA_MS, now());
    XPUTE_CHECK(seen.a >= 0);
    XPUTE_CHECK(seen.a <= r.budget_ms * 0.3);
}

XPUTE_TEST(tick_the_visible_phase_cannot_take_the_content_share_and_content_sees_it) {
    Tick<8> t = tick();
    t.on_tick(
        Phase::visible,
        [](TickContext &c) {
            seen.a = c.pass.remaining();
            // Runs the budget it was shown to the floor.
            c.pass.run(range(1000), [](int) { return burn(0.05), true; });
        },
        0);
    t.on_tick(Phase::content, [](TickContext &c) { seen.b = c.pass.remaining(); }, 0);
    TickReport r = t.run(0.016, QUOTA_MS, now());
    XPUTE_CHECK(seen.a <= r.budget_ms * 0.8 + 0.01);
    XPUTE_CHECK(seen.b >= r.budget_ms * 0.1);
}

XPUTE_TEST(tick_a_step_that_unregisters_itself_mid_tick_is_not_run_again) {
    Tick<8> t = tick();
    std::uint32_t id = t.on_tick(
        Phase::cosmetic,
        [](TickContext &c) {
            seen.runs++;
            c.off_tick(seen.id);
        },
        0);
    seen.id = id;
    t.run(0.016, QUOTA_MS, now());
    t.run(0.016, QUOTA_MS, now());
    XPUTE_CHECK(seen.runs == 1);
}

XPUTE_TEST(tick_a_step_that_unregisters_an_earlier_one_does_not_make_the_tick_skip_the_step_after_it) {
    Tick<8> t = tick();
    std::uint32_t first = t.on_tick(Phase::visible, [](TickContext &) { seen.order.push_back("first"); }, 0);
    t.on_tick(
        Phase::visible,
        [](TickContext &c) {
            seen.order.push_back("second");
            c.off_tick(seen.id);
        },
        0);
    t.on_tick(Phase::visible, [](TickContext &) { seen.order.push_back("third"); }, 0);
    seen.id = first;
    t.run(0.016, QUOTA_MS, now());
    XPUTE_CHECK((seen.order == std::vector<std::string_view>{"first", "second", "third"}));
}

XPUTE_TEST(tick_the_report_phase_sees_every_phase_before_it) {
    Tick<8> t = tick(Seen{.a = -1});
    t.on_tick(Phase::visible, [](TickContext &) { burn(2); }, 0);
    t.on_tick(
        Phase::report,
        [](TickContext &c) {
            seen.a = c.phase_ms[static_cast<std::size_t>(Phase::visible)];
            seen.b = c.phase_ms[static_cast<std::size_t>(Phase::report)];
        },
        0);
    t.run(0.016, QUOTA_MS, now());
    XPUTE_CHECK(seen.a >= 2);
    XPUTE_CHECK(seen.b == 0);
}

XPUTE_TEST(tick_a_storm_of_steps_that_report_work_stays_within_the_budget_plus_one_step) {
    Tick<8> t = tick();
    t.on_tick(Phase::visible, [](TickContext &c) { seen.runs = c.pass.run(range(2000), [](int) { return burn(COST), true; }).ran; }, 0);
    TickReport r = t.run(0.016, QUOTA_MS, now());
    XPUTE_CHECK(seen.runs >= 1);
    XPUTE_CHECK(seen.runs < 2000);
    XPUTE_CHECK(r.phase_ms[static_cast<std::size_t>(Phase::visible)] <= r.budget_ms + COST);
}

XPUTE_TEST(tick_a_storm_of_steps_that_cost_time_and_report_none_stays_within_the_hard_ceiling) {
    Tick<8> t = tick();
    t.on_tick(Phase::visible, [](TickContext &c) { c.pass.run(range(2000), [](int) { return burn(COST), false; }); }, 0);
    TickReport r = t.run(0.016, QUOTA_MS, now());
    XPUTE_CHECK(r.phase_ms[static_cast<std::size_t>(Phase::visible)] <= r.budget_ms + POLICY.hard_slack_ms + COST);
}

XPUTE_TEST(tick_what_the_turn_spent_before_the_tick_comes_off_the_budget) {
    Tick<8> t = tick();
    t.on_tick(Phase::visible, [](TickContext &c) { seen.a = c.pass.remaining(); }, 0);
    double edge = now();
    burn(2);
    t.run(1.0 / 60, QUOTA_MS, edge);
    XPUTE_CHECK(seen.a <= QUOTA_MS * 0.8 - 2 + 0.01);
}

XPUTE_TEST(tick_what_the_interaction_phase_spends_comes_off_the_budget) {
    Tick<8> t = tick();
    t.on_tick(Phase::interaction, [](TickContext &) { burn(1.5); }, 0);
    t.on_tick(
        Phase::visible,
        [](TickContext &c) {
            seen.a = c.pass.remaining();
            seen.b = c.pass.total_ms;
        },
        0);
    t.run(1.0 / 60, QUOTA_MS, now());
    XPUTE_CHECK(seen.a <= seen.b - 1.5);
}
