// pxl-kernel/world/paging/floor.rs

//! The floor is a law: every observation has an answer at some level. A
//! frame cannot wait on a fetch, so a fault is answered by the nearest
//! resident page over it (paging.rs `drawn_from`), and only something held
//! over every observed piece of ground makes that page exist. What keeps the
//! law is a fallback, and residency holds whatever the chosen one leases:
//! the coarse level held all around is one way, a small root or a procedural
//! stand-in would be others. Which is chosen is named once (`Floor`), and
//! the capacity counts its pages (paging.rs `HARD_PAGES`).
//!
//! The floor is held, not asked for when an observation finds nothing: added
//! only then, every turn toward ground not yet seen would leave a hole for a
//! round trip. It is leased whatever is observed, so only the first moment of
//! a cold start or a jump is without it.
//!
//! **The floor is the source's.** Its level is the coarsest the tile source
//! supplies whole, as the source declares it (`declare`): the published
//! bake's from spec/pxl/spec/bake/published.json `SOURCE`, the dev tile
//! server's from its `--top-level`, which it answers in the same form. It is
//! a property of the storage and not an exception for any one of them, and
//! it is never guessed from what fails. Until it is declared nothing is
//! observed (world/observation.rs `observation`): every region is unresolved,
//! the stated state of a cold start, and never a floor taken at a level the
//! source may not have.

use pxl_spec::space::lod::{fault_radius, pages_within, FAULT_PAGES};
use pxl_spec::space::pyramid::{LevelPage, TOP_LEVEL};

use crate::global::Global;

/// The level the tile source declared, once it has.
static DECLARED: Global<Option<u32>> = Global::new();

/// Takes what the tile source declares of itself: the coarsest level of the
/// pyramid it supplies whole.
pub fn declare(top_level: u32) {
    xpute_core::ensure!(top_level <= TOP_LEVEL, EINVAL, top_level);
    *DECLARED.get(Default::default) = Some(top_level.min(TOP_LEVEL));
}

/// The floor's level, once the source has declared it.
pub fn level() -> Option<u32> {
    *DECLARED.get(Default::default)
}

/// A way to keep every observation answered: the pages it leases for a view.
pub trait Fallback {
    /// The most pages it leases at once.
    const PAGES: u32;

    /// Each page it leases for an eye at `eye` whose ground is drawn out to
    /// `far`, over a source whose coarsest level supplied whole is `top`.
    fn floor(top: u32, eye: [f64; 3], far: f64, each: impl FnMut(LevelPage));
}

/// The source's top, every page of it within the far edge. Every piece of
/// ground an observation answers lies within the far edge along the ground,
/// and so does the page of the top over it: one of these.
pub struct TopRing;

impl Fallback for TopRing {
    /// The far edge is at most where the top ends, its fault radius, so its
    /// pages are at most the ones a fault can name there — as many at every
    /// level, since the radius and a page grow by the same factor.
    const PAGES: u32 = FAULT_PAGES;

    fn floor(top: u32, eye: [f64; 3], far: f64, each: impl FnMut(LevelPage)) {
        pages_within(top, [eye[0], eye[1]], far.min(fault_radius(top))).for_each(each);
    }
}

/// The fallback the kernel keeps its floor with.
pub type Floor = TopRing;
