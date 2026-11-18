// pxl-core/domain/layer/imagery.rs

//! The imagery as a layer of the pyramid. Only its top zoom is
//! baked, and only the floor's tiles around the eye are held (`AskScope::Floor`,
//! `MIN_ZOOM`). A tile holds its packet until the GPU has its texels, and
//! then the device generation that does, and nothing in the core after a
//! frame takes them. Wherever no tile is held, the global picture answers.

use pxl_global::Global;
use pxl_spec::bake::publication::BaseKind;
use pxl_spec::bake::tile::imagery::{self, IMAGERY_ZOOM};
use pxl_spec::bake::tile::{Occupancy, Tile};
use pxl_spec::space::coord::world::wrap_idx;
use pxl_spec::space::layer::demand::{GROUND_DEMAND_ACROSS, GROUND_DEMAND_TILE_CAP};
use pxl_spec::space::layer::quadrant::Quadrant;

use crate::domain::layer::tile_cache::TileCache;
use crate::domain::layer::{AskScope, Base};
use crate::sys::io::body::Body;
use crate::sys::io::demand::{PhaseList, TaskPhaseName};
use crate::sys::io::failure::Exit;
use crate::sys::io::param::{BODY_SLOT_CNT, CREDIT_CNT};
use pxl_spec::space::coord::zoom::Zoom;
use xpute_guest::clock::now;

/// A held imagery tile: its packet until the GPU has its texels, whose words
/// then live in the GPU layer of its slot.
pub(crate) enum ImageryTile {
    /// The packet the GPU does not have yet. The tile owns the packet and
    /// lets it go with itself. Every slot may hold one, and the request
    /// scheduler leaves body slots for them (sys/io/request.rs `spare_slot_cnt`).
    Waiting(Body),
    /// Sent to the device of this generation (`device_generation`).
    Sent(u32),
}

static IMAGERY: Global<TileCache<ImageryTile>> = Global::new();

impl Tile for ImageryTile {
    const KIND: BaseKind = BaseKind::Imagery;

    /// Only the top zoom is baked, so no child has a tile.
    fn occupancy(&self) -> Occupancy {
        Occupancy::ZERO
    }
}

impl Base for ImageryTile {
    const ASK_SCOPE: AskScope = AskScope::Floor;
    const MIN_ZOOM: Option<Zoom> = Some(IMAGERY_ZOOM);

    fn tile_cache() -> &'static mut TileCache<ImageryTile> {
        IMAGERY.get(TileCache::default)
    }

    fn receive(
        _tile: Quadrant,
        _slot: u32,
        pkt: Body,
        phases: &mut PhaseList,
    ) -> Result<ImageryTile, Exit> {
        let at = now();
        imagery::decode(&pkt)?;
        phases.push(TaskPhaseName::DecodeMs, now() - at);
        Ok(ImageryTile::Waiting(pkt))
    }
}

/// Offers each waiting packet's texels to `write`, for its slot's layer on
/// the device of generation `on`, until `write` takes no more; a packet
/// written goes back with its tile's change to `Sent`. A tile whose texels
/// went to an earlier device is fetched again, since its packet is gone.
pub(crate) fn upload(write: &mut dyn FnMut(u32, &[u8]) -> bool, on: u32) {
    let cache = ImageryTile::tile_cache();
    for slot in 0..IMAGERY_SLOT_CNT {
        let Some((p, tile)) = cache.in_slot(slot) else {
            continue;
        };
        if matches!(tile, ImageryTile::Sent(s) if *s != on) {
            cache.refetch(p);
        }
    }
    for slot in 0..IMAGERY_SLOT_CNT {
        let Some((_, tile)) = cache.in_slot(slot) else {
            continue;
        };
        let ImageryTile::Waiting(pkt) = tile else {
            continue;
        };
        // `receive` decoded this packet already, so decoding it again does
        // not fail.
        if !imagery::decode(pkt).map_or(true, |texels| write(slot, texels)) {
            return;
        }
        *tile = ImageryTile::Sent(on);
    }
}

/// The tile cache as the GPU of generation `on` reads it, computed on each
/// call and stored nowhere. `floor` is the view's floor at the top zoom. The
/// result is the window's south-west tile and, for each tile of the window
/// row by row, its GPU layer plus one, or zero where the global picture
/// answers.
pub(crate) fn window(
    floor: impl Iterator<Item = Quadrant> + Clone,
    on: u32,
) -> ([i32; 2], [u32; GROUND_DEMAND_TILE_CAP as usize]) {
    let mut layers = [0; GROUND_DEMAND_TILE_CAP as usize];
    let at = |p: Quadrant| p.at;
    let Some([first, _]) = floor.clone().next().map(at) else {
        return ([0; 2], layers);
    };
    // X wraps around the globe, so the window's west column is the floor
    // tile that lies the fewest tiles east of the first one.
    let around = 1 << IMAGERY_ZOOM;
    let east = |x: i32| wrap_idx(IMAGERY_ZOOM, x - first);
    let x0 = first + floor.clone().map(|p| east(at(p)[0])).min().unwrap_or(0);
    let y0 = floor.clone().map(|p| at(p)[1]).min().unwrap_or(0);
    let cache = ImageryTile::tile_cache();
    for p in floor {
        let Some(slot) = cache.slot(p) else {
            continue;
        };
        if matches!(cache.get(p), Some(ImageryTile::Sent(s)) if *s == on) {
            let [x, y] = at(p);
            let (dx, dy) = ((x - x0).rem_euclid(around) as u32, (y - y0) as u32);
            layers[(dy * GROUND_DEMAND_ACROSS + dx) as usize] = slot + 1;
        }
    }
    ([x0, y0], layers)
}

/// The imagery tiles held at once, each in a GPU layer of its own.
pub(crate) const IMAGERY_SLOT_CNT: u32 = TileCache::<ImageryTile>::SLOT_CNT;

const _: () = assert!(
    BODY_SLOT_CNT > CREDIT_CNT + IMAGERY_SLOT_CNT,
    "the body slots keep a body on every credit and one more beside the imagery's frames"
);

#[cfg(test)]
#[path = "imagery.test.rs"]
mod test;
