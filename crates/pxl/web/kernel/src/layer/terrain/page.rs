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
use pxl_spec::bake::format::{decode_terrain, decode_terrain_into, Ground, PageRoom, Terrain};
use pxl_spec::bake::pack::GroundCell;
use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pyramid::LevelPage;
use pxl_spec::space::pyramid::PAGE_CELLS;
use pxl_spec::space::xy::TO_XY;
use xpute_runtime::abi::fetch::FetchFail;
use xpute_runtime::mem::heap::alloc;

use crate::control::navigation::store::map_navigation;
use crate::global::Global;
use crate::host::now;
use crate::io::demand::{Category, Phases, TaskPhaseName};
use crate::io::policy;
use crate::io::sched::{declare, pending_count, Rank};
use crate::layer::terrain::fixed::{ele_at, ground_at, record_ele_at, record_ground_at, RECORD_BYTES};
use crate::layer::terrain::ground_snap::reground_map_center;
use crate::mem;
use crate::shell::map_store::{map_store, MapStatus};
use crate::world::chunk::node::chunk_page;
use crate::world::pyramid::window::{eye, ring_of, window, HELD_AXIS};
use crate::world::pyramid::{Cover, Domain, Layer, Pyramid};

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
    fn receive(record: u32, pkt: &[u8], phases: &mut Phases) -> Result<Terrain, FetchFail> {
        let t0 = now();
        let lanes = lanes_of(record)?;
        let room = PageRoom {
            // SAFETY: the record is this page's from the pyramid, at the
            // page's lane sizes, and nothing reads it until the page is set.
            cells: unsafe { core::slice::from_raw_parts_mut(lanes.cells as *mut GroundCell, CELLS) },
            ele: unsafe { core::slice::from_raw_parts_mut(lanes.ele as *mut i16, SAMPLES) },
        };
        decode_terrain_into(pkt, room).map_err(|_| FetchFail::Decode)?;
        phases.push(TaskPhaseName::decode_ms, now() - t0);
        // SAFETY: the lanes are this page's record until another page takes it.
        Ok(unsafe { open_terrain(lanes) })
    }

    /// A folded page is already packed — the client made it from pages it
    /// holds — so its cells go over as they are.
    fn keep(record: u32, page: &Terrain) -> Result<Terrain, FetchFail> {
        let lanes = lanes_of(record)?;
        write_lanes(page, lanes);
        // SAFETY: as `receive`.
        Ok(unsafe { open_terrain(lanes) })
    }

    fn absent(p: LevelPage) {
        let o = ocean();
        match &o.terrain {
            Some(t) => Terrain::pyramid().set(p, t.clone()),
            None if !o.waiting.contains(&p) => o.waiting.push(p),
            None => {}
        }
    }

    /// What the mesh can draw of the cover (present.rs).
    fn present(cover: Cover, d: &Domain, pyramid: &Pyramid<Terrain>) -> Cover {
        crate::layer::terrain::present::mesh(cover, d, |c| pyramid.get(c.page()).is_some())
    }

    /// A page is what the scheduler mounts from, and a leaf what a map
    /// centre that could not be grounded before may be grounded on now. It is
    /// no chunk's source changing: a chunk is mounted from a resident page,
    /// which stays until the chunk goes.
    fn changed(p: LevelPage, present: bool) {
        crate::layer::terrain::relief::page_changed(p, Terrain::pyramid().get(p));
        chunk_page(p, present);
        if p.level == 0 && present {
            reground_map_center();
        }
    }
}

const CELLS: usize = (PAGE_CELLS * PAGE_CELLS) as usize;
const SAMPLES: usize = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;

/// A page's lanes in the range: the ground cells, packed (pxl-spec
/// bake/pack.rs), and the padded elevation grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TerrainLanes {
    cells: usize,
    ele: usize,
}

/// Opens a page's lanes as the terrain the kernel reads.
///
/// # Safety
/// Every lane is a block of the kernel's memory held for this page for as
/// long as the page is resident, at the page's fixed lane sizes.
unsafe fn open_terrain(lanes: TerrainLanes) -> Terrain {
    // SAFETY: the caller's contract; a ground lane is one word a cell, an
    // i16 lane 2 bytes a sample.
    Terrain {
        ground: Ground {
            cells: unsafe { core::slice::from_raw_parts(lanes.cells as *const GroundCell, CELLS) }.into(),
        },
        ele: unsafe { core::slice::from_raw_parts(lanes.ele as *const i16, SAMPLES) }.into(),
    }
}

/// Writes a decoded page into `lanes`, which must be the page's own room.
fn write_lanes(t: &Terrain, lanes: TerrainLanes) {
    let bytes = |p: *const u8, n: usize| {
        // SAFETY: a lane read as its own bytes, where it lies.
        unsafe { core::slice::from_raw_parts(p, n) }
    };
    let cells = &t.ground.cells;
    let sources = [
        bytes(cells.as_ptr() as *const u8, cells.len() * core::mem::size_of::<GroundCell>()),
        bytes(t.ele.as_ptr() as *const u8, t.ele.len() * 2),
    ];
    for (src, to) in sources.iter().zip([lanes.cells, lanes.ele]) {
        // SAFETY: `to` is the page's room in the range, the lane's own size,
        // and apart from `src`.
        unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), to as *mut u8, src.len()) };
    }
}

/// Where a record's lanes are. NoRoom under a native test, which lays no
/// memory out.
fn lanes_of(record: u32) -> Result<TerrainLanes, FetchFail> {
    let base = mem::fixed::terrain_at().ok_or(FetchFail::NoRoom)? as usize;
    Ok(TerrainLanes {
        cells: ground_at(base, record),
        ele: ele_at(base, record),
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

/// The shared tile's body: written into a room of its own that belongs to no
/// page and nothing releases, and given to every page waiting on it. It is
/// not in the range: a record is one page's, and every open-ocean page reads
/// these same bytes.
pub fn adopt_ocean(pkt: &[u8]) -> Result<(), FetchFail> {
    if ocean().terrain.is_some() {
        return Ok(());
    }
    let decoded = decode_terrain(pkt).map_err(|_| FetchFail::Decode)?;
    if decoded.ground.len() != CELLS || decoded.ele.len() != SAMPLES {
        return Err(FetchFail::Decode);
    }
    let (at, _) = alloc(RECORD_BYTES).ok_or(FetchFail::NoRoom)?;
    let lanes = TerrainLanes {
        cells: record_ground_at(at),
        ele: record_ele_at(at),
    };
    write_lanes(&decoded, lanes);
    // SAFETY: a room nothing ever releases, at the page's lane sizes.
    let terrain = unsafe { open_terrain(lanes) };
    let o = ocean();
    for p in o.waiting.drain(..) {
        Terrain::pyramid().set(p, terrain.clone());
    }
    o.terrain = Some(terrain);
    Ok(())
}

// ---- the turn ----

/// Terrain's turn: its pyramid's, with the device's own leaf pinned —
/// panning moves the view away from the user's real position, and without
/// it the marker would lose the ground under it — the shared ocean tile
/// asked for while pages wait on it, and map_status following the queue.
pub fn terrain_step() {
    let device = map_navigation().device_coord.map(|c| LevelPage::new(0, TO_XY(L3_IDX(c.i))));
    if let Some(e) = eye() {
        let w = window();
        ocean().waiting.retain(|p| Some(*p) == device || (w.contains(&p.level) && ring_of(e.foot, *p) <= HELD_AXIS / 2));
    }
    let o = ocean();
    let ocean_wants: &[(u64, Rank)] = if o.terrain.is_none() && !o.waiting.is_empty() { &[(0, policy::ocean())] } else { &[] };
    declare(OCEAN_CATEGORY, ocean_wants);
    let terrain = Terrain::pyramid();
    terrain.pin(device);
    terrain.step();
    // Derived from the scheduler rather than tracked, so a later pan that
    // supersedes a batch cannot leave a stale SUCCESS behind.
    let map = map_store();
    if pending_count(Terrain::CATEGORY) + pending_count(OCEAN_CATEGORY) > 0 {
        map.map_status = MapStatus::Loading;
    } else if map.map_status == MapStatus::Loading {
        map.map_status = MapStatus::Success;
    }
}
