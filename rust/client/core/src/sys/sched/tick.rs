// pxl-core/sys/sched/tick.rs

//! The core's tick: each phase's steps run in order under one budget, the time
//! to the deadline, each phase leaving the phases after it a share of it
//! (sys/sched/phase.spec.rs). The turn runs the steps (turn.rs `world_tick`).

use xpute_core::clock::now;
use xpute_core::sched::pass::Pass;

use crate::sys::sched::param;
use crate::sys::sched::phase_spec::{Phase, PHASES};

/// The share of the budget a phase must leave to the phases after it. Only the
/// visible phase leaves one: the content's share.
fn reserve(phase: Phase) -> f64 {
    match phase {
        Phase::visible => param::CONTENT_SHARE,
        _ => 0.0,
    }
}

pub struct TickContext {
    pub delta: f64,
    pub pass: Pass,
    phase_ms: [f64; PHASES],
    invalidated: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TickReport {
    pub budget_ms: f64,
    pub phase_ms: [f64; PHASES],
    pub total_ms: f64,
    pub invalidated: bool,
}

impl TickContext {
    /// A tick of a turn that started at `t0` and whose deadline is
    /// `deadline_ms` after it, stepping `delta` seconds.
    pub fn new(delta: f64, deadline_ms: f64, t0: f64) -> TickContext {
        TickContext {
            delta,
            pass: Pass::new(deadline_ms, t0, param::HARD_SLACK_MS),
            phase_ms: [0.0; PHASES],
            invalidated: false,
        }
    }

    /// Marks the tick as wanting another turn. The core is run only when
    /// something asks for a turn, so a step that animates calls this on every
    /// tick.
    pub fn invalidate(&mut self) {
        self.invalidated = true;
    }

    /// Runs `phase`'s steps, which leave the phases after it their share.
    pub fn phase(&mut self, phase: Phase, steps: impl FnOnce(&mut TickContext)) {
        let start = now();
        self.pass.reserve_ms = self.pass.total_ms * reserve(phase);
        steps(self);
        self.pass.reserve_ms = 0.0;
        self.phase_ms[phase as usize] = now() - start;
    }

    /// What the tick that began at `start` came to.
    pub fn report(&self, start: f64) -> TickReport {
        TickReport {
            budget_ms: self.pass.total_ms,
            phase_ms: self.phase_ms,
            total_ms: now() - start,
            invalidated: self.invalidated,
        }
    }
}
