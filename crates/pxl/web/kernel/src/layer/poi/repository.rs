// pxl-kernel/layer/poi/repository.rs

//! A resident POI page: the packet the shell fetched into the heap block the
//! kernel gave its body (io/fetch.rs), and the page
//! opened over it. The lanes the index reads are views into that block, so
//! the block lives exactly as long as the page is resident (store.rs gives it
//! back with the page). The empty page — a 404 from the static bake, or a page
//! with nothing on it — is one shared packet nothing releases.
//!
//! Where a page is fetched from: the pre-baked static set when one is
//! configured, the dev tile server otherwise; a 404 from either resolves to
//! the empty page with no extra round trip.

use pxl_spec::bake::format::{encode_empty_poi_page, PoiPage};
use pxl_spec::space::pvf3::ivec::IVec;
use xpute_core::status::error::MarshalError;
use xpute_runtime::mem::heap::Block;

/// A POI page's name in its bake: the page's own key, which is what the bake
/// writes it under.
pub fn poi_name(page_idx: IVec) -> IVec {
    page_idx
}

pub struct ResidentPoiPage {
    /// The page reads the block's bytes where they lie, at an address that
    /// holds until the page is released.
    pub page: PoiPage<'static>,
    pub block: Option<Block>,
}

/// The shared empty page.
pub fn empty_page() -> Result<ResidentPoiPage, MarshalError> {
    Ok(ResidentPoiPage {
        // The one empty page, which every page without POI shares and which
        // is never released.
        page: PoiPage::new(Vec::leak(encode_empty_poi_page()?))?,
        block: None,
    })
}

/// Opens the page fetched into the block at `at`.
///
/// # Safety
/// The block is a heap block held for this page alone, `bytes`
/// long, until the page is released.
pub unsafe fn open_poi_page(at: usize, bytes: u32) -> Result<ResidentPoiPage, MarshalError> {
    // SAFETY: the caller's contract above.
    let pkt = unsafe { core::slice::from_raw_parts(at as *const u8, bytes as usize) };
    Ok(ResidentPoiPage {
        page: PoiPage::new(pkt)?,
        block: Some((at, bytes)),
    })
}
