// xpute-core/sched/pass.rs

//! A pass is a turn's remaining budget, spent on ordered lists of steps
//! (`run`). The invariant is forward progress: neither the budget nor a step
//! cap applies until one step has done work. An item that costs more than the
//! whole budget therefore still runs, and cannot stay at the head of its list
//! forever, blocking the items behind it.

use crate::clock::now;

#[derive(Clone, Copy, Debug, Default)]
pub struct Pass {
    /// The time the turn started. The budget counts from it, so the
    /// time the turn spent before this pass comes off what is left.
    pub t0: f64,
    pub total_ms: f64,
    /// The time held back for what the turn runs after this pass.
    pub reserve_ms: f64,
    /// A hard limit this far past the remaining budget. Unlike the budget, it
    /// applies before any step has done work, since a step can cost time and
    /// still report none.
    pub hard_slack_ms: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PassStat {
    pub ran: u32,
    /// Every step called, counting those that reported no work and the one
    /// that used up the budget.
    pub step_cnt: u32,
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

    /// `step` answers whether it did work. Only steps that did work count
    /// toward `max_steps`.
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
            // The hard limit applies even before any step has done work,
            // though never to the first item.
            if stat.step_cnt > 0 && at >= hard_deadline
                || stat.ran > 0 && (stat.ran >= cap || at >= deadline)
            {
                stat.stopped_early = true;
                break;
            }
            stat.step_cnt += 1;
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
