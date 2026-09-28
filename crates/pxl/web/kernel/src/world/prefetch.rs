// pxl-kernel/world/prefetch.rs

//! Speculation: which pages beyond what the view reads are worth holding
//! before they are asked for. A policy, and nothing rests on it: with none at
//! all the view is the same once its faults are answered, and only the
//! latency and the look while they are missing change. What it asks for is
//! a hint (io/demand.rs `Why::Hint`), which residency gives only a frame
//! nothing else holds (world/paging.rs), so however eager it is, it only
//! pushes itself out of the cache.

use pxl_spec::space::lod::{fault_radius, pages_within};
use pxl_spec::space::observation::Observation;
use pxl_spec::space::pyramid::{LevelPage, TOP_LEVEL};

/// Which pages beyond the demand to speculate on for an observation.
pub trait Prefetch {
    /// Each page worth holding before it is read, for `o`.
    fn speculate(&self, o: &Observation, each: &mut dyn FnMut(LevelPage));
}

/// None: the view is paged by its faults alone.
pub struct NoPrefetch;

impl Prefetch for NoPrefetch {
    fn speculate(&self, _o: &Observation, _each: &mut dyn FnMut(LevelPage)) {}
}

/// Every level's reach, all around the eye: the ring about the eye's foot
/// its level can be asked for in, whatever the eye faces. A turn changes nothing in
/// it, so a turn fetches nothing (Tanner, Migdal and Jones' clipmap is the
/// precedent). `slack` widens each ring by a factor and `margin` by the
/// units the eye can move before what it moves toward arrives.
pub struct Reach {
    pub slack: f64,
    pub margin: f64,
}

impl Prefetch for Reach {
    fn speculate(&self, o: &Observation, each: &mut dyn FnMut(LevelPage)) {
        let h = o.eye[2] - o.ground;
        for level in 0..=TOP_LEVEL {
            // A level is asked for nearer than where it ends, which on the
            // ground about the eye's foot is a disc as wide as that distance
            // leaves past the eye's height. None where the eye is higher.
            let end = fault_radius(level);
            if end <= h {
                continue;
            }
            let ring = (end * end - h * h).sqrt();
            pages_within(level, [o.eye[0], o.eye[1]], (ring * self.slack + self.margin).min(o.far)).for_each(&mut *each);
        }
    }
}
