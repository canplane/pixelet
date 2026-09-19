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

/// What a step reads of the frame beside the tick's own numbers.
#[derive(Clone, Copy, Debug, Default)]
pub struct TickState {
    /// The viewport in CSS pixels, and the device pixels each one is.
    pub width: f64,
    pub height: f64,
    pub dpr: f64,
    /// Seconds since the loop started.
    pub elapsed_s: f64,
    /// The host's measured frame interval and whether it ran this turn as
    /// interactive: the ledger's, which reads a frame against them.
    pub frame_ms: f64,
    pub interactive: bool,
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
/// what the ledger reports the step's time under.
pub fn on_tick(phase: Phase, step: TickStep, name: &'static str) -> u32 {
    tick().on_tick(phase, step, name)
}

pub fn off_tick(id: u32) {
    tick().off_tick(id);
}

/// What the previous tick spent, per phase. For the ledger.
pub fn tick_last() -> TickReport {
    tick().last()
}

/// The world's tick, on the quota the central scheduler strobed it with at
/// `edge_at`.
pub fn run_tick(state: &mut TickState, delta: f64, quota_ms: f64, edge_at: f64) -> TickReport {
    tick().run(state, delta, quota_ms, edge_at)
}
