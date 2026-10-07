// pxl-kernel/sched/tick.rs

//! The world's tick: xpute's tick mechanism with the kernel's phase shares,
//! settle time, declared quantum and step table.

use xpute_guest::sched::quantum::QuantumPolicy;
use xpute_guest::sched::tick::{Phase, StepStats, Tick, TickPolicy, TickReport};

use crate::param;
pub use crate::sched::profiler::spec::StepName;
use xpute_guest::global::Global;

#[derive(Clone, Copy, Debug, Default)]
pub struct TickState {
    pub width: f64,
    pub height: f64,
    pub dpr: f64,
    /// Behind the wall clock by what pauses cost (MAX_STEP_S).
    pub elapsed_s: f64,
    pub frame_ms: f64,
    pub interactive: bool,
    /// The raw gap, which past MAX_STEP_S exceeds the step. Only the profiler reads it.
    pub gap_s: f64,
}

pub type TickContext<'a> = xpute_guest::sched::tick::TickContext<'a, TickState>;
pub type TickStep = xpute_guest::sched::tick::TickStep<TickState>;

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

static TICK: Global<Tick<TickState, STEPS>> = Global::new();

fn tick() -> &'static mut Tick<TickState, STEPS> {
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

/// Returns the id `off_tick` takes.
pub fn on_tick(phase: Phase, step: TickStep, name: StepName) -> u32 {
    tick().on_tick(phase, step, name as u32)
}

pub fn off_tick(id: u32) {
    tick().off_tick(id);
}

pub fn run_tick(state: &mut TickState, delta: f64, quota_ms: f64, edge_at: f64) -> TickReport {
    tick().run(state, delta, quota_ms, edge_at)
}
