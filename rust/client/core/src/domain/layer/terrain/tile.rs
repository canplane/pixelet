// pxl-core/domain/layer/terrain/tile.rs

//! Terrain tiles as a layer of the pyramid. A tile's slot is a room in
//! `.fixed.terrain` for the parts read of the tile as a whole, its
//! separation lines and its elevation samples, which are decoded into it
//! straight from the packet. A tile's cells belong to the clip levels
//! (clipmap.rs): they cut what they want of them from the packet in the turn
//! it lands, and the packet is then freed. A tile the bake left without an
//! object is drawn from the tile over it, whose cells there are open water;
//! at the source's top, such a tile is void.

use crate::domain::layer::tile_cache::TileCache;
use pxl_spec::bake::separation;
use pxl_spec::bake::tile::terrain::{decode_into, TerrainTile, ELE_COLUMN};
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_core::mem;
use xpute_core::mem::heap::Block;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::sys::io::failure::Failure;

use crate::domain::layer::terrain::clipmap;
use crate::domain::layer::terrain::fixed::RANGE;
use crate::domain::layer::terrain::fixed::{ele_off, separation_column, SEPARATION_COLUMNS};
use crate::domain::layer::Base;
use crate::sys::io::demand::{PhaseList, Stream, TaskPhaseName};
use crate::sys::io::fetch;
use crate::sys::io::request;
use pxl_spec::bake::tile::Occupancy;
use xpute_core::clock::now;
use xpute_core::global::Global;
use xpute_core::mem::range::arr;

static TERRAIN: Global<TileCache<TerrainTile>> = Global::new();

impl Base for TerrainTile {
    const STREAM: Stream = Stream::terrain;
    const LATTICE: Lattice = GROUND;
    /// The tiles of the clip levels around the pivot and a destination.
    const PIN_CAP: u32 = clipmap::PINNED_TILES;

    fn occupancy(&self) -> Occupancy {
        self.occupancy
    }

    fn tile_cache() -> &'static mut TileCache<TerrainTile> {
        TERRAIN.get(TileCache::default)
    }

    /// Decodes the packet into the slot in one pass, so no tile-sized copy
    /// stands between arrival and drawing.
    fn receive(
        _tile: Quadrant,
        slot: u32,
        pkt: &[u8],
        phases: &mut PhaseList,
    ) -> Result<TerrainTile, Failure> {
        let t0 = now();
        // SAFETY: the slot is this tile's, and nothing reads it until the tile is set.
        let terrain = decode_into(pkt, unsafe { room_of(slot) }, unsafe { ele_of(slot) })?;
        phases.push(TaskPhaseName::decode_ms, now() - t0);
        Ok(terrain)
    }

    /// Lets the clip levels cut what they want from the packet, then frees
    /// it. No terrain packet is kept past its turn.
    fn keep(slot: Option<u32>, block: Block) {
        if let Some(t) = slot.and_then(|s| TerrainTile::tile_cache().in_slot(s)) {
            clipmap::cut(t, fetch::body(block));
        }
        fetch::release(Some(block));
    }

    fn empty() -> Option<TerrainTile> {
        Some(TerrainTile::default())
    }
}

/// How many tiles may be asked for their packets again now: the spare body
/// slots (sys/io/request.rs `spare_slots`) less the tiles already asked
/// again whose packets have not come (clipmap.rs `step`).
pub fn again_room() -> u32 {
    // A tile whose object failed is not counted, since it is not read again
    // while it is wanted.
    let coming = TerrainTile::tile_cache()
        .asked_again(|p| !request::has_object_failed(Stream::terrain, p.id()));
    request::spare_slots().saturating_sub(coming)
}

/// # Safety
/// As `room_of`'s.
unsafe fn ele_of(slot: u32) -> &'static mut [i16] {
    // SAFETY: as this function's, and the plane is plain numbers.
    unsafe { arr(RANGE.get(), ele_off(slot), ELE_COLUMN) }
}

/// # Safety
/// The slot's columns must belong to the tile given them, and to no other
/// tile, from now until another tile takes the slot.
unsafe fn room_of(slot: u32) -> separation::Room {
    /// Column `K` of the slot, as long as the layout makes it, with
    /// elements of the room's own type, which must be as wide as the layout
    /// counts them.
    ///
    /// # Safety
    /// As `room_of`'s.
    unsafe fn column<T, const K: usize>(slot: u32) -> &'static mut [T] {
        const {
            assert!(
                SEPARATION_COLUMNS[K].1 == size_of::<T>(),
                "a column's element as wide as the layout counts it"
            );
        };
        let (at, bytes) = separation_column(slot, K);
        // The layout keeps every column inside the range, so a column past
        // its end is a bug.
        let at = RANGE.get().at(at, bytes).or_bug(Errno::ENOTRECOVERABLE) as usize;
        // SAFETY: the column is core memory at its own size, never freed, and
        // the caller's contract makes it this tile's alone.
        unsafe { core::slice::from_raw_parts_mut(mem::at::<T>(at), SEPARATION_COLUMNS[K].0) }
    }
    // SAFETY: as this function's.
    unsafe {
        separation::Room {
            starts: column::<_, 0>(slot),
            points: column::<_, 1>(slot),
            tops: column::<_, 2>(slot),
            widths: column::<_, 3>(slot),
            kinds: column::<_, 4>(slot),
            forms: column::<_, 5>(slot),
            clears: column::<_, 6>(slot),
        }
    }
}
