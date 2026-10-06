// xpute-runtime/sched/tick.rs

//! A guest's cooperative scheduler: one tick a turn, its phases in priority
//! order against one budget, the host's quota counted from the rising edge.
//! The interaction phase is unbudgeted and must stay O(1). The visible phase
//! may not take `content_share`, or content starves while a load keeps it busy.

use super::frame_budget::FrameBudget;
use crate::clock::now;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

pub type Phase = usize;
pub const INTERACTION: Phase = 0;
pub const VISIBLE: Phase = 1;
pub const CONTENT: Phase = 2;
pub const COSMETIC: Phase = 3;
pub const REPORT: Phase = 4;
pub const PHASES: [&str; PHASE_COUNT] = ["interaction", "visible", "content", "cosmetic", "report"];
const PHASE_COUNT: usize = REPORT + 1;

/// Steps unregistered from inside one step.
const OFFS_MAX: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TickPolicy {
    /// The hard ceiling over what is left, so steps that report no work
    /// cannot walk the whole list.
    pub hard_slack_ms: f64,
    pub content_share: f64,
}

pub struct TickContext<'a, S> {
    pub state: &'a mut S,
    pub delta: f64,
    pub budget: FrameBudget,
    pub phase_ms: [f64; PHASE_COUNT],
    invalidated: bool,
    offs: [u32; OFFS_MAX],
    off_len: usize,
}

impl<S> TickContext<'_, S> {
    /// The guest is strobed only when something asked for a turn.
    pub fn invalidate(&mut self) {
        self.invalidated = true;
    }

    pub fn off_tick(&mut self, id: u32) {
        xpute_core::ensure!(self.off_len < OFFS_MAX, ENOSPC, OFFS_MAX);
        self.offs[self.off_len] = id;
        self.off_len += 1;
    }
}

pub type TickStep<S> = fn(&mut TickContext<S>);

/// Cumulative until reset; registrations sharing a phase and name share a row.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepStats {
    pub phase: Phase,
    pub name: u32,
    pub ms: f64,
    pub count: u32,
    pub max_ms: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TickReport {
    pub budget_ms: f64,
    pub phase_ms: [f64; PHASE_COUNT],
    pub total_ms: f64,
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

    /// `name` is the caller's number the step's time is reported under.
    pub fn on_tick(&mut self, phase: Phase, step: TickStep<S>, name: u32) -> u32 {
        xpute_core::ensure!(self.len < N, ENOSPC, N);
        let id = self.next_id;
        self.next_id += 1;
        self.registry[self.len] = Some(Registered { id, phase, step, name });
        self.len += 1;
        id
    }

    /// Inside a tick, a step asks its context instead.
    pub fn off_tick(&mut self, id: u32) {
        self.remove(id);
    }

    pub fn set_step_timing(&mut self, on: bool) {
        self.step_timing = on;
    }

    pub fn step_stats(&self) -> &[StepStats] {
        &self.stats[..self.stats_len]
    }

    pub fn step_stats_reset(&mut self) {
        self.stats_len = 0;
    }

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

    /// A full table drops the time.
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

    pub fn run(&mut self, state: &mut S, delta: f64, quota_ms: f64, t0: f64) -> TickReport {
        let start = now();
        let mut ctx = TickContext {
            state,
            delta,
            budget: FrameBudget::new(quota_ms, t0, self.policy.hard_slack_ms),
            phase_ms: [0.0; PHASE_COUNT],
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
