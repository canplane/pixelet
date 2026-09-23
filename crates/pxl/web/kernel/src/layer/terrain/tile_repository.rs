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
use pxl_spec::bake::format::{decode_terrain, Ground, Terrain, Tile};
use pxl_spec::space::pvf3::ivec::IVec;
use xpute_runtime::abi::fetch::FetchFail;
use xpute_runtime::mem::heap::alloc;

use crate::global::Global;
use crate::host::now;
use crate::io::demand::{Category, Phases};
use crate::layer::terrain::fixed::{ele_at, ground_at, record_ele_at, record_ground_at, RECORD_BYTES};
use crate::layer::terrain::level::{window, LevelPage};
use crate::layer::terrain::resident::{holder, take_slot};
use crate::layer::terrain::store::tile_store;
use crate::mem;
use crate::shell::config::env::TILE_BASE_URL;
use crate::shell::config::route::{Joined, TileBase};

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
    pub ground: [usize; 4],
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
    let u8_lane = |at: usize| unsafe { core::slice::from_raw_parts(at as *const u8, cells) }.into();
    Terrain {
        ground: Ground::from_planes(lanes.ground.map(u8_lane)),
        // SAFETY: the caller's contract; an i16 lane is 2 bytes a sample.
        ele: unsafe { core::slice::from_raw_parts(lanes.ele as *const i16, samples) }.into(),
    }
}

/// A body decoded, refused unless every lane is the page's size.
fn decode(pkt: &[u8]) -> Result<Terrain, FetchFail> {
    let t = decode_terrain(pkt).map_err(|_| FetchFail::Decode)?;
    let cells = (PAGE_CELL_SPAN * PAGE_CELL_SPAN) as usize;
    let samples = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
    let g = &t.ground;
    if g.planes().iter().any(|l| l.len() != cells) || t.ele.len() != samples {
        return Err(FetchFail::Decode);
    }
    Ok(t)
}

/// The range's base. None under a native test, which lays no memory out.
fn terrain_base() -> Option<usize> {
    mem::fixed::terrain_at().map(|at| at as usize)
}

/// A decoded page's lanes, as bytes to be written somewhere.
fn sources(t: &Terrain) -> [&[u8]; 5] {
    let g = &t.ground;
    let ele = &t.ele;
    // SAFETY: an i16 lane read as its bytes, where it lies.
    let ele_bytes = unsafe { core::slice::from_raw_parts(ele.as_ptr() as *const u8, ele.len() * 2) };
    let [a, b, c, d] = g.planes();
    [a, b, c, d, ele_bytes]
}

/// Writes a decoded page into `lanes`, which must be the page's own room.
fn write_lanes(t: &Terrain, lanes: TerrainLanes) {
    let at = [lanes.ground[0], lanes.ground[1], lanes.ground[2], lanes.ground[3], lanes.ele];
    for (src, &to) in sources(t).iter().zip(at.iter()) {
        // SAFETY: `to` is the page's room in the range, the lane's own size, and
        // apart from `src`, which is the decoded body.
        unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), to as *mut u8, src.len()) };
    }
}

/// Where a page lies in the range. Its record is its own coordinate reduced
/// modulo the range's axis, so this asks for nothing and cannot be refused.
fn lanes_of(p: LevelPage) -> Result<TerrainLanes, FetchFail> {
    let base = terrain_base().ok_or(FetchFail::NoRoom)?;
    let slot = crate::layer::terrain::fixed::slot_of(p);
    Ok(TerrainLanes {
        ground: [ground_at(base, slot, 0), ground_at(base, slot, 1), ground_at(base, slot, 2), ground_at(base, slot, 3)],
        ele: ele_at(base, slot),
    })
}

/// A page's fetched body, decoded and made resident: its lanes owned by the
/// page until release_terrain.
pub fn adopt_terrain(p: LevelPage, pkt: &[u8], phases: &mut Phases) -> Result<(), FetchFail> {
    // A page of a level the window has left arrives from a request kept
    // running past the move (tile_fetcher.rs). Its record is its level's
    // group, which a level in the window may share: it lands only where no
    // page of the window holds the record, and is dropped otherwise, as a
    // withdrawn request would have been.
    if !window().contains(&p.level) && holder(p).is_some_and(|q| window().contains(&q.level)) {
        return Ok(());
    }
    let t0 = now();
    let decoded = decode(pkt)?;
    let t1 = now();
    adopt(p, &decoded)?;
    phases.push("decode_ms", t1 - t0);
    phases.push("adopt_ms", now() - t1);
    Ok(())
}

/// A page the client folded itself from the pages under it that it holds
/// (tile_fetcher.rs), made resident as a fetched one is.
pub fn adopt_folded(p: LevelPage, t: &Terrain) -> Result<(), FetchFail> {
    adopt(p, t)
}

fn adopt(p: LevelPage, decoded: &Terrain) -> Result<(), FetchFail> {
    let lanes = lanes_of(p)?;
    write_lanes(decoded, lanes);
    // The record carried a page whose bytes this one has just written over:
    // one the view has left, or a level the window has.
    if let Some(gone) = take_slot(p) {
        tile_store().clear_page(gone);
    }
    // SAFETY: the lanes are this page's record in the range, at the page's lane
    // sizes, until another page's coordinate names the same record.
    let terrain = unsafe { open_terrain(lanes) };
    tile_store().set_page(p, Tile { terrain });
    Ok(())
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
        ground: [record_ground_at(at, 0), record_ground_at(at, 1), record_ground_at(at, 2), record_ground_at(at, 3)],
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
