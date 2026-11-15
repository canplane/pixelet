// pxl-core/link/bound.rs

//! The most bytes a packet the core lays or takes whose lists all have caps,
//! by construction: its fixed part is its type's size (the generated `SZ`),
//! and each list and string it carries at its cap, laid past the fixed part at
//! its alignment (the Canonical ABI's layout, xpute-link cabi.rs). The link's
//! frames are as large as the largest (lib.rs `link!`). A list with no cap is
//! not counted and rides in that frame whole: source.declare's `formats` and
//! `shards`, and the snapshot's address, its division parts and their names.
//! What the UI sends in past a frame is refused with EMSGSIZE.

use crate::abi::overlay::Snapshot;
use crate::domain::setting::SettingKey;
use crate::spec::perf::limit;
use pxl_render::gpu::surface::SURFACE_CAP;
use pxl_spec::asset::marker::{CHAT_TEXT_CHARS, MARKER_TEXT_CHARS};

/// A list of `cnt` elements of `bytes` each past what is laid, with the padding
/// to its alignment, at most 8.
const fn list(cnt: usize, bytes: usize) -> usize {
    7 + cnt * bytes
}

/// overlay.show's args: one snapshot (wit/overlay.wit), its draft's text, its
/// settings and its surfaces.
const SNAPSHOT: usize = {
    let draft = list(4 * MARKER_TEXT_CHARS as usize, 1);
    let settings = list(SettingKey::SLOT_CNT, 4);
    let surfaces = list(SURFACE_CAP, 4);
    Snapshot::SZ + draft + settings + surfaces
};

/// A perf read's return data: one page (perf.json5 LIMIT.PAGE_SZ), a list of
/// bytes.
const PERF_PAGE: usize = 8 + list(limit::PAGE_SZ as usize, 1);

/// scene.chat's args, which the core takes: one string (wit/scene.wit), a
/// line at its cap.
const CHAT: usize = 8 + list(4 * CHAT_TEXT_CHARS as usize, 1);

const fn max(a: usize, b: usize) -> usize {
    if a > b { a } else { b }
}

/// The largest packet the core lays or takes, a multiple of 8.
pub const PKT_CAP: u32 = max(SNAPSHOT, max(PERF_PAGE, CHAT)).next_multiple_of(8) as u32;
