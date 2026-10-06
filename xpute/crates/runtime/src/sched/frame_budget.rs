// xpute-runtime/sched/frame_budget.rs

//! Runs an ordered list of steps until a budget or a step cap says stop.
//! Forward progress is the invariant: neither applies until one step has
//! done work, so one expensive item cannot block everything behind it.

use crate::clock::now;

#[derive(Clone, Copy, Debug, Default)]
pub struct BudgetOpts {
    /// Waived until a step has done work.
    pub budget_ms: f64,
    /// Never waived: a step can cost time and still report no work, and
    /// ~2000 such deferrals once made one 4711 ms frame.
    pub hard_budget_ms: Option<f64>,
    pub max_steps: Option<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PassStats {
    pub ran: u32,
    pub visited: u32,
    pub elapsed_ms: f64,
    pub stopped_early: bool,
}

pub fn run_under_budget<T>(items: impl IntoIterator<Item = T>, mut step: impl FnMut(T) -> bool, opts: BudgetOpts) -> PassStats {
    let t0 = now();
    let deadline = t0 + opts.budget_ms;
    let hard_deadline = t0 + opts.hard_budget_ms.unwrap_or(f64::INFINITY);
    let cap = opts.max_steps.unwrap_or(u32::MAX);

    let mut ran = 0;
    let mut visited = 0;
    let mut stopped_early = false;

    for item in items {
        let at = now();
        // Before the waiver below, but the first item is always attempted.
        if visited > 0 && at >= hard_deadline {
            stopped_early = true;
            break;
        }
        if ran > 0 && (ran >= cap || at >= deadline) {
            stopped_early = true;
            break;
        }
        visited += 1;
        if step(item) {
            ran += 1;
        }
    }

    PassStats {
        ran,
        visited,
        elapsed_ms: now() - t0,
        stopped_early,
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FrameBudget {
    pub t0: f64,
    pub total_ms: f64,
    /// Held back for the phases after the current one.
    pub reserve_ms: f64,
    pub hard_slack_ms: f64,
}

impl FrameBudget {
    /// `t0` is the rising edge, so what the turn spent before comes off.
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

#[cfg(test)]
#[path = "frame_budget.test.rs"]
mod test;
