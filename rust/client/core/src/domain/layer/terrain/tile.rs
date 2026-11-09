// pxl-core/domain/layer/terrain/tile.rs

//! Terrain tiles as a layer of the pyramid. A tile's slot is a room in
//! `.fixed.terrain` for the part read of the tile as a whole, its elevation
//! samples, decoded into it straight from the packet; its separation lines,
//! sized by the tile, are vectors of its own on the heap and go with it.
//! A tile's cells belong to the clip levels
//! (clipmap.rs): they cut what they want of them from the packet in the turn
//! it lands, and the packet is then dropped. A tile the bake left without an
//! object is drawn from the tile over it, whose cells there are open water;
//! at the source's top, such a tile is void.

use pxl_spec::bake::tile::terrain::{decode_into, TerrainTile, ELE_COLUMN};
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use pxl_spec::space::layer::quadrant::Quadrant;

use crate::domain::layer::terrain::clipmap;
use crate::domain::layer::terrain::fixed::{ele_off, RANGE};
use crate::domain::layer::tile_cache::TileCache;
use crate::domain::layer::Base;
use crate::sys::io::body::Packet;
use crate::sys::io::demand::{PhaseList, Stream, TaskPhaseName};
use crate::sys::io::failure::Exit;
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
    /// stands between arrival and drawing, and has the clip levels cut what
    /// they want from it before it is dropped. No terrain packet is kept
    /// past its turn. A heap that refuses its lines waits on the heap.
    fn receive(
        tile: Quadrant,
        slot: u32,
        pkt: Packet,
        phases: &mut PhaseList,
    ) -> Result<TerrainTile, Exit> {
        let t0 = now();
        // SAFETY: the slot is this tile's, and nothing reads it until the tile is set.
        let decoded = decode_into::<Exit>(&pkt, unsafe { ele_of(slot) })?;
        phases.push(TaskPhaseName::decode_ms, now() - t0);
        clipmap::cut(tile, &pkt);
        Ok(decoded)
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
