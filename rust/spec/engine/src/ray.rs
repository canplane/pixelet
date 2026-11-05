// pxl-engine/ray.rs

//! gpu-portable: a ray's words as the GPU's cast leaves them at its drawn
//! pixel and the screen pass reads them there (pxl-render
//! frame/shader/screen_ray.wgsl), a layer of the world surface's rays a
//! word: what it stopped on, through which face, at which zoom and the
//! share of its cube it keeps (`first_word`); how far, f32 bits; the four palette indices the ground's asset is
//! drawn by (`ground::word`, none for any other), or in the steps view the
//! nodes its descent asked; and what lies about the face it entered
//! (`face::word`), the world's own answer about the cell, the same for
//! every pixel of it. A cell is painted from these, its own, alone, by its
//! value whichever source holds it. Where it met nothing, the first two
//! are zero: a hit's value is never void.

use crate::Zoom;

/// A ray's words, and the word of what lies about its face.
pub const WORDS: u32 = 4;
pub const FACES: u32 = 3;

/// The first word's fields: the value, one of the catalog's whichever
/// source holds it; the face it entered through, 2·axis plus one where its
/// outward normal points down the axis; the zoom of the cell it stopped on,
/// six bits; the eighths of its cube the cell keeps, 0 for all eight
/// (`answer::fill_eighths`), so the screen pass draws the box it is
/// (`answer::side`).
pub use crate::answer::VAL_BITS;
pub const FACE_BITS: u32 = 3;
use crate::answer;
pub use crate::answer::FILL_BITS;
use crate::count;
use crate::node_cache::MARKS;
use crate::WORKGROUPS_MOST;
pub use crate::ZOOM_BITS;

pub const FACE_SHIFT: u32 = VAL_BITS;
pub const ZOOM_SHIFT: u32 = FACE_SHIFT + FACE_BITS;
pub const FILL_SHIFT: u32 = ZOOM_SHIFT + ZOOM_BITS;
const _: () = assert!(FILL_SHIFT + FILL_BITS < 32);

/// The first word the cast's reading walk leaves a pixel it could not
/// finish from the node cache alone, for the walk that writes it to
/// find (pxl-render cast.wgsl `cast_cs`, miss.wgsl `miss_cs`): no hit's, as
/// `first_word` sets no bit past the fill share's, and not the zero of one that
/// met nothing. It lives within the cast's one pass: the walk again runs
/// after the reading walk in that pass and overwrites every pixel so
/// marked, so no reader past it, the screen pass's or any frame's, ever
/// sees it.
pub const MISS: u32 = u32::MAX;
const _: () = assert!(MISS >> (FILL_SHIFT + FILL_BITS) != 0);

/// The walk again's dispatches, as the reading walk counts what it marks
/// (pxl-render cast.wgsl `cast_cs`): over the marked pixels listed
/// (`node_cache::MARKS_AT`), `MISS_LANES` a workgroup, `MISS_ACROSS`
/// workgroups a row and a row each `MISS_LANES * MISS_ACROSS` listed
/// (`MISS_ARGS_LIST`); over every pixel, 8 × 8 a workgroup, where any
/// marked pixel found the list full, else none (`MISS_ARGS_WHOLE`); and the
/// count listed or not (`MISS_ARGS_CNT`); then what its pixels, or the
/// light's slots, counted where the `ray_counts` setting asks (`count::RAYS`
/// lanes from `count::REUSED`, `MISS_ARGS_COUNT`). Each frame's start
/// (`MISS_ARGS_EMPTY`) dispatches neither and counts none.
pub const MISS_LANES: u32 = 64;
pub const MISS_ACROSS: u32 = 64;
pub const MISS_ARGS_LIST: u32 = 0;
pub const MISS_ARGS_WHOLE: u32 = 3;
pub const MISS_ARGS_CNT: u32 = 6;
pub const MISS_ARGS_COUNT: u32 = 7;
pub const MISS_ARGS_WORDS: usize = (MISS_ARGS_COUNT + count::RAYS) as usize;
pub const MISS_ARGS_EMPTY: [u32; MISS_ARGS_WORDS] = [MISS_ACROSS, 0, 1, 0, 0, 1, 0, 0, 0, 0];
const _: () = assert!(
    MARKS.div_ceil(MISS_LANES * MISS_ACROSS) <= WORKGROUPS_MOST,
    "the rows over the marked pixels listed fit one dispatch"
);

/// The eye's walk again's words past those: the args what lies about the
/// faces its hits met is asked by, once a square of them (pxl-render
/// hit_lead.wgsl `hit_lead_cs`, face.wgsl `face_cs`), laid as the boxes' are
/// (`raster::ARGS_WORDS`: their count, their dispatch, where their asks
/// lie), at a storage binding's alignment, 256 bytes on every device, so
/// the faces' pass binds them alone as it binds the boxes'. Set each frame
/// the cast runs, as the words before them are.
pub const MISS_ARGS_ROOM: u32 = 64;
const _: () = assert!(MISS_ARGS_WORDS as u32 <= MISS_ARGS_ROOM);

/// The side of the pixels' pass's workgroup, a square of pixels (pxl-render cast.wgsl
/// `cast_cs`): the walk over every pixel is dispatched by it where the list is full
/// (`MISS_ARGS_WHOLE`).
pub const CAST_SIDE: u32 = 8;

/// The face a ray going `dir` enters across `axis`: 2·axis, and one more where it goes up
/// that axis, the face's outward normal pointing down it; a ray that does not move along the
/// axis enters it as one going down.
pub fn face(dir: [f32; 3], axis: u32) -> u32 {
    2 * axis + u32::from(crate::axis(dir, axis) > 0.0)
}

/// The first word of a hit at `asset`, an asset's word (`answer`): its
/// value and the share it keeps, the face and the zoom.
pub const fn first_word(asset: u32, face: u32, zoom: Zoom) -> u32 {
    let fill = asset >> answer::FILL_SHIFT & ((1 << FILL_BITS) - 1);
    answer::val(asset) | face << FACE_SHIFT | zoom << ZOOM_SHIFT | fill << FILL_SHIFT
}
