// pxl-core/domain/layer/division.rs

//! The division layer: the named divisions of the map that hold the pivot.
//! Only the tiles over the pivot are held. Their line is asked for as a
//! destination's line is, and the finest tile held on it names the cell the
//! pivot is in. Nothing of this layer is drawn; the names go straight into
//! the UI snapshot.
//!
//! Each tile's record is a room in `.fixed.division`, and the tile's cells
//! are decoded into it straight from the packet. The tile's divisions, a
//! handful, live on the heap.

use pxl_spec::bake::tile::division::{self, Division, DivisionTile, CELLS, DIVISION_LEAF};
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::zoom::{idx_of, Zoom};
use pxl_spec::space::layer::lattice::Grid;
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use pxl_spec::space::layer::quadrant::{Quadrant, LEAF_ZOOM};
use xpute_core::global::Global;
use xpute_core::mem::range;
use xpute_core::mem::range::OwnedRange;

use crate::domain::layer::tile_cache::TileCache;
use crate::domain::layer::{AskScope, Base};
use crate::sys::io::demand::{PhaseList, Stream};
use crate::sys::io::failure::Failure;

static DIVISION: Global<TileCache<DivisionTile>> = Global::new();

/// The `.fixed.division` section, which the boot hands in.
pub static RANGE: OwnedRange = OwnedRange::new();

/// The bytes of one record: a word for each cell, rounded up to whole cache
/// lines.
const RECORD_BYTES: u32 = range::arr_bytes(CELLS, 4);

/// The size of `.fixed.division`: one record for each slot of the tile
/// cache, end to end.
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

    fn receive(
        _tile: Quadrant,
        record: u32,
        pkt: &[u8],
        _phases: &mut PhaseList,
    ) -> Result<DivisionTile, Failure> {
        // SAFETY: the record is this tile's, and nothing reads it until the tile is set.
        let room = unsafe { range::arr::<u32>(RANGE.get(), record * RECORD_BYTES, CELLS) };
        Ok(division::decode_into(pkt, room)?)
    }
}

/// The finest tile on the pivot's line. The demand asks for it together with
/// every tile over it.
pub fn line(pivot: Option<Q3>) -> Option<Quadrant> {
    pivot.map(|at| {
        let [x, y, _] = idx_of(LEAF_ZOOM, &at);
        Quadrant::over(DivisionTile::LEAF, [x, y])
    })
}

/// The tile and cell that name the divisions around `at`: the finest tile
/// held over `at`, which is a held parent while the finer tile is on its way,
/// and the index of `at`'s cell in that tile. The same tile always names the
/// same divisions.
pub fn naming(at: &Q3) -> Option<(Quadrant, u32)> {
    let from = line(Some(*at)).and_then(|p| DivisionTile::tile_cache().finest_resident(p))?;
    Some((from, GROUND.cell_of(from.zoom, at).idx()))
}

/// Calls `each` with the divisions named at a cell from `naming`, from the
/// broadest to the narrowest.
pub fn each_division((from, cell): (Quadrant, u32), each: impl FnMut(&Division)) {
    if let Some(tile) = DivisionTile::tile_cache().get(from) {
        tile.name_at(cell).for_each(each);
    }
}
