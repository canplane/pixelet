// pxl-core/domain/control/edit/collision.rs

use pxl_spec::asset::id::{kind_of, Kind};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::zoom::{cube, idx_of, units, Zoom};
use pxl_spec::space::layer::lattice::GROUND;

use crate::domain::layer::object::asset::Asset;
use crate::domain::layer::object::meta::user_decor_pick;
use crate::domain::layer::object::store::Entry;
use crate::domain::query::decor;
use crate::domain::query::World;
use crate::domain::view::frame::coord;

impl Asset {
    /// The room, in units, the asset takes when placed at `corner` with its
    /// cube at `zoom`. A decor claims its own box in the cell, and anything
    /// else takes the whole cell.
    fn room(self, zoom: Zoom, corner: &Q3) -> Aabb {
        let cell = cube(zoom, idx_of(zoom, corner));
        match self.decor() {
            Some(decor) => {
                let (variant, turn) = user_decor_pick(corner, decor);
                let (at, m) = decor.stand(cell.min, variant, turn, 1.0);
                decor.claim(at, &m)
            }
            None => cell,
        }
    }
}

/// The columns over which `is_placement_blocked` reads the world, in units,
/// as the lowest corner and then the highest, each X then Y. They cover the
/// room, padded on every side by the farthest a placed decor reaches past
/// its cell (`World::placed_occupying`) and by the room's own width again,
/// since `World::editable` judges the room by the nodes around it.
pub fn placement_reads(asset: Asset, zoom: Zoom, corner: &Q3) -> [[f64; 2]; 2] {
    let room = asset.room(zoom, corner);
    let width = (room.max[0] - room.min[0]).max(room.max[1] - room.min[1]);
    let pad = decor::box_extent() + width;
    [
        [room.min[0] - pad, room.min[1] - pad],
        [room.max[0] + pad, room.max[1] + pad],
    ]
}

/// Whether something of `kind` may stand on an instance of `under`. A kind
/// that stands (`Kind::stands`) goes only on a kind that can be stood on
/// (`Kind::stood_on`).
fn stands_on(kind: Kind, under: Kind) -> bool {
    !kind.stands() || under.stood_on()
}

/// Whether placing `asset` at `corner` is refused. The placement and its
/// hover preview share this check. A placement is allowed only where the
/// world takes an edit (`World::editable`), where nothing placed shares the
/// room the asset's kind occupies (`World::placed_occupying`), and where
/// nothing in the cell under it is something it may not stand on
/// (`stands_on`). Only a placed marker can refuse it that way, since the
/// ground generates no markers.
pub fn is_placement_blocked(asset: Asset, zoom: Zoom, corner: &Q3) -> bool {
    let (world, room) = (World::now(), asset.room(zoom, corner));
    let blocked = !world.editable(&room) || world.placed_occupying(&room, |_| true).is_some();
    let under = Aabb {
        min: [
            room.min[0],
            room.min[1],
            room.min[2] - units(GROUND.leaf_cell_zoom()),
        ],
        max: [room.max[0], room.max[1], room.min[2]],
    };
    let refuses = |e: &Entry| {
        kind_of(e.state).is_some_and(|under| asset.kind().is_some_and(|k| !stands_on(k, under)))
    };
    blocked || world.placed_occupying(&under, refuses).is_some()
}

/// The center of the leaf cell whose min corner is the hover anchor
/// `anchor`. A tap recenters there, and the ghost box is drawn there.
pub fn anchor_center(anchor: &Q3) -> Q3 {
    let half = q3::from_real(units(GROUND.leaf_cell_zoom()) / 2.0);
    world::at(
        i64::from(anchor[0]) + half,
        i64::from(anchor[1]) + half,
        i64::from(anchor[2]) + half,
    )
}

/// The box of a placed object in world doubles, the space the pick works in.
pub fn object_aabb(obj: &Entry) -> Aabb {
    let min = coord::to_world(&obj.corner());
    let max = min.map(|v| v + obj.size());
    Aabb { min, max }
}

#[cfg(test)]
#[path = "collision.test.rs"]
mod test;
