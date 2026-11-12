// pxl-core/link/bound.rs

//! The most bytes a packet the core lays takes, by construction: every list
//! and string it carries has a cap, and each XTP node is bounded above by its
//! layout (xpute-kit wire/xtp encode.rs), its alignment's padding included.
//! The link's frames are as large as the largest (lib.rs `link!`). What the
//! UI sends in past them is refused with EMSGSIZE.

use pxl_render::gpu::surface::SURFACES;
use pxl_spec::asset::marker::MARKER_TEXT_CHARS;
use pxl_spec::division::{Subtype, NAME_BYTES};

use crate::domain::layer::object::repository::WANTS;
use crate::domain::setting::store::SettingKey;
use crate::perf::spec::LIMIT;
use crate::sys::io::file::KEY_BYTES;

/// A packet's header, and its root's table of `cnt` fields.
const fn packet(cnt: usize, fields: usize) -> usize {
    16 + 8 * (1 + cnt) + fields
}

/// A scalar of up to 8 bytes, padded to its alignment.
const SCALAR: usize = 16;

/// An array of `bytes`: its length's word and its bytes, padded to a word.
const fn seq(bytes: usize) -> usize {
    16 + bytes.next_multiple_of(8)
}

/// A string of `bytes`: as an array, with its terminating NUL.
const fn str(bytes: usize) -> usize {
    seq(bytes + 1)
}

/// A branch of `cnt` children, its table padded to a word, with what the
/// children take.
const fn branch(cnt: usize, children: usize) -> usize {
    8 + 8 * (1 + cnt) + children
}

/// overlay.show's args: one snapshot (wit/overlay.wit).
const SNAPSHOT: usize = {
    let lon_lat = branch(2, 2 * SCALAR);
    let pivot = branch(4, 3 * SCALAR + lon_lat);
    let location = branch(4, 4 * SCALAR);
    let companion = branch(2, 2 * SCALAR);
    let draft = str(4 * MARKER_TEXT_CHARS as usize);
    let orient = branch(1, SCALAR);
    let settings = branch(SettingKey::SLOTS, SettingKey::SLOTS * SCALAR);
    let part = branch(4, 3 * SCALAR + str(NAME_BYTES));
    let division = branch(Subtype::ALL.len(), Subtype::ALL.len() * part);
    let surfaces = branch(SURFACES, SURFACES * SCALAR);
    let path = branch(2, SCALAR + str(KEY_BYTES));
    let refused = 1 + WANTS as usize;
    let refused_files = branch(refused, refused * branch(1, path));
    let fields = pivot + location + companion + draft + orient + 4 * SCALAR + settings + division
        + surfaces + refused_files + 3 * SCALAR;
    packet(1, branch(16, fields))
};

/// A perf read's return data: one page (perf.json LIMIT.PAGE_BYTES).
const PERF_PAGE: usize = packet(1, seq(LIMIT::PAGE_BYTES as usize));

/// The largest packet the core lays, a multiple of 8.
pub const PKT_CAP: u32 = {
    let most = if SNAPSHOT > PERF_PAGE { SNAPSHOT } else { PERF_PAGE };
    most.next_multiple_of(8) as u32
};
