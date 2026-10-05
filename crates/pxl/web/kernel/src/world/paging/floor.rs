// pxl-kernel/world/paging/floor.rs

//! The floor: every observation has an answer at some zoom, because a
//! fallback's pages are held hard over every observed piece of ground. It is
//! asked for with every observation, not only when one finds nothing, or
//! every turn toward new ground would leave a hole for a round trip.
//!
//! Its zoom is the source's declared top (`declare`), never guessed from
//! what fails. Until declared, nothing is observed.

use pxl_spec::space::lod::{pages_within, FAULT_PAGES};
use pxl_spec::space::observation::Observation;
use pxl_spec::space::pyramid::{Page, TOP_Z};

use xpute_runtime::global::Global;

static DECLARED: Global<Option<u32>> = Global::new();

pub fn declare(top_z: u32) {
    xpute_core::ensure!(top_z >= TOP_Z, EINVAL, top_z);
    *DECLARED.get(Default::default) = Some(top_z);
}

pub fn top() -> Option<u32> {
    *DECLARED.get(Default::default)
}

pub trait Fallback {
    const PAGES: u32;

    fn floor(o: &Observation, each: impl FnMut(Page));
}

/// The top's pages with ground within the far edge of the eye itself, which
/// hold every observed piece of ground. An eye higher than the edge holds none:
/// a pixel there covers more than the top's cell, and the view has gone over to
/// the Earth's picture.
pub struct TopRing;

impl Fallback for TopRing {
    /// The far edge is at most the top's fault radius, and a sphere's cut by
    /// the ground is no wider, so these are at most the pages a fault can name there.
    const PAGES: u32 = FAULT_PAGES;

    fn floor(o: &Observation, each: impl FnMut(Page)) {
        let over = (o.eye[2] - o.ground).max(0.0);
        if over < o.far {
            pages_within(o.top, [o.eye[0], o.eye[1]], (o.far * o.far - over * over).sqrt()).for_each(each);
        }
    }
}

pub type Floor = TopRing;
