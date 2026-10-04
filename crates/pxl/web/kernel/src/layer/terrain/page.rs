// pxl-kernel/layer/terrain/page.rs

//! Terrain as a layer of the pyramid; a page's record is a room in
//! `.fixed.terrain`, decoded into straight from the packet. The bake leaves
//! open ocean (about 65% of the bbox) as no object, so a 404 means water and
//! every such page shares one decoded tile, fetched under its own category.

use pxl_spec::bake::deck::{self, MAX_BOXES, MAX_DECKS, MAX_POINTS};
use pxl_spec::bake::format::{decode_terrain_into, Block, PageRoom, Terrain, BLOCK_LANE, ELE_LANE, EXTENT_LANE, GROUND_LANE};
use pxl_spec::bake::pack::GroundCell;
use pxl_spec::space::pyramid::Page;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_runtime::mem::base::ptr_at;

use crate::sched::profiler::record::Failure;

use crate::io::demand::{Category, Phases, TaskPhaseName};
use crate::layer::terrain::fixed::{blocks_off, deck_off, ele_off, extent_off, ground_off, BLOCK_LANE_BYTES, DECK_LANE_BYTES, ELE_LANE_BYTES, EXTENT_LANE_BYTES, GROUND_LANE_BYTES};
use crate::mem::fixed::{at, Range};
use crate::world::pyramid::{Absent, Layer, Pyramid};
use xpute_runtime::clock::now;
use xpute_runtime::global::Global;

pub use pxl_spec::bake::format::OCEAN_KEY;

pub const OCEAN_CATEGORY: Category = Category::ocean;

static TERRAIN: Global<Pyramid<Terrain>> = Global::new();

impl Layer for Terrain {
    const CATEGORY: Category = Category::terrain;
    const ON_ABSENT: Absent = Absent::Shared(OCEAN_CATEGORY);
    // The leaves carry the terrain topic; the zooms above are read through the
    // chunks that draw them.

    fn pyramid() -> &'static mut Pyramid<Terrain> {
        TERRAIN.get(Pyramid::default)
    }

    /// One pass from the packet: nothing page-sized stands between arrival and
    /// drawing.
    fn receive(record: u32, pkt: &[u8], phases: &mut Phases) -> Result<Terrain, Failure> {
        let t0 = now();
        // SAFETY: the record is this page's, and nothing reads it until the page is set.
        let terrain = decode_terrain_into(pkt, unsafe { room_of(record) })?;
        phases.push(TaskPhaseName::decode_ms, now() - t0);
        Ok(terrain)
    }

    /// The ground under whatever this page is the finest resident one over
    /// changes, so chunks compare their sources next turn.
    fn changed(_p: Page, _present: bool) {
        crate::world::chunk::source::chunk_sources_changed();
    }
}

/// # Safety
/// The record's lanes are the page's that is given them, and no other's,
/// from now until another page takes the record.
unsafe fn room_of(record: u32) -> PageRoom {
    // Inside the range by the layout, so past its end is a bug.
    let lane = |off, bytes| at(Range::Terrain, off, bytes).or_bug(Errno::ENOTRECOVERABLE) as usize;
    let (cells, ele, extent, blocks) = (
        lane(ground_off(record), GROUND_LANE_BYTES),
        lane(ele_off(record), ELE_LANE_BYTES),
        lane(extent_off(record), EXTENT_LANE_BYTES),
        lane(blocks_off(record), BLOCK_LANE_BYTES),
    );
    let deck_lane = |k: usize| lane(deck_off(record, k), DECK_LANE_BYTES[k]);
    // SAFETY: each lane is kernel memory at its own size, never freed, and the
    // caller's contract makes it this page's alone.
    unsafe {
        PageRoom {
            cells: core::slice::from_raw_parts_mut(ptr_at::<GroundCell>(cells), GROUND_LANE),
            ele: core::slice::from_raw_parts_mut(ptr_at::<i16>(ele), ELE_LANE),
            extent: core::slice::from_raw_parts_mut(ptr_at::<[i16; 2]>(extent), EXTENT_LANE),
            blocks: core::slice::from_raw_parts_mut(ptr_at::<Block>(blocks), BLOCK_LANE),
            decks: deck::Room {
                starts: core::slice::from_raw_parts_mut(ptr_at::<u32>(deck_lane(0)), MAX_DECKS + 1),
                points: core::slice::from_raw_parts_mut(ptr_at::<[i16; 2]>(deck_lane(1)), MAX_POINTS),
                tops: core::slice::from_raw_parts_mut(ptr_at::<i16>(deck_lane(2)), MAX_POINTS),
                width: core::slice::from_raw_parts_mut(ptr_at::<u8>(deck_lane(3)), MAX_DECKS),
                kind: core::slice::from_raw_parts_mut(ptr_at::<u8>(deck_lane(4)), MAX_DECKS),
                form: core::slice::from_raw_parts_mut(ptr_at::<u8>(deck_lane(5)), MAX_DECKS),
                boxes: core::slice::from_raw_parts_mut(ptr_at::<[i16; 4]>(deck_lane(6)), MAX_BOXES),
            },
        }
    }
}

pub fn terrain_step() {
    let terrain = Terrain::pyramid();
    terrain.step(crate::control::navigation::destination::page());
}
