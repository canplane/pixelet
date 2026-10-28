// pxl-kernel/domain/control/interaction/collision.rs

use pxl_spec::asset::decor::Decor;
use pxl_spec::asset::id::{kind_of, Kind};
use pxl_spec::asset::DecorId;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::zoom::{cube, index_of, units, Zoom};
use pxl_spec::space::layer::lattice::GROUND;

use crate::domain::control::interaction::{ActiveTool, Holding};
use crate::domain::layer::object::meta::user_decor_pick;
use crate::domain::layer::object::size::{object_zoom_of, MARKER_ZOOM};
use crate::domain::layer::object::store::Entry;
use crate::domain::query::World;

/// What a placing tool puts down, each in the cube of its zoom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementTool {
    /// A cube at its tier's zoom.
    Object(Zoom),
    /// Its box, of the variant and turns its corner draws.
    Decor(DecorId),
    /// One cell.
    Marker,
}

impl PlacementTool {
    /// The tool `ui` holds, where it places; none for the pointer or the eraser.
    pub fn of(ui: &Holding) -> Option<PlacementTool> {
        match ui.active_tool {
            ActiveTool::Object => Some(PlacementTool::Object(object_zoom_of(ui.object_tier))),
            ActiveTool::Decor => Some(PlacementTool::Decor(ui.decor_id)),
            ActiveTool::Marker => Some(PlacementTool::Marker),
            ActiveTool::Pointer | ActiveTool::Eraser => None,
        }
    }

    /// The kind of what it places.
    pub fn kind(self) -> Kind {
        match self {
            PlacementTool::Object(_) => Kind::cube,
            PlacementTool::Decor(_) => Kind::decor,
            PlacementTool::Marker => Kind::marker,
        }
    }

    /// The zoom of the cube it places in.
    pub fn zoom(self) -> Zoom {
        match self {
            PlacementTool::Object(zoom) => zoom,
            PlacementTool::Decor(id) => Decor::of(id).row.cell_zoom(),
            PlacementTool::Marker => MARKER_ZOOM,
        }
    }

    /// The side of the cube it places in, in units.
    pub fn size(self) -> f64 {
        units(self.zoom())
    }
}

/// The room a placement at `corner` takes, in units.
fn room(tool: PlacementTool, corner: &Q3) -> Aabb {
    let zoom = tool.zoom();
    let cell = cube(zoom, index_of(zoom, corner));
    match tool {
        PlacementTool::Decor(id) => {
            let decor = Decor::of(id);
            let (variant, turn) = user_decor_pick(corner, decor);
            let (at, m) = decor.stand(cell.min, variant, turn, 1.0);
            decor.claim(at, &m)
        }
        PlacementTool::Object(_) | PlacementTool::Marker => cell,
    }
}

/// The columns `is_placement_blocked` reads the world over, in units, X then
/// Y, lowest then highest: its room's, and about it a placed decor's reach
/// past it, as `World::placed_occupying` reads them, and the room's own
/// width again, where `World::editable` bounds it by the nodes about it.
pub fn placement_reads(tool: PlacementTool, corner: &Q3) -> [[f64; 2]; 2] {
    let room = room(tool, corner);
    let width = (room.max[0] - room.min[0]).max(room.max[1] - room.min[1]);
    let pad = crate::domain::query::decor::strike::reach() + width;
    [
        [room.min[0] - pad, room.min[1] - pad],
        [room.max[0] + pad, room.max[1] + pad],
    ]
}

/// Whether what `tool` places may stand over an instance of `under`: what
/// stands (`Kind::stands`) only on what is stood on (`Kind::stood_on`).
fn goes_on(tool: Kind, under: Kind) -> bool {
    !tool.stands() || under.stood_on()
}

/// The shared check for the placement commit and its hover preview: its room
/// is where the world takes an edit (`World::editable`), nothing placed
/// shares the room its kind occupies (`World::placed_occupying`), which
/// holds its cell, and nothing in the cell under it is one it may not stand
/// on (`goes_on`), a marker, as the ground grows none.
pub fn is_placement_blocked(tool: PlacementTool, corner: &Q3) -> bool {
    let (world, room) = (World::now(), room(tool, corner));
    let taken = !world.editable(&room) || world.placed_occupying(&room, |_| true).is_some();
    let under = Aabb {
        min: [room.min[0], room.min[1], room.min[2] - units(GROUND.cell())],
        max: [room.max[0], room.max[1], room.min[2]],
    };
    let refuses = |e: &Entry| kind_of(e.state).is_some_and(|k| !goes_on(tool.kind(), k));
    taken || world.placed_occupying(&under, refuses).is_some()
}

/// The center of the cell a hover anchor (a min corner) points at: where a
/// tap recenters, which is where the ghost box is drawn.
pub fn anchor_center(anchor: &Q3) -> Q3 {
    let half = q3::from_real(units(GROUND.cell()) / 2.0);
    world::at(
        i64::from(anchor[0]) + half,
        i64::from(anchor[1]) + half,
        i64::from(anchor[2]) + half,
    )
}

/// In the world's doubles, the space the pick works in.
pub fn object_aabb(obj: &Entry) -> Aabb {
    let min = crate::domain::view::frame::coord::to_world(&obj.corner());
    let max = min.map(|v| v + obj.size());
    Aabb { min, max }
}

#[cfg(test)]
#[path = "collision.test.rs"]
mod test;
