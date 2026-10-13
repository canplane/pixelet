// pxl-kernel/control/interaction/collision.rs

use pxl_spec::block::decor::Decor;
use pxl_spec::block::DecorId;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::zoom::{cube, index_of, of_side, units, Zoom};
use pxl_spec::space::layer::lattice::GROUND;

use crate::layer::object::meta::user_decor_pick;
use crate::layer::object::size::MARKER_SIZE;
use crate::layer::object::store::{Block, Entry};
use crate::query::World;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementTool {
    Object,
    /// Its box, of the variant and turns its corner draws.
    Decor(DecorId),
    /// One cell.
    Marker,
}

/// The room a placement at `corner` takes, in units.
fn room(tool: PlacementTool, corner: &Q3, object_size: f64) -> Aabb {
    let cell = |zoom: Zoom| cube(zoom, index_of(zoom, corner));
    match tool {
        PlacementTool::Object => cell(of_side(object_size)),
        PlacementTool::Marker => cell(of_side(MARKER_SIZE)),
        PlacementTool::Decor(id) => {
            let decor = Decor::of(id);
            let (variant, turn) = user_decor_pick(corner, decor.variant_count());
            let (at, m) = decor.stand(cell(decor.cell_zoom()).min, variant, turn, 1.0);
            decor.claim(at, &m)
        }
    }
}

/// The shared check for the placement commit and its hover preview: its room
/// is where the world takes an edit (`World::editable`), and nothing placed,
/// nor a placed decor's box, shares it. A decor or a marker is stood on
/// nowhere, so neither is placed on one, grown or placed.
pub fn is_placement_blocked(tool: PlacementTool, corner: &Q3, object_size: f64) -> bool {
    let (world, room) = (World::now(), room(tool, corner, object_size));
    let taken = !world.editable(&room) || world.placed_sharing(&room, |_| true).is_some() || world.placed_claim(&room).is_some();
    let under = Aabb {
        min: [room.min[0], room.min[1], room.min[2] - units(GROUND.cell())],
        max: [room.max[0], room.max[1], room.min[2]],
    };
    let on_instance = || world.placed_sharing(&under, |e| e.block() == Block::Marker).is_some() || world.placed_claim(&under).is_some() || world.grown_claim(&under).is_some();
    taken || tool != PlacementTool::Object && on_instance()
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
