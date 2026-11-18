// pxl-core/domain/query/detail.rs

//! The size a place of the world is asked at: the camera grid's zoom at its
//! distance from the eye, the same in every direction, which is all the eye
//! does to a read (`grid_zoom`).

use crate::domain::layer::Base as _;
use crate::domain::layer::terrain::tile::IndexedTerrainTile;
use pxl_engine::cast::{Detail, Spacing};
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::{idx_of, nearest};
use pxl_spec::space::coord::zoom::{LEAF_ZOOM, MIN_ZOOM, Zoom};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::quadrant::Quadrant;

/// The coarsest zoom a cell is asked at: that of the coarsest cells a tile
/// draws.
pub(super) fn coarsest() -> Zoom {
    GROUND.grid_zoom(MIN_ZOOM)
}

/// The zooms a cell is asked at: no coarser than the cells a tile draws, and
/// no finer than what is placed is held.
pub(crate) fn zoom_range() -> [Zoom; 2] {
    [coarsest(), OBJECT_FINEST]
}

/// The cell zoom the ground is drawn with where a walk reads `read`: no finer
/// than the ground's leaf cells and, once the source declares its top, no
/// coarser than the cells a tile draws.
pub(super) fn drawn(read: Zoom) -> Zoom {
    let cell_zoom = read.min(GROUND.leaf_cell_zoom());
    IndexedTerrainTile::min_zoom().map_or(cell_zoom, |min| cell_zoom.max(GROUND.grid_zoom(min)))
}

/// The leaf tile over `at`: what an answer lacks there where the leaves'
/// cells were not read, by an eye that draws the ground coarser.
pub(super) fn leaf_at(at: &Q3) -> Quadrant {
    let [x, y, _] = idx_of(LEAF_ZOOM, at);
    Quadrant::new(LEAF_ZOOM, [x, y])
}

/// The size `eye` sets at a point: the camera grid's cell zoom at the
/// point's distance from it (pxl-spec space/camera/grid.rs `cell_zoom_at`),
/// the same in every direction, held within `zoom_range`. The point is over
/// `xy` at the height `z`, or, with none, at the eye's own height, the
/// column's point nearest the eye. It is the one thing a place's read takes
/// of the eye.
pub(super) fn grid_zoom(eye: V3, xy: [f64; 2], z: Option<f64>) -> Zoom {
    let d = detail(eye, [xy[0], xy[1], z.unwrap_or(eye[2])]);
    d.at(d.eye[0] * d.eye[0] + d.eye[1] * d.eye[1] + d.eye[2] * d.eye[2])
}

/// The level of detail (pxl-engine `cast::Detail`) of a ray across what the
/// eye sees, from `origin`: each node is asked at the size `eye` sets at the
/// node's own distance, held within `zoom_range`, as a pixel's ray asks it.
pub(super) fn detail(eye: V3, origin: V3) -> Detail {
    let eye = [
        nearest(eye[0], origin[0]) - origin[0],
        eye[1] - origin[1],
        eye[2] - origin[2],
    ];
    Detail {
        eye: eye.map(|v| v as f32),
        range: zoom_range(),
        spacing: Spacing::NONE,
    }
}

/// The level of detail of a walk that asks one size wherever it goes: the
/// cells of `zoom` alone (pxl-engine `cast::Detail`).
pub(super) fn one_zoom(zoom: Zoom) -> Detail {
    Detail {
        eye: [0.0; 3],
        range: [zoom, zoom],
        spacing: Spacing::NONE,
    }
}
