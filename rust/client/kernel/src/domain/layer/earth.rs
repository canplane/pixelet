// pxl-kernel/domain/layer/earth.rs

//! The Earth's picture as a layer of the pyramid: only its top zoom is baked,
//! and only the floor's tiles about the eye are held, so it brings no demand,
//! zoom or schedule of its own. A tile's texels go to the GPU once and are
//! kept nowhere here; the global picture answers wherever no tile is held.

use pxl_spec::bake::tile::earth::{self, EARTH_ZOOM};
use pxl_spec::space::layer::demand::{REACH_ACROSS, REACH_TILES};
use pxl_spec_core::wrap_idx;

use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use pxl_spec::space::layer::tile::Tile;
use xpute_guest::global::Global;
use xpute_guest::mem::heap::Block;

use crate::domain::layer::kept::{Kept, Offered};
use crate::domain::layer::tile_cache::TileCache;
use crate::domain::layer::{AskScope, Base};
use crate::sys::io::demand::{Phases, Stream};
use crate::sys::io::failure::Failure;
use pxl_spec::space::coord::zoom::Zoom;

/// What a held tile is: its texels are in its frame's layer on the GPU.
#[derive(Clone)]
pub struct Earth;

static EARTH: Global<TileCache<Earth>> = Global::new();

impl Base for Earth {
    const STREAM: Stream = Stream::earth;
    const LATTICE: Lattice = GROUND;
    const ASK_SCOPE: AskScope = AskScope::Floor;
    const TOP: Option<Zoom> = Some(EARTH_ZOOM);

    /// Only the top zoom is baked: no child is a tile.
    fn occupancy(&self) -> Occupancy {
        Occupancy::ZERO
    }

    fn tile_cache() -> &'static mut TileCache<Earth> {
        EARTH.get(TileCache::default)
    }

    fn receive(
        _tile: Tile,
        _record: u32,
        pkt: &[u8],
        _phases: &mut Phases,
    ) -> Result<Earth, Failure> {
        earth::decode(pkt)?;
        Ok(Earth)
    }

    /// The texels are kept until the GPU has them: every frame's, whose
    /// slots terrain's bound leaves them (domain/layer/terrain/tile.rs `most_kept`).
    fn keep(record: Option<u32>, block: Block) {
        kept().keep(record, block, EARTH_SLOTS);
    }
}

/// The texels the GPU does not hold yet, off their packets.
static KEPT: Global<Kept> = Global::new();

fn kept() -> &'static mut Kept {
    KEPT.get(|| Kept::new(TileCache::<Earth>::SLOTS))
}

/// Each waiting packet's texels to `write`, into its frame's layer, while it
/// takes them, on the device of generation `on`; a tile whose texels went
/// to a device since gone is fetched again, as its packet was freed once
/// sent.
pub fn earth_upload(write: &mut dyn FnMut(u32, &[u8]) -> bool, on: u32) {
    let kept = kept();
    for record in 0..EARTH_SLOTS {
        if kept.sent_on(record).is_some_and(|s| s != on) {
            if let Some(p) = Earth::tile_cache().in_slot(record) {
                Earth::tile_cache().again(p);
            }
        }
    }
    // A packet `receive` read reads again.
    // An Earth packet is wanted until its texels are written, at most a
    // frame's four of them kept.
    kept.send(
        on,
        &mut |record, pkt| match earth::decode(pkt).map(|texels| write(record, texels)) {
            Err(_) | Ok(true) => Offered::Taken,
            Ok(false) => Offered::Full,
        },
        &|_| true,
    );
}

/// The floor's tiles (`floor`, the view's) at the top as a window, its south-west tile first, and
/// per tile row by row its layer plus one, zero where the global picture
/// answers: the tile cache as the GPU of generation `on` reads it, kept nowhere.
pub fn earth_window(
    floor: impl Iterator<Item = Tile> + Clone,
    on: u32,
) -> ([i32; 2], [u32; REACH_TILES as usize]) {
    let mut layers = [0; REACH_TILES as usize];
    let at = |p: Tile| p.at;
    let Some([first, _]) = floor.clone().next().map(at) else {
        return ([0; 2], layers);
    };
    // X wraps: the window's west tile is the one fewest tiles east of the first.
    let around = 1 << EARTH_ZOOM;
    let east = |x: i32| wrap_idx(EARTH_ZOOM, x - first);
    let x0 = first + floor.clone().map(|p| east(at(p)[0])).min().unwrap_or(0);
    let y0 = floor.clone().map(|p| at(p)[1]).min().unwrap_or(0);
    for p in floor {
        let Some(record) = Earth::tile_cache().slot(p) else {
            continue;
        };
        if kept().sent_on(record) == Some(on) {
            let [x, y] = at(p);
            let (dx, dy) = ((x - x0).rem_euclid(around) as u32, (y - y0) as u32);
            layers[(dy * REACH_ACROSS + dx) as usize] = record + 1;
        }
    }
    ([x0, y0], layers)
}

/// The frames the GPU keeps a layer for.
pub const EARTH_SLOTS: u32 = TileCache::<Earth>::SLOTS;
