// pxl-kernel/world/prefetch.rs

//! Speculation: pages worth holding before they are read. Nothing rests on
//! it; hints only ever take frames nothing else holds.

use pxl_spec::space::lod::{pages_within, Tau};
use pxl_spec::space::observation::Observation;
use pxl_spec::space::pyramid::{Page, LEAF_Z, TOP_Z};

pub trait Prefetch {
    /// Never a page finer than what is drawn there at `tau`.
    fn speculate(&self, o: &Observation, tau: Tau, each: &mut dyn FnMut(Page));
}

pub struct NoPrefetch;

impl Prefetch for NoPrefetch {
    fn speculate(&self, _o: &Observation, _tau: Tau, _each: &mut dyn FnMut(Page)) {}
}

/// Every zoom's ring about the eye's foot, whatever it faces, so a turn
/// fetches nothing (the clipmap precedent: Tanner, Migdal and Jones).
pub struct Reach {
    pub slack: f64,
    pub margin: f64,
}

impl Prefetch for Reach {
    fn speculate(&self, o: &Observation, tau: Tau, each: &mut dyn FnMut(Page)) {
        let h = o.eye[2] - o.ground;
        for z in (TOP_Z..=LEAF_Z).rev() {
            // The disc a zoom's end leaves on the ground past the eye's height; none
            // where the eye is higher.
            let end = tau.fault_radius(z);
            if end <= h {
                continue;
            }
            let ring = (end * end - h * h).sqrt();
            pages_within(z, [o.eye[0], o.eye[1]], (ring * self.slack + self.margin).min(o.far)).for_each(&mut *each);
        }
    }
}
