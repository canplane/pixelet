// pxl-core/sys/sched/tick.rs

//! The core's tick: each phase's steps run in order, within the entry's
//! budget (sys/sched.rs `spent`), and each phase's time is kept for the
//! profiler (sys/sched/phase.spec.rs). The frame runs the steps (frame.rs
//! `frame::tick`).

use xpute_guest::clock::now;

use crate::sys::sched::param;
use crate::sys::sched::phase_spec::{Phase, PHASES};

pub struct TickContext {
    pub delta: f64,
    phase_ms: [f64; PHASES.len()],
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TickReport {
    pub budget_ms: f64,
    pub phase_ms: [f64; PHASES.len()],
    pub total_ms: f64,
}

impl TickContext {
    /// A tick stepping `delta` seconds.
    pub fn new(delta: f64) -> TickContext {
        TickContext {
            delta,
            phase_ms: [0.0; PHASES.len()],
        }
    }

    /// Runs `phase`'s steps, and keeps their time.
    pub fn phase(&mut self, phase: Phase, steps: impl FnOnce(&mut TickContext)) {
        let start = now();
        steps(self);
        self.phase_ms[phase as usize] = now() - start;
    }

    /// What the tick that began at `start` came to.
    pub fn report(&self, start: f64) -> TickReport {
        TickReport {
            budget_ms: param::BUDGET_MS,
            phase_ms: self.phase_ms,
            total_ms: now() - start,
        }
    }
}
