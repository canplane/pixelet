// pxl-kernel/world/frame/resolution.rs

//! Device pixels per CSS pixel the world is drawn at; fill cost goes with
//! its square. `auto` steps down while moving frames run over budget and up
//! once they show room. Without GPU timing a step up is a probe, and a
//! failed probe doubles the wait before the next. Still frames are not judged.

use pxl_render::gpu::surface::{DPR_MAX, DPR_MIN};

use crate::param;
use crate::sched::tick::TickState;
use crate::shell::setting::store::{settings_store, ResolutionMode};
use xpute_runtime::global::Global;

struct Resolution {
    ratio: f64,
    /// None where the device reports no GPU time.
    gpu_ms: Option<f64>,
    over_ms: f64,
    room_ms: f64,
    since_change_ms: f64,
    probe_hold_ms: f64,
    probing: bool,
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

/// 0 is a device that reports none.
pub fn resolution_observe(gpu_ms: f64) {
    resolution().gpu_ms = (gpu_ms > 0.0).then_some(gpu_ms);
}

pub fn resolution_step(state: &TickState) -> f64 {
    let top = state.dpr.clamp(DPR_MIN, DPR_MAX);
    let r = resolution();
    match settings_store().resolution {
        ResolutionMode::fixed => r.ratio = top,
        ResolutionMode::low => r.ratio = DPR_MIN,
        ResolutionMode::auto => {
            r.ratio = if r.ratio > 0.0 { r.ratio.clamp(DPR_MIN, top) } else { top };
            // The gap before the first moving turn is how long the view stood still,
            // not a frame's time; counting it alone triggered a step whenever the view
            // moved again.
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
