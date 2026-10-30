// cpp/xpute/guest/sched/tick.test.cpp
//
// A step is a plain function, so what a step sees goes in `seen`; and the
// clock is the test's, so a step costs what it says.

#include "tick.hpp"

#include <array>
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

// A program's phases, as a world's might number them.
namespace phase {
constexpr Phase interaction = 0;
constexpr Phase visible = 1;
constexpr Phase content = 2;
constexpr Phase cosmetic = 3;
constexpr Phase report = 4;
} // namespace phase

constexpr std::array<std::string_view, 5> PHASES{"interaction", "visible", "content", "cosmetic", "report"};

// What the visible phase holds back for content.
constexpr double CONTENT_SHARE = 0.2;
constexpr std::array<double, 5> HELD_BACK{0, CONTENT_SHARE, 0, 0, 0};

constexpr TickPolicy POLICY{.hard_slack_ms = 4, .held_back = HELD_BACK};
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

XPUTE_TEST(tick_phase_order) {
    Tick<8> t = tick();
    t.on_tick(phase::report, [](TickContext &) { seen.order.push_back("report"); }, 0);
    t.on_tick(phase::cosmetic, [](TickContext &) { seen.order.push_back("cosmetic"); }, 0);
    t.on_tick(phase::content, [](TickContext &) { seen.order.push_back("content"); }, 0);
    t.on_tick(phase::visible, [](TickContext &) { seen.order.push_back("visible"); }, 0);
    t.on_tick(phase::interaction, [](TickContext &) { seen.order.push_back("interaction"); }, 0);
    t.run(0.016, QUOTA_MS, now());
    XPUTE_CHECK((seen.order == std::vector<std::string_view>(PHASES.begin(), PHASES.end())));
}

XPUTE_TEST(tick_step_order) {
    Tick<8> t = tick();
    t.on_tick(phase::visible, [](TickContext &) { seen.order.push_back("1"); }, 0);
    t.on_tick(phase::visible, [](TickContext &) { seen.order.push_back("2"); }, 0);
    t.on_tick(phase::visible, [](TickContext &) { seen.order.push_back("3"); }, 0);
    t.run(0.016, QUOTA_MS, now());
    XPUTE_CHECK((seen.order == std::vector<std::string_view>{"1", "2", "3"}));
}

XPUTE_TEST(tick_shared_budget) {
    Tick<8> t = tick(Seen{.a = -1});
    t.on_tick(phase::visible, [](TickContext &c) { burn(c.pass.total_ms * 0.75); }, 0);
    t.on_tick(phase::content, [](TickContext &c) { seen.a = c.pass.remaining(); }, 0);
    TickReport r = t.run(0.016, QUOTA_MS, now());
    XPUTE_CHECK(seen.a >= 0);
    XPUTE_CHECK(seen.a <= r.budget_ms * 0.3);
}

XPUTE_TEST(tick_held_back_kept_for_later_phases) {
    Tick<8> t = tick();
    t.on_tick(
        phase::visible,
        [](TickContext &c) {
            seen.a = c.pass.remaining();
            // Runs the budget it was shown to the floor.
            c.pass.run(range(1000), [](int) { return burn(0.05), true; });
        },
        0);
    t.on_tick(phase::content, [](TickContext &c) { seen.b = c.pass.remaining(); }, 0);
    TickReport r = t.run(0.016, QUOTA_MS, now());
    XPUTE_CHECK(seen.a <= r.budget_ms * 0.8 + 0.01);
    XPUTE_CHECK(seen.b >= r.budget_ms * 0.1);
}

XPUTE_TEST(tick_self_unregister) {
    Tick<8> t = tick();
    std::uint32_t id = t.on_tick(
        phase::cosmetic,
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

XPUTE_TEST(tick_unregister_earlier_no_skip) {
    Tick<8> t = tick();
    std::uint32_t first = t.on_tick(phase::visible, [](TickContext &) { seen.order.push_back("first"); }, 0);
    t.on_tick(
        phase::visible,
        [](TickContext &c) {
            seen.order.push_back("second");
            c.off_tick(seen.id);
        },
        0);
    t.on_tick(phase::visible, [](TickContext &) { seen.order.push_back("third"); }, 0);
    seen.id = first;
    t.run(0.016, QUOTA_MS, now());
    XPUTE_CHECK((seen.order == std::vector<std::string_view>{"first", "second", "third"}));
}

XPUTE_TEST(tick_report_sees_all) {
    Tick<8> t = tick(Seen{.a = -1});
    t.on_tick(phase::visible, [](TickContext &) { burn(2); }, 0);
    t.on_tick(
        phase::report,
        [](TickContext &c) {
            seen.a = c.phase_ms[static_cast<std::size_t>(phase::visible)];
            seen.b = c.phase_ms[static_cast<std::size_t>(phase::report)];
        },
        0);
    t.run(0.016, QUOTA_MS, now());
    XPUTE_CHECK(seen.a >= 2);
    XPUTE_CHECK(seen.b == 0);
}

XPUTE_TEST(tick_busy_storm_capped) {
    Tick<8> t = tick();
    t.on_tick(phase::visible, [](TickContext &c) { seen.runs = c.pass.run(range(2000), [](int) { return burn(COST), true; }).ran; }, 0);
    TickReport r = t.run(0.016, QUOTA_MS, now());
    XPUTE_CHECK(seen.runs >= 1);
    XPUTE_CHECK(seen.runs < 2000);
    XPUTE_CHECK(r.phase_ms[static_cast<std::size_t>(phase::visible)] <= r.budget_ms + COST);
}

XPUTE_TEST(tick_silent_storm_capped) {
    Tick<8> t = tick();
    t.on_tick(phase::visible, [](TickContext &c) { c.pass.run(range(2000), [](int) { return burn(COST), false; }); }, 0);
    TickReport r = t.run(0.016, QUOTA_MS, now());
    XPUTE_CHECK(r.phase_ms[static_cast<std::size_t>(phase::visible)] <= r.budget_ms + POLICY.hard_slack_ms + COST);
}

XPUTE_TEST(tick_prior_spend_deducted) {
    Tick<8> t = tick();
    t.on_tick(phase::visible, [](TickContext &c) { seen.a = c.pass.remaining(); }, 0);
    double edge = now();
    burn(2);
    t.run(1.0 / 60, QUOTA_MS, edge);
    XPUTE_CHECK(seen.a <= QUOTA_MS * 0.8 - 2 + 0.01);
}

XPUTE_TEST(tick_interaction_spend_deducted) {
    Tick<8> t = tick();
    t.on_tick(phase::interaction, [](TickContext &) { burn(1.5); }, 0);
    t.on_tick(
        phase::visible,
        [](TickContext &c) {
            seen.a = c.pass.remaining();
            seen.b = c.pass.total_ms;
        },
        0);
    t.run(1.0 / 60, QUOTA_MS, now());
    XPUTE_CHECK(seen.a <= seen.b - 1.5);
}
