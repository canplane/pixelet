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
use crate::param;
use crate::sched::tick::TickState;
use crate::shell::setting::store::{settings_store, ResolutionMode};

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
    /// The last turn was judged: this turn's gap is a frame's.
    moving: bool,
}

static RESOLUTION: Global<Resolution> = Global::new();

fn resolution() -> &'static mut Resolution {
    RESOLUTION.get(|| Resolution {
        ratio: 0.0,
        gpu_ms: None,
        over_ms: 0.0,
        room_ms: 0.0,
        since_change_ms: 0.0,
        probe_hold_ms: param::RESOLUTION_PROBE_HOLD_MS,
        probing: false,
        moving: false,
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
            // The gap before the first moving turn is how long the view
            // stood still, not how long a frame took, so it is no frame's
            // time: counted, it alone held a step down, or up, whenever the
            // view moved again. The holds are runs of moving frames, and a
            // stop ends them.
            match (state.interactive, r.moving) {
                (true, true) => judge(r, top, state.gap_s * 1000.0, state.frame_ms.max(param::RESOLUTION_BUDGET_FLOOR_MS)),
                (true, false) => (r.over_ms, r.room_ms) = (0.0, 0.0),
                _ => {}
            }
            r.moving = state.interactive;
        }
    }
    r.ratio
}

fn judge(r: &mut Resolution, top: f64, gap_ms: f64, budget_ms: f64) {
    r.since_change_ms += gap_ms;
    let (over, room) = match r.gpu_ms {
        Some(gpu) => (gpu > budget_ms, gpu < param::RESOLUTION_ROOM * budget_ms),
        None => (gap_ms > param::RESOLUTION_MISSED * budget_ms, gap_ms <= param::RESOLUTION_MISSED * budget_ms),
    };
    r.over_ms = if over { r.over_ms + gap_ms } else { 0.0 };
    r.room_ms = if room { r.room_ms + gap_ms } else { 0.0 };
    if r.probing && r.since_change_ms >= param::RESOLUTION_PROBE_TRUST_MS {
        r.probing = false;
    }
    if r.since_change_ms < param::RESOLUTION_CHANGE_GAP_MS {
        return;
    }
    if r.over_ms >= param::RESOLUTION_OVER_HOLD_MS && r.ratio > DPR_MIN {
        if r.probing {
            r.probe_hold_ms = (r.probe_hold_ms * 2.0).min(param::RESOLUTION_PROBE_HOLD_MAX_MS);
            r.probing = false;
        }
        change(r, (r.ratio - param::RESOLUTION_STEP).max(DPR_MIN));
    } else if r.ratio < top && r.room_ms >= if r.gpu_ms.is_some() { param::RESOLUTION_ROOM_HOLD_MS } else { r.probe_hold_ms } {
        r.probing = r.gpu_ms.is_none();
        change(r, (r.ratio + param::RESOLUTION_STEP).min(top));
    }
}

fn change(r: &mut Resolution, ratio: f64) {
    r.ratio = ratio;
    r.since_change_ms = 0.0;
    r.over_ms = 0.0;
    r.room_ms = 0.0;
}
