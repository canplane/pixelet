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

use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::bake::format::{decode_terrain, decode_terrain_into, Ground, Index, Occupancy, PageRoom, Terrain};
use pxl_spec::bake::pack::GroundCell;
use pxl_spec::space::pyramid::LevelPage;
use pxl_spec::space::pyramid::{EXTENT, PAGE_CELLS};
use xpute_runtime::abi::fetch::FetchFail;
use xpute_runtime::mem::base::ptr_at;

use crate::sched::profiler::record::Failure;

use crate::global::Global;
use crate::host::now;
use crate::io::demand::{Ask, Category, Phases, TaskPhaseName};
use crate::io::sched::{declare, pending_count};
use crate::layer::terrain::fixed::{ele_off, extent_off, ground_off, ELE_LANE_BYTES, EXTENT_LANE_BYTES, GROUND_LANE_BYTES};
use crate::layer::terrain::ground_snap::reground_map_center;
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
        let lanes = lanes_of(record)?;
        let room = PageRoom {
            // SAFETY: the record is this page's from the pyramid, at the
            // page's lane sizes, and nothing reads it until the page is set.
            cells: unsafe { core::slice::from_raw_parts_mut(ptr_at::<GroundCell>(lanes.cells), CELLS) },
            ele: unsafe { core::slice::from_raw_parts_mut(ptr_at::<i16>(lanes.ele), SAMPLES) },
            extent: unsafe { core::slice::from_raw_parts_mut(ptr_at::<[i16; 2]>(lanes.extent), EXTENTS) },
        };
        let occupancy = decode_terrain_into(pkt, room)?;
        phases.push(TaskPhaseName::decode_ms, now() - t0);
        // SAFETY: the lanes are this page's record until another page takes it.
        Ok(unsafe { open_terrain(lanes, occupancy) })
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
    /// is the finest resident page over, and the map center stands on the
    /// ground drawn. The chunks are told nothing: which a view draws is the
    /// cover's, made again from the pyramid's revision, and a chunk whose
    /// page has gone leaves it.
    fn changed(_p: LevelPage, _present: bool) {
        // A chunk is read from its own page or the nearest one over it, so a
        // page coming or going changes what some are read from.
        crate::world::chunk::source::chunk_sources_changed();
        reground_map_center();
    }
}

const CELLS: usize = (PAGE_CELLS * PAGE_CELLS) as usize;
const SAMPLES: usize = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
const EXTENTS: usize = (EXTENT.page_cells() * EXTENT.page_cells()) as usize;

/// A page's lanes in the range: the ground cells, packed (pxl-spec
/// bake/pack.rs), the padded elevation grid and the index lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TerrainLanes {
    cells: usize,
    ele: usize,
    extent: usize,
}

/// Opens a page's lanes as the terrain the kernel reads, with the occupancy
/// the page carries by value.
///
/// # Safety
/// Every lane is a block of the kernel's memory held for this page for as
/// long as the page is resident, at the page's fixed lane sizes.
unsafe fn open_terrain(lanes: TerrainLanes, occupancy: Occupancy) -> Terrain {
    // SAFETY: the caller's contract; a ground lane is one `GroundCell` a
    // cell, an i16 lane 2 bytes a sample, an index lane 4 bytes a cell.
    Terrain {
        ground: Ground {
            cells: unsafe { core::slice::from_raw_parts(ptr_at::<GroundCell>(lanes.cells), CELLS) }.into(),
        },
        ele: unsafe { core::slice::from_raw_parts(ptr_at::<i16>(lanes.ele), SAMPLES) }.into(),
        index: Index {
            extent: unsafe { core::slice::from_raw_parts(ptr_at::<[i16; 2]>(lanes.extent), EXTENTS) }.into(),
            occupancy,
        },
    }
}

/// Writes a decoded page into `lanes`, which must be the page's own room.
fn write_lanes(t: &Terrain, lanes: TerrainLanes) {
    let bytes = |p: *const u8, n: usize| {
        // SAFETY: a lane read as its own bytes, where it lies.
        unsafe { core::slice::from_raw_parts(p, n) }
    };
    let cells = &t.ground.cells;
    let extent = &t.index.extent;
    let sources = [
        bytes(cells.as_ptr().cast::<u8>(), cells.len() * core::mem::size_of::<GroundCell>()),
        bytes(t.ele.as_ptr().cast::<u8>(), t.ele.len() * 2),
        bytes(extent.as_ptr().cast::<u8>(), extent.len() * 4),
    ];
    for (src, to) in sources.iter().zip([lanes.cells, lanes.ele, lanes.extent]) {
        // SAFETY: `to` is the page's room in the range, the lane's own size,
        // and apart from `src`.
        unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), ptr_at::<u8>(to), src.len()) };
    }
}

/// Where a record's lanes are. NoRoom under a native test, which lays no
/// memory out.
fn lanes_of(record: u32) -> Result<TerrainLanes, FetchFail> {
    let lane = |off, bytes| at(Range::Terrain, off, bytes).map(|a| a as usize).ok_or(FetchFail::NoRoom);
    Ok(TerrainLanes {
        cells: lane(ground_off(record), GROUND_LANE_BYTES)?,
        ele: lane(ele_off(record), ELE_LANE_BYTES)?,
        extent: lane(extent_off(record), EXTENT_LANE_BYTES)?,
    })
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
    let decoded = decode_terrain(pkt)?;
    if decoded.ground.len() != CELLS || decoded.ele.len() != SAMPLES || decoded.index.extent.len() != EXTENTS {
        return Err(FetchFail::Decode.into());
    }
    let lanes = lanes_of(ABSENT)?;
    write_lanes(&decoded, lanes);
    // SAFETY: `ABSENT`, which no page takes and is written once.
    let terrain = unsafe { open_terrain(lanes, decoded.index.occupancy) };
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
    terrain.step();
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
