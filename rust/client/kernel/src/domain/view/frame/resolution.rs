// pxl-kernel/domain/view/frame/resolution.rs

//! Device pixels per CSS pixel a surface is drawn at: the display's, and the
//! world's as the `resolution` setting says. The world's rays are cast at the
//! display's pixels times `scale` each way, and the renderer's finish brings
//! them up to the canvas's, the display's (pxl-render gpu/finish.rs): render
//! resolution apart from display resolution where a screen outgrows its GPU
//! (Unreal's screen percentage). `fixed` draws
//! whole, `low` at `RES_MIN`, `auto`'s floor, each with the camera grid's
//! beam rays on the cell's own lattice and turns at the display's rate, and
//! `auto` one of `RES_STEPS` steps between whole and `RES_MIN`, each a like
//! share of the last, so every move lays the pixels on one of a few grids, a
//! lattice up to `grid::LATTICE_MAX`, and a rate down to `RATE_MIN_HZ`.
//!
//! `auto` is a budget controller, as congestion control is, over three
//! knobs and the one budget: the rate, the interval it asks between turns, a
//! refresh a step, which buys back no work a frame but gives every frame's
//! work, whatever it runs over, more time; the scale, which buys back the
//! work done a drawn pixel at a time (span.json `per_px`); and the camera
//! grid's lattice, the beam rays and cells laid `lattice` cells apart
//! (pxl-engine `beam::Grid::of`, span.json `per_cell`), which buys back
//! theirs at a quarter at the coarsest, `grid::LATTICE_MAX`. The samples are
//! the very rays at every lattice, so the lattice changes no pixel drawn; the
//! samples, the light's passes, on its map's texels, and the finish and the
//! markers, at the display's pixels, cost what they cost at the scale's or
//! the lattice's knob. The rate is the runtime's: `TURN_PACE` asks it, each
//! turn's grant says what it is (`paced_ms`), and the judge keeps no copy.
//!
//! It judges only frames that move: the gap before the first is how long
//! the view stood still, so moving starts a fresh window, and the quiet
//! windows judged before the pause still count. A window is
//! `RES_WINDOW_MS` of the moving frames' budgets, the paced interval the
//! runtime grants (@xpute/runtime sched/frame_pacing.ts `FrameGrant::paced_ms`) no tighter
//! than `RES_BUDGET_FLOOR_MS`, and its load is the samples its frames took,
//! summed against their own budgets. Where the device keeps timestamps a
//! sample is a GPU read's that landed, which not every frame has, timed at
//! the pixels and the lattice drawn now: its per-pixel spans' time P
//! (pxl-render gpu/renderer.rs `px_ms`), its lattice's G (`cell_ms`) and
//! the rest of the GPU's busy time F; a window none landed in is not
//! judged. Timed, a window whose load, F + P + G, is over its budget by more
//! than `RES_BAND` steps down at once, and `RES_UP_WINDOWS` windows in a
//! row in which the step above, F + P·(its share over this one)² + G·(this
//! lattice over its)², fits under its budget, this one's by the interval
//! there over the interval here, by that much step up, so the knobs do not
//! swing between two steps. Down, the rate goes first, to `RATE_MIN_HZ`, then
//! the scale, to `RES_MIN`, then the lattice; up, the lattice first, then the
//! scale, then the rate. Where it keeps
//! none the sample is the frame's gap, which never falls below the paced
//! interval, so room cannot be foreseen and only the scale moves: a window
//! whose gaps average past `SCALE_MISSED` budgets steps down, and a step up
//! waits for `SCALE_PROBE_WINDOWS` quiet ones. Either way a step up is on
//! trial: one that goes over within `RES_TRUST_WINDOWS` steps back and doubles
//! the wait for the next, up to `RES_UP_WINDOWS_MAX`, which is what stops two
//! steps swinging where a prediction falls a little short, and one that
//! outlasts them stands and the wait is as it was. A change moves one knob a step: the scale's
//! rebuilds the targets, so no frame is summed while it waits to be drawn,
//! the lattice's is laid by the next cast, the frames that stand on the
//! last keeping its grid (pxl-render gpu/ray/cast.rs `prepare`), and either
//! asks for the frame that draws it; the rate's holds from the next grant.
//! The `RES_SETTLE_WINDOWS` after any are not judged.

use crate::domain::view::param;
use crate::domain::view::setting::store::{settings_store, ResMode};
use crate::sys::sched::edge;
use crate::sys::sched::pacing_spec::{PACE_HZ, RATE_MIN_HZ};
use pxl_engine::grid::LATTICE_MAX;
use pxl_render::api_spec::surface_role;
use pxl_render::gpu::api;
use pxl_render::gpu::surface::{surfaces, Surface};
use xpute_core::global::Global;

/// A frame as the turn paced it.
#[derive(Clone, Copy)]
pub struct Pace {
    /// The display's refresh, in ms.
    pub refresh_ms: f64,
    /// The interval the runtime paced it at, in ms, a whole number of
    /// refreshes.
    pub paced_ms: f64,
    /// The real gap since the last frame, in ms.
    pub gap_ms: f64,
    /// The view moved (the runtime's `interactive`).
    pub moving: bool,
}

impl Pace {
    /// The refresh's eighth the runtime allows for the spread of its measure
    /// (@xpute/runtime sched/frame_pacing.ts `pace_ms`).
    fn spread(&self) -> f64 {
        self.refresh_ms / 8.0
    }

    /// Whether turns go slower than the display's own rate: past the most
    /// refreshes within one interval at `PACE_HZ`, one at least.
    fn slowed(&self) -> bool {
        self.paced_ms > (1000.0 / PACE_HZ + self.spread()).max(self.refresh_ms)
    }

    /// Whether another refresh a turn keeps within `RATE_MIN_HZ`.
    fn may_slow(&self) -> bool {
        self.paced_ms + self.refresh_ms <= 1000.0 / RATE_MIN_HZ + self.spread()
    }
}

/// The frame's budget turns `paced_ms` apart take, in ms.
fn budget(paced_ms: f64) -> f64 {
    paced_ms.max(param::RES_BUDGET_FLOOR_MS)
}

/// A frame as a GPU read that landed timed it (pxl-render gpu/renderer.rs
/// `FrameStat`): its per-pixel spans' time (`px_ms`), its lattice's
/// (`cell_ms`) and its busy time, in ms, and the world's pixels and the
/// lattice it was drawn at.
#[derive(Clone, Copy)]
pub struct GpuMeasure {
    pub px_ms: f64,
    pub cell_ms: f64,
    pub busy_ms: f64,
    pub px: f64,
    pub lattice: f64,
}

/// A frame's cost, the judge's sample of it, in ms: the time the scale buys
/// back, the time the lattice buys back, and the rest, which neither does;
/// untimed, the gap alone, as the rest.
#[derive(Clone, Copy, Default)]
struct Cost {
    px_ms: f64,
    cell_ms: f64,
    rest_ms: f64,
}

impl Cost {
    fn add(&mut self, o: Cost) {
        self.px_ms += o.px_ms;
        self.cell_ms += o.cell_ms;
        self.rest_ms += o.rest_ms;
    }
}

/// The knobs: the interval between turns, in ms, the scale's step, 0 whole,
/// and the lattice, 1 the cell's own.
type Knobs = (f64, i32, u32);

/// The scale's and the lattice's knobs, and the window being judged.
struct Judge {
    step: i32,
    lattice: u32,
    /// The window's frames' budgets summed, in ms: its clock.
    window_ms: f64,
    /// The samples, and the budgets of the frames that took them, summed
    /// over the window, in ms: its load.
    cost: Cost,
    budget_ms: f64,
    /// Windows in a row with room: the step above would fit, or, untimed,
    /// none went over.
    quiet: u32,
    /// Windows ended since the last change.
    since: u32,
    /// How many times over the wait for a step up runs, for the steps up
    /// that fell back, and whether the step is one on trial.
    backoff: u32,
    on_trial: bool,
    moving: bool,
}

impl Judge {
    const NEW: Judge = Judge {
        step: 0,
        lattice: 1,
        window_ms: 0.0,
        cost: Cost {
            px_ms: 0.0,
            cell_ms: 0.0,
            rest_ms: 0.0,
        },
        budget_ms: 0.0,
        quiet: 0,
        since: u32::MAX,
        backoff: 1,
        on_trial: false,
        moving: false,
    };

    /// A fresh window: its sums, not the quiet windows already judged, so a
    /// pause in moving neither breaks a run toward a step up nor a probe's
    /// hold.
    fn restart(&mut self) {
        (self.window_ms, self.cost, self.budget_ms) = (0.0, Cost::default(), 0.0);
    }

    /// The knobs a step down: timed, the rate's, to `RATE_MIN_HZ`; the
    /// scale's, to `RES_MIN`; then, timed, the lattice's; none past all.
    fn down(&self, pace: &Pace, timed: bool) -> Option<Knobs> {
        if timed && pace.may_slow() {
            Some((pace.paced_ms + pace.refresh_ms, self.step, self.lattice))
        } else if self.step < param::RES_STEPS {
            Some((pace.paced_ms, self.step + 1, self.lattice))
        } else if timed && self.lattice < LATTICE_MAX {
            Some((pace.paced_ms, self.step, self.lattice * 2))
        } else {
            None
        }
    }

    /// The knobs a step up: the lattice's, then the scale's, then the rate's;
    /// none at all whole.
    fn up(&self, pace: &Pace) -> Option<Knobs> {
        if self.lattice > 1 {
            Some((pace.paced_ms, self.step, self.lattice / 2))
        } else if self.step > 0 {
            Some((pace.paced_ms, self.step - 1, self.lattice))
        } else if pace.slowed() {
            Some((pace.paced_ms - pace.refresh_ms, self.step, self.lattice))
        } else {
            None
        }
    }

    /// The windows with room a step up waits for.
    fn hold(&self, timed: bool) -> u32 {
        let wait = if timed {
            param::RES_UP_WINDOWS
        } else {
            param::SCALE_PROBE_WINDOWS
        };
        (wait * self.backoff).min(param::RES_UP_WINDOWS_MAX)
    }

    /// The load `cost` over `budget_ms` of frames paced as `pace` would be at
    /// the knobs `(paced_ms, step, lattice)`: the rest as it is, the
    /// per-pixel time by the pixels there over these, the lattice's by its
    /// corners and cells there over these, the budget by the frame's there
    /// over here.
    fn predicted(
        &self,
        cost: Cost,
        budget_ms: f64,
        pace: &Pace,
        (paced_ms, step, lattice): Knobs,
    ) -> f64 {
        let px = libm::pow(share(step) / share(self.step), 2.0);
        let cell = libm::pow(f64::from(self.lattice) / f64::from(lattice), 2.0);
        let rate = budget(pace.paced_ms) / budget(paced_ms);
        (cost.rest_ms + cost.px_ms * px + cost.cell_ms * cell) * rate / budget_ms
    }

    /// Counts one frame's budget to the window, and its sample, where it
    /// took one, against it; at a window's end, judges it, a knob a step at
    /// the most. The knobs it moved to, where it moved one.
    fn judge(&mut self, cost: Option<Cost>, pace: &Pace, timed: bool) -> Option<Knobs> {
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
        let here = (pace.paced_ms, self.step, self.lattice);
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
        (_, self.step, self.lattice) = to;
        self.since = 0;
        self.quiet = 0;
        Some(to)
    }
}

static JUDGE: Global<Judge> = Global::new();

fn state() -> &'static mut Judge {
    JUDGE.get(|| Judge::NEW)
}

/// The share of the display's pixels, each way, step `step` draws at.
fn share(step: i32) -> f64 {
    libm::pow(
        param::RES_MIN,
        f64::from(step) / f64::from(param::RES_STEPS),
    )
}

/// The share of the display's pixels, each way, the world is drawn at: the
/// setting's, `auto`'s by its step.
fn scale() -> f64 {
    match settings_store().res {
        ResMode::auto => share(state().step),
        ResMode::fixed => 1.0,
        ResMode::low => param::RES_MIN,
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

/// The camera grid's lattice, in cells (`grid::CELL`) a side, its beam rays
/// and cells are laid on (pxl-engine `beam::Grid::of`): `auto`'s, the
/// cell's own at any other setting. The eye's words take it as they are
/// written, so the next cast casts on it.
pub fn lattice() -> u32 {
    match settings_store().res {
        ResMode::auto => state().lattice,
        ResMode::fixed | ResMode::low => 1,
    }
}

/// Device pixels per CSS pixel a display of `dpr` is drawn at whole: its
/// own.
fn device(dpr: f64) -> f64 {
    dpr
}

/// Device pixels per CSS pixel of the display the world is shown on, its
/// surface's; one before it has one.
pub fn dpr() -> f64 {
    world()
        .map(|s| s.dpr)
        .filter(|&dpr| dpr > 0.0)
        .map_or(1.0, device)
}

fn world() -> Option<&'static mut Surface> {
    surfaces().find(|s| s.role == surface_role::WORLD)
}

/// The ratio a surface is drawn at: the display's; the world's scaled as the
/// setting says; a preview's within its thumbnail.
pub fn surface_ratio(role: u32, px: [u32; 2], dpr: f64) -> f64 {
    let device = device(dpr);
    match role {
        surface_role::WORLD => device * scale(),
        surface_role::PREVIEW => {
            device.min(param::CASE_LONG_SIDE_MAX * dpr / f64::from(px[0].max(px[1]).max(1)))
        }
        _ => device,
    }
}

/// A frame was paced: `gpu`, where a GPU read landed with it, is what that
/// read timed (`GpuMeasure`). A frame drawn at another size than now is not
/// counted, and a read timed at another size or lattice is no sample; the
/// world's surface takes the scale the setting and the judge leave, a
/// lattice changed asks for the frame that casts on it. The interval a rate changed asks of the
/// runtime, 0 for its own under any setting but `auto`.
pub fn load(pace: Pace, gpu: Option<GpuMeasure>) -> Option<f64> {
    let Some(s) = world() else { return None };
    let mut interval_ms = None;
    let k = state();
    let (started, lattice) = (pace.moving && !k.moving, k.lattice);
    k.moving = pace.moving;
    if settings_store().res != ResMode::auto {
        (k.step, k.lattice, k.quiet) = (0, 1, 0);
        k.restart();
        interval_ms = pace.slowed().then_some(0.0);
    } else if started {
        k.restart();
    } else if pace.moving && pace.paced_ms > 0.0 && s.render_px() == (s.width, s.height) {
        let drawn = f64::from(s.width) * f64::from(s.height);
        let timed = api::device().is_some_and(|d| d.timestamps);
        let cost = if timed {
            gpu.filter(|t| {
                t.busy_ms > 0.0 && t.px > 0.0 && t.px == drawn && t.lattice == f64::from(k.lattice)
            })
            .map(|t| Cost {
                px_ms: t.px_ms,
                cell_ms: t.cell_ms,
                rest_ms: (t.busy_ms - t.px_ms - t.cell_ms).max(0.0),
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
    if s.ratio != ratio || k.lattice != lattice {
        s.ratio = ratio;
        edge::request_world_tick();
    }
    interval_ms
}

#[cfg(test)]
#[path = "resolution.test.rs"]
mod test;
