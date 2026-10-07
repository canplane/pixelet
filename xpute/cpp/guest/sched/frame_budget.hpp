// cpp/guest/sched/frame_budget.hpp

// Runs an ordered list of steps until a wall-time budget or a step cap says
// stop. Neither applies until one step has done work, so a single expensive
// item can never block everything behind it. A step returning false did no
// work and does not count toward the cap.

#pragma once

#include <algorithm>
#include <cstdint>
#include <limits>
#include <optional>

#include "../clock.hpp"

namespace xpute {

struct BudgetOpts {
    // Waived until a step completes.
    double budget_ms = 0;
    // Never waived: a step may cost real time and still report false, and a run
    // of such deferrals once made a 4.7-second frame of steps each under 42 ms.
    std::optional<double> hard_budget_ms;
    std::optional<std::uint32_t> max_steps;
};

struct PassStats {
    std::uint32_t ran = 0;
    // Includes no-op steps and the one that tripped the stop.
    std::uint32_t visited = 0;
    double elapsed_ms = 0;
    bool stopped_early = false;

    bool operator==(const PassStats &) const = default;
};

template <class Items, class Step> PassStats run_under_budget(Items &&items, Step &&step, BudgetOpts opts) {
    double t0 = now();
    double deadline = t0 + opts.budget_ms;
    double hard_deadline = t0 + opts.hard_budget_ms.value_or(std::numeric_limits<double>::infinity());
    std::uint32_t cap = opts.max_steps.value_or(UINT32_MAX);

    PassStats stats;
    for (auto &&item : items) {
        double at = now();
        // Checked first so the waiver below cannot out-vote it; gated on `visited`
        // so the first item is always attempted.
        if (stats.visited > 0 && at >= hard_deadline) {
            stats.stopped_early = true;
            break;
        }
        if (stats.ran > 0 && (stats.ran >= cap || at >= deadline)) {
            stats.stopped_early = true;
            break;
        }
        stats.visited++;
        if (step(item)) stats.ran++;
    }
    stats.elapsed_ms = now() - t0;
    return stats;
}

struct FrameBudget {
    double t0 = 0;
    double total_ms = 0;
    // Held back from the visible phase for content.
    double reserve_ms = 0;
    double hard_slack_ms = 0;

    // `t0` is the rising edge: what the turn spent before the tick is spent.
    FrameBudget(double total, double rise, double slack) noexcept : t0(rise), total_ms(total), hard_slack_ms(slack) {}

    double remaining() const noexcept { return std::max(t0 + total_ms - reserve_ms - now(), 0.0); }

    template <class Items, class Step> PassStats run(Items &&items, Step &&step, std::optional<std::uint32_t> max_steps = std::nullopt) const {
        double left = remaining();
        return run_under_budget(items, step, BudgetOpts{.budget_ms = left, .hard_budget_ms = left + hard_slack_ms, .max_steps = max_steps});
    }
};

} // namespace xpute
