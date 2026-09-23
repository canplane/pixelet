// pxl-kernel/layer/terrain/tile_repository.rs

//! Terrain transport's kernel half: where a page is fetched from, and a
//! fetched body decoded and copied lane by lane into `.fixed.terrain`, where
//! the kernel reads it (io/fetch.rs hands the body over).
//!
//! The pre-bake pass skips pure open-ocean pages entirely — about 65% of the
//! coverage bbox — so the static bucket has no object for them at all. A 404
//! there isn't a failure, it's the signal for "nothing but water here", and
//! every such page resolves to one shared tile: baking an empty source at
//! different pages was verified to produce identical bytes, so a single decode
//! can be aliased by all of them. The dev tile server answers the same way,
//! since it serves a tile set laid out as the bucket is.
//!
//! The shared tile is fetched under its own category, so the pages waiting on
//! it settle at once and its one fetch is not tied to any of them. A failure
//! is not kept: while pages wait, it is asked for again (tile_fetcher.rs).

use pxl_spec::bake::constant::{ELE_PADDED_SPAN, PAGE_CELL_SPAN};
use pxl_spec::bake::format::{decode_terrain, decode_terrain_into, Ground, PageRoom, Terrain, Tile};
use pxl_spec::bake::pack::GroundCell;
use pxl_spec::space::pvf3::ivec::IVec;
use xpute_runtime::abi::fetch::FetchFail;
use xpute_runtime::mem::heap::alloc;

use crate::global::Global;
use crate::host::now;
use crate::io::demand::{Category, Phases};
use crate::layer::terrain::fixed::{ele_at, ground_at, record_ele_at, record_ground_at, RECORD_BYTES};
use crate::layer::terrain::level::{ring_of, window, LevelPage, HELD_AXIS};
use crate::layer::terrain::resident::{free_record, page_in, put, record_of, RECORDS};
use crate::layer::terrain::resolve::covered;
use crate::layer::terrain::store::tile_store;
use crate::mem;
use crate::shell::config::env::TILE_BASE_URL;
use crate::shell::config::route::{Joined, TileBase};
use crate::world::frame::range_anchor::range_anchor;

/// Object key of the single shared open-ocean tile in the static bucket.
pub const OCEAN_KEY: &str = "_ocean";

/// The category the shared ocean tile is fetched under.
pub const OCEAN_CATEGORY: Category = Category::Ocean;

/// A page's URL. A page's key names it within its level and the level is the
/// path segment before it.
pub fn tile_url(p: LevelPage) -> Joined<Joined<TileBase, u32>, IVec> {
    Joined(Joined(TILE_BASE_URL(), p.level), p.page)
}

/// The shared ocean tile, which the bake writes under a name rather than a
/// coordinate — one page's worth of open water every open-water page reads,
/// at every level, so it sits beside the levels rather than in one.
pub fn ocean_url() -> Joined<TileBase, &'static str> {
    Joined(TILE_BASE_URL(), OCEAN_KEY)
}

/// A page's lanes in the range: land, water, structure and transport, each
/// PAGE_CELL_SPAN² bytes, and the padded elevation grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainLanes {
    /// The page's ground cells, packed (pxl-spec bake/pack.rs).
    pub cells: usize,
    pub ele: usize,
}

/// Opens a page's lanes as the terrain the kernel reads.
///
/// # Safety
/// Every lane is a block of the kernel's memory held for this page for as
/// long as the page is resident, at the page's fixed lane sizes.
pub unsafe fn open_terrain(lanes: TerrainLanes) -> Terrain {
    let cells = (PAGE_CELL_SPAN * PAGE_CELL_SPAN) as usize;
    let samples = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
    // SAFETY: the caller's contract.
    Terrain {
        // SAFETY: the caller's contract; a ground lane is one word a cell.
        ground: Ground {
            cells: unsafe { core::slice::from_raw_parts(lanes.cells as *const GroundCell, cells) }.into(),
        },
        // SAFETY: the caller's contract; an i16 lane is 2 bytes a sample.
        ele: unsafe { core::slice::from_raw_parts(lanes.ele as *const i16, samples) }.into(),
    }
}

/// A body decoded, refused unless every lane is the page's size. For the
/// paths that need a page in hand — the shared ocean tile, which is held
/// rather than recorded. A fetched page never takes it: it goes straight
/// into its record (`decode_into`).
fn decode(pkt: &[u8]) -> Result<Terrain, FetchFail> {
    let t = decode_terrain(pkt).map_err(|_| FetchFail::Decode)?;
    let cells = (PAGE_CELL_SPAN * PAGE_CELL_SPAN) as usize;
    let samples = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
    if t.ground.len() != cells || t.ele.len() != samples {
        return Err(FetchFail::Decode);
    }
    Ok(t)
}

/// A fetched page written into `lanes`, the record's own room, in one pass
/// from the packet: `decode_terrain_into` reads the wire's planes where they
/// lie and packs a cell at a time, so nothing page-sized stands between the
/// page arriving and the ground being drawn from it.
///
/// # Safety
/// `lanes` is a record of the range, at the page's lane sizes, and nothing
/// else is reading it.
unsafe fn decode_into(pkt: &[u8], lanes: TerrainLanes) -> Result<(), FetchFail> {
    let cells = (PAGE_CELL_SPAN * PAGE_CELL_SPAN) as usize;
    let samples = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
    let room = PageRoom {
        // SAFETY: the caller's contract.
        cells: unsafe { core::slice::from_raw_parts_mut(lanes.cells as *mut GroundCell, cells) },
        ele: unsafe { core::slice::from_raw_parts_mut(lanes.ele as *mut i16, samples) },
    };
    decode_terrain_into(pkt, room).map_err(|_| FetchFail::Decode)
}

/// The range's base. None under a native test, which lays no memory out.
fn terrain_base() -> Option<usize> {
    mem::fixed::terrain_at().map(|at| at as usize)
}

/// A decoded page's lanes, as bytes to be written somewhere: the cells and
/// the elevation, each read where it lies.
fn sources(t: &Terrain) -> [&[u8]; 2] {
    let bytes = |p: *const u8, n: usize| {
        // SAFETY: a lane read as its own bytes, where it lies.
        unsafe { core::slice::from_raw_parts(p, n) }
    };
    let cells = &t.ground.cells;
    [
        bytes(cells.as_ptr() as *const u8, cells.len() * core::mem::size_of::<GroundCell>()),
        bytes(t.ele.as_ptr() as *const u8, t.ele.len() * 2),
    ]
}

/// Writes a decoded page into `lanes`, which must be the page's own room.
fn write_lanes(t: &Terrain, lanes: TerrainLanes) {
    for (src, &to) in sources(t).iter().zip([lanes.cells, lanes.ele].iter()) {
        // SAFETY: `to` is the page's room in the range, the lane's own size, and
        // apart from `src`, which is the decoded body.
        unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), to as *mut u8, src.len()) };
    }
}

/// The record `p` should be written into: its own where it is already
/// resident, else a free one, else the page worth least of those resident.
///
/// None where every record carries a page worth more than `p`, which is what
/// an arrival nobody wants any more meets when the camera's own pages fill
/// the range. It is dropped then rather than turning out a page that is
/// wanted — the request it came from was withdrawn, and this is the same
/// answer arriving late.
fn record_for(p: LevelPage) -> Option<u32> {
    if let Some(record) = record_of(p) {
        return Some(record);
    }
    if let Some(record) = free_record() {
        return Some(record);
    }
    let anchor = range_anchor()?;
    let view = window();
    let mine = worth(p, anchor, &view);
    let mut worst: Option<(Worth, u32)> = None;
    for record in 0..RECORDS {
        let Some(held) = page_in(record) else { continue };
        let w = worth(held, anchor, &view);
        if worst.is_none_or(|(low, _)| w > low) {
            worst = Some((w, record));
        }
    }
    worst.filter(|(w, _)| *w > mine).map(|(_, record)| record)
}

/// How readily a resident page gives up its record, highest first. A page
/// the camera does not want goes before one it does; among those, one whose
/// ground something else already draws goes before one nothing covers, since
/// letting that one go opens a hole; and among equals the one furthest out,
/// the rim being margin.
///
/// Ordered by comparison rather than by a score so that no two of these
/// trade against each other: a page the camera wants is never given up for
/// one it does not, whatever the distance.
type Worth = (bool, bool, u32);

fn worth(p: LevelPage, anchor: IVec, view: &std::ops::Range<u32>) -> Worth {
    let wanted = view.contains(&p.level) && ring_of(anchor, p) <= HELD_AXIS / 2;
    (!wanted, covered(p, view), ring_of(anchor, p))
}

/// Where a record's lanes are.
fn lanes_of(slot: u32) -> Result<TerrainLanes, FetchFail> {
    let base = terrain_base().ok_or(FetchFail::NoRoom)?;
    Ok(TerrainLanes {
        cells: ground_at(base, slot),
        ele: ele_at(base, slot),
    })
}

/// A page's fetched body, decoded and made resident: its lanes owned by the
/// page until release_terrain.
pub fn adopt_terrain(p: LevelPage, pkt: &[u8], phases: &mut Phases) -> Result<(), FetchFail> {
    let t0 = now();
    // An arrival the camera has moved past keeps its body only where the
    // range has room to spare, which is `record_for`'s answer; there is
    // nothing to report when it has not, since the request it answers was
    // withdrawn a turn or more ago.
    let Some(record) = record_for(p) else { return Ok(()) };
    let lanes = lanes_of(record)?;
    // SAFETY: `record_for` gave this record to this page, and nothing reads a
    // record between here and the store being told below.
    unsafe { decode_into(pkt, lanes)? };
    phases.push("decode_ms", now() - t0);
    take_record(p, record, lanes);
    Ok(())
}

/// A page the client folded itself from the pages under it that it holds
/// (tile_fetcher.rs), made resident as a fetched one is.
pub fn adopt_folded(p: LevelPage, t: &Terrain) -> Result<(), FetchFail> {
    // A folded page is already packed — the client made it from pages it
    // holds — so its cells go over as they are.
    let Some(record) = record_for(p) else { return Ok(()) };
    let lanes = lanes_of(record)?;
    write_lanes(t, lanes);
    take_record(p, record, lanes);
    Ok(())
}

/// The record is this page's from here: whatever it carried is gone, and the
/// store reads the page out of it.
fn take_record(p: LevelPage, record: u32, lanes: TerrainLanes) {
    if let Some(gone) = put(p, record) {
        tile_store().clear_page(gone);
    }
    // SAFETY: the lanes are this page's record in the range, at the page's lane
    // sizes, until another page's coordinate names the same record.
    let terrain = unsafe { open_terrain(lanes) };
    tile_store().set_page(p, Tile { terrain });
}

struct Ocean {
    terrain: Option<Terrain>,
    /// Pages the bucket has no object for, waiting on the shared tile.
    waiting: Vec<LevelPage>,
}

static OCEAN: Global<Ocean> = Global::new();

fn ocean() -> &'static mut Ocean {
    OCEAN.get(|| Ocean { terrain: None, waiting: Vec::new() })
}

/// A page the static bucket has no object for: the shared tile, now or once
/// it has arrived.
pub fn ocean_page(p: LevelPage) {
    let o = ocean();
    match &o.terrain {
        Some(t) => tile_store().set_page(p, Tile { terrain: t.clone() }),
        None if !o.waiting.contains(&p) => o.waiting.push(p),
        None => {}
    }
}

/// The shared tile's body: written into a record of its own that belongs to no
/// page and nothing releases, and given to every page waiting on it.
///
/// It is not in the range. Every open-ocean page reads these same bytes, and a
/// record is one page's; a page whose tile is the shared one leaves its
/// own record untouched.
pub fn adopt_ocean(pkt: &[u8]) -> Result<(), FetchFail> {
    if ocean().terrain.is_some() {
        return Ok(());
    }
    let decoded = decode(pkt)?;
    let (at, _) = alloc(RECORD_BYTES).ok_or(FetchFail::NoRoom)?;
    let lanes = TerrainLanes {
        cells: record_ground_at(at),
        ele: record_ele_at(at),
    };
    write_lanes(&decoded, lanes);
    // SAFETY: a record nothing ever releases, at the page's lane sizes.
    let terrain = unsafe { open_terrain(lanes) };
    let o = ocean();
    for p in o.waiting.drain(..) {
        tile_store().set_page(p, Tile { terrain: terrain.clone() });
    }
    o.terrain = Some(terrain);
    Ok(())
}

/// Whether the shared tile is wanted: some page waits and it has not arrived.
pub fn ocean_wanted() -> bool {
    let o = ocean();
    o.terrain.is_none() && !o.waiting.is_empty()
}

/// Keeps waiting only the pages still held.
pub fn ocean_retain(held: impl Fn(LevelPage) -> bool) {
    ocean().waiting.retain(|&p| held(p));
}
