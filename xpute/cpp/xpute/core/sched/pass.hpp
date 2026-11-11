// cpp/xpute/core/sched/pass.hpp

// A pass is a turn's remaining budget, spent on ordered lists of steps (`run`).
// The invariant is forward progress: neither the budget nor a step cap
// applies until one step has done work. An item that costs more than the
// whole budget therefore still runs, and cannot stay at the head of its list
// forever, blocking the items behind it.

#pragma once

#include <algorithm>
#include <cstdint>
#include <optional>

#include "xpute/core/clock.hpp"

namespace xpute {

struct PassStat {
    std::uint32_t ran = 0;
    // Every step called, counting those that reported no work and the one
    // that used up the budget.
    std::uint32_t step_cnt = 0;
    double elapsed_ms = 0;
    bool stopped_early = false;

    bool operator==(const PassStat &) const = default;
};

struct Pass {
    // The time the turn started. The budget counts from it, so the
    // time the turn spent before this pass comes off what is left.
    double t0 = 0;
    double total_ms = 0;
    // The time held back for the phases after the current one.
    double reserve_ms = 0;
    // A hard limit this far past the remaining budget. Unlike the budget, it
    // applies before any step has done work, since a step can cost time and
    // still report none.
    double hard_slack_ms = 0;

    Pass(double total, double start, double slack) noexcept : t0(start), total_ms(total), hard_slack_ms(slack) {}

    double remaining() const noexcept { return std::max(t0 + total_ms - reserve_ms - now(), 0.0); }

    // `step` answers whether it did work. Only steps that did work count
    // toward `max_steps`.
    template <class Range, class Step> PassStat run(Range &&items, Step &&step, std::optional<std::uint32_t> max_steps = std::nullopt) const {
        double start = now();
        double deadline = start + remaining();
        double hard_deadline = deadline + hard_slack_ms;
        std::uint32_t cap = max_steps.value_or(UINT32_MAX);
        PassStat stat;
        for (auto &&item : items) {
            double at = now();
            // The hard limit applies even before any step has done work,
            // though never to the first item.
            if ((stat.step_cnt > 0 && at >= hard_deadline) || (stat.ran > 0 && (stat.ran >= cap || at >= deadline))) {
                stat.stopped_early = true;
                break;
            }
            stat.step_cnt++;
            if (step(item)) stat.ran++;
        }
        stat.elapsed_ms = now() - start;
        return stat;
    }
};

} // namespace xpute
