// pxl-kernel/world/chunk.rs

//! Chunks: what each holds, which a view needs, and the tier each is built at.
//!
//! A chunk: a cube of the camera grid's cells, `CHUNK_STEPS` zooms finer than
//! itself, built and drawn as one. Never fetched or held; only its address
//! exists, and an empty one is never made. It names only the grid's cells:
//! which of a layer's tiles holds them is its lattice's (`Lattice::read_tile`).

pub mod build;
pub mod decor;
pub mod domain;
pub mod frontier;
pub mod mesh_slot;
pub mod place;
pub mod quality;
pub mod source;
pub mod stage;
pub mod table;

use pxl_spec::space::camera::grid::CHUNK_STEPS;
use pxl_spec::space::coord::pvf3::ivec::IVec;
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::v3::Aabb;
use pxl_spec::space::coord::world::wrap_cell;
use pxl_spec::space::coord::zoom::{base_of, index_of, units, Zoom, SHARD_ZOOM};
use pxl_spec::space::hierarchy::tree::{child_of, parent_of};

pub const CHILDREN: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chunk {
    zoom: Zoom,
    at: [i64; 3],
}

impl Chunk {
    /// The chunk at `zoom` holding `at`.
    pub fn holding(zoom: Zoom, at: &Pvf3) -> Chunk {
        Chunk { zoom, at: index_of(zoom, at) }
    }

    pub fn indexed(zoom: Zoom, at: [i64; 3]) -> Chunk {
        Chunk { zoom, at }
    }

    /// As the bounds of a range in chunk order.
    pub fn column_holding(zoom: Zoom, at: &Pvf3) -> std::ops::RangeInclusive<Chunk> {
        let [x, y, _] = index_of(zoom, at);
        Chunk { zoom, at: [x, y, i64::MIN] }..=Chunk { zoom, at: [x, y, i64::MAX] }
    }

    pub fn beside(self, dx: i64, dy: i64, dh: i64) -> Chunk {
        let [x, y, h] = self.at;
        Chunk {
            zoom: self.zoom,
            at: [x + dx, y + dy, h + dh],
        }
    }

    pub fn zoom(self) -> Zoom {
        self.zoom
    }

    /// Its cells': the camera grid's, `CHUNK_STEPS` zooms finer.
    pub fn cell_zoom(self) -> Zoom {
        self.zoom + CHUNK_STEPS
    }

    /// The lane's copy, as `holding` makes it: one name for every round of the world.
    pub fn named(self) -> Chunk {
        Chunk {
            zoom: self.zoom,
            at: wrap_cell(self.zoom, self.at),
        }
    }

    /// The corner at no height, as the 2D stores key it; where it stands is `rect`'s.
    pub fn corner(self) -> IVec {
        let [x, y, _] = self.at;
        base_of(self.zoom, [x, y, 0]).i
    }

    /// From the index, which runs on round the world as the view's frame does.
    pub fn rect(self) -> (f64, f64, f64) {
        let span = units(self.zoom);
        (self.at[0] as f64 * span, self.at[1] as f64 * span, span)
    }

    /// From the index, not the corner point: a point's height is 16 bits
    /// (±32,768 units) and the top chunk is 65,536 tall.
    pub fn z_range(self) -> (f64, f64) {
        let span = units(self.zoom);
        let z0 = self.at[2] as f64 * span;
        (z0, z0 + span)
    }

    pub fn region(self) -> Aabb {
        let span = units(self.zoom);
        let (x0, y0, _) = self.rect();
        let z0 = self.z_range().0;
        Aabb {
            min: [x0, y0, z0],
            max: [x0 + span, y0 + span, z0 + span],
        }
    }

    pub fn node(self) -> (Zoom, [i64; 3]) {
        (self.zoom, self.at)
    }

    pub fn column(self) -> (Zoom, [i64; 2]) {
        let [x, y, _] = self.at;
        (self.zoom, [x, y])
    }

    /// None for a 3D root.
    pub fn parent(self) -> Option<Chunk> {
        (self.zoom > SHARD_ZOOM).then(|| Chunk {
            zoom: self.zoom - 1,
            at: parent_of(self.at),
        })
    }

    pub fn ancestors(self) -> impl Iterator<Item = Chunk> {
        std::iter::successors(self.parent(), |k| k.parent())
    }

    /// In `slot_of`'s order.
    pub fn children(self) -> [Chunk; CHILDREN] {
        std::array::from_fn(|slot| Chunk {
            zoom: self.zoom + 1,
            at: child_of(self.at, slot),
        })
    }
}
