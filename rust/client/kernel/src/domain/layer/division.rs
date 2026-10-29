// pxl-kernel/domain/layer/division.rs

//! The divisions as a layer of the pyramid, held over the pivot alone: its
//! line is asked as a destination's, and the deepest tile held on it names
//! the cell. Nothing here is drawn; the name is read straight into the UI
//! snapshot. A tile's record is a room in `.fixed.division` for its cells,
//! decoded into straight from the packet; its divisions, a handful, are on
//! the heap.

use pxl_spec::bake::tile::division::{self, Division, DivisionTile, CELLS, DIVISION_LEAF};
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::zoom::{idx_of, Zoom};
use pxl_spec::space::layer::lattice::Cells;
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};
use xpute_guest::global::Global;

use crate::domain::layer::tile_cache::TileCache;
use crate::domain::layer::{AskScope, Base};
use crate::sys::io::demand::{Phases, Stream};
use crate::sys::io::failure::Failure;

static DIVISION: Global<TileCache<DivisionTile>> = Global::new();

/// `.fixed.division`, handed in at boot.
pub static RANGE: crate::sys::mem::fixed::Held = crate::sys::mem::fixed::Held::new();

/// A record's cells, a word each, on a whole cache line.
const RECORD_BYTES: u32 = crate::sys::mem::fixed::lane_bytes(CELLS, 4);

/// A record a frame, end to end.
pub const DIVISION_BYTES: u32 = TileCache::<DivisionTile>::SLOTS * RECORD_BYTES;

impl Base for DivisionTile {
    const STREAM: Stream = Stream::division;
    const LATTICE: Lattice = GROUND;
    const ASK_SCOPE: AskScope = AskScope::Line;
    const LEAF: Zoom = DIVISION_LEAF;

    fn occupancy(&self) -> Occupancy {
        self.occupancy
    }

    fn tile_cache() -> &'static mut TileCache<DivisionTile> {
        DIVISION.get(TileCache::default)
    }

    /// Its cells into its record; its divisions, a handful over the pivot,
    /// held on the heap with their names decoded.
    fn receive(
        _tile: Tile,
        record: u32,
        pkt: &[u8],
        _phases: &mut Phases,
    ) -> Result<DivisionTile, Failure> {
        // SAFETY: the record is this tile's, and nothing reads it until the tile is set.
        let room = unsafe {
            crate::sys::mem::fixed::lane::<u32>(RANGE.get(), record * RECORD_BYTES, CELLS)
        };
        Ok(division::decode_into(pkt, room)?)
    }
}

/// The finest tile of the pivot's line, which demand asks for with the line.
pub fn line(pivot: Option<Q3>) -> Option<Tile> {
    pivot.map(|at| {
        let [x, y, _] = idx_of(LEAF_ZOOM, &at);
        Tile::over(DivisionTile::LEAF, [x, y])
    })
}

/// Where the divisions of the cell `at` is in are named: the deepest tile
/// held over it, the held parent while the tile under it is on its way, and
/// the cell in it. A tile's identity fixes what it names.
pub fn naming(at: &Q3) -> Option<(Tile, u32)> {
    let from = line(Some(*at)).and_then(|p| DivisionTile::tile_cache().drawn_from(p))?;
    Some((from, GROUND.cell_of(from.zoom, at).idx()))
}

/// The divisions named at `naming`'s cell, broad to narrow.
pub fn each_division((from, cell): (Tile, u32), each: impl FnMut(&Division)) {
    if let Some(tile) = DivisionTile::tile_cache().get(from) {
        tile.name_at(cell).for_each(each);
    }
}
