// pxl-kernel/world/paging/floor.rs

//! The floor: every observation has an answer at some level, because a
//! fallback's pages are held hard over every observed piece of ground. It is
//! asked for with every observation, not only when one finds nothing, or
//! every turn toward new ground would leave a hole for a round trip.
//!
//! Its level is the source's declared top (`declare`), never guessed from
//! what fails. Until declared, nothing is observed.

use pxl_spec::space::lod::{fault_radius, pages_within, FAULT_PAGES};
use pxl_spec::space::pyramid::{LevelPage, TOP_LEVEL};

use xpute_runtime::global::Global;

static DECLARED: Global<Option<u32>> = Global::new();

pub fn declare(top_level: u32) {
    xpute_core::ensure!(top_level <= TOP_LEVEL, EINVAL, top_level);
    *DECLARED.get(Default::default) = Some(top_level.min(TOP_LEVEL));
}

pub fn level() -> Option<u32> {
    *DECLARED.get(Default::default)
}

pub trait Fallback {
    const PAGES: u32;

    fn floor(top: u32, eye: [f64; 3], far: f64, each: impl FnMut(LevelPage));
}

/// The top level's pages within the far edge, which hold every observed piece of ground.
pub struct TopRing;

impl Fallback for TopRing {
    /// The far edge is at most the top's fault radius, so these are at most
    /// the pages a fault can name there.
    const PAGES: u32 = FAULT_PAGES;

    fn floor(top: u32, eye: [f64; 3], far: f64, each: impl FnMut(LevelPage)) {
        pages_within(top, [eye[0], eye[1]], far.min(fault_radius(top))).for_each(each);
    }
}

pub type Floor = TopRing;
