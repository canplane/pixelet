// pxl-core/domain/scene/resolution.rs

//! The device pixels per CSS pixel each surface is drawn at. A surface takes
//! the display's ratio, except the world, which the `resolution` setting may
//! scale down. The world's rays are cast at the display's pixels times the
//! scale on each axis, and the renderer's finish brings them up to the
//! display's pixels (pxl-render pass/finish.rs). Render resolution is so kept
//! apart from display resolution, as Unreal's screen percentage keeps it, for
//! a screen with more pixels than its GPU can fill.
//!
//! `fixed` draws the world whole and `low` draws it at `RES_MIN`. `auto`
//! chooses one of `RES_STEP_CNT` steps between whole and `RES_MIN`, each the same
//! fraction of the one before, so the pixels lie on one of a few grids.
//!
//! `auto` is a budget controller, in the manner of congestion control. Its
//! budget is the frame's target interval, a whole number of the display's
//! measured intervals, which `auto`'s frame rate sets apart (rate.rs). Its
//! knob, the scale, saves only the work done once for each drawn pixel
//! (span.json5 `GPU_SPAN_PER_PX`). The camera grid's cells, the samples, the light's
//! passes over its map's texels, the finish and the markers cost the same at
//! every scale.

use crate::domain::scene::param;
use crate::domain::scene::rate;
use crate::domain::setting::{settings, ResMode};
use crate::sys::sched;
use pxl_render::gpu::api;
use pxl_render::gpu::surface::{surfaces, Surface};
use pxl_render::renderer::GpuRead;
use pxl_render::spec::api::surface_role;
use pxl_global::Global;

/// How a frame was paced.
#[derive(Clone, Copy)]
pub(crate) struct Pace {
    /// The real time since the last frame, in ms.
    pub gap_ms: f64,
    /// The display's interval, in ms (sys/sched/display.rs).
    pub display_ms: f64,
    /// Whether the frame asked for the next (sys/sched.rs `invalidate`).
    pub moving: bool,
}

/// The budget, in ms, of a frame at the frame rate's target interval.
fn budget(display_ms: f64) -> f64 {
    f64::from(rate::interval()) * display_ms
}

/// The judge's sample of a frame's cost, in ms. `px_ms` is the time the scale
/// can save and `rest_ms` the time it cannot. Without GPU timestamps the
/// frame's whole gap counts as `rest_ms`.
#[derive(Clone, Copy, Default)]
struct Cost {
    px_ms: f64,
    rest_ms: f64,
}

impl Cost {
    fn add(&mut self, o: Cost) {
        self.px_ms += o.px_ms;
        self.rest_ms += o.rest_ms;
    }
}

/// `auto`'s judge: the scale's step and the window being judged.
///
/// It judges only frames in which the view moves. The gap before the first
/// moving frame is how long the view stood still, so motion starts a fresh
/// window, while the quiet windows judged before the pause still count. A
/// window lasts until its frames' budgets sum to `RES_WINDOW_MS`. A frame's
/// budget is its target interval (`budget`). A window's load is its samples
/// summed over the budgets of the frames that took them.
///
/// Where the device keeps GPU timestamps, a sample is a GPU read that landed,
/// which not every frame has, timed at the pixels drawn now. It splits into P,
/// the time of the per-pixel spans (pxl-render renderer.rs
/// `GpuRead::px_ms`), and F, the rest of the GPU's busy time. A window in
/// which no read landed is not judged. A window whose load F + P is over its
/// budget by more than `RES_BAND` steps down at once. The judge steps up after
/// `RES_UP_WINDOW_CNT` windows in a row in which the step above would fit under
/// its budget by `RES_BAND` as well; the band keeps the scale from swinging
/// between two steps. The load predicted at the step above keeps F and takes P
/// times the square of that step's share over this one's.
///
/// Where the device keeps no timestamps, a sample is the frame's gap. A gap
/// never falls below the budget, so the room a step up would have cannot be
/// foreseen. A window whose gaps average more than `SCALE_MISSED` budgets
/// steps down, and a step up waits for `SCALE_PROBE_WINDOW_CNT` quiet windows.
///
/// Either way, a step up is on trial. One that goes over within
/// `RES_TRUST_WINDOW_CNT` steps back down and doubles the wait for the next step
/// up, to at most `RES_UP_WINDOW_CAP`; this stops two steps swinging where a
/// prediction falls a little short. A step that outlasts its trial stands, and
/// the wait returns to its first length.
///
/// A change moves the scale by one step. It rebuilds the render targets, so
/// no frame is counted until one is drawn at the new size, and it asks for
/// that frame. The `RES_SETTLE_WINDOW_CNT` windows after a change are not
/// judged.
struct Judge {
    step: i32,
    /// The sum of the window's frames' budgets, in ms, which is the window's
    /// clock.
    window_ms: f64,
    /// The window's samples summed, in ms. Over `budget_ms`, the budgets of
    /// the frames that took them, they give the window's load.
    cost: Cost,
    budget_ms: f64,
    /// The windows in a row with room: the step above would have fit, or,
    /// without timestamps, no window went over.
    quiet: u32,
    /// The windows ended since the last change.
    since: u32,
    /// The factor on the wait for a step up. Each step up that fails its
    /// trial doubles it.
    backoff: u32,
    /// Whether the step in force is a step up still on trial.
    on_trial: bool,
    moving: bool,
}

impl Judge {
    const NEW: Judge = Judge {
        step: 0,
        window_ms: 0.0,
        cost: Cost {
            px_ms: 0.0,
            rest_ms: 0.0,
        },
        budget_ms: 0.0,
        quiet: 0,
        since: u32::MAX,
        backoff: 1,
        on_trial: false,
        moving: false,
    };

    /// Starts a fresh window. It clears the window's sums but keeps the count
    /// of quiet windows already judged, so a pause in motion breaks neither a
    /// run toward a step up nor the wait before a probe.
    fn restart(&mut self) {
        (self.window_ms, self.cost, self.budget_ms) = (0.0, Cost::default(), 0.0);
    }

    /// The step down, or None at `RES_MIN`.
    fn down(&self) -> Option<i32> {
        (self.step < param::RES_STEP_CNT).then_some(self.step + 1)
    }

    /// The step up, or None when the world is drawn whole.
    fn up(&self) -> Option<i32> {
        (self.step > 0).then_some(self.step - 1)
    }

    /// The number of windows with room that a step up waits for.
    fn hold(&self, timed: bool) -> u32 {
        let wait = if timed {
            param::RES_UP_WINDOW_CNT
        } else {
            param::SCALE_PROBE_WINDOW_CNT
        };
        (wait * self.backoff).min(param::RES_UP_WINDOW_CAP)
    }

    /// The load that frames which cost `cost` over budgets summing to
    /// `budget_ms` would have at `step`. The rest stays as it is, and the
    /// per-pixel time scales by the pixel count there over here.
    fn predicted(&self, cost: Cost, budget_ms: f64, step: i32) -> f64 {
        let px = libm::pow(share(step) / share(self.step), 2.0);
        (cost.rest_ms + cost.px_ms * px) / budget_ms
    }

    /// Adds one frame to the window: its budget always, and its sample if it
    /// took one. At the window's end it judges the window and moves the scale
    /// by at most one step.
    fn judge(&mut self, cost: Option<Cost>, pace: &Pace, timed: bool) {
        let budget_ms = budget(pace.display_ms);
        self.window_ms += budget_ms;
        if let Some(cost) = cost {
            self.cost.add(cost);
            self.budget_ms += budget_ms;
        }
        if self.window_ms < param::RES_WINDOW_MS {
            return;
        }
        let (cost, sampled) = (self.cost, self.budget_ms);
        self.restart();
        self.since = self.since.saturating_add(1);
        if sampled == 0.0 {
            return;
        }
        let load = self.predicted(cost, sampled, self.step);
        if self.since <= param::RES_SETTLE_WINDOW_CNT {
            return;
        }
        if self.on_trial && self.since > param::RES_TRUST_WINDOW_CNT {
            self.on_trial = false;
            self.backoff = 1;
        }
        let (down, up) = (self.down(), self.up());
        let (over, room) = if timed {
            (
                load > 1.0 + param::RES_BAND,
                up.is_some_and(|u| self.predicted(cost, sampled, u) < 1.0 - param::RES_BAND),
            )
        } else {
            let over = load > param::SCALE_MISSED;
            (over, !over)
        };
        let to = match (down, up) {
            (Some(to), _) if over => {
                if self.on_trial {
                    self.backoff = self.backoff.saturating_mul(2);
                    self.on_trial = false;
                }
                to
            }
            (_, Some(to)) if room => {
                self.quiet += 1;
                if self.quiet < self.hold(timed) {
                    return;
                }
                self.on_trial = true;
                to
            }
            _ => {
                self.quiet = 0;
                return;
            }
        };
        self.step = to;
        self.since = 0;
        self.quiet = 0;
    }
}

static JUDGE: Global<Judge> = Global::new();

fn state() -> &'static mut Judge {
    JUDGE.get(|| Judge::NEW)
}

/// The share of the display's pixels on each axis that step `step` draws at.
fn share(step: i32) -> f64 {
    libm::pow(
        param::RES_MIN,
        f64::from(step) / f64::from(param::RES_STEP_CNT),
    )
}

/// The share of the display's pixels on each axis that the setting, and
/// under `auto` the judge's step, asks the world to be drawn at.
fn scale() -> f64 {
    match settings().res {
        ResMode::Auto => share(state().step),
        ResMode::Fixed => 1.0,
        ResMode::Low => param::RES_MIN,
    }
}

/// The share of the display's pixels on each axis that the world was last
/// drawn at. It lags `scale` while a change waits to be drawn, and it is 1
/// before the world has a surface.
pub(crate) fn drawn_scale() -> f64 {
    world()
        .filter(|s| s.width > 0 && s.px_w > 0)
        .map_or(1.0, |s| {
            let dpr = if s.dpr > 0.0 { s.dpr } else { 1.0 };
            f64::from(s.width) / (f64::from(s.px_w) * device(s.dpr) / dpr)
        })
}

/// The device pixels per CSS pixel at which a display of `dpr` is drawn
/// whole.
fn device(dpr: f64) -> f64 {
    dpr
}

/// Device pixels per CSS pixel of the display the world is shown on, as the
/// world's surface reports it. It is 1 before there is a surface.
pub(crate) fn dpr() -> f64 {
    world()
        .map(|s| s.dpr)
        .filter(|&dpr| dpr > 0.0)
        .map_or(1.0, device)
}

fn world() -> Option<&'static mut Surface> {
    surfaces().find(|s| s.role == surface_role::WORLD)
}

/// The device pixels per CSS pixel a surface is drawn at. A surface takes the
/// display's ratio, the world scales it as the setting says, and a character
/// surface caps it so that the surface stays thumbnail size
/// (`CHARACTER_LONG_SIDE_MAX`).
pub(crate) fn surface_ratio(role: u32, px: [u32; 2], dpr: f64) -> f64 {
    let device = device(dpr);
    match role {
        surface_role::WORLD => device * scale(),
        surface_role::CHARACTER => {
            device.min(param::CHARACTER_LONG_SIDE_MAX * dpr / f64::from(px[0].max(px[1]).max(1)))
        }
        _ => device,
    }
}

/// Counts a paced frame toward the judge, and sets the world surface's ratio
/// to the scale the setting and the judge leave. `gpu` is what a GPU read that
/// landed with the frame timed. A frame drawn at another size than the
/// current one is not counted, and a read timed at another pixel count is not
/// a sample.
pub(crate) fn load(pace: Pace, gpu: Option<GpuRead>) {
    let Some(s) = world() else {
        return;
    };
    let k = state();
    let started = pace.moving && !k.moving;
    k.moving = pace.moving;
    if settings().res != ResMode::Auto {
        (k.step, k.quiet) = (0, 0);
        k.restart();
    } else if started {
        k.restart();
    } else if pace.moving && pace.display_ms > 0.0 && s.render_px() == (s.width, s.height) {
        let px = f64::from(s.width) * f64::from(s.height);
        let timed = api::device().is_some_and(|d| d.timestamps);
        let cost = if timed {
            gpu.filter(|t| t.busy_ms > 0.0 && t.px > 0.0 && t.px == px)
                .map(|t| {
                    let px_ms = t.px_ms();
                    Cost {
                        px_ms,
                        rest_ms: (t.busy_ms - px_ms).max(0.0),
                    }
                })
        } else {
            Some(Cost {
                rest_ms: pace.gap_ms,
                ..Cost::default()
            })
        };
        k.judge(cost, &pace, timed);
    }
    let ratio = surface_ratio(s.role, [s.px_w, s.px_h], s.dpr);
    if s.ratio != ratio {
        s.ratio = ratio;
        sched::invalidate();
    }
}

#[cfg(test)]
#[path = "resolution.test.rs"]
mod test;
