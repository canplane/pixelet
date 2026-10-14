// pxl-kernel/layer/terrain/tile.rs

//! Terrain as a layer of the pyramid; a tile's record is a room in
//! `.fixed.terrain`, decoded into straight from the packet. A tile the bake
//! left as no object is drawn from the tile over it, whose cells there are
//! open water; at the source's top, it is void.

use crate::layer::residency::Residency;
use pxl_spec::bake::separation::{self, MAX_BOXES, MAX_LINES, MAX_POINTS};
use pxl_spec::bake::tile::terrain::{decode_into, GroundId, TerrainTile, TileRoom, GROUND_LANE, HEIGHT_LANE, LEVEL_LANE, WAYS_LANE};
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use xpute_guest::mem::base::ptr_at;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::io::failure::Failure;

use crate::io::demand::{Phases, Stream, TaskPhaseName};
use crate::layer::terrain::fixed::RANGE;
use crate::layer::terrain::fixed::{ground_off, height_off, level_off, separation_off, ways_off, GROUND_LANE_BYTES, HEIGHT_LANE_BYTES, LEVEL_LANE_BYTES, SEPARATION_LANE_BYTES, WAYS_LANE_BYTES};
use crate::layer::Base;
use pxl_spec::bake::tile::Occupancy;
use xpute_guest::clock::now;
use xpute_guest::global::Global;

static TERRAIN: Global<Residency<TerrainTile>> = Global::new();

impl Base for TerrainTile {
    const STREAM: Stream = Stream::terrain;
    const LATTICE: Lattice = GROUND;

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
    fn receive(record: u32, pkt: &[u8], phases: &mut Phases) -> Result<TerrainTile, Failure> {
        let t0 = now();
        // SAFETY: the record is this tile's, and nothing reads it until the tile is set.
        let terrain = decode_into(pkt, unsafe { room_of(record) })?;
        phases.push(TaskPhaseName::decode_ms, now() - t0);
        Ok(terrain)
    }
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
