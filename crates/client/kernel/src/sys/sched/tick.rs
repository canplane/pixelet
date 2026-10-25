// pxl-kernel/sys/sched/tick.rs

//! The world's tick: xpute's tick mechanism with the kernel's phase shares,
//! settle time, declared quantum and step table.

use xpute_guest::sched::quantum::QuantumPolicy;
use xpute_guest::sched::tick::{Phase, StepStats, Tick, TickPolicy, TickReport};

use crate::sys::sched::param;
use xpute_guest::global::Global;

/// The tick's steps keep their own state; xpute's shared state is none.
pub type TickContext<'a> = xpute_guest::sched::tick::TickContext<'a, ()>;
pub type TickStep = xpute_guest::sched::tick::TickStep<()>;

pub const QUANTUM: QuantumPolicy = QuantumPolicy {
    margin_share: param::QUANTUM_MARGIN_SHARE,
    batch_frames: param::QUANTUM_BATCH_FRAMES,
    settle_ms: param::QUANTUM_SETTLE_MS,
};

pub const POLICY: TickPolicy = TickPolicy {
    hard_slack_ms: param::HARD_SLACK_MS,
    content_share: param::CONTENT_SHARE,
};

pub const HARD_SLACK_MS: f64 = POLICY.hard_slack_ms;

/// With room to spare.
const STEPS: usize = 32;

static TICK: Global<Tick<(), STEPS>> = Global::new();

fn tick() -> &'static mut Tick<(), STEPS> {
    TICK.get(|| Tick::new(POLICY))
}

pub fn set_step_timing(on: bool) {
    tick().set_step_timing(on);
}

pub fn tick_step_stats() -> &'static [StepStats] {
    tick().step_stats()
}

pub fn tick_step_stats_reset() {
    tick().step_stats_reset();
}

/// Returns the id `off_tick` takes. `name` is the caller's index into its own
/// step names (the turn's, turn/step.spec.rs).
pub fn on_tick(phase: Phase, step: TickStep, name: u32) -> u32 {
    tick().on_tick(phase, step, name)
}

pub fn off_tick(id: u32) {
    tick().off_tick(id);
}

pub fn run_tick(delta: f64, quota_ms: f64, edge_at: f64) -> TickReport {
    tick().run(&mut (), delta, quota_ms, edge_at)
}
