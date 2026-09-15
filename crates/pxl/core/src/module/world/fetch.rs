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
//! kernel's throughout: a POI page keeps it as the page, so a POI body is a
//! block of the page section; every other body is decoded or read and released
//! in the same command, so it is a block of the mesh section, which is what
//! churns. A terrain body in the page section was an 8 MB block (4.2 MB,
//! rounded) the resident page count never counted: with the pages' 1 MB lanes
//! spread over the section and six bodies in flight, a body found no 8 MB piece
//! and its page was refused, and a pan left chunks with no page to build from.

use core::fmt::{self, Write};

use pxl_shared::spec::space::pvf3::ivec::IVec;
use xpute_core::kit::array::{ArrayBuffer, U8Array};
use xpute_core::kit::hash::fnv1a64_str;

use crate::global::Global;
use crate::kernel::kernel;
use crate::lib::env::config::PUBLISHED_TILES;
use crate::memory::{MESH_POOL, PAGE_POOL};
use crate::module::map::ui::overlay::{search_modal, use_reverse_geocode};
use crate::module::world::geocode::repository::geocode_url;
use crate::module::world::poi::{repository as poi_repository, store as poi_store, use_poi_fetcher};
use crate::module::world::sched::demand::{Demand, ErrorText, Phases};
use crate::module::world::sched::fetch_scheduler::fetch_settle;
use crate::module::world::sched::ledger::Outcome;
use crate::module::world::terrain::{tile_repository, use_tile_fetcher};

/// A body's block: its address and size.
type Block = (usize, u32);

/// The section a category's bodies are held in: a POI page keeps its body.
fn body_pool(category: &str) -> u32 {
    if category == use_poi_fetcher::CATEGORY {
        PAGE_POOL
    } else {
        MESH_POOL
    }
}

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
        let d = self.0;
        let subject = d.key.split_once(':').map_or(d.key.as_str(), |(_, s)| s);
        match d.category {
            use_tile_fetcher::CATEGORY | tile_repository::OCEAN_CATEGORY => write!(f, "{}", tile_repository::tile_url(subject)),
            use_poi_fetcher::CATEGORY => write!(f, "{}", poi_repository::poi_url(subject)),
            use_reverse_geocode::CATEGORY => write!(f, "{}", geocode_url(subject)),
            search_modal::CATEGORY => write!(f, "{}", search_modal::search_request_url()),
            _ => Ok(()),
        }
    }
}

/// Room for `bytes` of `key`'s response body: the block's address, or 0 when
/// its section has none.
pub fn fetch_body(key: &str, bytes: u32) -> u32 {
    let Some(slot) = bodies().iter().position(|(_, b)| b.1 == 0) else { return 0 };
    let category = key.split_once(':').map_or("", |(c, _)| c);
    let Some(pool) = kernel().pools.get_mut(body_pool(category) as usize) else { return 0 };
    let Some(at) = pool.acquire(bytes) else { return 0 };
    bodies()[slot] = (fnv1a64_str(key), (at, bytes));
    at as u32
}

/// The body the shell reports, only when it is one given room for `key`.
fn take_body(key: &str, at: u32, bytes: u32) -> Option<Block> {
    if bytes == 0 {
        return None;
    }
    let hash = fnv1a64_str(key);
    let slot = bodies().iter().position(|&(h, b)| h == hash && b == (at as usize, bytes))?;
    bodies()[slot] = (0, (0, 0));
    Some((at as usize, bytes))
}

fn release(category: &str, block: Option<Block>) {
    if let (Some((at, bytes)), Some(pool)) = (block, kernel().pools.get_mut(body_pool(category) as usize)) {
        pool.release(at, bytes);
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
fn text<'a>((at, bytes): Block) -> Result<&'a str, String> {
    // SAFETY: as `view`'s: the block is this body's alone, and it is released
    // only after the text has been read.
    let raw = unsafe { core::slice::from_raw_parts(at as *const u8, bytes as usize) };
    core::str::from_utf8(raw).map_err(|_| "a body that is not UTF-8".to_string())
}

fn page_of(subject: &str) -> Result<IVec, String> {
    subject.parse().map_err(|_| format!("{subject} is not a page"))
}

/// The shell's report that a started demand's fetch finished: `outcome` 0
/// with the response's status and body, 1 aborted, 2 failed with `error`.
pub fn fetch_arrived(key: &str, outcome: u32, status: u32, body: (u32, u32), error: Option<&str>, net_ms: f64) {
    let block = take_body(key, body.0, body.1);
    let (category, subject) = key.split_once(':').unwrap_or(("", key));
    let mut phases = Phases::default();
    phases.push("net_ms", net_ms);
    if let Some((_, bytes)) = block {
        phases.push("bytes", bytes as f64);
    }
    let (outcome, error) = match outcome {
        0 => match answer(category, subject, status, block, &mut phases) {
            Ok(()) => (Outcome::Ok, None),
            Err(message) => {
                let mut text = ErrorText::new();
                let _ = write!(text, "Error: [{category}/{subject}] {message}");
                (Outcome::Error, Some(text))
            }
        },
        1 => {
            release(category, block);
            (Outcome::Aborted, None)
        }
        _ => {
            release(category, block);
            (Outcome::Error, error.map(ErrorText::from))
        }
    };
    if outcome != Outcome::Ok {
        failed(category, subject, outcome == Outcome::Aborted);
    }
    fetch_settle(key, outcome, Some(phases), error);
}

fn failed(category: &str, subject: &str, aborted: bool) {
    match category {
        search_modal::CATEGORY => search_modal::search_failed(subject, aborted),
        use_reverse_geocode::CATEGORY => {
            if let Ok(page) = subject.parse::<IVec>() {
                use_reverse_geocode::reverse_geocode_failed(page, aborted);
            }
        }
        _ => {}
    }
}

/// What a response means for its category. The body is released here, unless
/// a POI page keeps it.
fn answer(category: &str, subject: &str, status: u32, block: Option<Block>, phases: &mut Phases) -> Result<(), String> {
    // Only a static tile origin has no object for a page; against the live
    // API a 404 is a genuine error (tile_repository.rs).
    let absent = status == 404 && PUBLISHED_TILES();
    let ok = (200..300).contains(&status);
    let refused = || format!("http {status}");
    match category {
        use_tile_fetcher::CATEGORY => {
            let result = page_of(subject).and_then(|page| match block {
                _ if absent => {
                    tile_repository::ocean_page(page);
                    Ok(())
                }
                Some(b) if ok => tile_repository::adopt_terrain(page, view(b), phases),
                _ => Err(refused()),
            });
            release(category, block);
            result
        }
        tile_repository::OCEAN_CATEGORY => {
            let result = match block {
                Some(b) if ok => tile_repository::adopt_ocean(view(b)),
                _ => Err(refused()),
            };
            release(category, block);
            result
        }
        use_poi_fetcher::CATEGORY => {
            let page = match page_of(subject) {
                Ok(page) => page,
                Err(e) => {
                    release(category, block);
                    return Err(e);
                }
            };
            match block {
                _ if absent => {
                    release(category, block);
                    poi_store::set_pois(page, poi_repository::empty_page().map_err(|e| e.to_string())?);
                    Ok(())
                }
                // SAFETY: the block is the page section's, given for this body
                // alone; the page keeps it and the store releases it with the page.
                Some((at, bytes)) if ok => match unsafe { poi_repository::open_poi_page(at, bytes) } {
                    Ok(resident) => {
                        poi_store::set_pois(page, resident);
                        Ok(())
                    }
                    Err(e) => {
                        release(category, block);
                        Err(e.to_string())
                    }
                },
                _ => {
                    release(category, block);
                    Err(refused())
                }
            }
        }
        search_modal::CATEGORY => {
            let result = match block {
                Some(b) if ok => text(b).map(|body| search_modal::search_arrived(subject, Some(body))),
                None if ok => {
                    search_modal::search_arrived(subject, Some(""));
                    Ok(())
                }
                _ => Err(refused()),
            };
            release(category, block);
            result
        }
        use_reverse_geocode::CATEGORY => {
            let result = page_of(subject).and_then(|page| match block {
                _ if status == 404 => {
                    use_reverse_geocode::reverse_geocode_arrived(page, None);
                    Ok(())
                }
                Some(b) if ok => text(b).map(|body| use_reverse_geocode::reverse_geocode_arrived(page, Some(body))),
                _ => Err(refused()),
            });
            release(category, block);
            result
        }
        _ => {
            release(category, block);
            Err("no such category".to_string())
        }
    }
}
