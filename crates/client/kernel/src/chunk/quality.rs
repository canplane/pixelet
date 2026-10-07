// pxl-kernel/chunk/quality.rs

//! The quality policy: sole owner of the effective quality `τ_eff`, which
//! demand and drawing both read (as an `Effective` only this module makes),
//! so nothing is drawn finer than demand fetches.
//!
//! Feasibility is a pure function of the observation, what is known and the
//! arena:
//!
//! ```text
//! τ* = the finest τ ≥ τ_target with frontier(O, K, τ) + headroom ≤ B
//! ```
//!
//! Counts come from the goal's one walk (domain.rs, priced by frontier.rs
//! `Need`). Smoothing coarsens at a publish, and refines one step after
//! `QUALITY_WAIT_MS` with `QUALITY_MARGIN` to spare.

use pxl_spec::space::camera::grid::{target_cell_px, Tau, SPLIT};

use crate::chunk::domain;
use crate::chunk::frontier::{Need, Slots, HEADROOM};
use crate::chunk::mesh_slot::{Pool, ARENA, POOLS};
use crate::chunk::Chunk;
use crate::kernel;
use crate::param;
use crate::view::frame::pose::view;
use xpute_guest::global::Global;

const _: () = assert!(param::QUALITY_WAIT_MS > 0.0, "a policy asked twice at one instant would step finer twice");
const _: () = assert!(param::TAU_STEP > 1.0, "a step that is not coarser");
// The headroom, the roots' room among it, fits the arena, so the floor's
// answer is always drawn.
const _: () = assert!(
    HEADROOM[Pool::Fine as usize] <= ARENA[Pool::Fine as usize] && HEADROOM[Pool::Coarse as usize] <= ARENA[Pool::Coarse as usize],
    "the headroom does not fit the arena: a view could be left with no move it can make"
);

/// The quality at which a cell spans as many logical pixels at the optical
/// axis as the camera grid's at the reference view (`target_cell_px`), never
/// finer than `SPLIT`. The screen plane
/// stands `height / (2 · tan(fov / 2))` pixels from the eye, so `n` of them
/// span `2 · atan(n / 2 / that)`.
pub fn target() -> Tau {
    let v = view();
    let half = target_cell_px() * libm::tan(v.fov.to_radians() / 2.0) / v.height;
    Tau::of((2.0 * libm::atan(half) * SPLIT).max(1.0))
}

pub fn ladder() -> [Tau; param::TAU_STEPS + 1] {
    let target = target().scale();
    core::array::from_fn(|k| Tau::of(target * param::TAU_STEP.powi(k as i32)))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effective(Tau);

impl Effective {
    pub fn tau(self) -> Tau {
        self.0
    }
}

pub fn fits(count: Slots, arena: Slots, headroom: Slots, spare: Slots) -> bool {
    (0..POOLS).all(|p| count[p] + headroom[p] + spare[p] <= arena[p])
}

/// The finest step that fits without and with the margin, asking only the
/// steps the smoothing at `at` reads: the one in force, the one finer, and
/// coarser ones until one fits. Every coarser step fits too, since counts
/// never grow coarser.
pub fn around(at: usize, len: usize, mut fit: impl FnMut(usize, bool) -> bool) -> (usize, usize) {
    let last = len - 1;
    if !fit(at, false) {
        let star = (at + 1..len).find(|&k| fit(k, false)).unwrap_or(last);
        return (star, star);
    }
    let finer = |fit: &mut dyn FnMut(usize, bool) -> bool, roomy: bool| if at > 0 && fit(at - 1, roomy) { at - 1 } else { at };
    (finer(&mut fit, false), finer(&mut fit, true))
}

/// Room the policy keeps past a step before it goes finer.
fn margin() -> Slots {
    ARENA.map(|n| (n as f64 * param::QUALITY_MARGIN) as u32)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Smooth {
    at: usize,
    room_since: Option<f64>,
}

impl Smooth {
    pub fn at(&self) -> usize {
        self.at
    }

    /// Coarser is taken at once; finer one step at a time, once that step has
    /// been roomy for `wait_ms`.
    pub fn follow(&mut self, star: usize, roomy: usize, wait_ms: f64, now: f64) -> usize {
        if self.at < star {
            self.at = star;
            self.room_since = None;
        } else if self.at > star && roomy < self.at {
            let since = *self.room_since.get_or_insert(now);
            if now - since >= wait_ms {
                self.at -= 1;
                self.room_since = None;
            }
        } else {
            self.room_since = None;
        }
        self.at
    }
}

#[derive(Default)]
struct Quality {
    smooth: Smooth,
    star: usize,
    /// What the walk prices chunks at, read each turn into a buffer kept.
    priced: Vec<(Chunk, u32)>,
}

static QUALITY: Global<Quality> = Global::new();

/// The goal in force's τ, not the live ladder's.
pub fn effective() -> Effective {
    Effective(domain::current().map_or_else(|| ladder()[QUALITY.get(Quality::default).smooth.at()], |d| d.tau()))
}

pub fn reading() -> [usize; 2] {
    let q = QUALITY.get(Quality::default);
    [q.smooth.at(), q.star]
}

/// Starts the next goal walk over the ladder.
pub fn sight() {
    let q = QUALITY.get(Quality::default);
    kernel().sched.priced(&mut q.priced);
    domain::start(&ladder(), &q.priced);
}

/// A slice of the goal walk, then the step that fits.
pub fn step(stop: &dyn Fn() -> bool) {
    let q = QUALITY.get(Quality::default);
    let sched = &kernel().sched;
    let price = Need {
        slots: |c| sched.slots_of(c),
        admit: true,
    };
    domain::advance(q.smooth.at(), &price, stop);
    let Some(d) = domain::current() else { return };
    let spare = margin();
    let (star, roomy) = around(q.smooth.at(), param::TAU_STEPS + 1, |k, roomy| {
        d.need(k).is_some_and(|n| fits(n, ARENA, HEADROOM, if roomy { spare } else { [0; POOLS] }))
    });
    q.star = star;
    let at = q.smooth.follow(star, roomy, param::QUALITY_WAIT_MS, xpute_guest::clock::now());
    domain::put(at);
}

#[cfg(test)]
#[path = "quality.test.rs"]
mod test;
