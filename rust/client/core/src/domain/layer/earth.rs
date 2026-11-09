// pxl-core/domain/layer/earth.rs

//! The Earth's picture as a layer of the pyramid. Only its top zoom is
//! baked, and only the floor's tiles around the eye are held (`AskScope::Floor`,
//! `MIN_ZOOM`). A tile holds its packet until the GPU has its texels, and
//! then the device generation that does, and nothing in the core after a
//! frame takes them. Wherever no tile is held, the global picture answers.

use pxl_spec::bake::publication::BaseLayer;
use pxl_spec::bake::tile::earth::{self, EARTH_ZOOM};
use pxl_spec::bake::tile::{Occupancy, Tile};
use pxl_spec::space::coord::world::wrap_idx;
use pxl_spec::space::layer::demand::{GROUND_DEMAND_ACROSS, GROUND_DEMAND_TILES};
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_core::global::Global;

use crate::domain::layer::tile_cache::TileCache;
use crate::domain::layer::{AskScope, Base};
use crate::sys::io::body::Packet;
use crate::sys::io::demand::PhaseList;
use crate::sys::io::failure::Exit;
use crate::sys::io::param::{BODY_SLOTS, MAX_CREDITS};
use pxl_spec::space::coord::zoom::Zoom;

/// A held Earth tile: its packet until the GPU has its texels, whose words
/// then live in the GPU layer of its slot.
pub enum EarthTile {
    /// The packet the GPU does not have yet. The tile owns the packet and
    /// lets it go with itself. Every slot may hold one, and the request
    /// scheduler leaves body slots for them (sys/io/request.rs `spare_slots`).
    Waiting(Packet),
    /// Sent to the device of this generation (`device_generation`).
    Sent(u32),
}

static EARTH: Global<TileCache<EarthTile>> = Global::new();

impl Tile for EarthTile {
    const LAYER: BaseLayer = BaseLayer::earth;

    /// Only the top zoom is baked, so no child has a tile.
    fn occupancy(&self) -> Occupancy {
        Occupancy::ZERO
    }
}

impl Base for EarthTile {
    const ASK_SCOPE: AskScope = AskScope::Floor;
    const MIN_ZOOM: Option<Zoom> = Some(EARTH_ZOOM);

    fn tile_cache() -> &'static mut TileCache<EarthTile> {
        EARTH.get(TileCache::default)
    }

    fn receive(
        _tile: Quadrant,
        _slot: u32,
        pkt: Packet,
        _phases: &mut PhaseList,
    ) -> Result<EarthTile, Exit> {
        earth::decode(&pkt)?;
        Ok(EarthTile::Waiting(pkt))
    }
}

/// Offers each waiting packet's texels to `write`, for its slot's layer on
/// the device of generation `on`, until `write` takes no more; a packet
/// written goes back with its tile's change to `Sent`. A tile whose texels
/// went to an earlier device is fetched again, since its packet is gone.
pub fn upload(write: &mut dyn FnMut(u32, &[u8]) -> bool, on: u32) {
    let cache = EarthTile::tile_cache();
    for slot in 0..EARTH_SLOTS {
        let Some((p, tile)) = cache.in_slot(slot) else {
            continue;
        };
        if matches!(tile, EarthTile::Sent(s) if *s != on) {
            cache.refetch(p);
        }
    }
    for slot in 0..EARTH_SLOTS {
        let Some((_, tile)) = cache.in_slot(slot) else {
            continue;
        };
        let EarthTile::Waiting(pkt) = tile else {
            continue;
        };
        // `receive` decoded this packet already, so decoding it again does
        // not fail.
        if !earth::decode(pkt).map_or(true, |texels| write(slot, texels)) {
            return;
        }
        *tile = EarthTile::Sent(on);
    }
}

/// The tile cache as the GPU of generation `on` reads it, computed on each
/// call and stored nowhere. `floor` is the view's floor at the top zoom. The
/// result is the window's south-west tile and, for each tile of the window
/// row by row, its GPU layer plus one, or zero where the global picture
/// answers.
pub fn window(
    floor: impl Iterator<Item = Quadrant> + Clone,
    on: u32,
) -> ([i32; 2], [u32; GROUND_DEMAND_TILES as usize]) {
    let mut layers = [0; GROUND_DEMAND_TILES as usize];
    let at = |p: Quadrant| p.at;
    let Some([first, _]) = floor.clone().next().map(at) else {
        return ([0; 2], layers);
    };
    // X wraps around the globe, so the window's west column is the floor
    // tile that lies the fewest tiles east of the first one.
    let around = 1 << EARTH_ZOOM;
    let east = |x: i32| wrap_idx(EARTH_ZOOM, x - first);
    let x0 = first + floor.clone().map(|p| east(at(p)[0])).min().unwrap_or(0);
    let y0 = floor.clone().map(|p| at(p)[1]).min().unwrap_or(0);
    let cache = EarthTile::tile_cache();
    for p in floor {
        let Some(slot) = cache.slot(p) else {
            continue;
        };
        if matches!(cache.get(p), Some(EarthTile::Sent(s)) if *s == on) {
            let [x, y] = at(p);
            let (dx, dy) = ((x - x0).rem_euclid(around) as u32, (y - y0) as u32);
            layers[(dy * GROUND_DEMAND_ACROSS + dx) as usize] = slot + 1;
        }
    }
    ([x0, y0], layers)
}

/// The Earth tiles held at once, each in a GPU layer of its own.
pub const EARTH_SLOTS: u32 = TileCache::<EarthTile>::SLOTS;

const _: () = assert!(
    BODY_SLOTS > MAX_CREDITS + EARTH_SLOTS,
    "the body slots keep a body on every credit and one more beside the Earth's frames"
);

#[cfg(test)]
#[path = "earth.test.rs"]
mod test;
