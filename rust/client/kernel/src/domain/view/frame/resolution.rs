// pxl-kernel/domain/view/frame/resolution.rs

//! Device pixels per CSS pixel a surface is drawn at: the display's, and the
//! world's as the `resolution` setting says. The world's rays are cast at the
//! display's pixels times `scale` each way, and the renderer's finish brings
//! them up to the canvas's, the display's (pxl-render gpu/finish.rs): render
//! resolution apart from display resolution where a screen outgrows its GPU
//! (Unreal's screen percentage). `fixed` draws
//! whole, `low` a device pixel a CSS pixel, each with the camera grid's
//! beam rays on the cell's own lattice, and `auto` one of `SCALE_STEPS`
//! steps between whole and `SCALE_MIN`, each a like share of the last, so
//! every move lays the pixels on one of a few grids, and a lattice up to
//! `grid::LATTICE_MAX`.
//!
//! `auto` is a budget controller, as congestion control is, over two knobs
//! and the one budget: the scale, which buys back the work done a drawn
//! pixel at a time (span.json `per_px`), and the camera grid's lattice, the
//! beam rays and cells laid `lattice` cells apart (pxl-spec-core
//! `beam::Grid::of`, span.json `per_cell`), which buys back theirs at a
//! quarter at the coarsest, `grid::LATTICE_MAX`. The samples are the very
//! rays at every lattice, so the lattice changes no pixel drawn; the
//! samples, the light's passes, on its map's texels, and the finish and the
//! markers, at the display's pixels, cost what they cost at either knob.
//!
//! It judges only frames that move: the gap before the first is how long
//! the view stood still, so moving starts a fresh window, and the quiet
//! windows judged before the pause still count. A window is
//! `RES_WINDOW_MS` of the moving frames' budgets, the paced interval the
//! host grants (@xpute/host sched/frame_pacing.ts `FrameGrant::paced_ms`) no tighter
//! than `RES_BUDGET_FLOOR_MS`, and its load is the samples its frames took,
//! summed against their own budgets. Where the device keeps timestamps a
//! sample is a GPU read's that landed, which not every frame has, timed at
//! the pixels and the lattice drawn now: its per-pixel spans' time P
//! (pxl-render gpu/renderer.rs `px_ms`), its lattice's G (`cell_ms`) and
//! the rest of the GPU's busy time F; a window none landed in is not
//! judged. Timed, a window whose load, F + P + G, is over its budget by more
//! than `RES_BAND` steps down at once, and `RES_UP_WINDOWS` windows in a
//! row in which the step above, F + P·(its share over this one)² + G·(this
//! lattice over its)², fits under it by that much step up, so the knobs do
//! not swing between two steps. Down, the scale goes first, to `SCALE_MIN`,
//! then the lattice; up, the lattice first, then the scale. Where it keeps
//! none the sample is the frame's gap, which never falls below the paced
//! interval, so room cannot be foreseen and only the scale moves: a window
//! whose gaps average past `SCALE_MISSED` budgets steps down, and a step up
//! is a probe taken after a hold of quiet windows; one that goes over within
//! `SCALE_TRUST_WINDOWS` steps back and doubles the hold, up to
//! `SCALE_PROBE_WINDOWS_MAX`. A change moves one knob a step: the scale's
//! rebuilds the targets, so no frame is summed while it waits to be drawn,
//! and the lattice's casts in full, as the eye's grid is another
//! (pxl-render gpu/ray/cast.rs `eye_moved`); the `RES_SETTLE_WINDOWS`
//! after either are not judged, and it asks for the frame that draws it.

use crate::domain::view::param;
use crate::domain::view::setting::store::{settings_store, ResMode};
use crate::sys::sched::edge;
use pxl_render::api_spec::surface_role;
use pxl_render::gpu::api;
use pxl_render::gpu::surface::{surfaces, Surface};
use pxl_spec_core::grid::LATTICE_MAX;
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

/// The knobs, the scale's step, 0 whole, and the lattice, 1 the cell's own,
/// and the window being judged.
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
    /// Untimed: the quiet windows a probe up waits for, and whether the step
    /// is one on trial.
    hold: u32,
    probing: bool,
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
        hold: param::SCALE_PROBE_WINDOWS,
        probing: false,
        moving: false,
    };

    /// A fresh window: its sums, not the quiet windows already judged, so a
    /// pause in moving neither breaks a run toward a step up nor a probe's
    /// hold.
    fn restart(&mut self) {
        (self.window_ms, self.cost, self.budget_ms) = (0.0, Cost::default(), 0.0);
    }

    /// The knobs a step down: the scale's, to `SCALE_MIN`, then, timed, the
    /// lattice's; none past both.
    fn down(&self, timed: bool) -> Option<(i32, u32)> {
        if self.step < param::SCALE_STEPS {
            Some((self.step + 1, self.lattice))
        } else if timed && self.lattice < LATTICE_MAX {
            Some((self.step, self.lattice * 2))
        } else {
            None
        }
    }

    /// The knobs a step up: the lattice's, then the scale's; none at both
    /// whole.
    fn up(&self) -> Option<(i32, u32)> {
        if self.lattice > 1 {
            Some((self.step, self.lattice / 2))
        } else if self.step > 0 {
            Some((self.step - 1, self.lattice))
        } else {
            None
        }
    }

    /// The load `cost` over `budget_ms` would be at the knobs `(step,
    /// lattice)`: the rest as it is, the per-pixel time by the pixels there
    /// over these, the lattice's by its corners and cells there over these.
    fn predicted(&self, cost: Cost, budget_ms: f64, (step, lattice): (i32, u32)) -> f64 {
        let px = libm::pow(share(step) / share(self.step), 2.0);
        let cell = libm::pow(f64::from(self.lattice) / f64::from(lattice), 2.0);
        (cost.rest_ms + cost.px_ms * px + cost.cell_ms * cell) / budget_ms
    }

    /// Counts one frame's budget to the window, and its sample, where it
    /// took one, against it; at a window's end, judges it, a knob a step at
    /// the most. Whether a knob changed.
    fn judge(&mut self, cost: Option<Cost>, budget_ms: f64, timed: bool) -> bool {
        self.window_ms += budget_ms;
        if let Some(cost) = cost {
            self.cost.add(cost);
            self.budget_ms += budget_ms;
        }
        if self.window_ms < param::RES_WINDOW_MS {
            return false;
        }
        let (cost, sampled) = (self.cost, self.budget_ms);
        self.restart();
        self.since = self.since.saturating_add(1);
        if sampled == 0.0 {
            return false;
        }
        let load = self.predicted(cost, sampled, (self.step, self.lattice));
        if self.since <= param::RES_SETTLE_WINDOWS {
            return false;
        }
        if self.probing && self.since > param::SCALE_TRUST_WINDOWS {
            self.probing = false;
        }
        let (down, up) = (self.down(timed), self.up());
        let (over, room, hold) = if timed {
            (
                load > 1.0 + param::RES_BAND,
                up.is_some_and(|u| self.predicted(cost, sampled, u) < 1.0 - param::RES_BAND),
                param::RES_UP_WINDOWS,
            )
        } else {
            let over = load > param::SCALE_MISSED;
            (over, !over, self.hold)
        };
        let to = match (down, up) {
            (Some(to), _) if over => {
                if self.probing {
                    self.hold = (self.hold * 2).min(param::SCALE_PROBE_WINDOWS_MAX);
                    self.probing = false;
                }
                to
            }
            (_, Some(to)) if room => {
                self.quiet += 1;
                if self.quiet < hold {
                    return false;
                }
                self.probing = !timed;
                to
            }
            _ => {
                self.quiet = 0;
                return false;
            }
        };
        (self.step, self.lattice) = to;
        self.since = 0;
        self.quiet = 0;
        true
    }
}

static JUDGE: Global<Judge> = Global::new();

fn state() -> &'static mut Judge {
    JUDGE.get(|| Judge::NEW)
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
    match settings_store().res {
        ResMode::auto => share(state().step),
        ResMode::fixed => 1.0,
        ResMode::low => 1.0 / device,
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
/// and cells are laid on (pxl-spec-core `beam::Grid::of`): `auto`'s, the
/// cell's own at any other setting. The eye's words take it as they are
/// written, so the frame drawn next casts on it.
pub fn lattice() -> u32 {
    match settings_store().res {
        ResMode::auto => state().lattice,
        ResMode::fixed | ResMode::low => 1,
    }
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
/// setting says; a preview's within its thumbnail.
pub fn surface_ratio(role: u32, px: [u32; 2], dpr: f64) -> f64 {
    let device = device(dpr);
    match role {
        surface_role::WORLD => device * scale(device),
        surface_role::PREVIEW => {
            device.min(param::CASE_LONG_SIDE_MAX * dpr / f64::from(px[0].max(px[1]).max(1)))
        }
        _ => device,
    }
}

/// A frame was paced: `gpu`, where a GPU read landed with it, is what that
/// read timed (`GpuMeasure`). A frame drawn at another size than now is not
/// counted, and a read timed at another size or lattice is no sample; the
/// world's surface takes the scale the setting and the judge leave, and a
/// lattice changed asks for the frame that casts on it.
pub fn load(pace: Pace, gpu: Option<GpuMeasure>) {
    let Some(s) = world() else { return };
    let k = state();
    let (started, lattice) = (pace.moving && !k.moving, k.lattice);
    k.moving = pace.moving;
    if settings_store().res != ResMode::auto {
        (k.step, k.lattice, k.quiet) = (0, 1, 0);
        k.restart();
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
        k.judge(cost, pace.paced_ms.max(param::RES_BUDGET_FLOOR_MS), timed);
    }
    let ratio = surface_ratio(s.role, [s.px_w, s.px_h], s.dpr);
    if s.ratio != ratio || k.lattice != lattice {
        s.ratio = ratio;
        edge::request_world_tick();
    }
}

#[cfg(test)]
#[path = "resolution.test.rs"]
mod test;
