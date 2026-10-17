// pxl-kernel/layer/residency/floor.rs

//! The floor: every observation has an answer at some zoom, because a
//! fallback's tiles are held hard over every observed piece of ground. It is
//! asked for with every observation, not only when one finds nothing, or
//! every turn toward new ground would leave a hole for a round trip.
//!
//! Its zoom is the source's declared top (io/source.rs), never guessed from
//! what fails. Until declared, nothing is observed.

use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::layer::demand::{reach_tiles, tiles_within};
use pxl_spec::space::layer::tile::Tile;

use pxl_spec::space::coord::zoom::Zoom;

use crate::layer::Base;

pub fn top() -> Option<Zoom> {
    crate::io::source::top()
}

pub trait Fallback {
    const TILES: u32;

    /// `top`: the coarsest zoom the source supplies whole.
    fn floor(o: &Observation, top: Zoom, each: impl FnMut(Tile));
}

/// The top's tiles with ground within the far edge of the eye itself, which
/// hold every observed piece of ground. An eye higher than the edge holds none:
/// a pixel there covers more than the top's cell, and the view has gone over to
/// the Earth's picture.
pub struct TopRing<L>(core::marker::PhantomData<L>);

impl<L: Base> Fallback for TopRing<L> {
    /// The far edge is at most the top's reach, and a sphere's cut by the
    /// ground is no wider, so these are at most the tiles a miss can name
    /// there, in the layer's own tiles.
    const TILES: u32 = reach_tiles(L::LATTICE);

    fn floor(o: &Observation, top: Zoom, each: impl FnMut(Tile)) {
        if let Some(r) = ring(o) {
            tiles_within(top, [o.eye[0], o.eye[1]], r).for_each(each);
        }
    }
}

/// How far from the eye the top's ground is held: the far edge cut by the
/// ground; none where the eye is over the edge.
pub fn ring(o: &Observation) -> Option<f64> {
    let over = (o.eye[2] - o.ground).max(0.0);
    (over < o.reach).then(|| (o.reach * o.reach - over * over).sqrt())
}

pub type Floor<L> = TopRing<L>;
