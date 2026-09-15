// pxl_core/module/world/terrain/tile_repository.rs

//! Terrain transport's kernel half: where a page is fetched from, and a
//! fetched body decoded and copied lane by lane into the page section, where
//! the kernel reads it (module/world/fetch.rs hands the body over).
//!
//! The pre-bake pass skips pure open-ocean pages entirely — about 65% of the
//! coverage bbox — so the static bucket has no object for them at all. A 404
//! there isn't a failure, it's the signal for "nothing but water here", and
//! every such page resolves to one shared tile: baking an empty source at
//! different pages was verified to produce identical bytes, so a single decode
//! can be aliased by all of them. Only reachable when a static tile origin is
//! configured; against the live API a 404 is a genuine error.
//!
//! The shared tile is fetched under its own category, so the pages waiting on
//! it settle at once and its one fetch is not tied to any of them. A failure
//! is not kept: while pages wait, it is asked for again (use_tile_fetcher.rs).

use pxl_shared::bake::constant::{ELE_PADDED_SPAN, PAGE_CELL_SPAN};
use pxl_shared::bake::format::{decode_terrain, Ground, Terrain, Tile};
use pxl_shared::spec::space::pvf3::ivec::IVec;
use xpute_core::kit::array::{ArrayBuffer, I16Array, U8Array};

use super::resident::{into_core, Block};
use super::store::use_tile_store;
use crate::global::{now, Global};
use crate::kernel::kernel;
use crate::lib::env::config::TILE_BASE_URL;
use crate::lib::env::route::{Joined, TileBase};
use crate::memory::PAGE_POOL;
use crate::module::world::sched::demand::Phases;

/// Object key of the single shared open-ocean tile in the static bucket.
pub const OCEAN_KEY: &str = "_ocean";

/// The category the shared ocean tile is fetched under.
pub const OCEAN_CATEGORY: &str = "ocean";

pub fn tile_url(key: &str) -> Joined<'_, TileBase> {
    Joined(TILE_BASE_URL(), key)
}

/// A page's lanes in the page section: land, water, structure and transport,
/// each PAGE_CELL_SPAN² bytes, and the padded elevation grid.
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
    let u8_lane = |at: usize| {
        // SAFETY: the caller's contract.
        let buffer = unsafe { ArrayBuffer::from_raw(at as *mut u8, cells) };
        U8Array::over(buffer, 0, cells)
    };
    // SAFETY: the caller's contract; an i16 lane is 2 bytes a sample.
    let ele_buffer = unsafe { ArrayBuffer::from_raw(lanes.ele as *mut u8, samples * 2) };
    Terrain {
        ground: Ground {
            land: u8_lane(lanes.ground[0]),
            water: u8_lane(lanes.ground[1]),
            structure: u8_lane(lanes.ground[2]),
            transport: u8_lane(lanes.ground[3]),
        },
        ele: I16Array::over(ele_buffer, 0, samples),
    }
}

/// A body decoded, refused unless every lane is the page's size.
fn decode(pkt: U8Array) -> Result<Terrain, String> {
    let t = decode_terrain(pkt).map_err(|e| e.to_string())?;
    let cells = (PAGE_CELL_SPAN * PAGE_CELL_SPAN) as usize;
    let samples = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
    let g = &t.ground;
    if [&g.land, &g.water, &g.structure, &g.transport].iter().any(|l| l.length() != cells) || t.ele.length() != samples {
        return Err("a page whose lanes are not a page's size".to_string());
    }
    Ok(t)
}

/// Copies a decoded page's lanes into blocks of their own in the page section:
/// per lane, since a whole page would round up to twice what it holds
/// (resident.rs). None of them is kept when one is refused.
fn copy_lanes(t: &Terrain) -> Result<(TerrainLanes, [Block; 5]), String> {
    let pool = kernel().pools.get_mut(PAGE_POOL as usize).ok_or_else(|| "no page section".to_string())?;
    let g = &t.ground;
    let ele = t.ele.as_slice();
    // SAFETY: an i16 lane read as its bytes, where it lies.
    let ele_bytes = unsafe { core::slice::from_raw_parts(ele.as_ptr() as *const u8, ele.len() * 2) };
    let sources: [&[u8]; 5] = [g.land.as_slice(), g.water.as_slice(), g.structure.as_slice(), g.transport.as_slice(), ele_bytes];
    let mut blocks = [(0usize, 0u32); 5];
    for (i, src) in sources.iter().enumerate() {
        let Some(at) = pool.acquire(src.len() as u32) else {
            for &(at, bytes) in &blocks[..i] {
                pool.release(at, bytes);
            }
            return Err("the page section is full".to_string());
        };
        // SAFETY: `at` is a block of at least `src.len()` bytes the pool just
        // gave, apart from `src`.
        unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), at as *mut u8, src.len()) };
        blocks[i] = (at, src.len() as u32);
    }
    let lanes = TerrainLanes {
        ground: [blocks[0].0, blocks[1].0, blocks[2].0, blocks[3].0],
        ele: blocks[4].0,
    };
    Ok((lanes, blocks))
}

/// A page's fetched body, decoded and made resident: its lanes owned by the
/// page until release_terrain.
pub fn adopt_terrain(page_idx: IVec, pkt: U8Array, phases: &mut Phases) -> Result<(), String> {
    let t0 = now();
    let decoded = decode(pkt)?;
    let t1 = now();
    let (lanes, blocks) = copy_lanes(&decoded)?;
    phases.push("decode_ms", t1 - t0);
    phases.push("adopt_ms", now() - t1);
    into_core(page_idx, blocks.to_vec(), true);
    // SAFETY: the lanes are blocks of the page section this page now owns, at
    // the page's lane sizes, until release_terrain.
    let terrain = unsafe { open_terrain(lanes) };
    use_tile_store().set_tile(page_idx, Tile { terrain });
    Ok(())
}

struct Ocean {
    terrain: Option<Terrain>,
    /// Pages the bucket has no object for, waiting on the shared tile.
    waiting: Vec<IVec>,
}

static OCEAN: Global<Ocean> = Global::new();

fn ocean() -> &'static mut Ocean {
    OCEAN.get(|| Ocean { terrain: None, waiting: Vec::new() })
}

/// A page the static bucket has no object for: the shared tile, now or once
/// it has arrived.
pub fn ocean_page(page_idx: IVec) {
    let o = ocean();
    match &o.terrain {
        Some(t) => use_tile_store().set_tile(page_idx, Tile { terrain: t.clone() }),
        None if !o.waiting.contains(&page_idx) => o.waiting.push(page_idx),
        None => {}
    }
}

/// The shared tile's body: copied into blocks no page owns and nothing
/// releases, and given to every page waiting on it.
pub fn adopt_ocean(pkt: U8Array) -> Result<(), String> {
    if ocean().terrain.is_some() {
        return Ok(());
    }
    let (lanes, _) = copy_lanes(&decode(pkt)?)?;
    // SAFETY: blocks of the page section nothing ever releases, at the page's
    // lane sizes.
    let terrain = unsafe { open_terrain(lanes) };
    let o = ocean();
    for page_idx in o.waiting.drain(..) {
        use_tile_store().set_tile(page_idx, Tile { terrain: terrain.clone() });
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
pub fn ocean_retain(held: impl Fn(IVec) -> bool) {
    ocean().waiting.retain(|&p| held(p));
}
