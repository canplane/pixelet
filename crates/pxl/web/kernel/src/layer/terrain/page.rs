// pxl-kernel/layer/terrain/page.rs

//! Terrain as a layer of the pyramid; a page's record is a room in
//! `.fixed.terrain`, decoded into straight from the packet. The bake leaves
//! open ocean (about 65% of the bbox) as no object, so a 404 means water and
//! every such page shares one decoded tile, fetched under its own category.

use pxl_spec::bake::format::{decode_terrain_into, Block, PageRoom, Terrain, BLOCK_LANE, ELE_LANE, EXTENT_LANE, GROUND_LANE};
use pxl_spec::bake::pack::GroundCell;
use pxl_spec::space::pyramid::LevelPage;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_runtime::mem::base::ptr_at;

use crate::sched::profiler::record::Failure;

use crate::io::demand::{Category, Phases, TaskPhaseName};
use crate::io::sched::pending_count;
use crate::layer::terrain::fixed::{blocks_off, deck_off, ele_off, extent_off, ground_off, BLOCK_LANE_BYTES, DECK_LANE_BYTES, ELE_LANE_BYTES, EXTENT_LANE_BYTES, GROUND_LANE_BYTES};
use crate::mem::fixed::{at, Range};
use crate::shell::map_store::{map_store, MapStatus};
use crate::world::pyramid::{Absent, Layer, Pyramid};
use xpute_runtime::clock::now;
use xpute_runtime::global::Global;

pub use pxl_spec::bake::format::OCEAN_KEY;

pub const OCEAN_CATEGORY: Category = Category::ocean;

static TERRAIN: Global<Pyramid<Terrain>> = Global::new();

impl Layer for Terrain {
    const CATEGORY: Category = Category::terrain;
    const ON_ABSENT: Absent = Absent::Shared(OCEAN_CATEGORY);
    // The leaves carry the terrain topic; the levels above are read through the
    // chunks that draw them.

    fn pyramid() -> &'static mut Pyramid<Terrain> {
        TERRAIN.get(Pyramid::default)
    }

    /// One pass from the packet: nothing page-sized stands between arrival and
    /// drawing.
    fn receive(record: u32, pkt: &[u8], phases: &mut Phases) -> Result<Terrain, Failure> {
        let t0 = now();
        // SAFETY: the record is this page's from the pyramid, and nothing
        // reads it until the page is set.
        let terrain = decode_terrain_into(pkt, unsafe { room_of(record) })?;
        phases.push(TaskPhaseName::decode_ms, now() - t0);
        Ok(terrain)
    }

    /// The ground under whatever this page is the finest resident one over
    /// changes, so chunks compare their sources next turn.
    fn changed(_p: LevelPage, _present: bool) {
        crate::world::chunk::source::chunk_sources_changed();
    }
}

/// A record's lanes, as the room a page is written into.
///
/// # Safety
/// The record's lanes are the page's that is given them, and no other's,
/// from now until another page takes the record.
unsafe fn room_of(record: u32) -> PageRoom {
    // A record's lanes are inside the range by its layout (layer/terrain/
    // fixed.rs), so one past its end is a bug.
    let lane = |off, bytes| at(Range::Terrain, off, bytes).or_bug(Errno::ENOTRECOVERABLE) as usize;
    let (cells, ele, extent, blocks) = (
        lane(ground_off(record), GROUND_LANE_BYTES),
        lane(ele_off(record), ELE_LANE_BYTES),
        lane(extent_off(record), EXTENT_LANE_BYTES),
        lane(blocks_off(record), BLOCK_LANE_BYTES),
    );
    // SAFETY: each lane is a block of the kernel's memory at its own size,
    // placed once and never freed, and the caller's contract makes it this
    // page's alone.
    unsafe {
        PageRoom {
            cells: core::slice::from_raw_parts_mut(ptr_at::<GroundCell>(cells), GROUND_LANE),
            ele: core::slice::from_raw_parts_mut(ptr_at::<i16>(ele), ELE_LANE),
            extent: core::slice::from_raw_parts_mut(ptr_at::<[i16; 2]>(extent), EXTENT_LANE),
            blocks: core::slice::from_raw_parts_mut(ptr_at::<Block>(blocks), BLOCK_LANE),
        }
    }
}

pub fn terrain_step() {
    let terrain = Terrain::pyramid();
    terrain.step(crate::control::navigation::destination::page());
    crate::world::pyramid::cover::step::<Terrain>();
    // Derived from the scheduler so a superseded batch cannot leave a stale
    // Success. A failed fetch keeps its want and is retried, so it reads as Loading.
    let map = map_store();
    if pending_count(Terrain::CATEGORY) + pending_count(OCEAN_CATEGORY) > 0 {
        map.map_status = MapStatus::Loading;
    } else if map.map_status == MapStatus::Loading {
        map.map_status = MapStatus::Success;
    }
}
