// pxl_core/memory/fixed.rs

//! What is in `.fixed`, and where each of its ranges begins.
//!
//! The map above (memory.rs) knows `.fixed` as one span between the module's
//! data and the heap — how wide it is, and that it begins on a page. What is
//! inside it is here, so that adding a range is a line in this file and
//! nothing in the map.
//!
//! A range is placed once when the module boots, never freed, and of a size
//! settled at compile time. That is the whole of what makes one belong here:
//! anything that is asked for, refused or given back belongs in the heap
//! instead.
//!
//! Ranges, in the order they are laid:
//!
//!   .fixed.terrain   the resident terrain, one record a page, addressed by
//!                    the page's own coordinate reduced modulo the set's axis
//!                    (module/world/terrain/fixed.rs), so nothing in it is
//!                    allocated, asked for or refused.

use xpute_core::sys::memory::{nth_at, span_of};

use super::{map, PAGE_BYTES};
use crate::module::world::terrain::fixed::TERRAIN_BYTES;

/// Each range's bytes, in address order. What a range holds is its own
/// affair; this says how many bytes it takes and nothing more.
const RANGES: [u32; 1] = [TERRAIN_BYTES];

/// What `.fixed` spans, which is where the heap begins past it.
pub const BYTES: u32 = span_of(&RANGES, PAGE_BYTES);

/// Where the nth range begins, `base` being where `.fixed` does.
const fn range_at(base: u32, n: usize) -> u32 {
    nth_at(base, &RANGES, PAGE_BYTES, n)
}

/// Where `.fixed.terrain` lies, once the module has laid its memory out.
pub fn terrain_at() -> Option<u32> {
    map().map(|m| range_at(m.fixed.offset, 0))
}
