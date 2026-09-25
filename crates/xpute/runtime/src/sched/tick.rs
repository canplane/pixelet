// xpute-runtime/sched/tick.rs

//! A guest's scheduler: the middle of the scheduler's three levels.
//!
//! The guest knows little of what is outside it. The central scheduler
//! strobes it with a quota (sched/quantum.rs); in that turn the guest takes
//! what waits on its submission ring, runs its own tasks here, cooperatively,
//! and leaves what goes back on its completion ring. The quota says about how
//! long the turn should take, not a deadline: nothing here is real time. The
//! tick sees the quota and the clock, and nothing of the host.
//!
//! One tick a turn, its phases in a fixed order against one budget. The order
//! *is* the priority:
//!
//!   interaction   what the hand is doing. Not budgeted, and must stay O(1):
//!                 the budget clock is already running, so anything spent
//!                 here comes off what the phases below receive.
//!   visible       what is on screen now.
//!   content       what is about to be on screen.
//!   cosmetic      what only looks better for running.
//!   report        the guest's own record of the turn, after everything
//!                 else, so it sees everything.
//!
//! Within a phase, steps run in the order they registered.
//!
//! **The budget is the quota, counted from the rising edge**, not from the
//! tick's own start: what the guest did before the tick in the same turn has
//! already been spent.
//!
//! **The content phase is guaranteed a share.** The visible phase may not take
//! `content_share` of the budget: phases run in priority order on one budget,
//! and without the reserve content got only the forward-progress floor while
//! a load kept the visible phase busy.
//!
//! What this does not do: preempt. A step still runs to completion. Steps are
//! small by construction instead.
//!
//! The step table is the tick's own and fixed at N: a guest registers its
//! steps when it starts, and a step past the table is refused there rather
//! than grown into.

use super::frame_budget::{run_under_budget, BudgetOpts, PassStats};
use crate::clock::now;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

pub type Phase = usize;
pub const INTERACTION: Phase = 0;
pub const VISIBLE: Phase = 1;
pub const CONTENT: Phase = 2;
pub const COSMETIC: Phase = 3;
pub const REPORT: Phase = 4;
pub const PHASES: [&str; 5] = ["interaction", "visible", "content", "cosmetic", "report"];

/// Steps a step may unregister in one call.
const OFFS_MAX: usize = 8;

/// The guest's numbers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TickPolicy {
    /// How far a pass may run past what is left, so a step that costs real
    /// time and reports nothing done cannot walk the whole list —
    /// frame_budget.rs's hard ceiling, as slack over the pool.
    pub hard_slack_ms: f64,
    /// The share of the budget held back from the visible phase for content.
    pub content_share: f64,
}

/// One turn's allowance, shared by every phase.
///
/// `run` is `run_under_budget` with the numbers filled in from what is left,
/// so a caller states what it wants to do and never what it may spend.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameBudget {
    pub t0: f64,
    pub total_ms: f64,
    /// Held back from whoever asks now, for the phases after it. The tick sets
    /// it before the visible phase and clears it before content.
    pub reserve_ms: f64,
    pub hard_slack_ms: f64,
}

impl FrameBudget {
    /// `t0` is the rising edge, not this call's: what the turn spent before
    /// comes off the budget.
    pub fn new(total_ms: f64, t0: f64, hard_slack_ms: f64) -> FrameBudget {
        FrameBudget {
            t0,
            total_ms,
            reserve_ms: 0.0,
            hard_slack_ms,
        }
    }

    pub fn remaining(&self) -> f64 {
        (self.t0 + self.total_ms - self.reserve_ms - now()).max(0.0)
    }

    pub fn run<T>(&self, items: impl IntoIterator<Item = T>, step: impl FnMut(T) -> bool, max_steps: Option<u32>) -> PassStats {
        let remaining = self.remaining();
        run_under_budget(
            items,
            step,
            BudgetOpts {
                budget_ms: remaining,
                hard_budget_ms: Some(remaining + self.hard_slack_ms),
                max_steps,
            },
        )
    }
}

/// What a step may ask of the turn, and the guest's own state it runs over.
pub struct TickContext<'a, S> {
    pub state: &'a mut S,
    pub delta: f64,
    pub budget: FrameBudget,
    /// Wall time each phase has taken so far this tick, filled in as phases
    /// complete — so the report phase can read everything before it.
    pub phase_ms: [f64; 5],
    invalidated: bool,
    offs: [u32; OFFS_MAX],
    off_len: usize,
}

impl<S> TickContext<'_, S> {
    /// Asks for a turn after this one: the guest is strobed only when
    /// something asked, so a step that changes something over time asks for
    /// the next.
    pub fn invalidate(&mut self) {
        self.invalidated = true;
    }

    /// Unregisters a step from inside the tick, before the next step runs.
    pub fn off_tick(&mut self, id: u32) {
        xpute_core::ensure!(self.off_len < OFFS_MAX, ENOSPC, OFFS_MAX);
        self.offs[self.off_len] = id;
        self.off_len += 1;
    }
}

pub type TickStep<S> = fn(&mut TickContext<S>);

/// Wall time per step, cumulative until reset. The phase totals can say a
/// phase is heavy and not which of its steps is; this can. Two registrations
/// under one name in one phase add into one row.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepStats {
    pub phase: Phase,
    pub name: u32,
    pub ms: f64,
    pub count: u32,
    pub max_ms: f64,
}

/// What a turn spent: what the guest reports back to the central scheduler.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TickReport {
    pub budget_ms: f64,
    pub phase_ms: [f64; 5],
    /// The whole tick, interaction through report.
    pub total_ms: f64,
    /// A turn after this one was asked for.
    pub invalidated: bool,
}

struct Registered<S> {
    id: u32,
    phase: Phase,
    step: TickStep<S>,
    name: u32,
}

impl<S> Clone for Registered<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S> Copy for Registered<S> {}

pub struct Tick<S, const N: usize> {
    policy: TickPolicy,
    registry: [Option<Registered<S>>; N],
    len: usize,
    next_id: u32,
    stats: [StepStats; N],
    stats_len: usize,
    step_timing: bool,
    last_report: TickReport,
}

impl<S, const N: usize> Tick<S, N> {
    pub fn new(policy: TickPolicy) -> Self {
        Tick {
            policy,
            registry: [None; N],
            len: 0,
            next_id: 1,
            stats: [StepStats::default(); N],
            stats_len: 0,
            step_timing: false,
            last_report: TickReport::default(),
        }
    }

    /// Registers a step into a phase. Returns the id `off_tick` takes. `name`
    /// is what the step's time is reported under: the caller's own number for
    /// it, which the words for it are looked up by where they are printed.
    pub fn on_tick(&mut self, phase: Phase, step: TickStep<S>, name: u32) -> u32 {
        xpute_core::ensure!(self.len < N, ENOSPC, N);
        let id = self.next_id;
        self.next_id += 1;
        self.registry[self.len] = Some(Registered { id, phase, step, name });
        self.len += 1;
        id
    }

    /// Unregisters a step outside a tick; inside one, a step asks its context.
    pub fn off_tick(&mut self, id: u32) {
        self.remove(id);
    }

    /// Whether steps are timed one by one.
    pub fn set_step_timing(&mut self, on: bool) {
        self.step_timing = on;
    }

    pub fn step_stats(&self) -> &[StepStats] {
        &self.stats[..self.stats_len]
    }

    pub fn step_stats_reset(&mut self) {
        self.stats_len = 0;
    }

    /// What the previous turn spent.
    pub fn last(&self) -> TickReport {
        self.last_report
    }

    fn remove(&mut self, id: u32) -> Option<usize> {
        let at = (0..self.len).find(|&k| self.registry[k].is_some_and(|r| r.id == id))?;
        self.registry.copy_within(at + 1..self.len, at);
        self.len -= 1;
        self.registry[self.len] = None;
        Some(at)
    }

    /// A step's time into its row; a table with no row left drops it.
    fn record(&mut self, phase: Phase, name: u32, ms: f64) {
        let at = match (0..self.stats_len).find(|&k| self.stats[k].phase == phase && self.stats[k].name == name) {
            Some(k) => k,
            None if self.stats_len < N => {
                self.stats[self.stats_len] = StepStats { phase, name, ..Default::default() };
                self.stats_len += 1;
                self.stats_len - 1
            }
            None => return,
        };
        let stat = &mut self.stats[at];
        stat.ms += ms;
        stat.count += 1;
        stat.max_ms = stat.max_ms.max(ms);
    }

    fn run_phase(&mut self, phase: Phase, ctx: &mut TickContext<S>) {
        let start = now();
        // Index loop rather than an iterator: a step may unregister itself or a
        // sibling, and a snapshot would run something that just asked not to be.
        let mut i = 0;
        while i < self.len {
            let r = self.registry[i].or_bug(Errno::ENOTRECOVERABLE);
            i += 1;
            if r.phase != phase {
                continue;
            }
            if self.step_timing {
                let t = now();
                (r.step)(ctx);
                self.record(r.phase, r.name, now() - t);
            } else {
                (r.step)(ctx);
            }
            for k in 0..ctx.off_len {
                if self.remove(ctx.offs[k]).is_some_and(|at| at < i) {
                    i -= 1;
                }
            }
            ctx.off_len = 0;
        }
        ctx.phase_ms[phase] = now() - start;
    }

    /// The guest's turn, on the quota it was strobed with at `t0`.
    pub fn run(&mut self, state: &mut S, delta: f64, quota_ms: f64, t0: f64) -> TickReport {
        let start = now();
        let mut ctx = TickContext {
            state,
            delta,
            budget: FrameBudget::new(quota_ms, t0, self.policy.hard_slack_ms),
            phase_ms: [0.0; 5],
            invalidated: false,
            offs: [0; OFFS_MAX],
            off_len: 0,
        };

        self.run_phase(INTERACTION, &mut ctx);
        ctx.budget.reserve_ms = ctx.budget.total_ms * self.policy.content_share;
        self.run_phase(VISIBLE, &mut ctx);
        ctx.budget.reserve_ms = 0.0;
        for phase in [CONTENT, COSMETIC, REPORT] {
            self.run_phase(phase, &mut ctx);
        }

        self.last_report = TickReport {
            budget_ms: ctx.budget.total_ms,
            phase_ms: ctx.phase_ms,
            total_ms: now() - start,
            invalidated: ctx.invalidated,
        };
        self.last_report
    }
}

#[cfg(test)]
#[path = "tick.test.rs"]
mod test;
