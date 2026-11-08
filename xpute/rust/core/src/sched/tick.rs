// xpute-core/sched/tick.rs

//! A core's cooperative scheduler, which runs one tick a turn. A tick runs
//! the program's phases in the order the program numbers them, and all of
//! them share one budget: the runtime's deadline, counted from the rising
//! edge. While a phase runs, the share of the budget its policy holds back
//! for the later phases is not its to spend, so a busy phase cannot starve
//! them. The program decides what the phases are and what each holds back.
//! Each step keeps its own state.

use super::pass::Pass;
use crate::clock::now;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

/// A phase, by its place in the program's order.
pub type Phase = u8;

/// A tick runs at most this many phases.
pub const PHASES_MAX: usize = 8;

/// The most steps one step may unregister while it runs.
const OFFS_MAX: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TickPolicy {
    /// A hard limit this far past the remaining budget, so steps that report
    /// no work cannot run through the whole list (`Pass::hard_slack_ms`).
    pub hard_slack_ms: f64,
    /// One entry for each of the program's phases, in the order they run.
    /// Each is the fraction of the budget its phase must leave for the
    /// phases after it; 0 leaves none.
    pub held_back: &'static [f64],
}

pub struct TickContext {
    pub delta: f64,
    pub pass: Pass,
    pub phase_ms: [f64; PHASES_MAX],
    invalidated: bool,
    offs: [u32; OFFS_MAX],
    off_len: usize,
}

impl TickContext {
    /// Marks the tick as wanting another turn (`TickReport::invalidated`).
    /// The core is rung only when something asks for a turn, so a step that
    /// animates calls this on every tick.
    pub fn invalidate(&mut self) {
        self.invalidated = true;
    }

    pub fn off_tick(&mut self, id: u32) {
        xpute_kit::ensure!(self.off_len < OFFS_MAX, ENOSPC, OFFS_MAX);
        self.offs[self.off_len] = id;
        self.off_len += 1;
    }
}

pub type TickStep = fn(&mut TickContext);

/// A step's time, summed until `step_stats_reset`. Registrations with the
/// same phase and name add into one row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepStat {
    pub phase: Phase,
    pub name: u32,
    pub ms: f64,
    pub cnt: u32,
    pub max_ms: f64,
}

impl StepStat {
    const fn new(phase: Phase, name: u32) -> StepStat {
        StepStat {
            phase,
            name,
            ms: 0.0,
            cnt: 0,
            max_ms: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TickReport {
    pub budget_ms: f64,
    pub phase_ms: [f64; PHASES_MAX],
    pub total_ms: f64,
    pub invalidated: bool,
}

#[derive(Clone, Copy)]
struct Entry {
    id: u32,
    phase: Phase,
    step: TickStep,
    name: u32,
}

pub struct Tick<const N: usize> {
    policy: TickPolicy,
    registry: [Option<Entry>; N],
    len: usize,
    next_id: u32,
    stats: [StepStat; N],
    stats_len: usize,
    step_timing: bool,
    last_report: TickReport,
}

impl<const N: usize> Tick<N> {
    pub fn new(policy: TickPolicy) -> Self {
        xpute_kit::ensure!(
            policy.held_back.len() <= PHASES_MAX,
            EINVAL,
            policy.held_back.len()
        );
        Tick {
            policy,
            registry: [None; N],
            len: 0,
            next_id: 1,
            // The rows past `stats_len` are free room, not steps.
            stats: [StepStat::new(0, 0); N],
            stats_len: 0,
            step_timing: false,
            last_report: TickReport::default(),
        }
    }

    /// `name` is a number of the caller's choosing, under which the step's
    /// time is reported (`StepStat`).
    pub fn on_tick(&mut self, phase: Phase, step: TickStep, name: u32) -> u32 {
        xpute_kit::ensure!(self.len < N, ENOSPC, N);
        xpute_kit::ensure!(
            usize::from(phase) < self.policy.held_back.len(),
            EINVAL,
            phase
        );
        let id = self.next_id;
        self.next_id += 1;
        self.registry[self.len] = Some(Entry {
            id,
            phase,
            step,
            name,
        });
        self.len += 1;
        id
    }

    /// For use outside a tick. A step inside a tick unregisters through its
    /// context (`TickContext::off_tick`).
    pub fn off_tick(&mut self, id: u32) {
        self.remove(id);
    }

    pub fn set_step_timing(&mut self, on: bool) {
        self.step_timing = on;
    }

    pub fn step_stats(&self) -> &[StepStat] {
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

    /// When the table has no free row, the time of a step not yet in it is
    /// dropped.
    fn record(&mut self, phase: Phase, name: u32, ms: f64) {
        let at = match (0..self.stats_len)
            .find(|&k| self.stats[k].phase == phase && self.stats[k].name == name)
        {
            Some(k) => k,
            None if self.stats_len < N => {
                self.stats[self.stats_len] = StepStat::new(phase, name);
                self.stats_len += 1;
                self.stats_len - 1
            }
            None => return,
        };
        let stat = &mut self.stats[at];
        stat.ms += ms;
        stat.cnt += 1;
        stat.max_ms = stat.max_ms.max(ms);
    }

    fn run_phase(&mut self, phase: Phase, ctx: &mut TickContext) {
        let start = now();
        // An index, not an iterator: a step may unregister itself or another
        // step, and a snapshot would still run a step just unregistered.
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
        ctx.phase_ms[usize::from(phase)] = now() - start;
    }

    pub fn run(&mut self, delta: f64, deadline_ms: f64, t0: f64) -> TickReport {
        let start = now();
        let mut ctx = TickContext {
            delta,
            pass: Pass::new(deadline_ms, t0, self.policy.hard_slack_ms),
            phase_ms: [0.0; PHASES_MAX],
            invalidated: false,
            offs: [0; OFFS_MAX],
            off_len: 0,
        };

        let held_back = self.policy.held_back;
        for (phase, &share) in (0..).zip(held_back) {
            ctx.pass.reserve_ms = ctx.pass.total_ms * share;
            self.run_phase(phase, &mut ctx);
        }
        ctx.pass.reserve_ms = 0.0;

        self.last_report = TickReport {
            budget_ms: ctx.pass.total_ms,
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
