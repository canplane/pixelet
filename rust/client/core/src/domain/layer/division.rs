// pxl-core/domain/layer/division.rs

//! The division layer: the named divisions of the map that hold the pivot.
//! Only the tiles over the pivot are held. Their line is asked for as a
//! destination's line is, and the finest tile held on it names the cell the
//! pivot is in. Nothing of this layer is drawn; the names go straight into
//! the UI snapshot.
//!
//! Each tile's slot is a room in the division's fixed range, and the tile's cells
//! are decoded into it straight from the packet. The tile's divisions, a
//! handful, live on the heap.

use pxl_spec::bake::tile::division::{self, Division, DivisionTile, CELL_CNT};
use pxl_spec::bake::tile::Tile;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::zoom::LEAF_ZOOM;
use pxl_spec::space::coord::world::idx_of;
use pxl_spec::space::layer::lattice::{Grid, GROUND};
use pxl_spec::space::layer::quadrant::Quadrant;
use pxl_global::Global;

use crate::sys::io::body::Body;
use crate::domain::layer::tile_cache::TileCache;
use crate::domain::layer::{AskScope, Base};
use crate::sys::io::demand::PhaseList;
use crate::sys::io::failure::Exit;
use crate::sys::range::{arr_bytes, OwnedRange};

static DIVISION: Global<TileCache<DivisionTile>> = Global::new();

/// The division's range in `.fixed` (sys/range.rs), which the boot hands in.
pub(crate) static RANGE: OwnedRange = OwnedRange::new();

/// The bytes of one slot's room: a word for each cell, rounded up to whole cache
/// lines.
const ROOM_BYTES: u32 = arr_bytes(CELL_CNT, 4);

/// The size of the division's range: one room for each slot of the tile
/// cache, end to end.
pub(crate) const DIVISION_BYTES: u32 = TileCache::<DivisionTile>::SLOT_CNT * ROOM_BYTES;

impl Base for DivisionTile {
    const ASK_SCOPE: AskScope = AskScope::Line;

    fn tile_cache() -> &'static mut TileCache<DivisionTile> {
        DIVISION.get(TileCache::default)
    }

    fn receive(
        _tile: Quadrant,
        slot: u32,
        pkt: Body,
        _phases: &mut PhaseList,
    ) -> Result<DivisionTile, Exit> {
        // SAFETY: the slot is this tile's, and nothing reads it until the tile is set.
        let room = unsafe { RANGE.arr::<u32>(slot * ROOM_BYTES, CELL_CNT) };
        Ok(division::decode_into(&pkt, room)?)
    }
}

/// The finest tile on the pivot's line. The demand asks for it together with
/// every tile over it.
pub(crate) fn line(pivot: Option<Q3>) -> Option<Quadrant> {
    pivot.map(|at| {
        let [x, y, _] = idx_of(LEAF_ZOOM, &at);
        Quadrant::over(DivisionTile::LEAF, [x, y])
    })
}

/// The tile and cell that name the divisions around `at`: the finest tile
/// held over `at`, which is a held parent while the finer tile is on its way,
/// and the index of `at`'s cell in that tile. The same tile always names the
/// same divisions.
pub(crate) fn naming(at: &Q3) -> Option<(Quadrant, u32)> {
    let from = line(Some(*at)).and_then(|p| DivisionTile::tile_cache().finest_resident(p))?;
    Some((from, GROUND.cell_of(from.zoom, at).idx()))
}

/// Calls `each` with the divisions named at a cell from `naming`, from the
/// broadest to the narrowest.
pub(crate) fn each_division((from, cell): (Quadrant, u32), each: impl FnMut(Division<'_>)) {
    if let Some(tile) = DivisionTile::tile_cache().get(from) {
        tile.name_at(cell).for_each(each);
    }
}
