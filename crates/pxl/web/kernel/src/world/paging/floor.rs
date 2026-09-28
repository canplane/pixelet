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

use pxl_spec::space::pyramid::{LevelPage, TOP_LEVEL};

use pxl_spec::space::lod::{fault_radius, pages_within, FAULT_PAGES};

/// A way to keep every observation answered: the pages it leases for a view.
pub trait Fallback {
    /// The most pages it leases at once.
    const PAGES: u32;

    /// Each page it leases for an eye at `eye` whose ground is drawn out to
    /// `far`.
    fn floor(eye: [f64; 3], far: f64, each: impl FnMut(LevelPage));
}

/// The pyramid's top, every page of it within the far edge. Every piece of
/// ground an observation answers lies within the far edge along the ground,
/// and so does the page of the top over it: one of these.
pub struct TopRing;

impl Fallback for TopRing {
    /// The far edge is at most where the top level ends, the top's fault
    /// radius, so its pages are at most the ones a fault can name there.
    const PAGES: u32 = FAULT_PAGES;

    fn floor(eye: [f64; 3], far: f64, each: impl FnMut(LevelPage)) {
        pages_within(TOP_LEVEL, [eye[0], eye[1]], far.min(fault_radius(TOP_LEVEL))).for_each(each);
    }
}

/// The fallback the kernel keeps its floor with.
pub type Floor = TopRing;
