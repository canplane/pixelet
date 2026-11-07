// pxl-kernel/domain/layer/terrain/tile.rs

//! Terrain as a layer of the pyramid; a tile's record is a room in
//! `.fixed.terrain` for what is read of it whole, its lines, decoded into
//! straight from the packet. Its cells are its clip levels'
//! (domain/layer/terrain/clipmap.rs), filled off the packet, which is kept only
//! until the GPU has what they want of it (`uploads`). A tile the bake left as
//! no object is drawn from the tile over it, whose cells there are open water;
//! at the source's top, it is void.

use crate::domain::layer::tile_cache::TileCache;
use crate::domain::layer::upload::Uploads;
use pxl_spec::bake::separation;
use pxl_spec::bake::tile::terrain::{decode_into, TerrainTile};
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_core::mem;
use xpute_core::mem::heap::Block;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::sys::io::failure::Failure;

use crate::domain::layer::terrain::clipmap;
use crate::domain::layer::terrain::fixed::RANGE;
use crate::domain::layer::terrain::fixed::{separation_column, SEPARATION_COLUMNS};
use crate::domain::layer::Base;
use crate::sys::io::demand::{Phases, Stream, TaskPhaseName};
use crate::sys::io::fetch;
use crate::sys::io::request;
use pxl_spec::bake::tile::Occupancy;
use xpute_core::clock::now;
use xpute_core::global::Global;

static TERRAIN: Global<TileCache<TerrainTile>> = Global::new();

impl Base for TerrainTile {
    const STREAM: Stream = Stream::terrain;
    const LATTICE: Lattice = GROUND;
    /// The pivot's and a destination's clip levels' tiles.
    const PIN_CAP: u32 = clipmap::PINNED_TILES;

    fn occupancy(&self) -> Occupancy {
        self.occupancy
    }

    fn tile_cache() -> &'static mut TileCache<TerrainTile> {
        TERRAIN.get(TileCache::default)
    }

    fn empty() -> Option<TerrainTile> {
        Some(TerrainTile::void())
    }

    /// One pass from the packet: nothing tile-sized stands between arrival and
    /// drawing.
    fn receive(
        _tile: Quadrant,
        record: u32,
        pkt: &[u8],
        phases: &mut Phases,
    ) -> Result<TerrainTile, Failure> {
        let t0 = now();
        // SAFETY: the record is this tile's, and nothing reads it until the tile is set.
        let terrain = decode_into(pkt, unsafe { room_of(record) })?;
        phases.push(TaskPhaseName::decode_ms, now() - t0);
        Ok(terrain)
    }

    /// What its clip levels want of it is the GPU's, off the packet: kept only
    /// where a clip level wants one of its bricks, so an ancestor's or another
    /// zoom's packet no clip level reads takes no spare slot
    /// (sys/io/request.rs `spare_slots`).
    fn keep(record: Option<u32>, block: Block) {
        let wanted = |r: &u32| {
            TerrainTile::tile_cache()
                .in_slot(*r)
                .is_some_and(clipmap::is_tile_wanted)
        };
        match record {
            Some(r) if !wanted(&r) => {
                uploads().forget(r);
                fetch::release(Some(block));
            }
            _ => uploads().keep(record, block, request::spare_slots()),
        }
    }
}

/// How many tiles may be asked again for their packets now: the spare
/// slots (sys/io/request.rs `spare_slots`) less the packets kept, which only
/// a drawn frame gives back, and those asked again and not yet come, so
/// none is fetched only to be freed. A packet past them is freed once
/// decoded, and its tile asked again here (domain/layer/terrain/clipmap.rs
/// `step`).
pub fn again_room() -> u32 {
    // Not one whose object failed: it is not read again while it is wanted.
    let coming = TerrainTile::tile_cache()
        .asked_again(|p| !request::has_object_failed(Stream::terrain, p.id()));
    request::spare_slots().saturating_sub(uploads().cnt() + coming)
}

/// The tiles whose bricks the clip levels want filled, off their packets.
static UPLOADS: Global<Uploads> = Global::new();

pub fn uploads() -> &'static mut Uploads {
    UPLOADS.get(|| Uploads::new(TileCache::<TerrainTile>::SLOTS))
}

/// Whether `tile`'s packet is kept, its bricks still to be handed.
pub fn pending_for(tile: Quadrant) -> bool {
    TerrainTile::tile_cache()
        .slot(tile)
        .is_some_and(|f| uploads().waiting(f))
}

/// # Safety
/// The record's columns are the tile's that is given them, and no other's,
/// from now until another tile takes the record.
unsafe fn room_of(record: u32) -> separation::Room {
    /// Column `K` of the record, as long as the layout makes it, its element
    /// the room's own and as wide as the layout counts it.
    ///
    /// # Safety
    /// As `room_of`'s.
    unsafe fn column<T, const K: usize>(record: u32) -> &'static mut [T] {
        const {
            assert!(
                SEPARATION_COLUMNS[K].1 == size_of::<T>(),
                "a column's element as wide as the layout counts it"
            );
        };
        let (at, bytes) = separation_column(record, K);
        // Inside the range by the layout, so past its end is a bug.
        let at = RANGE.get().at(at, bytes).or_bug(Errno::ENOTRECOVERABLE) as usize;
        // SAFETY: the column is kernel memory at its own size, never freed, and
        // the caller's contract makes it this tile's alone.
        unsafe { core::slice::from_raw_parts_mut(mem::at::<T>(at), SEPARATION_COLUMNS[K].0) }
    }
    // SAFETY: as this function's.
    unsafe {
        separation::Room {
            starts: column::<_, 0>(record),
            points: column::<_, 1>(record),
            tops: column::<_, 2>(record),
            width: column::<_, 3>(record),
            kind: column::<_, 4>(record),
            form: column::<_, 5>(record),
            clear: column::<_, 6>(record),
        }
    }
}
