// pxl-spec/space/layer/octant.rs

//! A cube of the world under a shard, the 3D `Quadrant`: named as `wrap_cell`
//! names it, so one cube has one name, and its key, the address a placed object
//! is written at.

use crate::space::coord::q3::Q3;
use crate::space::coord::world::wrap_cell;
use crate::space::coord::zoom::{idx_of, Zoom};
use crate::space::hierarchy::morton::{key_of, name};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Octant {
    zoom: Zoom,
    at: [i32; 3],
}

impl Octant {
    pub fn new(zoom: Zoom, at: [i32; 3]) -> Octant {
        Octant {
            zoom,
            at: wrap_cell(zoom, at),
        }
    }

    /// The cube of `zoom` that holds `p`.
    pub fn holding(zoom: Zoom, p: &Q3) -> Octant {
        Octant::new(zoom, idx_of(zoom, p))
    }

    pub fn zoom(self) -> Zoom {
        self.zoom
    }

    pub fn at(self) -> [i32; 3] {
        self.at
    }

    /// Its key, its octkey as text: the shard's quadkey, then a digit 0 to 7 a zoom.
    pub fn key(self) -> String {
        name::<3>(self.id())
    }

    /// Its id, its morton code as a number (`morton.rs`).
    pub fn id(self) -> u128 {
        key_of::<3>(self.zoom, self.at)
    }
}
