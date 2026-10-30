// cpp/xpute/guest/sched/tick.hpp

// A guest's cooperative scheduler: one tick a turn, its phases in the order
// the program numbers them, against one budget counted from the rising edge.
// While a phase runs, the share of the budget its policy holds back for the
// phases after it is not its to take, so they are not starved while it keeps
// busy. What the phases are, and what each holds back, are the program's. A
// step keeps its own state.

#pragma once

#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <span>
#include <string_view>

#include "xpute/kit/status/bug.hpp"
#include "xpute/guest/clock.hpp"
#include "pass.hpp"

namespace xpute {

// A phase, by its place in the program's order.
using Phase = std::uint8_t;

// The most phases a tick runs.
inline constexpr std::size_t PHASES_MAX = 8;

struct TickPolicy {
    // Hard ceiling over what is left, so a step that costs time and reports
    // nothing done cannot walk the whole list.
    double hard_slack_ms;
    // The program's phases, in the order they run: each the share of the
    // budget held back from it for the phases after it, 0 for none.
    std::span<const double> held_back;
};

inline constexpr std::size_t OFFS_MAX = 8;

class TickContext {
  public:
    double delta;
    Pass pass;
    std::array<double, PHASES_MAX> phase_ms{};

    // The guest is strobed only when asked, so a step that animates asks here.
    void invalidate() noexcept { invalidated_ = true; }

    void off_tick(std::uint32_t id) noexcept {
        ensure(off_len_ < OFFS_MAX, Errno::enospc, OFFS_MAX);
        offs_[off_len_++] = id;
    }

  private:
    template <std::size_t> friend class Tick;
    TickContext(double d, Pass p) noexcept : delta(d), pass(p) {}

    bool invalidated_ = false;
    std::array<std::uint32_t, OFFS_MAX> offs_{};
    std::size_t off_len_ = 0;
};

using TickStep = void (*)(TickContext &);

// Two registrations under one name in one phase add into one row.
struct StepStat {
    Phase phase = 0;
    std::uint32_t name = 0;
    double ms = 0;
    std::uint32_t cnt = 0;
    double max_ms = 0;

    bool operator==(const StepStat &) const = default;
};

struct TickReport {
    double budget_ms = 0;
    std::array<double, PHASES_MAX> phase_ms{};
    double total_ms = 0;
    bool invalidated = false;

    bool operator==(const TickReport &) const = default;
};

template <std::size_t N> class Tick {
  public:
    explicit Tick(TickPolicy policy) noexcept : policy_(policy) {
        ensure(policy.held_back.size() <= PHASES_MAX, Errno::einval, policy.held_back.size());
    }

    std::uint32_t on_tick(Phase phase, TickStep step, std::uint32_t name) noexcept {
        ensure(len_ < N, Errno::enospc, N);
        ensure(phase < policy_.held_back.size(), Errno::einval, phase);
        std::uint32_t id = next_id_++;
        registry_[len_++] = Registered{id, phase, step, name};
        return id;
    }

    // Inside a tick, a step asks its context instead.
    void off_tick(std::uint32_t id) noexcept { (void)remove(id); }

    void set_step_timing(bool on) noexcept { step_timing_ = on; }

    std::span<const StepStat> step_stats() const noexcept { return {stats_.data(), stats_len_}; }
    void step_stats_reset() noexcept { stats_len_ = 0; }

    TickReport last() const noexcept { return last_report_; }

    TickReport run(double delta, double quota_ms, double t0) noexcept {
        double start = now();
        TickContext ctx(delta, Pass(quota_ms, t0, policy_.hard_slack_ms));
        for (std::size_t phase = 0; phase < policy_.held_back.size(); phase++) {
            ctx.pass.reserve_ms = ctx.pass.total_ms * policy_.held_back[phase];
            run_phase(static_cast<Phase>(phase), ctx);
        }
        ctx.pass.reserve_ms = 0;
        last_report_ = TickReport{ctx.pass.total_ms, ctx.phase_ms, now() - start, ctx.invalidated_};
        return last_report_;
    }

  private:
    struct Registered {
        std::uint32_t id;
        Phase phase;
        TickStep step;
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

    // A table with no row left drops the sample.
    void record(Phase phase, std::uint32_t name, double ms) noexcept {
        std::size_t at = 0;
        while (at < stats_len_ && !(stats_[at].phase == phase && stats_[at].name == name)) at++;
        if (at == stats_len_) {
            if (stats_len_ == N) return;
            stats_[stats_len_++] = StepStat{.phase = phase, .name = name};
        }
        StepStat &s = stats_[at];
        s.ms += ms;
        s.cnt++;
        s.max_ms = std::max(s.max_ms, ms);
    }

    void run_phase(Phase phase, TickContext &ctx) noexcept {
        double start = now();
        // An index, not a snapshot: a step may unregister itself or a sibling.
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
        ctx.phase_ms[static_cast<std::size_t>(phase)] = now() - start;
    }

    TickPolicy policy_;
    std::array<Registered, N> registry_{};
    std::size_t len_ = 0;
    std::uint32_t next_id_ = 1;
    std::array<StepStat, N> stats_{};
    std::size_t stats_len_ = 0;
    bool step_timing_ = false;
    TickReport last_report_{};
};

} // namespace xpute
