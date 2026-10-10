// pxl-kernel/layer/residency/floor.rs

//! The floor: every observation has an answer at some zoom, because a
//! fallback's tiles are held hard over every observed piece of ground. It is
//! asked for with every observation, not only when one finds nothing, or
//! every turn toward new ground would leave a hole for a round trip.
//!
//! Its zoom is the source's declared top (`declare`), never guessed from
//! what fails. Until declared, nothing is observed.

use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::layer::demand::{tiles_within, REACH_TILES};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::Tile;

use pxl_spec::space::coord::zoom::Zoom;
use xpute_guest::global::Global;

static DECLARED: Global<Option<Zoom>> = Global::new();

pub fn declare(top_z: Zoom) {
    xpute_core::ensure!(top_z >= GROUND.top(), EINVAL, top_z);
    *DECLARED.get(Default::default) = Some(top_z);
}

pub fn top() -> Option<Zoom> {
    *DECLARED.get(Default::default)
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
pub struct TopRing;

impl Fallback for TopRing {
    /// The far edge is at most the top's reach, and a sphere's cut by the
    /// ground is no wider, so these are at most the tiles a miss can name there.
    const TILES: u32 = REACH_TILES;

    fn floor(o: &Observation, top: Zoom, each: impl FnMut(Tile)) {
        let over = (o.eye[2] - o.ground).max(0.0);
        if over < o.far() {
            tiles_within(top, [o.eye[0], o.eye[1]], (o.far() * o.far() - over * over).sqrt()).for_each(each);
        }
    }
}

pub type Floor = TopRing;
