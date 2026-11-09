// pxl-core/domain/scene/resolution.rs

//! The device pixels per CSS pixel each surface is drawn at. A surface takes
//! the display's ratio, except the world, which the `resolution` setting may
//! scale down. The world's rays are cast at the display's pixels times the
//! scale on each axis, and the renderer's finish brings them up to the
//! display's pixels (pxl-render pass/finish.rs). Render resolution is so kept
//! apart from display resolution, as Unreal's screen percentage keeps it, for
//! a screen with more pixels than its GPU can fill.
//!
//! `fixed` draws the world whole and `low` draws it at `RES_MIN`; both turn at
//! the display's rate. `auto` chooses one of `RES_STEPS` steps between whole
//! and `RES_MIN`, each the same fraction of the one before, so the pixels lie
//! on one of a few grids. `auto` may also slow the turns down to
//! `RATE_MIN_HZ`.
//!
//! `auto` is a budget controller, in the manner of congestion control, with
//! one budget and two knobs. The rate knob is the interval between turns, one
//! display refresh a step. It saves no work in a frame, but it gives all of a
//! frame's work more time. The scale knob saves only the work done once for
//! each drawn pixel (span.json `per_px`). The camera grid's cells, the
//! samples, the light's passes over its map's texels, the finish and the
//! markers cost the same at every scale. The runtime owns the rate: xpute's `SYS_PACE`
//! asks it for one, each turn's SYS_FRAME reports the rate in force (`paced_ms`),
//! and the judge keeps no copy of it.

use crate::domain::scene::param;
use crate::domain::setting::store::{settings, ResMode};
use crate::sys::sched::edge;
use crate::sys::sched::pacing_spec::{PACE_HZ, RATE_MIN_HZ};
use pxl_render::api_spec::surface_role;
use pxl_render::gpu::api;
use pxl_render::gpu::surface::{surfaces, Surface};
use pxl_render::renderer::GpuRead;
use xpute_core::global::Global;

/// How the turn paced a frame.
#[derive(Clone, Copy)]
pub struct Pace {
    /// The display's refresh interval, in ms.
    pub refresh_ms: f64,
    /// The interval the runtime paced the frame at, in ms. It is a whole
    /// number of refreshes.
    pub paced_ms: f64,
    /// The real time since the last frame, in ms.
    pub gap_ms: f64,
    /// Whether the view moved (the runtime's `interactive`).
    pub moving: bool,
}

impl Pace {
    /// The tolerance the runtime allows for the spread of its measure of the
    /// refresh, an eighth of a refresh (@xpute/runtime sched/frame_pacing.ts
    /// `pace_ms`).
    fn spread(&self) -> f64 {
        self.refresh_ms / 8.0
    }

    /// Whether turns run slower than the display's own rate. They do when the
    /// paced interval is longer than one interval at `PACE_HZ`, with the
    /// spread allowed, and longer than one refresh.
    fn slowed(&self) -> bool {
        self.paced_ms > (1000.0 / PACE_HZ + self.spread()).max(self.refresh_ms)
    }

    /// Whether one more refresh a turn still keeps the rate at or above
    /// `RATE_MIN_HZ`.
    fn may_slow(&self) -> bool {
        self.paced_ms + self.refresh_ms <= 1000.0 / RATE_MIN_HZ + self.spread()
    }
}

/// The budget, in ms, of a frame whose turns are `paced_ms` apart.
fn budget(paced_ms: f64) -> f64 {
    paced_ms.max(param::RES_BUDGET_FLOOR_MS)
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

/// The two knobs: the interval between turns in ms, and the scale's step,
/// where step 0 draws whole.
type Setpoint = (f64, i32);

/// `auto`'s judge: the scale's step and the window being judged.
///
/// It judges only frames in which the view moves. The gap before the first
/// moving frame is how long the view stood still, so motion starts a fresh
/// window, while the quiet windows judged before the pause still count. A
/// window lasts until its frames' budgets sum to `RES_WINDOW_MS`. A frame's
/// budget is the interval the runtime paced it at (@xpute/runtime
/// sched/frame_pacing.ts `FrameGrant::paced_ms`), and never less than
/// `RES_BUDGET_FLOOR_MS`. A window's load is its samples summed over the
/// budgets of the frames that took them.
///
/// Where the device keeps GPU timestamps, a sample is a GPU read that landed,
/// which not every frame has, timed at the pixels drawn now. It splits into P,
/// the time of the per-pixel spans (pxl-render renderer.rs
/// `GpuRead::px_ms`), and F, the rest of the GPU's busy time. A window in
/// which no read landed is not judged. A window whose load F + P is over its
/// budget by more than `RES_BAND` steps down at once. The judge steps up after
/// `RES_UP_WINDOWS` windows in a row in which the step above would fit under
/// its budget by `RES_BAND` as well; the band keeps the knobs from swinging
/// between two steps. The load predicted at the step above keeps F, takes P
/// times the square of that step's share over this one's, and is measured
/// against the budget of that step's interval. Down, the rate moves first, to
/// `RATE_MIN_HZ`, and then the scale, to `RES_MIN`. Up, the scale moves first,
/// then the rate.
///
/// Where the device keeps no timestamps, a sample is the frame's gap. A gap
/// never falls below the paced interval, so the room a step up would have
/// cannot be foreseen, and only the scale moves. A window whose gaps average
/// more than `SCALE_MISSED` budgets steps down, and a step up waits for
/// `SCALE_PROBE_WINDOWS` quiet windows.
///
/// Either way, a step up is on trial. One that goes over within
/// `RES_TRUST_WINDOWS` steps back down and doubles the wait for the next step
/// up, to at most `RES_UP_WINDOWS_MAX`; this stops two steps swinging where a
/// prediction falls a little short. A step that outlasts its trial stands, and
/// the wait returns to its first length.
///
/// A change moves one knob by one step. A change of scale rebuilds the
/// render targets, so no frame is counted until one is drawn at the new size,
/// and it asks for that frame. A change of rate holds from the next grant. The
/// `RES_SETTLE_WINDOWS` windows after any change are not judged.
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

    /// The knobs one step down, or None when neither can go lower.
    fn down(&self, pace: &Pace, timed: bool) -> Option<Setpoint> {
        if timed && pace.may_slow() {
            Some((pace.paced_ms + pace.refresh_ms, self.step))
        } else if self.step < param::RES_STEPS {
            Some((pace.paced_ms, self.step + 1))
        } else {
            None
        }
    }

    /// The knobs one step up, or None when the world is drawn whole at the
    /// display's rate.
    fn up(&self, pace: &Pace) -> Option<Setpoint> {
        if self.step > 0 {
            Some((pace.paced_ms, self.step - 1))
        } else if pace.slowed() {
            Some((pace.paced_ms - pace.refresh_ms, self.step))
        } else {
            None
        }
    }

    /// The number of windows with room that a step up waits for.
    fn hold(&self, timed: bool) -> u32 {
        let wait = if timed {
            param::RES_UP_WINDOWS
        } else {
            param::SCALE_PROBE_WINDOWS
        };
        (wait * self.backoff).min(param::RES_UP_WINDOWS_MAX)
    }

    /// The load that frames paced as `pace`, which cost `cost` over budgets
    /// summing to `budget_ms`, would have at the knobs `(paced_ms, step)`. The
    /// rest stays as it is, the per-pixel time scales by the pixel count there
    /// over here, and the budget by a frame's budget there over here.
    fn predicted(
        &self,
        cost: Cost,
        budget_ms: f64,
        pace: &Pace,
        (paced_ms, step): Setpoint,
    ) -> f64 {
        let px = libm::pow(share(step) / share(self.step), 2.0);
        let rate = budget(pace.paced_ms) / budget(paced_ms);
        (cost.rest_ms + cost.px_ms * px) * rate / budget_ms
    }

    /// Adds one frame to the window: its budget always, and its sample if it
    /// took one. At the window's end it judges the window and moves at most
    /// one knob by one step. It returns the knobs it moved to, if it moved
    /// one.
    fn judge(&mut self, cost: Option<Cost>, pace: &Pace, timed: bool) -> Option<Setpoint> {
        let budget_ms = budget(pace.paced_ms);
        self.window_ms += budget_ms;
        if let Some(cost) = cost {
            self.cost.add(cost);
            self.budget_ms += budget_ms;
        }
        if self.window_ms < param::RES_WINDOW_MS {
            return None;
        }
        let (cost, sampled) = (self.cost, self.budget_ms);
        self.restart();
        self.since = self.since.saturating_add(1);
        if sampled == 0.0 {
            return None;
        }
        let here = (pace.paced_ms, self.step);
        let load = self.predicted(cost, sampled, pace, here);
        if self.since <= param::RES_SETTLE_WINDOWS {
            return None;
        }
        if self.on_trial && self.since > param::RES_TRUST_WINDOWS {
            self.on_trial = false;
            self.backoff = 1;
        }
        let (down, up) = (self.down(pace, timed), self.up(pace));
        let (over, room) = if timed {
            (
                load > 1.0 + param::RES_BAND,
                up.is_some_and(|u| self.predicted(cost, sampled, pace, u) < 1.0 - param::RES_BAND),
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
                    return None;
                }
                self.on_trial = true;
                to
            }
            _ => {
                self.quiet = 0;
                return None;
            }
        };
        (_, self.step) = to;
        self.since = 0;
        self.quiet = 0;
        Some(to)
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
        f64::from(step) / f64::from(param::RES_STEPS),
    )
}

/// The share of the display's pixels on each axis that the setting, and
/// under `auto` the judge's step, asks the world to be drawn at.
fn scale() -> f64 {
    match settings().res {
        ResMode::auto => share(state().step),
        ResMode::fixed => 1.0,
        ResMode::low => param::RES_MIN,
    }
}

/// The share of the display's pixels on each axis that the world was last
/// drawn at. It lags `scale` while a change waits to be drawn, and it is 1
/// before the world has a surface.
pub fn drawn_scale() -> f64 {
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
pub fn dpr() -> f64 {
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
pub fn surface_ratio(role: u32, px: [u32; 2], dpr: f64) -> f64 {
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
/// a sample. It returns the interval to ask the runtime for when the rate
/// changes. Under any setting but `auto` that is 0 while the turns are
/// slowed, which asks for the runtime's own rate.
pub fn load(pace: Pace, gpu: Option<GpuRead>) -> Option<f64> {
    let s = world()?;
    let mut interval_ms = None;
    let k = state();
    let started = pace.moving && !k.moving;
    k.moving = pace.moving;
    if settings().res != ResMode::auto {
        (k.step, k.quiet) = (0, 0);
        k.restart();
        interval_ms = pace.slowed().then_some(0.0);
    } else if started {
        k.restart();
    } else if pace.moving && pace.paced_ms > 0.0 && s.render_px() == (s.width, s.height) {
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
        interval_ms = k
            .judge(cost, &pace, timed)
            .map(|(paced_ms, ..)| paced_ms)
            .filter(|&paced_ms| paced_ms != pace.paced_ms);
    }
    let ratio = surface_ratio(s.role, [s.px_w, s.px_h], s.dpr);
    if s.ratio != ratio {
        s.ratio = ratio;
        edge::request_world_tick();
    }
    interval_ms
}

#[cfg(test)]
#[path = "resolution.test.rs"]
mod test;
