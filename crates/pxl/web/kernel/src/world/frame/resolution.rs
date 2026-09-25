// pxl-kernel/world/frame/resolution.rs

//! How many device pixels the world is drawn at to a CSS pixel. The fill is
//! the GPU's largest cost and goes with the square of this, so it is the one
//! number that trades sharpness for frames most directly.
//!
//! `fixed` draws at the device's ratio, at most DPR_MAX, and `low` at one.
//! `auto` starts at the device's and moves a step at a time: down while the
//! frames run over their budget — the display's interval, but never tighter
//! than sixty frames a second — up again once they show room. What says a
//! frame ran over is the GPU's own time where the device reports it, and
//! where it does not, the interval the host measured, which only says a frame
//! was missed and never how much room there was — so there a step up is a
//! probe, and one the frames could not carry makes the next probe wait twice
//! as long.
//!
//! Only a moving view is judged: a still one draws nothing new, and its
//! frames say nothing about what a moving one costs.

use pxl_render::gpu::surface::{DPR_MAX, DPR_MIN};

use crate::global::Global;
use crate::sched::tick::TickState;
use crate::shell::setting::store::{settings_store, ResolutionMode};

/// One step up or down: a fifth to a quarter of the fill between the two
/// ends, small enough that a step is not seen as a jump.
const STEP: f64 = 0.25;
/// How long the frames run over before a step down: long enough that one
/// slow frame — a chunk's upload, a page arriving — is not taken for a load.
const OVER_HOLD_MS: f64 = 500.0;
/// How long the GPU's time stays under ROOM of the budget before a step up.
const ROOM_HOLD_MS: f64 = 2000.0;
/// The share of the budget under which the GPU has room for the next step:
/// a step up adds at most (2.0 / 1.75)² ≈ 1.3 times the fill, so half the
/// budget leaves the step room and the frame's other work its share.
const ROOM: f64 = 0.5;
/// An interval this many budgets long is a missed frame, where the device
/// reports no GPU time: a frame slips by whole vsyncs.
const MISSED: f64 = 1.5;
/// How long the frames hold their budget before a first probe up, where the
/// device reports no GPU time, and how long a probe waits at most after the
/// ones before it came back down.
const PROBE_HOLD_MS: f64 = 5000.0;
const PROBE_HOLD_MAX_MS: f64 = 60_000.0;
/// How long a probe up has to carry the frames before it is trusted.
const PROBE_TRUST_MS: f64 = 2000.0;
/// The frame the budget is at most as tight as: a sixtieth of a second. A
/// faster display's own interval would trade the device's sharpness for
/// frames past sixty, which the eye gains less from than it loses.
const BUDGET_FLOOR_MS: f64 = 1000.0 / 60.0;
/// The least time between two changes: a change makes the world's targets
/// again (pxl-render gpu/renderer.rs `targets`), which is work of its own.
const CHANGE_GAP_MS: f64 = 1000.0;

struct Resolution {
    ratio: f64,
    /// The GPU's time in the last frame it reported; none where it reports
    /// none.
    gpu_ms: Option<f64>,
    over_ms: f64,
    room_ms: f64,
    since_change_ms: f64,
    probe_hold_ms: f64,
    /// The last step up was a probe not yet trusted.
    probing: bool,
}

static RESOLUTION: Global<Resolution> = Global::new();

fn resolution() -> &'static mut Resolution {
    RESOLUTION.get(|| Resolution {
        ratio: 0.0,
        gpu_ms: None,
        over_ms: 0.0,
        room_ms: 0.0,
        since_change_ms: 0.0,
        probe_hold_ms: PROBE_HOLD_MS,
        probing: false,
    })
}

/// The GPU's time in the frame just drawn, as the renderer read it; 0 is
/// a device that reports none.
pub fn resolution_observe(gpu_ms: f64) {
    resolution().gpu_ms = (gpu_ms > 0.0).then_some(gpu_ms);
}

/// The ratio the world is drawn at this turn.
pub fn resolution_step(state: &TickState) -> f64 {
    let top = state.dpr.clamp(DPR_MIN, DPR_MAX);
    let r = resolution();
    match settings_store().resolution {
        ResolutionMode::fixed => r.ratio = top,
        ResolutionMode::low => r.ratio = DPR_MIN,
        ResolutionMode::auto => {
            r.ratio = if r.ratio > 0.0 { r.ratio.clamp(DPR_MIN, top) } else { top };
            if state.interactive {
                judge(r, top, state.gap_s * 1000.0, state.frame_ms.max(BUDGET_FLOOR_MS));
            }
        }
    }
    r.ratio
}

fn judge(r: &mut Resolution, top: f64, gap_ms: f64, budget_ms: f64) {
    r.since_change_ms += gap_ms;
    let (over, room) = match r.gpu_ms {
        Some(gpu) => (gpu > budget_ms, gpu < ROOM * budget_ms),
        None => (gap_ms > MISSED * budget_ms, gap_ms <= MISSED * budget_ms),
    };
    r.over_ms = if over { r.over_ms + gap_ms } else { 0.0 };
    r.room_ms = if room { r.room_ms + gap_ms } else { 0.0 };
    if r.probing && r.since_change_ms >= PROBE_TRUST_MS {
        r.probing = false;
    }
    if r.since_change_ms < CHANGE_GAP_MS {
        return;
    }
    if r.over_ms >= OVER_HOLD_MS && r.ratio > DPR_MIN {
        if r.probing {
            r.probe_hold_ms = (r.probe_hold_ms * 2.0).min(PROBE_HOLD_MAX_MS);
            r.probing = false;
        }
        change(r, (r.ratio - STEP).max(DPR_MIN));
    } else if r.ratio < top && r.room_ms >= if r.gpu_ms.is_some() { ROOM_HOLD_MS } else { r.probe_hold_ms } {
        r.probing = r.gpu_ms.is_none();
        change(r, (r.ratio + STEP).min(top));
    }
}

fn change(r: &mut Resolution, ratio: f64) {
    r.ratio = ratio;
    r.since_change_ms = 0.0;
    r.over_ms = 0.0;
    r.room_ms = 0.0;
}
