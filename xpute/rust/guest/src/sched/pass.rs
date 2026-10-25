// xpute-guest/sched/pass.rs

//! What is left of a turn's budget, spent on ordered lists of steps (`run`).
//! Forward progress is the invariant: neither the budget nor a step cap
//! applies until one step has done work, so one expensive item cannot block
//! everything behind it.

use crate::clock::now;

#[derive(Clone, Copy, Debug, Default)]
pub struct Pass {
    /// The rising edge, so what the turn spent before comes off.
    pub t0: f64,
    pub total_ms: f64,
    /// Held back for the phases after the current one.
    pub reserve_ms: f64,
    /// Past what is left, never waived: a step can cost time and still report
    /// no work.
    pub hard_slack_ms: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PassStat {
    pub ran: u32,
    /// With the steps that did no work, and the one that tripped the stop.
    pub visited: u32,
    pub elapsed_ms: f64,
    pub stopped_early: bool,
}

impl Pass {
    pub fn new(total_ms: f64, t0: f64, hard_slack_ms: f64) -> Pass {
        Pass {
            t0,
            total_ms,
            reserve_ms: 0.0,
            hard_slack_ms,
        }
    }

    pub fn remaining(&self) -> f64 {
        (self.t0 + self.total_ms - self.reserve_ms - now()).max(0.0)
    }

    /// `step` answers whether it did work; only work counts to `max_steps`.
    pub fn run<T>(
        &self,
        items: impl IntoIterator<Item = T>,
        mut step: impl FnMut(T) -> bool,
        max_steps: Option<u32>,
    ) -> PassStat {
        let t0 = now();
        let deadline = t0 + self.remaining();
        let hard_deadline = deadline + self.hard_slack_ms;
        let cap = max_steps.unwrap_or(u32::MAX);
        let mut stat = PassStat::default();
        for item in items {
            let at = now();
            // Before the waiver below, but the first item is always tried.
            if stat.visited > 0 && at >= hard_deadline
                || stat.ran > 0 && (stat.ran >= cap || at >= deadline)
            {
                stat.stopped_early = true;
                break;
            }
            stat.visited += 1;
            if step(item) {
                stat.ran += 1;
            }
        }
        stat.elapsed_ms = now() - t0;
        stat
    }
}

#[cfg(test)]
#[path = "pass.test.rs"]
mod test;
