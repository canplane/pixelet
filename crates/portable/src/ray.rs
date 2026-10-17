// pxl-portable/ray.rs

//! gpu-portable: a ray's words as the GPU's cast leaves them at its drawn
//! pixel and the screen pass reads them there (pxl-render
//! frame/shader/screen_rays.wgsl), a layer of the world surface's rays a
//! word: what it stopped on, through which face and at which zoom
//! (`first_word`); how far, f32 bits; and the four looks a coarse ground
//! block folds (`ground::word`), or in the steps view the nodes its descent
//! asked. A block is painted from these, its own, alone. Where it met
//! nothing, the first two are zero.

use crate::Zoom;

/// A ray's words.
pub const WORDS: u32 = 3;

/// The first word's fields: the value, a ground look or a placed catalog
/// value; what it met (`KIND_*`); the face it entered through, 2·axis plus
/// one where its outward normal points down the axis; the zoom of the block
/// it stopped on, six bits.
pub const VALUE_BITS: u32 = 16;
pub const KIND_BITS: u32 = 2;
pub const FACE_BITS: u32 = 3;
pub use crate::ZOOM_BITS;
pub const KIND_SHIFT: u32 = VALUE_BITS;
pub const FACE_SHIFT: u32 = KIND_SHIFT + KIND_BITS;
pub const ZOOM_SHIFT: u32 = FACE_SHIFT + FACE_BITS;
const _: () = assert!(ZOOM_SHIFT + ZOOM_BITS <= 32);
pub const KIND_NONE: u32 = 0;
pub const KIND_GROUND: u32 = 1;
pub const KIND_PLACED: u32 = 2;

/// The `bits` low bits of `w` past `shift`.
const fn field(w: u32, shift: u32, bits: u32) -> u32 {
    (w >> shift) & ((1 << bits) - 1)
}

pub const fn first_word(value: u32, kind: u32, face: u32, zoom: Zoom) -> u32 {
    value & ((1 << VALUE_BITS) - 1) | kind << KIND_SHIFT | face << FACE_SHIFT | zoom << ZOOM_SHIFT
}

pub const fn kind_of(w: u32) -> u32 {
    field(w, KIND_SHIFT, KIND_BITS)
}

pub const fn face_of(w: u32) -> u32 {
    field(w, FACE_SHIFT, FACE_BITS)
}

pub const fn zoom_of(w: u32) -> Zoom {
    field(w, ZOOM_SHIFT, ZOOM_BITS)
}
