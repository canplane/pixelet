// cpp/xpute/runtime/sched/frame_budget.hpp

// The pass: the innermost of the scheduler's three levels (quantum.hpp grants
// a guest its slice, tick.hpp spends it in phases, this spends a phase's
// share on one list of steps).
//
// It is deliberately not a registry. Deciding what is eligible and in what
// order is the caller's; what generalizes is the part underneath: run an
// ordered list of steps until a wall-time budget or a step cap says stop,
// without ever starving the queue.
//
// Forward progress is the invariant. Neither the budget nor the cap applies
// until at least one step has done work, so a frame already over budget
// still commits one step, and a single expensive item can never block
// everything behind it.
//
// A step returning false did not complete work: it does not count toward the
// cap. It does not mean the step was free — see `hard_budget_ms`.

#pragma once

#include <cstdint>
#include <limits>
#include <optional>

#include "../clock.hpp"

namespace xpute {

struct BudgetOpts {
    // Wall time this pass may consume, in ms. Waived while the pass has not
    // yet completed a step — that is the forward-progress guarantee.
    double budget_ms = 0;
    // An absolute ceiling on the pass, never waived. The soft budget alone is
    // not enough: a step may cost real time and still report false, and with
    // the soft budget waived until something succeeds, a run of expensive
    // deferrals once made a 4.7-second frame of steps that each stayed under
    // 42 ms. None for no ceiling.
    std::optional<double> hard_budget_ms;
    // A cap on completed steps, independent of time. None for no cap.
    std::optional<std::uint32_t> max_steps;
};

struct PassStats {
    // Steps that reported doing work.
    std::uint32_t ran = 0;
    // Items visited, the no-op steps and the one that tripped the stop among
    // them.
    std::uint32_t visited = 0;
    double elapsed_ms = 0;
    // The loop stopped early: there was more it would have run.
    bool stopped_early = false;

    bool operator==(const PassStats &) const = default;
};

// Runs `step` over `items` in the order given, stopping once the budget or
// the step cap is reached — except that the first step to do real work
// always runs. Answers what it managed.
template <class Items, class Step> PassStats run_under_budget(Items &&items, Step &&step, BudgetOpts opts) {
    double t0 = now();
    double deadline = t0 + opts.budget_ms;
    double hard_deadline = t0 + opts.hard_budget_ms.value_or(std::numeric_limits<double>::infinity());
    std::uint32_t cap = opts.max_steps.value_or(UINT32_MAX);

    PassStats stats;
    for (auto &&item : items) {
        double at = now();
        // The ceiling applies even to a pass that has achieved nothing, and
        // is checked first so the waiver below cannot out-vote it; gated on
        // `visited` so the first item is always attempted.
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

} // namespace xpute
