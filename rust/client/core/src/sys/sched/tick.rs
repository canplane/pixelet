// pxl-core/sys/sched/tick.rs

//! The core's one tick: xpute's tick run over the core's phases
//! (sys/sched/phase.spec.rs), with the share of the budget each phase leaves
//! to the phases after it.

use xpute_core::global::Global;
pub use xpute_core::sched::tick::TickContext;
use xpute_core::sched::tick::{Tick, TickPolicy};

use crate::sys::sched::param;
use crate::sys::sched::phase_spec::Phase;

/// The share of the budget each phase must leave to the phases after it.
/// Only the visible phase leaves one: the content's share.
const RESERVE: [f64; Phase::ALL.len()] = {
    let mut share = [0.0; Phase::ALL.len()];
    share[Phase::visible as usize] = param::CONTENT_SHARE;
    share
};

const POLICY: TickPolicy = TickPolicy {
    hard_slack_ms: param::HARD_SLACK_MS,
    reserve: &RESERVE,
};

/// Room for the steps registered at boot, with some to spare.
const STEPS: usize = 32;

static TICK: Global<Tick<STEPS>> = Global::new();

/// The core's tick. A step's `name` is an index into the step names of the
/// caller that registered it; the turn's names are in turn/step.spec.rs.
pub fn tick() -> &'static mut Tick<STEPS> {
    TICK.get(|| Tick::new(POLICY))
}
