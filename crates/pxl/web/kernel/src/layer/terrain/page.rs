// pxl-kernel/layer/terrain/page.rs

//! Terrain as a layer of the pyramid (world/pyramid.rs): a page is ground and
//! elevation, and its record is a room in `.fixed.terrain` (fixed.rs), where
//! a fetched body is decoded straight into place and the chunks read it.
//!
//! The bake leaves pure open ocean as no object — about 65% of the coverage
//! bbox — so a 404 is not a failure but "nothing but water here", and every
//! such page reads one shared tile: baking an empty source at different pages
//! was verified to produce identical bytes, so one decode serves them all. It
//! is fetched under its own category, so the pages waiting on it settle at
//! once and its one fetch is not tied to any of them.

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

/// Object key of the single shared open-ocean tile in the static bucket.
pub const OCEAN_KEY: &str = "_ocean";

/// The category the shared ocean tile is fetched under.
pub const OCEAN_CATEGORY: Category = Category::ocean;

static TERRAIN: Global<Pyramid<Terrain>> = Global::new();

impl Layer for Terrain {
    const CATEGORY: Category = Category::terrain;
    // The leaves carry the terrain topic, so its revision is what a reader of
    // the ground compares; the levels above are the view's alone and are read
    // through the chunks that draw them.

    fn pyramid() -> &'static mut Pyramid<Terrain> {
        TERRAIN.get(Pyramid::default)
    }

    /// Decoded into the record in one pass from the packet: the wire's planes
    /// are read where they lie and packed a cell at a time, so nothing
    /// page-sized stands between the page arriving and the ground being drawn
    /// from it.
    fn receive(record: u32, pkt: &[u8], phases: &mut Phases) -> Result<Terrain, Failure> {
        let t0 = now();
        // SAFETY: the record is this page's from the pyramid, and nothing
        // reads it until the page is set.
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

    /// A page coming or going changes the ground drawn under whatever it
    /// is the finest resident page over: a chunk is read from its own page or
    /// the nearest one over it, so the chunks compare their sources at the
    /// next turn. Which chunks a view draws is the cover's, made again from
    /// the pyramid's revision.
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
    let deck_lane = |k: usize| lane(deck_off(record, k), DECK_LANE_BYTES[k]);
    // SAFETY: each lane is a block of the kernel's memory at its own size,
    // placed once and never freed, and the caller's contract makes it this
    // page's alone.
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
                piers: core::slice::from_raw_parts_mut(ptr_at::<u8>(deck_lane(3)), MAX_POINTS),
                width: core::slice::from_raw_parts_mut(ptr_at::<u8>(deck_lane(4)), MAX_DECKS),
                kind: core::slice::from_raw_parts_mut(ptr_at::<u8>(deck_lane(5)), MAX_DECKS),
                form: core::slice::from_raw_parts_mut(ptr_at::<u8>(deck_lane(6)), MAX_DECKS),
            },
        }
    }
}

// ---- the shared ocean ----

struct Ocean {
    terrain: Option<Terrain>,
    /// Pages the bucket has no object for, waiting on the shared tile.
    waiting: Vec<LevelPage>,
}

static OCEAN: Global<Ocean> = Global::new();

fn ocean() -> &'static mut Ocean {
    OCEAN.get(|| Ocean { terrain: None, waiting: Vec::new() })
}

/// The shared tile's body: written into `ABSENT`, the record every open-ocean
/// page reads, and given to every page waiting on it.
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

// ---- the turn ----

/// Terrain's turn: its pyramid's, with the shared ocean tile asked for while
/// pages wait on it, and map_status following the queue.
pub fn terrain_step() {
    let terrain = Terrain::pyramid();
    // A page waiting on the shared tile is held as a page in `ABSENT` is:
    // while something asks for it.
    if eye().is_some() {
        ocean().waiting.retain(|&p| terrain.asked(p));
    }
    let o = ocean();
    let ocean_wants: &[(u64, Ask)] = if o.terrain.is_none() && !o.waiting.is_empty() { &[(0, Ask::PLAIN)] } else { &[] };
    declare(OCEAN_CATEGORY, ocean_wants);
    terrain.step(crate::control::navigation::destination::page(), crate::layer::decor::cell::pages());
    crate::world::pyramid::cover::step::<Terrain>();
    // Derived from the scheduler rather than tracked, so a later pan that
    // supersedes a batch cannot leave a stale SUCCESS behind. Nothing sets
    // Error: this reads only whether pages are pending, so a page that failed
    // is not told apart from one delivered, and the shell's Error toast is
    // never met. Known, and left alone.
    let map = map_store();
    if pending_count(Terrain::CATEGORY) + pending_count(OCEAN_CATEGORY) > 0 {
        map.map_status = MapStatus::Loading;
    } else if map.map_status == MapStatus::Loading {
        map.map_status = MapStatus::Success;
    }
}
