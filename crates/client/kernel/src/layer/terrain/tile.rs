// pxl-kernel/layer/terrain/tile.rs

//! Terrain as a layer of the pyramid; a tile's record is a room in
//! `.fixed.terrain`, decoded into straight from the packet. A tile the bake
//! left as no object is drawn from the tile over it, whose cells there are
//! open water; at the source's top, it is void.

use crate::layer::residency::Residency;
use pxl_spec::bake::format::{decode_terrain_into, GroundCell, TerrainTile, TileRoom, COARSE_LANE, ELE_LANE, EXTENT_LANE, GROUND_LANE};
use pxl_spec::bake::separation::{self, MAX_BOXES, MAX_LINES, MAX_POINTS};
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_guest::mem::base::ptr_at;

use crate::io::failure::Failure;

use crate::io::demand::{Category, Phases, TaskPhaseName};
use crate::layer::terrain::fixed::RANGE;
use crate::layer::terrain::fixed::{coarse_off, ele_off, extent_off, ground_off, separation_off, COARSE_LANE_BYTES, ELE_LANE_BYTES, EXTENT_LANE_BYTES, GROUND_LANE_BYTES, SEPARATION_LANE_BYTES};
use crate::layer::Layer;
use pxl_spec::bake::format::Occupancy;
use xpute_guest::clock::now;
use xpute_guest::global::Global;

static TERRAIN: Global<Residency<TerrainTile>> = Global::new();

impl Layer for TerrainTile {
    const CATEGORY: Category = Category::terrain;
    const LATTICE: Lattice = GROUND;

    fn occupancy(&self) -> Occupancy {
        self.index.occupancy
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
        let terrain = decode_terrain_into(pkt, unsafe { room_of(record) })?;
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
    let (ground, ele, extent, coarse) = (
        lane(ground_off(record), GROUND_LANE_BYTES),
        lane(ele_off(record), ELE_LANE_BYTES),
        lane(extent_off(record), EXTENT_LANE_BYTES),
        lane(coarse_off(record), COARSE_LANE_BYTES),
    );
    let separation_lane = |k: usize| lane(separation_off(record, k), SEPARATION_LANE_BYTES[k]);
    // SAFETY: each lane is kernel memory at its own size, never freed, and the
    // caller's contract makes it this tile's alone.
    unsafe {
        TileRoom {
            ground: core::slice::from_raw_parts_mut(ptr_at::<GroundCell>(ground), GROUND_LANE),
            ele: core::slice::from_raw_parts_mut(ptr_at::<i16>(ele), ELE_LANE),
            extent: core::slice::from_raw_parts_mut(ptr_at::<[i16; 2]>(extent), EXTENT_LANE),
            coarse: core::slice::from_raw_parts_mut(ptr_at::<[i16; 2]>(coarse), COARSE_LANE),
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
