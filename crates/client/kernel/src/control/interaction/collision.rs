// pxl-kernel/control/interaction/collision.rs

use pxl_spec::block::decor::Decor;
use pxl_spec::block::DecorId;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::zoom::{cube, index_of, units, Zoom};
use pxl_spec::space::layer::lattice::GROUND;

use crate::control::interaction::{ActiveTool, Holding};
use crate::layer::object::meta::user_decor_pick;
use crate::layer::object::size::{object_zoom_of, MARKER_ZOOM};
use crate::layer::object::store::{Block, Entry};
use crate::query::World;

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
/// Y, lowest then highest: its room's, and about it what decor grown under
/// it stands, turns and claims by — a decor's reach past it, the cells its
/// pose votes by about each — and the room's own width again, where
/// `World::editable` bounds it by the nodes about it.
pub fn placement_reads(tool: PlacementTool, corner: &Q3) -> [[f64; 2]; 2] {
    let room = room(tool, corner);
    let width = (room.max[0] - room.min[0]).max(room.max[1] - room.min[1]);
    let pad = crate::decor::strike::reach() + f64::from(pxl_spec_core::scatter::ALIGN_NEIGHBOR_RADIUS + 1) * units(GROUND.cell()) + width;
    [[room.min[0] - pad, room.min[1] - pad], [room.max[0] + pad, room.max[1] + pad]]
}

/// The shared check for the placement commit and its hover preview: its room
/// is where the world takes an edit (`World::editable`), and nothing placed,
/// nor a placed decor's box, shares it. A decor or a marker is stood on
/// nowhere, so neither is placed on one, grown or placed.
pub fn is_placement_blocked(tool: PlacementTool, corner: &Q3) -> bool {
    let (world, room) = (World::now(), room(tool, corner));
    let taken = !world.editable(&room) || world.placed_sharing(&room, |_| true).is_some() || world.placed_claim(&room).is_some();
    let under = Aabb {
        min: [room.min[0], room.min[1], room.min[2] - units(GROUND.cell())],
        max: [room.max[0], room.max[1], room.min[2]],
    };
    let on_instance = || world.placed_sharing(&under, |e| e.block() == Block::Marker).is_some() || world.placed_claim(&under).is_some() || world.grown_claim(&under).is_some();
    taken || !matches!(tool, PlacementTool::Object(_)) && on_instance()
}

/// The center of the cell a hover anchor (a min corner) points at: where a
/// tap recenters, which is where the ghost box is drawn.
pub fn anchor_center(anchor: &Q3) -> Q3 {
    let half = q3::from_real(units(GROUND.cell()) / 2.0);
    world::at(i64::from(anchor[0]) + half, i64::from(anchor[1]) + half, i64::from(anchor[2]) + half)
}

/// In the world's doubles, the space the pick works in.
pub fn object_aabb(obj: &Entry) -> Aabb {
    let min = crate::view::frame::coord::to_world(&obj.corner());
    let max = min.map(|v| v + obj.size());
    Aabb { min, max }
}
