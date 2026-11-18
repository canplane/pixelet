// pxl-core/sys/sched/tick.rs

//! The core's tick: each phase's steps run in order, within the entry's
//! budget (sys/sched.rs `spent`), the visible phase leaving the content's
//! share of it unspent (`reserve`), and each phase's time is kept for the
//! profiler (spec/phase.spec.rs). The frame runs the steps (frame.rs
//! `frame::tick`).

use xpute_guest::clock::now;

use crate::spec::phase::Phase;
use crate::sys::sched;

pub(crate) struct TickContext {
    pub delta: f64,
    phase_ms: [f64; Phase::ALL.len()],
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TickReport {
    pub budget_ms: f64,
    /// Whether the entry's budget was spent as the tick ended (sys/sched.rs
    /// `spent`).
    pub overran: bool,
    pub phase_ms: [f64; Phase::ALL.len()],
    pub total_ms: f64,
}

impl TickContext {
    /// A tick stepping `delta` seconds.
    pub(crate) fn new(delta: f64) -> TickContext {
        TickContext {
            delta,
            phase_ms: [0.0; Phase::ALL.len()],
        }
    }

    /// Runs `phase`'s steps, and keeps their time.
    pub(crate) fn phase(&mut self, phase: Phase, steps: impl FnOnce(&mut TickContext)) {
        let start = now();
        sched::reserve(phase == Phase::Visible);
        steps(self);
        sched::reserve(false);
        self.phase_ms[phase as usize] = now() - start;
    }

    /// What the tick that began at `start` came to.
    pub(crate) fn report(&self, start: f64) -> TickReport {
        TickReport {
            budget_ms: sched::deadline_ms(),
            overran: sched::spent(),
            phase_ms: self.phase_ms,
            total_ms: now() - start,
        }
    }
}
