// pxl-core/domain/query/detail.rs

//! The zoom a point of the world is read at: the camera grid's at its
//! distance from the eye, the one rule every reader of the world reads by.

use crate::domain::layer::Base as _;
use crate::domain::layer::terrain::tile::TerrainTile;
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

/// The cell zoom the eye's rays draw the ground with at `at`: the zoom a walk
/// reads there (`cell_zoom_read`), no finer than the ground's leaf cells and,
/// once the source declares its top, no coarser than the cells a tile draws.
pub(crate) fn cell_zoom_drawn(eye: V3, at: V3) -> Zoom {
    let cell_zoom = cell_zoom_read(eye, at).min(GROUND.leaf_cell_zoom());
    TerrainTile::min_zoom().map_or(cell_zoom, |min| cell_zoom.max(GROUND.grid_zoom(min)))
}

/// The leaf tile over `at`: what an answer lacks there where the leaves'
/// cells were not read, by an eye that draws the ground coarser.
pub(super) fn leaf_at(at: &Q3) -> Quadrant {
    let [x, y, _] = idx_of(LEAF_ZOOM, at);
    Quadrant::new(LEAF_ZOOM, [x, y])
}

/// The camera grid's level of detail (pxl-engine `cast::Detail`) for a walk
/// from `origin`, seen from `eye`: each node is asked at the cell zoom whose
/// cell, drawn whole, spans at least one grid cell at the node's distance
/// from the eye (pxl-spec space/camera/grid.rs `cell_zoom_at`), wherever the
/// walk's line starts, held within `zoom_range`. This is the rule that sizes
/// every drawn cell. The eye is taken on the origin's lap.
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

/// The cell zoom a walk asks at `at`: the camera grid's at its distance from
/// `eye`, held within `zoom_range` (`detail`).
pub(crate) fn cell_zoom_read(eye: V3, at: V3) -> Zoom {
    let d = detail(eye, at);
    d.at(d.eye[0] * d.eye[0] + d.eye[1] * d.eye[1] + d.eye[2] * d.eye[2])
}
