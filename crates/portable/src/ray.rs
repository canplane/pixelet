// pxl-portable/ray.rs

//! gpu-portable: a ray's words as the GPU's cast leaves them at its drawn
//! pixel and the screen pass reads them there (pxl-render
//! frame/shader/screen_rays.wgsl), a layer of the world surface's rays a
//! word: what it stopped on, through which face and at which zoom
//! (`first_word`); how far, f32 bits; and in the steps view the nodes its
//! descent asked. A block is painted from these, its own, alone, by its
//! value whichever source holds it. Where it met nothing, the first two are
//! zero: a stop's value is never void.

use crate::Zoom;

/// A ray's words.
pub const WORDS: u32 = 3;

/// The first word's fields: the value, one of the catalog's whichever
/// source holds it (doc/minutes/20261005-the-ground-is-a-carto-palette.md,
/// "One value space"); the face it entered through, 2·axis plus one where
/// its outward normal points down the axis; the zoom of the block it
/// stopped on, six bits.
pub const VALUE_BITS: u32 = 16;
pub const FACE_BITS: u32 = 3;
pub use crate::ZOOM_BITS;
pub const FACE_SHIFT: u32 = VALUE_BITS;
pub const ZOOM_SHIFT: u32 = FACE_SHIFT + FACE_BITS;
const _: () = assert!(ZOOM_SHIFT + ZOOM_BITS <= 32);

pub const fn first_word(value: u32, face: u32, zoom: Zoom) -> u32 {
    value & ((1 << VALUE_BITS) - 1) | face << FACE_SHIFT | zoom << ZOOM_SHIFT
}
