// pxl-core/sys/sched/tick.rs

//! The world's tick: xpute's tick with the core's phases
//! (sys/sched/phase.spec.rs), what each holds back, and its step table.

use xpute_core::global::Global;
pub use xpute_core::sched::tick::TickContext;
use xpute_core::sched::tick::{Tick, TickPolicy};

use crate::sys::sched::param;
use crate::sys::sched::phase_spec::Phase;

/// Each phase's share held back for the phases after it: the visible phase
/// keeps the content's.
const HELD_BACK: [f64; Phase::ALL.len()] = {
    let mut held = [0.0; Phase::ALL.len()];
    held[Phase::visible as usize] = param::CONTENT_SHARE;
    held
};

const POLICY: TickPolicy = TickPolicy {
    hard_slack_ms: param::HARD_SLACK_MS,
    held_back: &HELD_BACK,
};

/// The steps registered at boot, with room to spare.
const STEPS: usize = 32;

static TICK: Global<Tick<STEPS>> = Global::new();

/// The world's tick; a step's `name` is the caller's index into its own step
/// names (the turn's, turn/step.spec.rs).
pub fn tick() -> &'static mut Tick<STEPS> {
    TICK.get(|| Tick::new(POLICY))
}
