// pxl-kernel/domain/view/frame/resolution.rs

//! Device pixels per CSS pixel a surface is drawn at: the display's, and the
//! world's as the `resolution` setting says. The world's rays are cast at the
//! display's pixels times `scale` each way, and the renderer's finish brings
//! them up to the canvas's, the display's (pxl-render gpu/finish.rs): render
//! resolution apart from display resolution where a screen outgrows its GPU
//! (Unreal's screen percentage). `fixed` draws
//! whole, `low` a device pixel a CSS pixel, and `auto` one of `SCALE_STEPS`
//! steps between whole and `SCALE_MIN`, each a like share of the last, so
//! every move lays the pixels on one of a few grids.
//!
//! `auto` judges only frames that move: the gap before the first is how long
//! the view stood still, so moving starts a fresh window, and the quiet
//! windows judged before the pause still count. A window is
//! `SCALE_WINDOW_MS` of the moving frames' budgets, the paced interval the
//! host grants (xpute host sched/quantum.ts `Grant::paced_ms`) no tighter
//! than `SCALE_BUDGET_FLOOR_MS`, and its load is the samples its frames took,
//! summed against their own budgets. Where the device keeps timestamps the
//! sample is the GPU time of the passes drawn at the world's pixels
//! (pxl-render gpu/renderer.rs `px_ms`), from a read that landed, which
//! not every frame has, and a window none landed in is not judged. Timed, a
//! window over its budget by more than `SCALE_BAND` steps down at once, and
//! `SCALE_UP_WINDOWS` windows in a row in which the step above, cost times its
//! pixels over these, fits under it by that much step up, so the scale does
//! not swing between two steps. Where it keeps none the sample is the frame's
//! gap, which never falls below the paced interval, so room cannot be
//! foreseen: a window whose gaps average past `SCALE_MISSED` budgets steps
//! down, and a step up is a probe taken after a hold of quiet windows; one
//! that goes over within `SCALE_TRUST_WINDOWS` steps back and doubles the
//! hold, up to `SCALE_PROBE_WINDOWS_MAX`. A change rebuilds the targets, so
//! no frame is summed while it waits to be drawn, the `SCALE_SETTLE_WINDOWS`
//! after it are not judged, and it asks for the frame that draws it.

use crate::domain::view::param;
use crate::domain::view::setting::store::{settings_store, ResolutionMode};
use pxl_render::api_spec::surface_role;
use pxl_render::gpu::surface::{surfaces, Surface};
use xpute_guest::global::Global;

/// A frame as the turn paced it.
#[derive(Clone, Copy)]
pub struct Pace {
    /// The interval the host paced it at, in ms.
    pub paced_ms: f64,
    /// The real gap since the last frame, in ms.
    pub gap_ms: f64,
    /// The view moved (the host's `interactive`).
    pub moving: bool,
}

/// The step the world is drawn at, 0 whole, and the window being judged.
struct Scale {
    step: i32,
    /// The window's frames' budgets summed, in ms: its clock.
    window_ms: f64,
    /// The samples, and the budgets of the frames that took them, summed
    /// over the window, in ms: its load.
    spent_ms: f64,
    budget_ms: f64,
    /// Windows in a row with room: the step above would fit, or, untimed,
    /// none went over.
    quiet: u32,
    /// Windows ended since the last change.
    since: u32,
    /// Untimed: the quiet windows a probe up waits for, and whether the step
    /// is one on trial.
    hold: u32,
    probing: bool,
    moving: bool,
}

impl Scale {
    const NEW: Scale = Scale {
        step: 0,
        window_ms: 0.0,
        spent_ms: 0.0,
        budget_ms: 0.0,
        quiet: 0,
        since: u32::MAX,
        hold: param::SCALE_PROBE_WINDOWS,
        probing: false,
        moving: false,
    };

    /// A fresh window: its sums, not the quiet windows already judged, so a
    /// pause in moving neither breaks a run toward a step up nor a probe's
    /// hold.
    fn restart(&mut self) {
        (self.window_ms, self.spent_ms, self.budget_ms) = (0.0, 0.0, 0.0);
    }

    /// Counts one frame's budget to the window, and its sample, where it
    /// took one, against it; at a window's end, judges it. Whether the step
    /// changed.
    fn judge(&mut self, spent_ms: Option<f64>, budget_ms: f64, timed: bool) -> bool {
        self.window_ms += budget_ms;
        if let Some(spent_ms) = spent_ms {
            self.spent_ms += spent_ms;
            self.budget_ms += budget_ms;
        }
        if self.window_ms < param::SCALE_WINDOW_MS {
            return false;
        }
        let (spent, sampled) = (self.spent_ms, self.budget_ms);
        self.restart();
        self.since = self.since.saturating_add(1);
        if sampled == 0.0 {
            return false;
        }
        let load = spent / sampled;
        if self.since <= param::SCALE_SETTLE_WINDOWS {
            return false;
        }
        if self.probing && self.since > param::SCALE_TRUST_WINDOWS {
            self.probing = false;
        }
        let (over, room, hold) = if timed {
            // What the step above would cost: its pixels over these.
            let above = load * libm::pow(share(self.step - 1) / share(self.step), 2.0);
            (
                load > 1.0 + param::SCALE_BAND,
                above < 1.0 - param::SCALE_BAND,
                param::SCALE_UP_WINDOWS,
            )
        } else {
            let over = load > param::SCALE_MISSED;
            (over, !over, self.hold)
        };
        let step = if over && self.step < param::SCALE_STEPS {
            if self.probing {
                self.hold = (self.hold * 2).min(param::SCALE_PROBE_WINDOWS_MAX);
                self.probing = false;
            }
            self.step + 1
        } else if room && self.step > 0 {
            self.quiet += 1;
            if self.quiet < hold {
                return false;
            }
            self.probing = !timed;
            self.step - 1
        } else {
            self.quiet = 0;
            return false;
        };
        self.step = step;
        self.since = 0;
        self.quiet = 0;
        true
    }
}

static SCALE: Global<Scale> = Global::new();

fn state() -> &'static mut Scale {
    SCALE.get(|| Scale::NEW)
}

/// The share of the display's pixels, each way, step `step` draws at.
fn share(step: i32) -> f64 {
    libm::pow(
        param::SCALE_MIN,
        f64::from(step) / f64::from(param::SCALE_STEPS),
    )
}

/// The share of the display's pixels, each way, the world is drawn at on a
/// display of `device` pixels a CSS pixel: the setting's, `auto`'s by its step.
fn scale(device: f64) -> f64 {
    match settings_store().resolution {
        ResolutionMode::auto => share(state().step),
        ResolutionMode::fixed => 1.0,
        ResolutionMode::low => 1.0 / device,
    }
}

/// The share of the display's pixels, each way, the world was last drawn
/// at: its rays' width over the display's, which lags `scale` while a change
/// waits to be drawn; whole before it has any.
pub fn drawn_scale() -> f64 {
    world()
        .filter(|s| s.width > 0 && s.px_w > 0)
        .map_or(1.0, |s| {
            let dpr = if s.dpr > 0.0 { s.dpr } else { 1.0 };
            f64::from(s.width) / (f64::from(s.px_w) * device(s.dpr) / dpr)
        })
}

/// Device pixels per CSS pixel a display of `dpr` is drawn at whole: its
/// own. How many of them the eye resolves turns on how near it is held,
/// which no page is told: a phone's CSS pixel, held at a forearm, spans half
/// again the reference pixel's angle, so its display's three are about the
/// eye's limit there.
fn device(dpr: f64) -> f64 {
    dpr
}

fn world() -> Option<&'static mut Surface> {
    surfaces().find(|s| s.role == surface_role::WORLD)
}

/// The ratio a surface is drawn at: the display's; the world's scaled as the
/// setting says; a case's within its thumbnail.
pub fn surface_ratio(role: u32, px: [u32; 2], dpr: f64) -> f64 {
    let device = device(dpr);
    match role {
        surface_role::WORLD => device * scale(device),
        surface_role::CASE => {
            device.min(param::CASE_LONG_SIDE_MAX * dpr / f64::from(px[0].max(px[1]).max(1)))
        }
        _ => device,
    }
}

/// A frame was paced: `gpu`, where a GPU read landed with it, is the time of
/// its passes drawn at the world's pixels and those pixels. A frame drawn at
/// another size than now is not counted, and a read timed at one is no
/// sample; the world's surface takes the scale the setting and the judge
/// leave.
pub fn load(pace: Pace, gpu: Option<(f64, f64)>) {
    let Some(s) = world() else { return };
    let k = state();
    let started = pace.moving && !k.moving;
    k.moving = pace.moving;
    if settings_store().resolution != ResolutionMode::auto {
        (k.step, k.quiet) = (0, 0);
        k.restart();
    } else if started {
        k.restart();
    } else if pace.moving && pace.paced_ms > 0.0 && s.render_px() == (s.width, s.height) {
        let drawn = f64::from(s.width) * f64::from(s.height);
        let timed = pxl_render::gpu::api::device().is_some_and(|d| d.timestamps);
        let spent = if timed {
            gpu.filter(|&(ms, px)| ms > 0.0 && px > 0.0 && px == drawn)
                .map(|(ms, _)| ms)
        } else {
            Some(pace.gap_ms)
        };
        k.judge(
            spent,
            pace.paced_ms.max(param::SCALE_BUDGET_FLOOR_MS),
            timed,
        );
    }
    let ratio = surface_ratio(s.role, [s.px_w, s.px_h], s.dpr);
    if s.ratio != ratio {
        s.ratio = ratio;
        crate::sys::sched::edge::request_world_tick();
    }
}

#[cfg(test)]
#[path = "resolution.test.rs"]
mod test;
