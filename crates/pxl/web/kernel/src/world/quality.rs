// pxl-kernel/world/quality.rs

//! The quality policy: the one owner of the effective quality, `τ_eff`,
//! which both the observation's demand (world/pyramid.rs) and what is drawn
//! of it (world/pyramid/domain.rs, world/chunk/frontier.rs) read — so the
//! two scale the one criterion by the one factor, and nothing is drawn
//! finer than demand fetches for it. What they read is an `Effective`, which
//! nothing but this module can make: no other code chooses a quality.
//!
//! Two parts, with two contracts.
//!
//! **Feasibility** (`feasible`) is a function and nothing else: of the
//! observation O, what the index lanes know K, and the arena B,
//!
//! ```text
//! τ* = the finest τ ≥ τ_target with frontier(O, K, τ) + headroom ≤ B
//! ```
//!
//! with `frontier` the goal cut at τ, counted by the materializer before
//! anything is allocated (frontier.rs `need`): conservative, never less at a
//! finer τ, and never less with looser bounds — so the qualities that fit
//! are every one from some point coarser, and the search is only how that
//! point is found. It has no history and reads nothing of what the arena
//! has done: not an allocation, not a refusal. Safety rests on it alone.
//!
//! **Its evaluation asks only what the smoothing reads** (`around`): whether
//! the step in force fits, and if it does, whether the step finer does, with
//! and without the margin; if it does not, the steps coarser in turn until
//! one does, which is τ* since nothing finer fits either. Every step's cut
//! is made from the one sight of the screen (world/pyramid/domain.rs), and
//! only the steps asked are cut and counted — two a rebuild at rest, not the
//! ladder. By the count's order this is `feasible` exactly, wherever the
//! smoothing stands.
//!
//! The goal is bounded by the leak rule's reach (domain.rs `split_reach`),
//! not by the screen, and no step of the ladder is sure to fit whatever the
//! world holds: near the eye every step refines, and a world solid about
//! the eye is refined as far as its error leaks. What is sure is the cover:
//! it has a pool of its own, as many slots as it can have pieces
//! (mesh_slot.rs `COVER_SLOTS`), and the headroom fits the other pools
//! beside nothing, asserted below, so the floor's answer is always drawn
//! and a view past the arena at the coarsest step is that step's frontier
//! as far as it fits, never a hole (frontier.rs).
//!
//! **Smoothing** (`Smooth`) moves `τ_eff` toward `τ*` over time and decides
//! nothing about what fits. Where `τ_eff` is finer than `τ*`, it is set to
//! `τ*` in the same frame, whatever came before: the arena is never short of
//! what the goal needs, not for a frame. Where `τ*` is finer, it steps finer
//! one step at a time, each once the step has fit with `param::QUALITY_MARGIN`
//! to spare for `param::QUALITY_WAIT_MS`: so a view at rest comes back to
//! `τ*`, and one moving does not flap between two steps, building and
//! throwing away a level's pieces each time. The AIMD of the record is this
//! behavior, and only this.

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
// In slots: the cover's pool is every piece it can have, and the headroom —
// the reserve one coarsening needs and a slot each refinement under way
// holds — fits each other pool.
const _: () = assert!(
    cover_pieces(PUBLISHED_TOP_LEVEL, MERCATOR_RADIUS_U) <= ARENA[Pool::Cover as usize],
    "the published source's cover does not fit its pool: ground it answers could stay unresolved"
);
const _: () = assert!(
    HEADROOM[Pool::Fine as usize] <= ARENA[Pool::Fine as usize] && HEADROOM[Pool::Coarse as usize] <= ARENA[Pool::Coarse as usize],
    "the headroom does not fit the arena: a view could be left with no move it can make"
);

/// The quality the view is held to where the arena has room for it: the
/// angle `param::CENTER_CELL_LOGICAL_PX` of the screen's logical pixels
/// span centered on its optical axis, as a factor on the finest cell
/// everything is sized for (pxl-spec space/lod.rs `SPLIT`), and never finer
/// than that one. On a screen `height` logical pixels tall at a field of
/// view `fov` up, the plane stands `height / (2 · tan(fov / 2))` of them
/// from the eye, so `n` of them about the axis span `2 · atan(n / 2 / that)`.
/// A function of the screen's logical height and its field of view, and of
/// nothing the screen is rasterized at.
pub fn target() -> Tau {
    let v = view();
    let half = param::CENTER_CELL_LOGICAL_PX * (v.fov.to_radians() / 2.0).tan() / v.height;
    Tau::of((2.0 * half.atan() * SPLIT).max(1.0))
}

/// The qualities the policy chooses among, finest first: the target, and
/// `TAU_STEPS` steps coarser.
pub fn ladder() -> [Tau; param::TAU_STEPS + 1] {
    let target = target().scale();
    core::array::from_fn(|k| Tau::of(target * param::TAU_STEP.powi(k as i32)))
}

/// The effective quality, as the policy hands it out: the one value what is
/// demanded and what is drawn are both held to. Only this module makes one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effective(Tau);

impl Effective {
    pub fn tau(self) -> Tau {
        self.0
    }
}

/// Whether `count`, with `headroom` and `spare` beside it, fits `arena` in
/// every pool.
pub fn fits(count: Slots, arena: Slots, headroom: Slots, spare: Slots) -> bool {
    (0..POOLS).all(|p| count[p] + headroom[p] + spare[p] <= arena[p])
}

/// The finest step of the ladder whose `frontier` count, with `headroom` and
/// `spare` beside it, fits `arena` in every pool; none where not even the
/// coarsest does. `frontier` is finest first and never less at a finer step,
/// so every step from the one found on fits too.
pub fn feasible(frontier: &[Slots], arena: Slots, headroom: Slots, spare: Slots) -> Option<usize> {
    (0..frontier.len()).find(|&k| fits(frontier[k], arena, headroom, spare))
}

/// The feasible step and the roomy one — `feasible` without and with the
/// margin — as far as the smoothing at `at` reads them, from `fit(k, roomy)`,
/// whether step `k` of `len` fits without or with it, asked of as few steps
/// as that takes: the step in force, the one finer, and where the step in
/// force does not fit the ones coarser until one does. For counts never less
/// at a finer step, `Smooth::follow` moves on these as on `feasible`'s own;
/// where not even the coarsest fits, both are the coarsest, as there.
pub fn around(at: usize, len: usize, mut fit: impl FnMut(usize, bool) -> bool) -> (usize, usize) {
    let last = len - 1;
    if !fit(at, false) {
        let star = (at + 1..len).find(|&k| fit(k, false)).unwrap_or(last);
        return (star, star);
    }
    let finer = |fit: &mut dyn FnMut(usize, bool) -> bool, roomy: bool| if at > 0 && fit(at - 1, roomy) { at - 1 } else { at };
    (finer(&mut fit, false), finer(&mut fit, true))
}

/// What each pool keeps free past the frontier before a finer step is
/// taken: `QUALITY_MARGIN` of it, and none of the cover's, whose pieces are
/// the roots' at every quality and never past its slots.
fn margin() -> Slots {
    core::array::from_fn(|p| if p == Pool::Cover as usize { 0 } else { (ARENA[p] as f64 * param::QUALITY_MARGIN) as u32 })
}

/// Where on the ladder the effective quality is, and since when the step
/// finer than it has had room.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Smooth {
    at: usize,
    room_since: Option<f64>,
}

impl Smooth {
    /// Which step of the ladder is in force.
    pub fn at(&self) -> usize {
        self.at
    }

    /// The step in force at `now`, in milliseconds, following `star` — the
    /// feasible step — and `roomy`, the finest step that is feasible with the
    /// margin to spare. Coarser than in force is taken at once; finer, a step
    /// at a time, once the step finer has been roomy for `wait_ms`.
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

/// The policy as the kernel runs it: where the smoothing is, and the count
/// of each step asked since the domain or what is known of the meshes last
/// moved, and which walk that was of.
#[derive(Default)]
struct Quality {
    smooth: Smooth,
    /// The feasible step the last turn found, for the profiler.
    star: usize,
    counts: [Option<Slots>; param::TAU_STEPS + 1],
    counted: Option<(u32, u32)>,
}

static QUALITY: Global<Quality> = Global::new();

/// The effective quality: what the view is cut at and what its demand names
/// pages at.
pub fn effective() -> Effective {
    Effective(ladder()[QUALITY.get(Quality::default).smooth.at()])
}

/// The step in force and the feasible step, on the ladder from the target:
/// what the profiler reads the policy by.
pub fn reading() -> [usize; 2] {
    let q = QUALITY.get(Quality::default);
    [q.smooth.at(), q.star]
}

/// The policy's turn, before anything reads the quality: the view's domain
/// made again where it moved, the steps the smoothing reads counted where
/// the domain or what is known of the meshes moved (`around`), the
/// effective step moved toward the feasible one, and the domain cut there.
/// Idempotent within an instant, so every reader may ask it first.
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
