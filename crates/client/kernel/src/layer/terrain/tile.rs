// pxl-kernel/layer/terrain/tile.rs

//! Terrain as a layer of the pyramid; a tile's record is a room in
//! `.fixed.terrain`, decoded into straight from the packet, which is kept
//! until the GPU has its looks and samples (`kept`); its cells, which the
//! GPU draws, come back into the room, and the tile is read from then on
//! (`back`). A tile the bake
//! left as no object is drawn from the tile over it, whose cells there are
//! open water; at the source's top, it is void.

use crate::layer::kept::Kept;
use crate::layer::residency::Residency;
use pxl_spec::bake::separation::{self, MAX_BOXES, MAX_LINES, MAX_POINTS};
use pxl_spec::bake::tile::terrain::{decode_into, GroundId, TerrainTile, TileRoom, GROUND_LANE, HEIGHT_LANE, LEVEL_LANE, WAYS_LANE};
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use pxl_spec::space::layer::tile::Tile;
use xpute_guest::mem::base::ptr_at;
use xpute_guest::mem::heap::Block;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::io::failure::Failure;

use crate::io::demand::{Phases, Stream, TaskPhaseName};
use crate::layer::terrain::fixed::RANGE;
use crate::layer::terrain::fixed::{
    back_off, ground_off, height_off, level_off, separation_off, ways_off, GROUND_LANE_BYTES, HEIGHT_LANE_BYTES, LEVEL_LANE_BYTES, SEPARATION_LANE_BYTES, WAYS_LANE_BYTES,
};
use crate::layer::Base;
use pxl_render::gpu::api_spec::read;
use pxl_render::gpu::renderer::CellsBack;
use pxl_spec::bake::tile::Occupancy;
use xpute_guest::clock::now;
use xpute_guest::global::Global;

static TERRAIN: Global<Residency<TerrainTile>> = Global::new();

impl Base for TerrainTile {
    const STREAM: Stream = Stream::terrain;
    const LATTICE: Lattice = GROUND;
    const AWAITS: bool = true;

    fn occupancy(&self) -> Occupancy {
        self.occupancy
    }
    // The leaves carry the terrain topic; the zooms above are read through the
    // chunks that draw them.

    fn residency() -> &'static mut Residency<TerrainTile> {
        TERRAIN.get(Residency::default)
    }

    fn empty() -> Option<TerrainTile> {
        Some(TerrainTile::void())
    }

    /// One pass from the packet: nothing tile-sized stands between arrival and
    /// drawing.
    fn receive(tile: Tile, record: u32, pkt: &[u8], phases: &mut Phases) -> Result<TerrainTile, Failure> {
        let t0 = now();
        // An earlier tile's cells still on their way no longer land here.
        words(record).fill(0);
        // SAFETY: the record is this tile's, and nothing reads it until the tile is set.
        let terrain = decode_into(pkt, tile.zoom, unsafe { room_of(record) })?;
        phases.push(TaskPhaseName::decode_ms, now() - t0);
        Ok(terrain)
    }

    /// The four looks a cell folds and the samples are the GPU's alone, off
    /// the packet.
    fn keep(record: Option<u32>, block: Block) {
        kept().keep(record, block);
    }
}

/// The tiles whose looks and samples the GPU does not hold yet, off their packets.
static KEPT: Global<Kept> = Global::new();

pub fn kept() -> &'static mut Kept {
    KEPT.get(|| Kept::new(Residency::<TerrainTile>::FRAMES))
}

/// The last ticket a frame's cells were asked back under.
static TICKET: Global<u32> = Global::new();

/// Frame `record`'s words, its heights' and its levels': 0 while nothing is
/// asked, the ticket asked, then the ticket with `read::DONE` once they are
/// in its room (pxl-render `Gpu::read_buffer`).
fn words(record: u32) -> &'static mut [u32; 2] {
    // Inside the range by the layout, so past its end is a bug.
    let at = RANGE.get().at(back_off(record), 8).or_bug(Errno::ENOTRECOVERABLE) as usize;
    // SAFETY: kernel memory, never freed, the record's own two words.
    unsafe { &mut *ptr_at::<[u32; 2]>(at) }
}

/// Whether frame `record`'s cells are back, heights and levels both.
pub fn back(record: u32) -> bool {
    words(record).iter().all(|w| w & read::DONE != 0)
}

/// Where frame `record`'s cells come back to, under a ticket of their own,
/// which the frame's words hold once the GPU takes the frame (`asked`).
pub fn cells_back(record: u32) -> CellsBack {
    let t = TICKET.get(|| 0);
    // Never 0, which asks nothing, nor DONE's bit.
    *t = (*t % (read::DONE - 1)) + 1;
    let lane = |off, bytes| RANGE.get().at(off, bytes).or_bug(Errno::ENOTRECOVERABLE);
    let words = lane(back_off(record), 8);
    CellsBack {
        heights: lane(height_off(record), HEIGHT_LANE_BYTES),
        levels: lane(level_off(record), LEVEL_LANE_BYTES),
        words: [words, words + 4],
        ticket: *t,
    }
}

/// The GPU took frame `record`'s tile: its cells come back under `ticket`.
pub fn asked(record: u32, ticket: u32) {
    *words(record) = [ticket; 2];
}

/// # Safety
/// The record's lanes are the tile's that is given them, and no other's,
/// from now until another tile takes the record.
unsafe fn room_of(record: u32) -> TileRoom {
    // Inside the range by the layout, so past its end is a bug.
    let lane = |off, bytes| RANGE.get().at(off, bytes).or_bug(Errno::ENOTRECOVERABLE) as usize;
    let (ground, heights, ways, levels) = (
        lane(ground_off(record), GROUND_LANE_BYTES),
        lane(height_off(record), HEIGHT_LANE_BYTES),
        lane(ways_off(record), WAYS_LANE_BYTES),
        lane(level_off(record), LEVEL_LANE_BYTES),
    );
    let separation_lane = |k: usize| lane(separation_off(record, k), SEPARATION_LANE_BYTES[k]);
    // SAFETY: each lane is kernel memory at its own size, never freed, and the
    // caller's contract makes it this tile's alone.
    unsafe {
        TileRoom {
            ground: core::slice::from_raw_parts_mut(ptr_at::<GroundId>(ground), GROUND_LANE),
            heights: core::slice::from_raw_parts_mut(ptr_at::<i16>(heights), HEIGHT_LANE),
            ways: core::slice::from_raw_parts_mut(ptr_at::<[i16; 2]>(ways), WAYS_LANE),
            levels: core::slice::from_raw_parts_mut(ptr_at::<i32>(levels), LEVEL_LANE),
            separation: separation::Room {
                starts: core::slice::from_raw_parts_mut(ptr_at::<u32>(separation_lane(0)), MAX_LINES + 1),
                points: core::slice::from_raw_parts_mut(ptr_at::<[i16; 2]>(separation_lane(1)), MAX_POINTS),
                tops: core::slice::from_raw_parts_mut(ptr_at::<i16>(separation_lane(2)), MAX_POINTS),
                width: core::slice::from_raw_parts_mut(ptr_at::<u8>(separation_lane(3)), MAX_LINES),
                kind: core::slice::from_raw_parts_mut(ptr_at::<u8>(separation_lane(4)), MAX_LINES),
                form: core::slice::from_raw_parts_mut(ptr_at::<u8>(separation_lane(5)), MAX_LINES),
                clear: core::slice::from_raw_parts_mut(ptr_at::<u8>(separation_lane(6)), MAX_LINES),
                boxes: core::slice::from_raw_parts_mut(ptr_at::<[i16; 4]>(separation_lane(7)), MAX_BOXES),
            },
        }
    }
}
