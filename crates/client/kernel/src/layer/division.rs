// pxl-kernel/layer/division.rs

//! The divisions as a layer of the pyramid, held over the focus alone: its
//! line is asked as a destination's, and the deepest tile held on it names
//! the cell. Nothing here is drawn; the name is read straight into the UI
//! snapshot. A tile's record is a room in `.fixed.division` for its cells,
//! decoded into straight from the packet; its divisions, a handful, are on
//! the heap.

use pxl_spec::bake::tile::division::{self, Division, DivisionTile, CELLS, DIVISION_LEAF};
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::zoom::{index_of, Zoom};
use pxl_spec::space::layer::lattice::Cells;
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};
use xpute_guest::global::Global;

use crate::io::demand::{Phases, Stream};
use crate::io::failure::Failure;
use crate::layer::residency::Residency;
use crate::layer::{Asks, Base};

static DIVISION: Global<Residency<DivisionTile>> = Global::new();

/// `.fixed.division`, handed in at boot.
pub static RANGE: crate::mem::fixed::Held = crate::mem::fixed::Held::new();

/// A record's cells, a word each, on a whole cache line.
const RECORD_BYTES: u32 = ((CELLS * 4).div_ceil(64) * 64) as u32;

/// A record a frame, end to end.
pub const DIVISION_BYTES: u32 = Residency::<DivisionTile>::FRAMES * RECORD_BYTES;

impl Base for DivisionTile {
    const STREAM: Stream = Stream::division;
    const LATTICE: Lattice = GROUND;
    const ASKS: Asks = Asks::Line;
    const LEAF: Zoom = DIVISION_LEAF;

    fn occupancy(&self) -> Occupancy {
        self.occupancy
    }

    fn residency() -> &'static mut Residency<DivisionTile> {
        DIVISION.get(Residency::default)
    }

    /// Its cells into its record; its divisions, a handful over the focus,
    /// held on the heap with their names decoded.
    fn receive(_tile: Tile, record: u32, pkt: &[u8], _phases: &mut Phases) -> Result<DivisionTile, Failure> {
        // SAFETY: the record is this tile's, and nothing reads it until the tile is set.
        let room = unsafe { crate::mem::fixed::lane::<u32>(RANGE.get(), record * RECORD_BYTES, CELLS) };
        Ok(division::decode_into(pkt, room)?)
    }
}

/// The finest tile of the focus's line, which demand asks for with the line.
pub fn line(focus: Option<Q3>) -> Option<Tile> {
    focus.map(|at| {
        let [x, y, _] = index_of(LEAF_ZOOM, &at);
        Tile::over(DivisionTile::LEAF, [x, y])
    })
}

/// Where the divisions of the cell `at` is in are named: the deepest tile
/// held over it, the held parent while the tile under it is on its way, and
/// the cell in it. A tile's identity fixes what it names.
pub fn naming(at: &Q3) -> Option<(Tile, u32)> {
    let from = line(Some(*at)).and_then(|p| DivisionTile::residency().drawn_from(p))?;
    Some((from, GROUND.cell_of(from.zoom, at).index()))
}

/// The divisions named at `naming`'s cell, broad to narrow.
pub fn named((from, cell): (Tile, u32), each: impl FnMut(&Division)) {
    if let Some(tile) = DivisionTile::residency().get(from) {
        tile.name_at(cell).for_each(each);
    }
}
