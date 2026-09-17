// pxl_core/module/world/poi/repository.rs

//! A resident POI page: the packet the shell fetched into the block of the
//! page section the kernel gave its body (module/world/fetch.rs), and the page
//! opened over it. The lanes the index reads are views into that block, so
//! the block lives exactly as long as the page is resident (store.rs gives it
//! back with the page). The empty page — a 404 from the static bake, or a page
//! with nothing on it — is one shared packet nothing releases.
//!
//! Where a page is fetched from: the pre-baked static set when one is
//! configured, the live API otherwise; a 404 from the static set resolves to
//! the empty page with no extra round trip.

use pxl_shared::bake::format::{encode_empty_poi_page, PoiPage};
use pxl_shared::spec::space::pvf3::ivec::IVec;
use xpute_core::kit::array::{ArrayBuffer, U8Array};
use xpute_core::sys::error::MarshalError;
use xpute_core::sys::mem::Block;

use crate::lib::env::config::POI_TILE_BASE_URL;
use crate::lib::env::route::{Joined, TileBase};

pub fn poi_url(page_idx: IVec) -> Joined<TileBase, IVec> {
    Joined(POI_TILE_BASE_URL(), page_idx)
}

pub struct ResidentPoiPage {
    pub page: PoiPage,
    pub block: Option<Block>,
}

/// The shared empty page.
pub fn empty_page() -> Result<ResidentPoiPage, MarshalError> {
    Ok(ResidentPoiPage {
        page: PoiPage::new(encode_empty_poi_page()?)?,
        block: None,
    })
}

/// Opens the page fetched into the block at `at`.
///
/// # Safety
/// The block is one of the page section's, held for this page alone, `bytes`
/// long, until the page is released.
pub unsafe fn open_poi_page(at: usize, bytes: u32) -> Result<ResidentPoiPage, MarshalError> {
    // SAFETY: the caller's contract above.
    let buffer = unsafe { ArrayBuffer::from_raw(at as *mut u8, bytes as usize) };
    let pkt = U8Array::over(buffer, 0, bytes as usize);
    Ok(ResidentPoiPage {
        page: PoiPage::new(pkt)?,
        block: Some((at, bytes)),
    })
}
