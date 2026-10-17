// pxl-kernel/query/changed.rs

//! Where the ground's answers moved since the frame turn before: the ground
//! tiles whose answer moved (their identities, which fix their contents) and
//! where the objects were written. A reader that stands on what it asked
//! asks again only where one of them touches the columns it read, so a
//! change stops at the ground it touches.

use crate::layer::Base as _;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::world::{nearest_box, REACH};
use pxl_spec::space::layer::tile::Tile;
use xpute_guest::global::Global;

#[derive(Default)]
struct Changed {
    ground: Vec<Tile>,
    written: Vec<Aabb>,
}

static CHANGED: Global<Changed> = Global::new();

fn changed() -> &'static mut Changed {
    CHANGED.get(Changed::default)
}

/// The frame turn's changes, taken from the ground's residency and the
/// object store in place of the last turn's; true where any moved.
pub(crate) fn take() -> bool {
    let c = changed();
    TerrainTile::residency().take_moved(&mut c.ground);
    crate::layer::object::store::take_written(&mut c.written);
    !c.ground.is_empty() || !c.written.is_empty()
}

/// Where the objects were written, as the last take found them.
pub fn written() -> &'static [Aabb] {
    &changed().written
}

/// Whether a ground change, or one of `layer`'s tiles a reader also reads
/// whose answer moved, touches the columns over `xy`, its corners in units,
/// X on any lap. Taken once a frame turn, in its visible phase, after its
/// arrivals are admitted, and held until the next; what moves between frame
/// turns waits in the residency and the store for the next take. So a
/// reader asking every frame turn, in its effects or its interaction phase,
/// sees each change exactly once.
pub fn within(xy: [[f64; 2]; 2], layer: &[Tile]) -> bool {
    let columns = Aabb {
        min: [xy[0][0], xy[0][1], REACH.min[2]],
        max: [xy[1][0], xy[1][1], REACH.max[2]],
    };
    let about = (columns.min[0] + columns.max[0]) / 2.0;
    let c = changed();
    let mut regions = c.ground.iter().chain(layer).map(|t| t.region()).chain(c.written.iter().copied());
    regions.any(|r| nearest_box(&r, about).shares(columns))
}
