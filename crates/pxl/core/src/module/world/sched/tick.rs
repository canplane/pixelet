// pxl_core/module/world/sched/tick.rs

//! The frame, as one tick.
//!
//! There is one tick, run once per animation frame the world asked for, and
//! it runs its phases in a fixed order against one budget. The order *is* the
//! priority:
//!
//!   interaction   camera, frame rebasing, the puck — what the hand is doing.
//!                 Not budgeted, and must stay O(1): the budget clock is
//!                 already running, so anything spent here comes off what
//!                 the phases below receive.
//!   visible       LOD recheck and commit — what is on screen now.
//!   content       index sweeps, placement — what is about to be on screen.
//!   cosmetic      fades, environment lerps, decor and billboard packing.
//!   report        the ledger, after everything else, so it sees everything.
//!
//! Within a phase, steps run in the order they registered.
//!
//! **The budget is derived, not chosen.** This reads the ledger's own record
//! of what the rest of the frame costs — the main-thread render time outside
//! this tick — and hands the phases what is left of the frame after that, so a
//! heavier scene gets less scheduling time and a lighter one gets more. It is
//! clamped: never so little that nothing makes progress, never so much that a
//! GC pause has no room.
//!
//! **A still view gets a bigger budget.** The budget exists to keep motion
//! smooth, and when nothing on screen is moving there is no motion to keep
//! smooth: a frame that takes two vsyncs instead of one shows the same image
//! twice. So once the view has held still for SETTLE_MS, the tick is sized for
//! throughput instead, and the backlog a pan left behind drains several times
//! faster. The view steps in the interaction phase say when they moved
//! (`view_moving`), and the budget is decided after that phase, so the frame a
//! gesture starts on is already back on the small budget.
//!
//! What this does not do: preempt. A step still runs to completion. Steps are
//! small by construction instead.
//!
//! The frame loop itself — an animation frame when something asked, the
//! viewport's size — is the shell's; it calls `run_tick`.

use super::frame_budget::{run_under_budget, BudgetOpts, PassStats};
use super::ledger::ledger_last_frame;
use crate::global::{now, Global};
use crate::module::world::change::take_tick_request;

pub type Phase = usize;
pub const INTERACTION: Phase = 0;
pub const VISIBLE: Phase = 1;
pub const CONTENT: Phase = 2;
pub const COSMETIC: Phase = 3;
pub const REPORT: Phase = 4;
pub const PHASES: [&str; 5] = ["interaction", "visible", "content", "cosmetic", "report"];

/// What a step may ask of the frame: another frame, the viewport, the clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct TickState {
    /// A step asked for a frame after this one: the tick runs only when
    /// something asked, so a step that changes something over time asks for the
    /// next.
    pub invalidated: bool,
    /// The viewport in CSS pixels, and the device pixels each one is.
    pub width: f64,
    pub height: f64,
    pub dpr: f64,
    /// Seconds since the loop started.
    pub elapsed_s: f64,
}

pub struct TickContext<'a> {
    pub state: &'a mut TickState,
    pub delta: f64,
    /// Decided after the interaction phase; steps there must not read it.
    pub budget: FrameBudget,
    /// Why the budget is the size it is — both set with it, for the ledger.
    pub frame_ms: f64,
    pub settled: bool,
    /// Wall time each phase has taken so far this tick, filled in as phases
    /// complete — so the report phase can read everything before it.
    pub phase_ms: [f64; 5],
    moved: bool,
}

impl TickContext<'_> {
    /// Asks for a frame after this one.
    pub fn invalidate(&mut self) {
        self.state.invalidated = true;
    }

    /// Called by an interaction step whose view is still moving — the camera
    /// damping toward its target, the frame rotating or translating.
    pub fn view_moving(&mut self) {
        self.moved = true;
    }
}

pub type TickStep = fn(&mut TickContext);

/// The display's frame interval, measured rather than assumed.
///
/// It was `1000 / 60`, which is wrong on any faster display: the ledger read a
/// p50 frame of 5.8 ms while the tick was sized for 16.7, so a storm could
/// spend more than a whole frame scheduling and drop every other one.
///
/// Taken as a low percentile of recent frame gaps, not their mean. The mean
/// includes the frames this tick itself made long, so sizing the budget from
/// it would feed an overrun back into a bigger budget. Gaps past
/// MAX_FRAME_GAP_MS are not frames at all — an idle canvas draws nothing, and
/// the gap before the next frame can be seconds.
///
/// Only gaps between two moving frames count, and the percentile is the 25th
/// rather than the 10th. A still frame runs long on purpose, and the browser
/// answers a late frame by firing the next one almost at once — the ledger
/// read a "2.1 ms display" on a 170 Hz screen from those catch-up gaps, which
/// pinned every moving frame to the budget floor.
const FRAME_WINDOW: usize = 64;
const MAX_FRAME_GAP_MS: f64 = 50.0;
/// Left unspent on purpose, for the collector and for the frames the estimate
/// below is wrong about — a share of the frame rather than a constant, so a
/// fast display does not lose most of its frame to it.
const MARGIN_SHARE: f64 = 0.15;
/// The clamp. The floor is what keeps a starved frame making progress; the
/// ceiling is what keeps a cheap scene from letting one tick grow into most of
/// a frame and leaving nothing for a spike.
const MIN_BUDGET_MS: f64 = 2.0;
const MAX_BUDGET_MS: f64 = 8.0;
/// How long the view must hold still before the tick stops guarding
/// smoothness. Long enough that a pause inside one drag does not count.
const SETTLE_MS: f64 = 150.0;
/// The still-view budget. Past a 60 Hz frame on purpose: at 170 Hz this is two
/// or three vsyncs of an unchanging image, and at 60 Hz one frame plus render —
/// either way nothing the eye can follow is waiting on it.
const SETTLED_BUDGET_MS: f64 = 12.0;
/// How far a pass may run past its share, so a step that costs real time and
/// reports nothing done cannot walk the whole list — frame_budget.rs's own
/// hard ceiling, expressed as slack over the pool rather than as a second
/// constant per pass.
pub const HARD_SLACK_MS: f64 = 4.0;
/// Smoothing on the render estimate. One frame is noise; ten is a trend.
const RENDER_EMA: f64 = 0.1;
/// The share of the budget the visible phase may not take: what content gets
/// even in a storm. Phases run in priority order on one budget, and without
/// this content got only the forward-progress floor — one unit a frame — while
/// a load kept the visible phase busy. A placement of a few dozen units then
/// took 25 frames and 3.3 s on a phone, and an index rebuild 49 frames
/// (2026-09-13). A fixed share, not a feedback loop.
const CONTENT_SHARE: f64 = 0.2;

/// One frame's allowance, shared by every phase.
///
/// `run` is `run_under_budget` with the numbers filled in from what is left,
/// so a caller states what it wants to do and never what it may spend.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameBudget {
    pub t0: f64,
    pub total_ms: f64,
    /// Held back from whoever asks now, for the phases after it. The tick sets
    /// it before the visible phase and clears it before content.
    pub reserve_ms: f64,
}

impl FrameBudget {
    /// `t0` is the tick's start, not this call's: the interaction phase runs
    /// before the budget is sized, and what it spent comes off the budget.
    pub fn new(total_ms: f64, t0: f64) -> FrameBudget {
        FrameBudget { t0, total_ms, reserve_ms: 0.0 }
    }

    pub fn remaining(&self) -> f64 {
        (self.t0 + self.total_ms - self.reserve_ms - now()).max(0.0)
    }

    pub fn run<T>(&self, items: impl IntoIterator<Item = T>, step: impl FnMut(T) -> bool, max_steps: Option<u32>) -> PassStats {
        let remaining = self.remaining();
        run_under_budget(
            items,
            step,
            BudgetOpts {
                budget_ms: remaining,
                hard_budget_ms: Some(remaining + HARD_SLACK_MS),
                max_steps,
            },
        )
    }
}

/// Per-step wall time, keyed `phase/name`, cumulative since the ledger window
/// began (log.rs resets it). The phase totals said the interaction phase cost a
/// phone 7 ms a frame and could not say which of its five steps did; this can.
/// Two registrations under one name add into one row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StepStats {
    pub name: String,
    pub ms: f64,
    pub count: u32,
    pub max_ms: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TickReport {
    pub budget_ms: f64,
    /// The display's measured frame interval when the budget was sized.
    pub frame_ms: f64,
    /// The view had held still for SETTLE_MS — the budget is the large one.
    pub settled: bool,
    /// Wall time each phase took this frame.
    pub phase_ms: [f64; 5],
    /// The whole tick, interaction through report.
    pub total_ms: f64,
    /// A frame after this one was asked for.
    pub invalidated: bool,
}

struct Registered {
    id: u32,
    phase: Phase,
    step: TickStep,
    name: &'static str,
}

struct Tick {
    registry: Vec<Registered>,
    next_id: u32,
    step_stats: Vec<StepStats>,
    step_timing: bool,
    frame_gaps: [f64; FRAME_WINDOW],
    frame_gap_at: usize,
    render_ema_ms: f64,
    moved_at: f64,
    prev_settled: bool,
    last_report: TickReport,
}

static TICK: Global<Tick> = Global::new();

fn tick() -> &'static mut Tick {
    TICK.get(|| Tick {
        registry: Vec::new(),
        next_id: 1,
        step_stats: Vec::new(),
        step_timing: false,
        frame_gaps: [1000.0 / 60.0; FRAME_WINDOW],
        frame_gap_at: 0,
        render_ema_ms: 4.0,
        moved_at: f64::NEG_INFINITY,
        prev_settled: true,
        last_report: TickReport::default(),
    })
}

/// Whether steps are timed one by one: dev builds only.
pub fn set_step_timing(on: bool) {
    tick().step_timing = on;
}

pub fn tick_step_stats() -> Vec<StepStats> {
    tick().step_stats.clone()
}

pub fn tick_step_stats_reset() {
    tick().step_stats.clear();
}

/// Registers a step into a phase. Returns the id `off_tick` takes. `name` is
/// what the ledger reports the step's time under.
pub fn on_tick(phase: Phase, step: TickStep, name: &'static str) -> u32 {
    let t = tick();
    let id = t.next_id;
    t.next_id += 1;
    t.registry.push(Registered { id, phase, step, name });
    id
}

pub fn off_tick(id: u32) {
    tick().registry.retain(|r| r.id != id);
}

/// What the previous tick spent, per phase. For the ledger.
pub fn tick_last() -> TickReport {
    tick().last_report
}

fn observe_frame_gap(t: &mut Tick, delta_s: f64) {
    let ms = delta_s * 1000.0;
    if ms.is_nan() || ms <= 0.0 || ms > MAX_FRAME_GAP_MS {
        return;
    }
    t.frame_gaps[t.frame_gap_at] = ms;
    t.frame_gap_at = (t.frame_gap_at + 1) % FRAME_WINDOW;
}

fn frame_ms(t: &Tick) -> f64 {
    let mut gaps = t.frame_gaps;
    gaps.sort_by(f64::total_cmp);
    gaps[FRAME_WINDOW / 4]
}

/// Main-thread render cost outside this tick, learned from the ledger: the
/// whole rAF task minus the scheduler's own pass.
fn observe_render_cost(t: &mut Tick) {
    let Some(last) = ledger_last_frame() else { return };
    let outside = (last.task_ms - last.sched_ms).max(0.0);
    t.render_ema_ms += (outside - t.render_ema_ms) * RENDER_EMA;
}

fn derive_budget_ms(t: &Tick, frame: f64, settled: bool) -> f64 {
    if settled {
        return SETTLED_BUDGET_MS;
    }
    MAX_BUDGET_MS.min(MIN_BUDGET_MS.max(frame - frame * MARGIN_SHARE - t.render_ema_ms))
}

fn run_phase(phase: Phase, ctx: &mut TickContext, t0: f64) {
    let start = now();
    // Index loop rather than an iterator: a step may unregister itself or a
    // sibling, and a snapshot would run something that just asked not to be.
    let mut i = 0;
    while i < tick().registry.len() {
        let (entry_phase, step, name) = {
            let r = &tick().registry[i];
            (r.phase, r.step, r.name)
        };
        i += 1;
        if entry_phase != phase {
            continue;
        }
        if !tick().step_timing {
            step(ctx);
        } else {
            let s0 = now();
            step(ctx);
            let ms = now() - s0;
            let key = format!("{}/{}", PHASES[phase], name);
            let stats = &mut tick().step_stats;
            let stat = match stats.iter().position(|s| s.name == key) {
                Some(p) => &mut stats[p],
                None => {
                    stats.push(StepStats { name: key, ..Default::default() });
                    stats.last_mut().expect("just pushed")
                }
            };
            stat.ms += ms;
            stat.count += 1;
            stat.max_ms = stat.max_ms.max(ms);
        }
        if ctx.moved {
            ctx.moved = false;
            tick().moved_at = t0;
        }
    }
    ctx.phase_ms[phase] = now() - start;
}

pub fn run_tick(state: &mut TickState, delta: f64) -> TickReport {
    let t0 = now();
    observe_render_cost(tick());
    state.invalidated = false;
    let mut ctx = TickContext {
        state,
        delta,
        // Sized properly once the interaction phase has said whether the view moved.
        budget: FrameBudget::new(0.0, t0),
        frame_ms: 0.0,
        settled: false,
        phase_ms: [0.0; 5],
        moved: false,
    };

    run_phase(INTERACTION, &mut ctx, t0);
    let t = tick();
    ctx.settled = t0 - t.moved_at >= SETTLE_MS;
    if !ctx.settled && !t.prev_settled {
        observe_frame_gap(t, delta);
    }
    t.prev_settled = ctx.settled;
    ctx.frame_ms = frame_ms(t);
    ctx.budget = FrameBudget::new(derive_budget_ms(t, ctx.frame_ms, ctx.settled), t0);
    ctx.budget.reserve_ms = ctx.budget.total_ms * CONTENT_SHARE;
    run_phase(VISIBLE, &mut ctx, t0);
    ctx.budget.reserve_ms = 0.0;
    for phase in [CONTENT, COSMETIC, REPORT] {
        run_phase(phase, &mut ctx, t0);
    }

    // A change made during the tick asks for the frame that delivers it.
    if take_tick_request() {
        ctx.state.invalidated = true;
    }
    let report = TickReport {
        budget_ms: ctx.budget.total_ms,
        frame_ms: ctx.frame_ms,
        settled: ctx.settled,
        phase_ms: ctx.phase_ms,
        total_ms: now() - t0,
        invalidated: ctx.state.invalidated,
    };
    tick().last_report = report;
    report
}
