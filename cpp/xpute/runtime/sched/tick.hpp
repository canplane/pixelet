// cpp/xpute/runtime/sched/tick.hpp

// A guest's scheduler: the middle of the scheduler's three levels.
//
// The host strobes the guest with a quota (quantum.hpp); in that turn the
// guest takes what waits on its submission ring, runs its own steps here,
// cooperatively, and leaves what goes back on its completion ring. The quota
// says about how long the turn should take, not a deadline.
//
// One tick a turn, its phases in a fixed order against one budget. The order
// is the priority:
//
//   interaction   what the hand is doing. Not budgeted, and must stay O(1):
//                 the budget clock is already running, so anything spent
//                 here comes off what the phases below receive.
//   visible       what is on screen now.
//   content       what is about to be on screen.
//   cosmetic      what only looks better for running.
//   report        the guest's own record of the turn, after everything
//                 else, so it sees everything.
//
// Within a phase, steps run in the order they registered. The budget is the
// quota counted from the rising edge, not from the tick's own start: what
// the guest did before the tick in the same turn has been spent. The content
// phase is guaranteed a share: the visible phase may not take
// `content_share` of the budget, or a load that kept it busy would leave
// content only the forward-progress floor.
//
// Nothing preempts: a step runs to completion, and steps are small by
// construction instead. The step table is fixed at N: a guest registers its
// steps when it starts, and a step past the table is refused there.

#pragma once

#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <span>
#include <string_view>

#include "../../core/status/bug.hpp"
#include "../clock.hpp"
#include "frame_budget.hpp"

namespace xpute {

using Phase = std::size_t;
inline constexpr Phase INTERACTION = 0;
inline constexpr Phase VISIBLE = 1;
inline constexpr Phase CONTENT = 2;
inline constexpr Phase COSMETIC = 3;
inline constexpr Phase REPORT = 4;
inline constexpr std::array<std::string_view, 5> PHASES{"interaction", "visible", "content", "cosmetic", "report"};

// The guest's numbers.
struct TickPolicy {
    // How far a pass may run past what is left, so a step that costs real
    // time and reports nothing done cannot walk the whole list: the pass's
    // hard ceiling, as slack over the pool.
    double hard_slack_ms = 0;
    // The share of the budget held back from the visible phase for content.
    double content_share = 0;
};

// One turn's allowance, shared by every phase. `run` is `run_under_budget`
// with the numbers filled in from what is left, so a caller states what it
// wants to do and never what it may spend.
struct FrameBudget {
    double t0 = 0;
    double total_ms = 0;
    // Held back from whoever asks now, for the phases after it. The tick sets
    // it before the visible phase and clears it before content.
    double reserve_ms = 0;
    double hard_slack_ms = 0;

    // `t0` is the rising edge, not this call's: what the turn spent before
    // comes off the budget.
    FrameBudget(double total, double rise, double slack) noexcept : t0(rise), total_ms(total), hard_slack_ms(slack) {}

    double remaining() const noexcept { return std::max(t0 + total_ms - reserve_ms - now(), 0.0); }

    template <class Items, class Step> PassStats run(Items &&items, Step &&step, std::optional<std::uint32_t> max_steps = std::nullopt) const {
        double left = remaining();
        return run_under_budget(items, step, BudgetOpts{.budget_ms = left, .hard_budget_ms = left + hard_slack_ms, .max_steps = max_steps});
    }
};

// Steps a step may unregister in one call.
inline constexpr std::size_t OFFS_MAX = 8;

// What a step may ask of the turn, and the guest's own state it runs over.
template <class S> class TickContext {
  public:
    S &state;
    double delta;
    FrameBudget budget;
    // Wall time each phase has taken so far this tick, filled in as phases
    // complete — so the report phase can read everything before it.
    std::array<double, 5> phase_ms{};

    // Asks for a turn after this one: the guest is strobed only when
    // something asked, so a step that changes something over time asks for
    // the next.
    void invalidate() noexcept { invalidated_ = true; }

    // Unregisters a step from inside the tick, before the next step runs.
    void off_tick(std::uint32_t id) noexcept {
        ensure(off_len_ < OFFS_MAX, Errno::enospc, OFFS_MAX);
        offs_[off_len_++] = id;
    }

  private:
    template <class, std::size_t> friend class Tick;
    TickContext(S &s, double d, FrameBudget b) noexcept : state(s), delta(d), budget(b) {}

    bool invalidated_ = false;
    std::array<std::uint32_t, OFFS_MAX> offs_{};
    std::size_t off_len_ = 0;
};

template <class S> using TickStep = void (*)(TickContext<S> &);

// Wall time per step, cumulative until reset. The phase totals can say a
// phase is heavy and not which of its steps is; this can. Two registrations
// under one name in one phase add into one row.
struct StepStats {
    Phase phase = 0;
    std::uint32_t name = 0;
    double ms = 0;
    std::uint32_t count = 0;
    double max_ms = 0;

    bool operator==(const StepStats &) const = default;
};

// What a turn spent: what the guest reports back to the host.
struct TickReport {
    double budget_ms = 0;
    std::array<double, 5> phase_ms{};
    // The whole tick, interaction through report.
    double total_ms = 0;
    // A turn after this one was asked for.
    bool invalidated = false;

    bool operator==(const TickReport &) const = default;
};

template <class S, std::size_t N> class Tick {
  public:
    explicit Tick(TickPolicy policy) noexcept : policy_(policy) {}

    // Registers a step into a phase, and answers the id `off_tick` takes.
    // `name` is what the step's time is reported under: the caller's own
    // number for it, which the words for it are looked up by where they are
    // printed.
    std::uint32_t on_tick(Phase phase, TickStep<S> step, std::uint32_t name) noexcept {
        ensure(len_ < N, Errno::enospc, N);
        std::uint32_t id = next_id_++;
        registry_[len_++] = Registered{id, phase, step, name};
        return id;
    }

    // Unregisters a step outside a tick; inside one, a step asks its context.
    void off_tick(std::uint32_t id) noexcept { (void)remove(id); }

    // Whether steps are timed one by one.
    void set_step_timing(bool on) noexcept { step_timing_ = on; }

    std::span<const StepStats> step_stats() const noexcept { return {stats_.data(), stats_len_}; }
    void step_stats_reset() noexcept { stats_len_ = 0; }

    // What the previous turn spent.
    TickReport last() const noexcept { return last_report_; }

    // The guest's turn, on the quota it was strobed with at `t0`.
    TickReport run(S &state, double delta, double quota_ms, double t0) noexcept {
        double start = now();
        TickContext<S> ctx(state, delta, FrameBudget(quota_ms, t0, policy_.hard_slack_ms));
        run_phase(INTERACTION, ctx);
        ctx.budget.reserve_ms = ctx.budget.total_ms * policy_.content_share;
        run_phase(VISIBLE, ctx);
        ctx.budget.reserve_ms = 0;
        for (Phase phase : {CONTENT, COSMETIC, REPORT}) run_phase(phase, ctx);
        last_report_ = TickReport{ctx.budget.total_ms, ctx.phase_ms, now() - start, ctx.invalidated_};
        return last_report_;
    }

  private:
    struct Registered {
        std::uint32_t id;
        Phase phase;
        TickStep<S> step;
        std::uint32_t name;
    };

    std::optional<std::size_t> remove(std::uint32_t id) noexcept {
        for (std::size_t at = 0; at < len_; at++) {
            if (registry_[at].id != id) continue;
            std::copy(registry_.begin() + at + 1, registry_.begin() + len_, registry_.begin() + at);
            len_--;
            return at;
        }
        return std::nullopt;
    }

    // A step's time into its row; a table with no row left drops it.
    void record(Phase phase, std::uint32_t name, double ms) noexcept {
        std::size_t at = 0;
        while (at < stats_len_ && !(stats_[at].phase == phase && stats_[at].name == name)) at++;
        if (at == stats_len_) {
            if (stats_len_ == N) return;
            stats_[stats_len_++] = StepStats{.phase = phase, .name = name};
        }
        StepStats &s = stats_[at];
        s.ms += ms;
        s.count++;
        s.max_ms = std::max(s.max_ms, ms);
    }

    void run_phase(Phase phase, TickContext<S> &ctx) noexcept {
        double start = now();
        // An index rather than an iterator: a step may unregister itself or a
        // sibling, and a snapshot would run something that just asked not to
        // be.
        std::size_t i = 0;
        while (i < len_) {
            Registered r = registry_[i++];
            if (r.phase != phase) continue;
            if (step_timing_) {
                double t = now();
                r.step(ctx);
                record(r.phase, r.name, now() - t);
            } else {
                r.step(ctx);
            }
            for (std::size_t k = 0; k < ctx.off_len_; k++) {
                std::optional<std::size_t> at = remove(ctx.offs_[k]);
                if (at && *at < i) i--;
            }
            ctx.off_len_ = 0;
        }
        ctx.phase_ms[phase] = now() - start;
    }

    TickPolicy policy_;
    std::array<Registered, N> registry_{};
    std::size_t len_ = 0;
    std::uint32_t next_id_ = 1;
    std::array<StepStats, N> stats_{};
    std::size_t stats_len_ = 0;
    bool step_timing_ = false;
    TickReport last_report_{};
};

} // namespace xpute
