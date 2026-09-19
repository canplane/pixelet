// xpute-runtime/sched/frame_budget.rs

//! The pass: the innermost of the scheduler's three levels
//! (sched/quantum.rs grants a guest its slice, sched/tick.rs spends it in
//! phases, this spends a phase's share on one list of steps).
//!
//! It is deliberately *not* a registry. Deciding what is eligible and in what
//! order is the caller's (a policy: distance, visibility, whatever the guest
//! ranks by); what generalizes is the part underneath: run an ordered list of
//! steps until a wall-time budget or a step cap says stop, without ever
//! starving the queue.
//!
//! **Forward progress is the invariant.** Neither the budget nor the cap
//! applies until at least one step has actually done work. A frame that is
//! already over budget before the loop starts still commits one step, so a
//! single expensive item can never permanently block everything behind it —
//! which is what separates cooperative scheduling from "skip work when
//! busy".
//!
//! A step returning `false` means "did not complete work": it doesn't count
//! toward the cap. It does **not** mean the step was free — see
//! `hard_budget_ms`, which exists because assuming otherwise cost a 4.7-second
//! frame.
//!
//! Shares nothing across calls itself. The frame-wide pool is sched/tick.rs's
//! FrameBudget, which calls this with whatever the frame has left.

use crate::clock::now;

#[derive(Clone, Copy, Debug, Default)]
pub struct BudgetOpts {
    /// wall-time this pass may consume, in ms. Waived while the pass has not
    /// yet completed a step — that is the forward-progress guarantee.
    pub budget_ms: f64,
    /// Absolute ceiling on the pass, never waived.
    ///
    /// `budget_ms` alone is not enough, and this is the measured reason: a step
    /// may cost real time and still report `false`. A chunk commit once
    /// scanned for ~15 ms and then deferred, a deliberate hand-off so a
    /// cheaper chunk got the frame's slot. With the soft budget waived until
    /// something succeeds, a run of expensive deferrals armed nothing: ~2000
    /// pending chunks produced a single 4711 ms frame while every individual
    /// step stayed under 42 ms.
    ///
    /// So the soft budget keeps the guarantee (something always runs) and this
    /// bounds what the guarantee is allowed to cost. None for no ceiling.
    pub hard_budget_ms: Option<f64>,
    /// hard cap on completed steps, independent of time. None for no cap.
    pub max_steps: Option<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PassStats {
    /// steps that reported doing work.
    pub ran: u32,
    /// items visited (including no-op steps and the one that tripped the stop).
    pub visited: u32,
    pub elapsed_ms: f64,
    /// true when the loop stopped early — there was more it would have run.
    pub stopped_early: bool,
}

/// Runs `step` over `items` in the order given, stopping once the budget or
/// the step cap is reached — except that the first step to do real work
/// always runs. Returns what it managed.
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
        // The ceiling applies even to a pass that has achieved nothing — see
        // hard_budget_ms on why "nothing achieved" is not the same as "nothing
        // spent". Checked first so it can never be out-voted by the waiver below,
        // but still gated on `visited` so the first item is always attempted:
        // forward progress is the invariant, and a caller passing a tiny ceiling
        // should get one bounded attempt rather than a pass that does nothing.
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

#[cfg(test)]
#[path = "frame_budget.test.rs"]
mod test;
