// pxl-core/domain/layer/earth.rs

//! The Earth's picture as a layer of the pyramid. Only its top zoom is
//! baked, and only the floor's tiles around the eye are held, so the layer
//! needs no demand, zoom or schedule of its own. A tile's texels go to the
//! GPU once and are not kept in the core. Wherever no tile is held, the
//! global picture answers.

use pxl_engine::wrap_idx;
use pxl_spec::bake::tile::earth::{self, EARTH_ZOOM};
use pxl_spec::space::layer::demand::{GROUND_DEMAND_ACROSS, GROUND_DEMAND_TILES};

use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_core::global::Global;
use xpute_core::mem::heap::Block;

use crate::domain::layer::tile_cache::TileCache;
use crate::domain::layer::upload::{Uploads, Uptake};
use crate::domain::layer::{AskScope, Base};
use crate::sys::io::demand::{Phases, Stream};
use crate::sys::io::failure::Failure;
use crate::sys::io::param::{BODY_SLOTS, MAX_CREDITS};
use pxl_spec::space::coord::zoom::Zoom;

/// A held Earth tile. It carries nothing in the core: its texels live in the
/// GPU layer of its slot.
#[derive(Clone)]
pub struct Earth;

static EARTH: Global<TileCache<Earth>> = Global::new();

impl Base for Earth {
    const STREAM: Stream = Stream::earth;
    const LATTICE: Lattice = GROUND;
    const ASK_SCOPE: AskScope = AskScope::Floor;
    const TOP: Option<Zoom> = Some(EARTH_ZOOM);

    /// Only the top zoom is baked, so no child has a tile.
    fn occupancy(&self) -> Occupancy {
        Occupancy::ZERO
    }

    fn tile_cache() -> &'static mut TileCache<Earth> {
        EARTH.get(TileCache::default)
    }

    fn receive(
        _tile: Quadrant,
        _record: u32,
        pkt: &[u8],
        _phases: &mut Phases,
    ) -> Result<Earth, Failure> {
        earth::decode(pkt)?;
        Ok(Earth)
    }

    /// Keeps the packet until the GPU has its texels. Every slot may keep
    /// one, and the request scheduler leaves body slots for them
    /// (sys/io/request.rs `spare_slots`).
    fn keep(record: Option<u32>, block: Block) {
        uploads().keep(record, block, EARTH_SLOTS);
    }
}

/// The packets whose texels the GPU does not hold yet.
static UPLOADS: Global<Uploads> = Global::new();

fn uploads() -> &'static mut Uploads {
    UPLOADS.get(|| Uploads::new(TileCache::<Earth>::SLOTS))
}

/// Offers each waiting packet's texels to `write`, for its slot's layer on
/// the device of generation `on`, until `write` takes no more. A tile whose
/// texels went to an earlier device is fetched again, since its packet was
/// freed once sent.
pub fn earth_upload(write: &mut dyn FnMut(u32, &[u8]) -> bool, on: u32) {
    let uploads = uploads();
    for record in 0..EARTH_SLOTS {
        if uploads.sent_on(record).is_some_and(|s| s != on) {
            if let Some(p) = Earth::tile_cache().in_slot(record) {
                Earth::tile_cache().again(p);
            }
        }
    }
    // `receive` decoded each of these packets already, so decoding one again
    // does not fail. A packet waits until its texels are written, so at most
    // one packet waits for each slot.
    uploads.send(
        on,
        &mut |record, pkt| match earth::decode(pkt).map(|texels| write(record, texels)) {
            Err(_) | Ok(true) => Uptake::Taken,
            Ok(false) => Uptake::Full,
        },
    );
}

/// The tile cache as the GPU of generation `on` reads it, computed on each
/// call and stored nowhere. `floor` is the view's floor at the top zoom. The
/// result is the window's south-west tile and, for each tile of the window
/// row by row, its GPU layer plus one, or zero where the global picture
/// answers.
pub fn earth_window(
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
    for p in floor {
        let Some(record) = Earth::tile_cache().slot(p) else {
            continue;
        };
        if uploads().sent_on(record) == Some(on) {
            let [x, y] = at(p);
            let (dx, dy) = ((x - x0).rem_euclid(around) as u32, (y - y0) as u32);
            layers[(dy * GROUND_DEMAND_ACROSS + dx) as usize] = record + 1;
        }
    }
    ([x0, y0], layers)
}

/// The Earth tiles held at once, each in a GPU layer of its own.
pub const EARTH_SLOTS: u32 = TileCache::<Earth>::SLOTS;

const _: () = assert!(
    BODY_SLOTS > MAX_CREDITS + EARTH_SLOTS,
    "the body slots keep a body on every credit and one more beside the Earth's frames"
);
