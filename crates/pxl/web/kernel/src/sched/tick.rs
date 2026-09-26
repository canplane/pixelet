// pxl-kernel/sched/tick.rs

//! The world's tick: xpute_runtime::sched::tick run over the world's frame
//! state, with the numbers this world measured. How a tick runs is the
//! mechanism's; what is here is what the kernel asks of it — its phases'
//! share, its settle time, and the quantum it declares to the central
//! scheduler (the boot block carries QUANTUM) — and the one step table the
//! kernel keeps.
//!
//! The table is for outside a tick: a step that unregisters asks its
//! context, and no step reaches these while the tick runs.

use xpute_runtime::sched::quantum::QuantumPolicy;
pub use xpute_runtime::sched::tick::{FrameBudget, Phase, StepStats, TickReport, CONTENT, COSMETIC, INTERACTION, PHASES, REPORT, VISIBLE};
use xpute_runtime::sched::tick::{Tick, TickPolicy};

use crate::global::Global;
pub use crate::sched::profiler::spec::StepName;

/// What a step reads of the frame beside the tick's own numbers.
#[derive(Clone, Copy, Debug, Default)]
pub struct TickState {
    /// The viewport in CSS pixels, and the device pixels each one is.
    pub width: f64,
    pub height: f64,
    pub dpr: f64,
    /// Seconds the world has been stepped, which past a pause is behind the
    /// wall clock by what the pause cost (sched/turn.rs's MAX_STEP_S).
    pub elapsed_s: f64,
    /// The host's measured frame interval and whether it ran this turn as
    /// interactive: the profiler's, which reads a frame against them.
    pub frame_ms: f64,
    pub interactive: bool,
    /// The gap the host measured before this turn, in seconds — what passed,
    /// which past MAX_STEP_S is more than the world is stepped by. The profiler
    /// wants this one; nothing that moves does (sched/turn.rs).
    pub gap_s: f64,
}

pub type TickContext<'a> = xpute_runtime::sched::tick::TickContext<'a, TickState>;
pub type TickStep = xpute_runtime::sched::tick::TickStep<TickState>;

/// The quantum the kernel declares to the central scheduler.
pub const QUANTUM: QuantumPolicy = QuantumPolicy {
    // A share of the frame rather than a constant, so a fast display does not
    // lose most of its frame to it.
    margin_share: 0.15,
    // Past a frame on purpose: with nothing moving, a turn that takes two
    // vsyncs shows the same image twice, and nothing the eye can follow is
    // waiting on it.
    batch_frames: 2.0,
    // Long enough that a pause inside one drag does not count.
    settle_ms: 150.0,
};

pub const POLICY: TickPolicy = TickPolicy {
    hard_slack_ms: 4.0,
    // Without a share of its own, content waits for as long as a load keeps
    // the visible phase busy. A fixed share, not a feedback loop.
    content_share: 0.2,
};

pub const HARD_SLACK_MS: f64 = POLICY.hard_slack_ms;

/// The steps sched/turn.rs registers at boot, with room to spare.
const STEPS: usize = 32;

static TICK: Global<Tick<TickState, STEPS>> = Global::new();

fn tick() -> &'static mut Tick<TickState, STEPS> {
    TICK.get(|| Tick::new(POLICY))
}

/// Whether steps are timed one by one: dev builds only.
pub fn set_step_timing(on: bool) {
    tick().set_step_timing(on);
}

pub fn tick_step_stats() -> &'static [StepStats] {
    tick().step_stats()
}

pub fn tick_step_stats_reset() {
    tick().step_stats_reset();
}

/// Registers a step into a phase. Returns the id `off_tick` takes. `name` is
/// what the profiler reports the step's time under.
pub fn on_tick(phase: Phase, step: TickStep, name: StepName) -> u32 {
    tick().on_tick(phase, step, name as u32)
}

pub fn off_tick(id: u32) {
    tick().off_tick(id);
}

/// The world's tick, on the quota the central scheduler strobed it with at
/// `edge_at`.
pub fn run_tick(state: &mut TickState, delta: f64, quota_ms: f64, edge_at: f64) -> TickReport {
    tick().run(state, delta, quota_ms, edge_at)
}
