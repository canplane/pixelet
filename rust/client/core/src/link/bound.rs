// pxl-core/link/bound.rs

//! The most bytes a packet the core lays takes, by construction: its fixed
//! part is its type's size (the generated `SZ`), and every list and string it
//! carries has a cap, each laid past the fixed part at its alignment (the
//! Canonical ABI's layout, xpute-link cabi.rs). The link's frames are as large
//! as the largest (lib.rs `link!`). What the UI sends in past them is refused
//! with EMSGSIZE.

use pxl_render::gpu::surface::SURFACE_CAP;
use pxl_spec::asset::marker::MARKER_TEXT_CHARS;
use pxl_spec::division::{Subtype, NAME_BYTES};

use crate::abi::overlay::{DivisionPart, Snapshot};
use crate::domain::setting::store::SettingKey;
use crate::spec::perf::limit;

/// A list of `cnt` elements of `bytes` each past what is laid, with the padding
/// to its alignment, at most 8.
const fn list(cnt: usize, bytes: usize) -> usize {
    7 + cnt * bytes
}

/// overlay.show's args: one snapshot (wit/overlay.wit), its draft's text, its
/// settings, its division's parts and their names, and its surfaces.
const SNAPSHOT: usize = {
    let draft = list(4 * MARKER_TEXT_CHARS as usize, 1);
    let settings = list(SettingKey::SLOT_CNT, 4);
    let part_cnt = Subtype::ALL.len();
    let division = list(part_cnt, DivisionPart::SZ) + part_cnt * list(NAME_BYTES, 1);
    let surfaces = list(SURFACE_CAP, 4);
    Snapshot::SZ + draft + settings + division + surfaces
};

/// A perf read's return data: one page (perf.json5 LIMIT.PAGE_BYTES), a list of
/// bytes.
const PERF_PAGE: usize = 8 + list(limit::PAGE_BYTES as usize, 1);

/// The largest packet the core lays, a multiple of 8.
pub const PKT_CAP: u32 = {
    let most = if SNAPSHOT > PERF_PAGE { SNAPSHOT } else { PERF_PAGE };
    most.next_multiple_of(8) as u32
};
