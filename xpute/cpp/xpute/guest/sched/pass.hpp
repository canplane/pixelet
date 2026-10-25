// cpp/xpute/guest/sched/pass.hpp

// What is left of a turn's budget, spent on ordered lists of steps (`run`).
// Neither the budget nor a step cap applies until one step has done work, so
// one expensive item can never block everything behind it.

#pragma once

#include <algorithm>
#include <cstdint>
#include <optional>

#include "xpute/guest/clock.hpp"

namespace xpute {

struct PassStat {
    std::uint32_t ran = 0;
    // With the steps that did no work, and the one that tripped the stop.
    std::uint32_t visited = 0;
    double elapsed_ms = 0;
    bool stopped_early = false;

    bool operator==(const PassStat &) const = default;
};

struct Pass {
    // The rising edge, so what the turn spent before comes off.
    double t0 = 0;
    double total_ms = 0;
    // Held back for the phases after the current one.
    double reserve_ms = 0;
    // Past what is left, never waived: a step can cost time and still report
    // no work.
    double hard_slack_ms = 0;

    Pass(double total, double rise, double slack) noexcept : t0(rise), total_ms(total), hard_slack_ms(slack) {}

    double remaining() const noexcept { return std::max(t0 + total_ms - reserve_ms - now(), 0.0); }

    // `step` answers whether it did work; only work counts to `max_steps`.
    template <class Items, class Step> PassStat run(Items &&items, Step &&step, std::optional<std::uint32_t> max_steps = std::nullopt) const {
        double start = now();
        double deadline = start + remaining();
        double hard_deadline = deadline + hard_slack_ms;
        std::uint32_t cap = max_steps.value_or(UINT32_MAX);
        PassStat stat;
        for (auto &&item : items) {
            double at = now();
            // Before the waiver below, but the first item is always tried.
            if ((stat.visited > 0 && at >= hard_deadline) || (stat.ran > 0 && (stat.ran >= cap || at >= deadline))) {
                stat.stopped_early = true;
                break;
            }
            stat.visited++;
            if (step(item)) stat.ran++;
        }
        stat.elapsed_ms = now() - start;
        return stat;
    }
};

} // namespace xpute
