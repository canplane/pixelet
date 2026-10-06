// pxl-kernel/world/prefetch.rs

//! Speculation: tiles worth holding before they are read. Nothing rests on
//! it; hints only ever take frames nothing else holds.

use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::layer::demand::tiles_within;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

pub trait Prefetch {
    /// Never a tile finer than what is drawn there at `tau`.
    fn speculate(&self, o: &Observation, tau: Tau, each: &mut dyn FnMut(Tile));
}

pub struct NoPrefetch;

impl Prefetch for NoPrefetch {
    fn speculate(&self, _o: &Observation, _tau: Tau, _each: &mut dyn FnMut(Tile)) {}
}

/// Every zoom's ring about the eye's foot, whatever it faces, so a turn
/// fetches nothing (the clipmap precedent: Tanner, Migdal and Jones).
pub struct Reach {
    pub slack: f64,
    pub margin: f64,
}

impl Prefetch for Reach {
    fn speculate(&self, o: &Observation, tau: Tau, each: &mut dyn FnMut(Tile)) {
        let h = o.eye[2] - o.ground;
        for zoom in (GROUND.top()..=LEAF_ZOOM).rev() {
            // The disc a zoom's end leaves on the ground past the eye's height; none
            // where the eye is higher.
            let end = GROUND.demand_reach(zoom) / tau.scale();
            if end <= h {
                continue;
            }
            let ring = (end * end - h * h).sqrt();
            tiles_within(zoom, [o.eye[0], o.eye[1]], (ring * self.slack + self.margin).min(o.far())).for_each(&mut *each);
        }
    }
}
