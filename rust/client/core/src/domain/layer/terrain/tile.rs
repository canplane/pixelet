// pxl-core/domain/layer/terrain/tile.rs

//! Terrain tiles as a layer of the pyramid. A tile's slot is a room in
//! `.fixed.terrain` for the part read of the tile as a whole, its elevation
//! samples, which are decoded into it straight from the packet; its
//! separation lines, sized by the tile, lie in a block of their own on the
//! heap, counted against the ways' pool's cap and freed with the tile
//! (pool.rs), and a tile with no line takes none.
//! A tile's cells belong to the clip levels
//! (clipmap.rs): they cut what they want of them from the packet in the turn
//! it lands, and the packet is then freed. A tile the bake left without an
//! object is drawn from the tile over it, whose cells there are open water;
//! at the source's top, such a tile is void.

use pxl_spec::bake::tile::terrain::{decode_into, TerrainTile, ELE_COLUMN};
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_core::mem::heap::Block;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::sys::io::failure::Exit;
use crate::sys::io::room::Refusal;

use crate::domain::layer::terrain::clipmap;
use crate::domain::layer::terrain::fixed::{ele_off, RANGE};
use crate::domain::layer::terrain::pool::{way_pool, PoolRefusal};
use crate::domain::layer::tile_cache::TileCache;
use crate::domain::layer::Base;
use crate::sys::io::demand::{PhaseList, Stream, TaskPhaseName};
use crate::sys::io::fetch;
use crate::sys::io::request;
use pxl_spec::bake::tile::Occupancy;
use xpute_core::clock::now;
use xpute_core::global::Global;
use xpute_core::mem::range::arr;

static TERRAIN: Global<TileCache<TerrainTile>> = Global::new();

/// Each tile slot's block of its separation, given back with its tile (`gone`). A tile with no
/// line takes none.
static BLOCKS: Global<[Option<Box<[u32]>>; TILE_SLOTS]> = Global::new();

const TILE_SLOTS: usize = TileCache::<TerrainTile>::SLOTS as usize;

fn blocks() -> &'static mut [Option<Box<[u32]>>; TILE_SLOTS] {
    BLOCKS.get(|| core::array::from_fn(|_| None))
}

/// Takes a block of `words` words for tile slot `slot`'s separation. A refusal a give-back or a
/// page can change waits as a want for room does (sys/io/request.rs `Wait::Room`).
fn take_block(slot: u32, words: usize) -> Result<&'static mut [u32], Exit> {
    match way_pool().take_tile(words as u32) {
        Ok(block) => {
            let held = &mut blocks()[slot as usize];
            *held = Some(block);
            let words = held.as_deref_mut().or_bug(Errno::ENOTRECOVERABLE);
            // SAFETY: the block is this tile's alone until it is given back with it (`gone`).
            Ok(unsafe { &mut *core::ptr::from_mut(words) })
        }
        Err(PoolRefusal::Cap) => Err(Exit::Refused(Refusal::ways())),
        Err(PoolRefusal::Heap) => Err(Exit::Refused(Refusal::now())),
    }
}

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
    ) -> Result<TerrainTile, Exit> {
        let t0 = now();
        // SAFETY: the slot is this tile's, and nothing reads it until the tile is set. The slot's
        // old tile, if any, gave its block back when it was removed (`gone`), before this ran.
        let terrain = decode_into(pkt, unsafe { ele_of(slot) }, |words| {
            take_block(slot, words)
        })?;
        phases.push(TaskPhaseName::decode_ms, now() - t0);
        Ok(terrain)
    }

    /// Gives back the block of the tile leaving slot `slot`, if it held one.
    fn gone(slot: u32) {
        if let Some(block) = blocks()[slot as usize].take() {
            way_pool().give_tile(block);
        }
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
/// The slot's samples must belong to the tile given them, and to no other
/// tile, from now until another tile takes the slot.
unsafe fn ele_of(slot: u32) -> &'static mut [i16] {
    // SAFETY: as this function's, and the plane is plain numbers.
    unsafe { arr(RANGE.get(), ele_off(slot), ELE_COLUMN) }
}
