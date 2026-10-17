// pxl-portable/ray.rs

//! gpu-portable: a ray's words as the GPU's cast leaves them at its drawn
//! pixel and the screen pass reads them there (pxl-render
//! frame/shader/screen_rays.wgsl), a layer of the world surface's rays a
//! word: what it stopped on, through which face and at which zoom
//! (`first_word`); how far, f32 bits; the block's face bits (`mask`), which
//! the face pass writes over the block's index bits it finds the block by;
//! the four looks a coarse ground block folds (`ground::word`), or in the
//! steps view the nodes its descent asked; and where the stop's column is
//! held and which shards about it are served (`place_word`), which the face
//! pass reads its block's neighbourhood off. Where it met nothing, the first
//! three are zero.

use crate::area::{Shard, SHARD_SERVED_BITS};
use crate::window::{Found, FIRST_CELL, FOUND_BITS};
use crate::{Zoom, SHARD_ZOOM};

/// A ray's words.
pub const WORDS: u32 = 5;

/// The fifth word, `place_word`'s.
pub const PLACE: u32 = 4;
const _: () = assert!(FOUND_BITS + SHARD_SERVED_BITS <= 32);
// A block a window's cells are read at is a shard's zoom or finer, so its
// shard is known wherever its column's place is.
const _: () = assert!(FIRST_CELL > SHARD_ZOOM);

/// Where the stop's column is held (`window::Found`) and, over it, which
/// of the nine shards about the stop are served (`area::Shard`): known at
/// a shard's zoom or finer and none coarser, as a node's shard is.
pub const fn place_word(found: Found, shard: Shard) -> u32 {
    found.word() | shard.served() << FOUND_BITS
}

/// `place_word`'s, for a stop at `zoom`.
pub const fn place_of(w: u32, zoom: Zoom) -> (Found, Shard) {
    let shard = if zoom >= SHARD_ZOOM { Shard::of_served(w >> FOUND_BITS) } else { Shard::UNKNOWN };
    (Found::of_word(w & ((1 << FOUND_BITS) - 1)), shard)
}

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
