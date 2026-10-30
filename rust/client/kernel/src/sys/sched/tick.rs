// pxl-kernel/sys/sched/tick.rs

//! The world's tick: xpute's tick mechanism with the kernel's phases
//! (sys/sched/phase.spec.rs), what each holds back, and its step table.

use xpute_guest::sched::tick::{StepStat, Tick, TickPolicy, TickReport};
pub use xpute_guest::sched::tick::{TickContext, TickStep};

use crate::sys::sched::param;
use crate::sys::sched::phase_spec::Phase;
use xpute_guest::global::Global;

/// Each phase's share held back for the phases after it: the visible phase
/// keeps the content's, or content starves while a load keeps it busy.
const HELD_BACK: [f64; Phase::ALL.len()] = {
    let mut held = [0.0; Phase::ALL.len()];
    held[Phase::visible as usize] = param::CONTENT_SHARE;
    held
};

pub const POLICY: TickPolicy = TickPolicy {
    hard_slack_ms: param::HARD_SLACK_MS,
    held_back: &HELD_BACK,
};

pub const HARD_SLACK_MS: f64 = POLICY.hard_slack_ms;

/// With room to spare.
const STEPS: usize = 32;

static TICK: Global<Tick<STEPS>> = Global::new();

fn tick() -> &'static mut Tick<STEPS> {
    TICK.get(|| Tick::new(POLICY))
}

pub fn set_step_timing(on: bool) {
    tick().set_step_timing(on);
}

pub fn tick_step_stats() -> &'static [StepStat] {
    tick().step_stats()
}

pub fn tick_step_stats_reset() {
    tick().step_stats_reset();
}

/// Returns the id `off_tick` takes. `name` is the caller's index into its own
/// step names (the turn's, turn/step.spec.rs).
pub fn on_tick(phase: Phase, step: TickStep, name: u32) -> u32 {
    tick().on_tick(phase as u8, step, name)
}

pub fn off_tick(id: u32) {
    tick().off_tick(id);
}

pub fn run_tick(delta: f64, quota_ms: f64, edge_at: f64) -> TickReport {
    tick().run(delta, quota_ms, edge_at)
}
