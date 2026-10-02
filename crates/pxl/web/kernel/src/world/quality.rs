// pxl-kernel/world/quality.rs

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
//! `frontier` (frontier.rs `need`) never counts less at a finer τ, so only
//! the steps the smoothing reads need counting (`around`). Smoothing moves
//! `τ_eff` coarser to `τ*` in the same frame, and finer one step at a time
//! after `QUALITY_WAIT_MS` with `QUALITY_MARGIN` to spare.

use pxl_render::look::globe::MERCATOR_RADIUS_U;
use pxl_spec::bake::published::PUBLISHED_TOP_LEVEL;
use pxl_spec::space::lod::{Tau, SPLIT};

use crate::global::Global;
use crate::kernel::kernel;
use crate::param;
use crate::world::chunk::frontier::{need, Slots, HEADROOM};
use crate::world::chunk::mesh_slot::{Pool, ARENA, POOLS};
use crate::world::frame::view::view;
use crate::world::pyramid::domain::{self, cover_pieces};

const _: () = assert!(param::QUALITY_WAIT_MS > 0.0, "a policy asked twice at one instant would step finer twice");
const _: () = assert!(param::TAU_STEP > 1.0, "a step that is not coarser");
// The cover's pool holds every piece it can have, and the headroom fits
// the other pools, so the floor's answer is always drawn.
const _: () = assert!(
    cover_pieces(PUBLISHED_TOP_LEVEL, MERCATOR_RADIUS_U) <= ARENA[Pool::Cover as usize],
    "the published source's cover does not fit its pool: ground it answers could stay unresolved"
);
const _: () = assert!(
    HEADROOM[Pool::Fine as usize] <= ARENA[Pool::Fine as usize] && HEADROOM[Pool::Coarse as usize] <= ARENA[Pool::Coarse as usize],
    "the headroom does not fit the arena: a view could be left with no move it can make"
);

/// The quality at which `CENTER_CELL_LOGICAL_PX` logical pixels at the
/// optical axis span one cell, never finer than `SPLIT`. The screen plane
/// stands `height / (2 · tan(fov / 2))` pixels from the eye, so `n` of them
/// span `2 · atan(n / 2 / that)`.
pub fn target() -> Tau {
    let v = view();
    let half = param::CENTER_CELL_LOGICAL_PX * (v.fov.to_radians() / 2.0).tan() / v.height;
    Tau::of((2.0 * half.atan() * SPLIT).max(1.0))
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

/// The finest step that fits; every coarser one fits too, since counts never
/// grow coarser.
pub fn feasible(frontier: &[Slots], arena: Slots, headroom: Slots, spare: Slots) -> Option<usize> {
    (0..frontier.len()).find(|&k| fits(frontier[k], arena, headroom, spare))
}

/// `feasible` without and with the margin, asking only the steps the
/// smoothing at `at` reads: the one in force, the one finer, and coarser ones
/// until one fits.
pub fn around(at: usize, len: usize, mut fit: impl FnMut(usize, bool) -> bool) -> (usize, usize) {
    let last = len - 1;
    if !fit(at, false) {
        let star = (at + 1..len).find(|&k| fit(k, false)).unwrap_or(last);
        return (star, star);
    }
    let finer = |fit: &mut dyn FnMut(usize, bool) -> bool, roomy: bool| if at > 0 && fit(at - 1, roomy) { at - 1 } else { at };
    (finer(&mut fit, false), finer(&mut fit, true))
}

/// None for the cover, whose pieces are the roots at every quality.
fn margin() -> Slots {
    core::array::from_fn(|p| if p == Pool::Cover as usize { 0 } else { (ARENA[p] as f64 * param::QUALITY_MARGIN) as u32 })
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
    counts: [Option<Slots>; param::TAU_STEPS + 1],
    counted: Option<(u32, u32)>,
}

static QUALITY: Global<Quality> = Global::new();

pub fn effective() -> Effective {
    Effective(ladder()[QUALITY.get(Quality::default).smooth.at()])
}

pub fn reading() -> [usize; 2] {
    let q = QUALITY.get(Quality::default);
    [q.smooth.at(), q.star]
}

/// The policy's turn, before anything reads the quality. Idempotent within
/// an instant, so every reader may call it first.
pub fn step() {
    let q = QUALITY.get(Quality::default);
    let ladder = ladder();
    domain::step(&ladder, Effective(ladder[q.smooth.at()]));
    if domain::current().is_none() {
        return;
    }
    let sched = &kernel().sched;
    let key = Some((domain::walks(), sched.needs_revision()));
    if q.counted != key {
        q.counted = key;
        q.counts = [None; param::TAU_STEPS + 1];
    }
    let counts = &mut q.counts;
    let mut count = |k: usize| *counts[k].get_or_insert_with(|| domain::cut_at(k).map_or([0; POOLS], |(d, cut)| need(d, cut, |c, cube| sched.slots_of(c, cube))));
    let spare = margin();
    let (star, roomy) = around(q.smooth.at(), ladder.len(), |k, roomy| fits(count(k), ARENA, HEADROOM, if roomy { spare } else { [0; POOLS] }));
    q.star = star;
    let at = q.smooth.follow(star, roomy, param::QUALITY_WAIT_MS, crate::host::now());
    domain::step(&ladder, Effective(ladder[at]));
}

#[cfg(test)]
#[path = "quality.test.rs"]
mod test;
