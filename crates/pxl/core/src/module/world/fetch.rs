// pxl_core/module/world/fetch.rs

//! The world's fetches at the shell's edge. The shell runs HTTP and nothing
//! else: for a demand the scheduler started it is told the URL
//! (`demand_url`); when a response arrives it asks for room for the body
//! (`fetch_body`), writes the bytes there, and reports the status
//! (`fetch_arrived`). Which category a key is, what a 404 means and how a
//! body decodes are decided here.
//!
//! A body's room is a block the kernel acquires and the shell only writes into,
//! the way a device writes into a buffer its driver allocated. The block is the
//! kernel's throughout. Every body is a block of the mesh section: a body is
//! decoded or read and released in the same command, which is what that section
//! is for. A terrain body no longer outlives its command — the page it carries
//! is written into `.fixed.terrain`, whose record is the page's own coordinate
//! — and a POI page still keeps its body as the page, until its per-page index is
//! rewritten (issue/BACKLOG.md).

use core::fmt;

use pxl_shared::spec::fetch::FetchFail;
use xpute_core::kit::array::{ArrayBuffer, U8Array};
use xpute_core::sys::mem::Block;

use crate::sys::global::Global;
use crate::lib::env::config::PUBLISHED_TILES;
use crate::module::map::ui::overlay::{search_modal, use_reverse_geocode};
use crate::module::world::geocode::repository::geocode_url;
use crate::module::world::poi::{repository as poi_repository, store as poi_store};
use crate::module::world::sched::demand::{Category, Demand, Phases};
use crate::module::world::sched::fetch_scheduler::{demand_of, fetch_settle};
use crate::module::world::sched::ledger::{Failure, Outcome};
use crate::module::world::terrain::{tile_repository, use_tile_fetcher};

/// Bodies given room and not yet reported: the fetches in flight, with room
/// to spare. A body past them is refused room, and its fetch fails.
const BODIES: usize = 16;

static BODIES_HELD: Global<[(u64, Block); BODIES]> = Global::new();

fn bodies() -> &'static mut [(u64, Block); BODIES] {
    BODIES_HELD.get(|| [(0, (0, 0)); BODIES])
}

/// Where a started demand is fetched from, as it is written where it is sent.
pub struct DemandUrl<'a>(pub &'a Demand);

impl fmt::Display for DemandUrl<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key = self.0.key;
        match key.category {
            Category::Terrain => write!(f, "{}", tile_repository::tile_url(key.subject)),
            // The one tile the stream has, at the name the bake gives it.
            Category::Ocean => write!(f, "{}", tile_repository::ocean_url()),
            Category::Poi => write!(f, "{}", poi_repository::poi_url(key.subject)),
            Category::Geocode => write!(f, "{}", geocode_url(key.subject)),
            Category::Search => write!(f, "{}", search_modal::search_request_url()),
        }
    }
}

/// Room for `bytes` of `key`'s response body: the block's address, or 0 when
/// its section has none.
pub fn fetch_body(key: u64, bytes: u32) -> u32 {
    let Some(slot) = bodies().iter().position(|(_, b)| b.1 == 0) else { return 0 };
    let Some(block) = xpute_core::sys::mem::take_block(bytes) else { return 0 };
    bodies()[slot] = (key, block);
    block.0 as u32
}

/// The body the shell reports, only when it is one given room for `key`.
fn take_body(key: u64, at: u32, bytes: u32) -> Option<Block> {
    if bytes == 0 {
        return None;
    }
    let slot = bodies().iter().position(|&(h, b)| h == key && b == (at as usize, bytes))?;
    bodies()[slot] = (0, (0, 0));
    Some((at as usize, bytes))
}

fn release(block: Option<Block>) {
    if let Some(block) = block {
        // SAFETY: a body's block, given room by `fetch_body` and reported once.
        unsafe { xpute_core::sys::mem::give_block(block) };
    }
}

/// A body's bytes, read where the shell wrote them.
fn view((at, bytes): Block) -> U8Array {
    // SAFETY: the block is one `fetch_body` acquired for this body alone,
    // `bytes` long, and it is not released while the view is read.
    let buffer = unsafe { ArrayBuffer::from_raw(at as *mut u8, bytes as usize) };
    U8Array::over(buffer, 0, bytes as usize)
}

/// A body's text, read where the shell wrote it.
fn text<'a>((at, bytes): Block) -> Result<&'a str, FetchFail> {
    // SAFETY: as `view`'s: the block is this body's alone, and it is released
    // only after the text has been read.
    let raw = unsafe { core::slice::from_raw_parts(at as *const u8, bytes as usize) };
    core::str::from_utf8(raw).map_err(|_| FetchFail::Decode)
}

/// The shell's report that a started demand's fetch finished: `outcome` 0
/// with the response's status and body, 1 aborted, 2 failed with `error`.
pub fn fetch_arrived(key: u64, outcome: u32, status: u32, body: (u32, u32), fail: u32, net_ms: f64) {
    let block = take_body(key, body.0, body.1);
    // A hash the scheduler no longer holds is one a reset forgot; its body is
    // already back.
    let Some(name) = demand_of(key) else {
        release(block);
        return;
    };
    let (category, subject) = (name.category, name.subject);
    let mut phases = Phases::default();
    phases.push("net_ms", net_ms);
    if let Some((_, bytes)) = block {
        phases.push("bytes", bytes as f64);
    }
    let (outcome, fail) = match outcome {
        0 => match answer(category, subject, status, block, &mut phases) {
            Ok(()) => (Outcome::Ok, FetchFail::None),
            Err(fail) => (Outcome::Error, fail),
        },
        1 => {
            release(block);
            (Outcome::Aborted, FetchFail::None)
        }
        // The shell's own failure, as the number it sent. A code this build
        // does not know reads as one the shell raised rather than as none.
        _ => {
            release(block);
            (Outcome::Error, fetch_fail_of(fail))
        }
    };
    if outcome != Outcome::Ok {
        failed(category, subject, outcome == Outcome::Aborted);
    }
    fetch_settle(name, outcome, Some(phases), Failure { fail, status: status as u16 });
}

fn failed(category: Category, subject: u64, aborted: bool) {
    match category {
        Category::Terrain if !aborted => use_tile_fetcher::terrain_failed(),
        Category::Search => search_modal::search_failed(subject, aborted),
        Category::Geocode => use_reverse_geocode::reverse_geocode_failed(subject, aborted),
        _ => {}
    }
}

/// What a response means for its category. The body is released here, unless
/// a POI page keeps it.
/// The reason a number names, or `Shell` for one this build does not know —
/// an older kernel reading a newer shell still counts the failure apart from
/// the rest.
fn fetch_fail_of(fail: u32) -> FetchFail {
    const KNOWN: [FetchFail; 5] = [FetchFail::Network, FetchFail::Shell, FetchFail::Status, FetchFail::NoRoom, FetchFail::Decode];
    KNOWN.into_iter().find(|f| *f as u32 == fail).unwrap_or(FetchFail::Shell)
}

fn answer(category: Category, subject: u64, status: u32, block: Option<Block>, phases: &mut Phases) -> Result<(), FetchFail> {
    // Only a static tile origin has no object for a page; against the live
    // API a 404 is a genuine error (tile_repository.rs).
    let absent = status == 404 && PUBLISHED_TILES();
    let ok = (200..300).contains(&status);
    let refused = || FetchFail::Status;
    match category {
        Category::Terrain => {
            let result = match block {
                _ if absent => {
                    tile_repository::ocean_page(subject);
                    Ok(())
                }
                Some(b) if ok => tile_repository::adopt_terrain(subject, view(b), phases),
                _ => Err(refused()),
            };
            release(block);
            result
        }
        Category::Ocean => {
            let result = match block {
                Some(b) if ok => tile_repository::adopt_ocean(view(b)),
                _ => Err(refused()),
            };
            release(block);
            result
        }
        Category::Poi => {
            let page = subject;
            match block {
                _ if absent => {
                    release(block);
                    poi_store::set_pois(page, poi_repository::empty_page().map_err(|_| FetchFail::Decode)?);
                    Ok(())
                }
                // SAFETY: the block is the page section's, given for this body
                // alone; the page keeps it and the store releases it with the page.
                Some((at, bytes)) if ok => match unsafe { poi_repository::open_poi_page(at, bytes) } {
                    Ok(resident) => {
                        poi_store::set_pois(page, resident);
                        Ok(())
                    }
                    Err(_) => {
                        release(block);
                        Err(FetchFail::Decode)
                    }
                },
                _ => {
                    release(block);
                    Err(refused())
                }
            }
        }
        Category::Search => {
            let result = match block {
                Some(b) if ok => text(b).map(|body| search_modal::search_arrived(subject, Some(body))),
                None if ok => {
                    search_modal::search_arrived(subject, Some(""));
                    Ok(())
                }
                _ => Err(refused()),
            };
            release(block);
            result
        }
        Category::Geocode => {
            let result = match block {
                _ if status == 404 => {
                    use_reverse_geocode::reverse_geocode_arrived(subject, None);
                    Ok(())
                }
                Some(b) if ok => text(b).map(|body| use_reverse_geocode::reverse_geocode_arrived(subject, Some(body))),
                _ => Err(refused()),
            };
            release(block);
            result
        }
    }
}
