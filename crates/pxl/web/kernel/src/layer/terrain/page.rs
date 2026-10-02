// pxl-kernel/layer/terrain/page.rs

//! Terrain as a layer of the pyramid; a page's record is a room in
//! `.fixed.terrain`, decoded into straight from the packet. The bake leaves
//! open ocean (about 65% of the bbox) as no object, so a 404 means water and
//! every such page shares one decoded tile, fetched under its own category.

use pxl_spec::bake::deck::{self, MAX_DECKS, MAX_POINTS};
use pxl_spec::bake::format::{decode_terrain_into, Block, PageRoom, Terrain, BLOCK_LANE, ELE_LANE, EXTENT_LANE, GROUND_LANE};
use pxl_spec::bake::pack::GroundCell;
use pxl_spec::space::pyramid::LevelPage;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_runtime::mem::base::ptr_at;

use crate::sched::profiler::record::Failure;

use crate::global::Global;
use crate::host::now;
use crate::io::demand::{Ask, Category, Phases, TaskPhaseName};
use crate::io::sched::{declare, pending_count};
use crate::layer::terrain::fixed::{blocks_off, deck_off, ele_off, extent_off, ground_off, BLOCK_LANE_BYTES, DECK_LANE_BYTES, ELE_LANE_BYTES, EXTENT_LANE_BYTES, GROUND_LANE_BYTES};
use crate::mem::fixed::{at, Range};
use crate::shell::map_store::{map_store, MapStatus};
use crate::world::observation::eye;
use crate::world::paging::ABSENT;
use crate::world::pyramid::{Layer, Pyramid};

pub const OCEAN_KEY: &str = "_ocean";

pub const OCEAN_CATEGORY: Category = Category::ocean;

static TERRAIN: Global<Pyramid<Terrain>> = Global::new();

impl Layer for Terrain {
    const CATEGORY: Category = Category::terrain;
    // The leaves carry the terrain topic; the levels above are read through the
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

    fn absent(p: LevelPage) {
        let o = ocean();
        match &o.terrain {
            Some(t) => Terrain::pyramid().set(p, t.clone()),
            None if !o.waiting.contains(&p) => o.waiting.push(p),
            None => {}
        }
    }

    /// The ground under whatever this page is the finest resident one over
    /// changes, so chunks compare their sources next turn.
    fn changed(_p: LevelPage, _present: bool) {
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
                portal: core::slice::from_raw_parts_mut(ptr_at::<u16>(deck_lane(6)), MAX_POINTS),
            },
        }
    }
}

struct Ocean {
    terrain: Option<Terrain>,
    waiting: Vec<LevelPage>,
}

static OCEAN: Global<Ocean> = Global::new();

fn ocean() -> &'static mut Ocean {
    OCEAN.get(|| Ocean { terrain: None, waiting: Vec::new() })
}

/// Written into `ABSENT`, the record every open-ocean page reads.
pub fn adopt_ocean(pkt: &[u8]) -> Result<(), Failure> {
    if ocean().terrain.is_some() {
        return Ok(());
    }
    // SAFETY: `ABSENT`, which no page takes and is written once.
    let terrain = decode_terrain_into(pkt, unsafe { room_of(ABSENT) })?;
    let o = ocean();
    for p in o.waiting.drain(..) {
        Terrain::pyramid().set(p, terrain.clone());
    }
    o.terrain = Some(terrain);
    Ok(())
}

pub fn terrain_step() {
    let terrain = Terrain::pyramid();
    // Held while something asks for it, as a page in `ABSENT` is.
    if eye().is_some() {
        ocean().waiting.retain(|&p| terrain.asked(p));
    }
    let o = ocean();
    let ocean_wants: &[(u64, Ask)] = if o.terrain.is_none() && !o.waiting.is_empty() { &[(0, Ask::PLAIN)] } else { &[] };
    declare(OCEAN_CATEGORY, ocean_wants);
    terrain.step(crate::control::navigation::destination::page(), crate::layer::decor::cell::pages(), f64::INFINITY);
    crate::world::pyramid::cover::step::<Terrain>();
    // Derived from the scheduler so a superseded batch cannot leave a stale
    // Success. Nothing sets Error, so the shell's Error toast is never met; known,
    // and left alone.
    let map = map_store();
    if pending_count(Terrain::CATEGORY) + pending_count(OCEAN_CATEGORY) > 0 {
        map.map_status = MapStatus::Loading;
    } else if map.map_status == MapStatus::Loading {
        map.map_status = MapStatus::Success;
    }
}
