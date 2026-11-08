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
use crate::domain::query::decor::intersect;
use crate::domain::query::World;
use crate::domain::view::frame::coord;

impl Asset {
    /// The room it takes at `corner`, in units, its cube at `zoom`.
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

/// The columns `is_placement_blocked` reads the world over, in units, X then
/// Y, lowest then highest: its room's, and about it a placed decor's extent
/// past it, as `World::placed_occupying` reads them, and the room's own
/// width again, where `World::editable` bounds it by the nodes about it.
pub fn placement_reads(asset: Asset, zoom: Zoom, corner: &Q3) -> [[f64; 2]; 2] {
    let room = asset.room(zoom, corner);
    let width = (room.max[0] - room.min[0]).max(room.max[1] - room.min[1]);
    let pad = intersect::box_extent() + width;
    [
        [room.min[0] - pad, room.min[1] - pad],
        [room.max[0] + pad, room.max[1] + pad],
    ]
}

/// Whether what is of `kind` may stand over an instance of `under`: what
/// stands (`Kind::stands`) only on what is stood on (`Kind::stood_on`).
fn goes_on(kind: Kind, under: Kind) -> bool {
    !kind.stands() || under.stood_on()
}

/// The shared check for the placement commit and its hover preview: its room
/// is where the world takes an edit (`World::editable`), nothing placed
/// shares the room its kind occupies (`World::placed_occupying`), which
/// holds its cell, and nothing in the cell under it is one it may not stand
/// on (`goes_on`), a marker, as the ground generates none.
pub fn is_placement_blocked(asset: Asset, zoom: Zoom, corner: &Q3) -> bool {
    let (world, room) = (World::now(), asset.room(zoom, corner));
    let taken = !world.editable(&room) || world.placed_occupying(&room, |_| true).is_some();
    let under = Aabb {
        min: [
            room.min[0],
            room.min[1],
            room.min[2] - units(GROUND.leaf_cell_zoom()),
        ],
        max: [room.max[0], room.max[1], room.min[2]],
    };
    let refuses = |e: &Entry| {
        kind_of(e.state).is_some_and(|under| asset.kind().is_some_and(|k| !goes_on(k, under)))
    };
    taken || world.placed_occupying(&under, refuses).is_some()
}

/// The center of the cell a hover anchor (a min corner) points at: where a
/// tap recenters, which is where the ghost box is drawn.
pub fn anchor_center(anchor: &Q3) -> Q3 {
    let half = q3::from_real(units(GROUND.leaf_cell_zoom()) / 2.0);
    world::at(
        i64::from(anchor[0]) + half,
        i64::from(anchor[1]) + half,
        i64::from(anchor[2]) + half,
    )
}

/// In the world's doubles, the space the pick works in.
pub fn object_aabb(obj: &Entry) -> Aabb {
    let min = coord::to_world(&obj.corner());
    let max = min.map(|v| v + obj.size());
    Aabb { min, max }
}

#[cfg(test)]
#[path = "collision.test.rs"]
mod test;
