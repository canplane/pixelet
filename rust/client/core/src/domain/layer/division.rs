// pxl-core/domain/layer/division.rs

//! The division layer: the named divisions of the map that hold the pivot.
//! Only the tiles over the pivot are held. The leaf over it is pinned with
//! its ancestors, and the finest of them that is resident names the cell the
//! pivot is in. Nothing of this layer is drawn; the names go straight into
//! the UI snapshot.
//!
//! Each tile's slot lies in the division's fixed range, and the tile's cells
//! are decoded into it straight from the packet. The tile's divisions, a
//! handful, live on the heap.

use pxl_global::Global;
use pxl_spec::bake::tile::Tile;
use pxl_spec::bake::tile::division::{self, CELL_CNT, Division, DivisionTile};
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::world::idx_of;
use pxl_spec::space::coord::zoom::LEAF_ZOOM;
use pxl_spec::space::layer::lattice::{GROUND, Grid};
use pxl_spec::space::layer::quadrant::Quadrant;

use crate::domain::layer::tile_cache::TileCache;
use crate::domain::layer::{AskScope, Base};
use crate::sys::io::body::Body;
use crate::sys::io::demand::{PhaseList, TaskPhaseName};
use crate::sys::io::failure::Exit;
use crate::sys::range::{OwnedRange, arr_sz};
use xpute_guest::clock::now;

static DIVISION: Global<TileCache<DivisionTile>> = Global::new();

/// The division's range in `.fixed` (sys/range.rs), which the boot hands in.
pub(crate) static RANGE: OwnedRange = OwnedRange::new();

/// One slot's bytes: a word for each cell, rounded up to whole cache lines.
const SLOT_SZ: u32 = arr_sz(CELL_CNT, 4);

/// The size of the division's range: each slot of the tile cache, end to
/// end.
pub(crate) const DIVISION_SZ: u32 = TileCache::<DivisionTile>::SLOT_CNT * SLOT_SZ;

impl Base for DivisionTile {
    const ASK_SCOPE: AskScope = AskScope::Pinned;

    fn tile_cache() -> &'static mut TileCache<DivisionTile> {
        DIVISION.get(TileCache::default)
    }

    fn receive(
        _tile: Quadrant,
        slot: u32,
        pkt: Body,
        phases: &mut PhaseList,
    ) -> Result<DivisionTile, Exit> {
        let at = now();
        // SAFETY: the slot is this tile's, and nothing reads it until the tile is set.
        let cells = unsafe { RANGE.arr::<u32>(slot * SLOT_SZ, CELL_CNT) };
        let tile = division::decode_into(&pkt, cells)?;
        phases.push(TaskPhaseName::DecodeMs, now() - at);
        Ok(tile)
    }
}

/// The leaf tile over the pivot. The demand pins it with its ancestors.
pub(crate) fn tile(pivot: Option<Q3>) -> Option<Quadrant> {
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
    let from = tile(Some(*at)).and_then(|p| DivisionTile::tile_cache().finest_resident(p))?;
    Some((from, GROUND.cell_of(from.zoom, at).idx()))
}

/// Calls `each` with the divisions named at a cell from `naming`, from the
/// broadest to the narrowest.
pub(crate) fn each_division((from, cell): (Quadrant, u32), each: impl FnMut(Division<'_>)) {
    if let Some(tile) = DivisionTile::tile_cache().get(from) {
        tile.name_at(cell).for_each(each);
    }
}
