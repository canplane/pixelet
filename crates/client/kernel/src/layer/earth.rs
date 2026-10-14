// pxl-kernel/layer/earth.rs

//! The Earth's picture as a layer of the pyramid: only its top zoom is baked,
//! and only the floor's tiles about the eye are held, so it brings no demand,
//! zoom or schedule of its own. A tile's texels go to the GPU once and are
//! kept nowhere here; the global picture answers wherever no tile is held.

use pxl_spec::bake::tile::earth::{self, EARTH_ZOOM};
use pxl_spec::space::coord::world::{around, wrap_index};
use pxl_spec::space::layer::demand::{REACH_ACROSS, REACH_TILES};

use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use pxl_spec::space::layer::tile::Tile;
use xpute_guest::global::Global;
use xpute_guest::mem::heap::Block;

use crate::io::demand::{Phases, Stream};
use crate::io::failure::Failure;
use crate::layer::kept::Kept;
use crate::layer::residency::Residency;
use crate::layer::{Asks, Base};
use pxl_spec::space::coord::zoom::Zoom;

/// What a held tile is: its texels are in its frame's layer on the GPU.
#[derive(Clone)]
pub struct Earth;

static EARTH: Global<Residency<Earth>> = Global::new();

impl Base for Earth {
    const STREAM: Stream = Stream::earth;
    const LATTICE: Lattice = GROUND;
    const ASKS: Asks = Asks::Floor;
    const TOP: Option<Zoom> = Some(EARTH_ZOOM);

    /// Only the top zoom is baked: no child is a tile.
    fn occupancy(&self) -> Occupancy {
        Occupancy::ZERO
    }

    fn residency() -> &'static mut Residency<Earth> {
        EARTH.get(Residency::default)
    }

    fn receive(_tile: Tile, _record: u32, pkt: &[u8], _phases: &mut Phases) -> Result<Earth, Failure> {
        earth::decode(pkt)?;
        Ok(Earth)
    }

    /// The texels are kept until the GPU has them.
    fn keep(record: Option<u32>, block: Block) {
        kept().keep(record, block);
    }
}

/// The texels the GPU does not hold yet, off their packets.
static KEPT: Global<Kept> = Global::new();

fn kept() -> &'static mut Kept {
    KEPT.get(|| Kept::new(Residency::<Earth>::FRAMES))
}

/// Each waiting packet's texels to `write`, into its frame's layer, while it
/// takes them.
pub fn earth_upload(write: &mut dyn FnMut(u32, &[u8]) -> bool) {
    // A packet `receive` read reads again.
    kept().send(&mut |record, pkt| earth::decode(pkt).map_or(true, |texels| write(record, texels)));
}

/// The floor's tiles (`floor`, the view's) at the top as a window, its south-west tile first, and
/// per tile row by row its layer plus one, zero where the global picture
/// answers: residency as the GPU reads it, kept nowhere.
pub fn earth_window(floor: impl Iterator<Item = Tile> + Clone) -> ([i32; 2], [u32; REACH_TILES as usize]) {
    let mut layers = [0; REACH_TILES as usize];
    let at = |p: Tile| p.at;
    let Some([first, _]) = floor.clone().next().map(at) else { return ([0; 2], layers) };
    // X wraps: the window's west tile is the one fewest tiles east of the first.
    let around = around(EARTH_ZOOM);
    let east = |x: i64| wrap_index(EARTH_ZOOM, x - first);
    let x0 = first + floor.clone().map(|p| east(at(p)[0])).min().unwrap_or(0);
    let y0 = floor.clone().map(|p| at(p)[1]).min().unwrap_or(0);
    for p in floor {
        let Some(record) = Earth::residency().frame(p) else { continue };
        if !kept().waiting(record) {
            let [x, y] = at(p);
            let (dx, dy) = ((x - x0).rem_euclid(around) as u32, (y - y0) as u32);
            layers[(dy * REACH_ACROSS + dx) as usize] = record + 1;
        }
    }
    ([x0 as i32, y0 as i32], layers)
}

/// The frames the GPU keeps a layer for.
pub const EARTH_FRAMES: u32 = Residency::<Earth>::FRAMES;
