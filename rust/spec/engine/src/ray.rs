// pxl-engine/ray.rs

//! gpu-portable: a ray's words as the GPU's cast leaves them at its drawn
//! pixel and the screen pass reads them there (pxl-render
//! frame/shader/screen_ray.wgsl), a layer of the world surface's rays a
//! word: what it stopped on, through which face, at which zoom and the
//! share of its cube it keeps (`first_word`); how far, f32 bits; the four palette indices the ground's asset is
//! drawn by (`ground::word`, none for any other); and what lies about the
//! face it entered (`face::word`), the world's own answer about the cell, the
//! same for every pixel of it. A cell is painted from these, its own, alone,
//! by its value whichever source holds it. Where it met nothing, every word is
//! zero: a hit's value is never void.

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
pub use crate::ZOOM_BITS;

pub const FACE_SHIFT: u32 = VAL_BITS;
pub const ZOOM_SHIFT: u32 = FACE_SHIFT + FACE_BITS;
pub const FILL_SHIFT: u32 = ZOOM_SHIFT + ZOOM_BITS;
const _: () = assert!(FILL_SHIFT + FILL_BITS < 32);

/// The first word the cast's reading walk leaves a pixel it could not
/// finish from the node cache alone, for the walk that writes it to
/// find (pxl-render cast.wgsl `cast_cs`, miss_hit.wgsl): no hit's, as
/// `first_word` sets no bit past the fill share's, and not the zero of one that
/// met nothing. It lives within the cast's one pass: the walk again runs
/// after the reading walk in that pass and overwrites every pixel so
/// marked, so no reader past it, the screen pass's or any frame's, ever
/// sees it.
pub const MISS: u32 = u32::MAX;
const _: () = assert!(MISS >> (FILL_SHIFT + FILL_BITS) != 0);

/// The walk again's dispatch, as the reading walk counts what it marks
/// (pxl-render cast.wgsl `cast_cs`): over the marked pixels listed
/// (`WALKED_AT`), `MISS_LANES` a workgroup, `MISS_ACROSS`
/// workgroups a row and a row each `MISS_LANES * MISS_ACROSS` listed
/// (`MISS_ARGS_LIST`); the count listed (`MISS_ARGS_CNT`); then what its pixels, or the
/// light's slots, counted where the `ray_counts` setting asks (`count::RAYS`
/// lanes from `count::REUSED`, `MISS_ARGS_COUNT`). Each frame's start
/// (`MISS_ARGS_EMPTY`) dispatches none and counts none. The light's slots' list is laid alike.
pub const MISS_LANES: u32 = 64;
pub const MISS_ACROSS: u32 = 64;
pub const MISS_ARGS_LIST: u32 = 0;
pub const MISS_ARGS_CNT: u32 = 3;
pub const MISS_ARGS_COUNT: u32 = 4;
pub const MISS_ARGS_WORDS: usize = (MISS_ARGS_COUNT + count::RAYS) as usize;
pub const MISS_ARGS_EMPTY: [u32; MISS_ARGS_WORDS] = [MISS_ACROSS, 0, 1, 0, 0, 0, 0];

/// The eye's walked pixels, past the camera grid's words in the beam rays' buffer
/// (`beam::Grid::walked_at`), room for every pixel drawn, so no list of them is ever full: the
/// pixels the reading walk marked (`MISS`), its row over its column, a word each
/// (`WALKED_LIST`), after their count, copied in from the walk's dispatch (`MISS_ARGS_CNT`)
/// once the reading walk is done (`WALKED_CNT`); then those its walk over the node cache left
/// for the walk that asks the world (pxl-render miss_hit.wgsl, miss.wgsl `miss_list_cs`), as
/// many at most, after their count, copied in once that walk is done (`walked_left_at`). A
/// pixel is listed once a cast, so each list holds at most the pixels drawn. `drawn` is the
/// rays' pixels, across times down; each place is a word from `Grid::walked_at`.
pub const WALKED_CNT: u32 = 0;
pub const WALKED_LIST: u32 = 1;

pub const fn walked_left_at(drawn: u32) -> u32 {
    WALKED_LIST + drawn
}

pub const fn walked_words(drawn: u32) -> u32 {
    walked_left_at(drawn) + 1 + drawn
}

/// What a walk over the node cache left, listed for the walk that asks the world (pxl-render
/// list.wgsl `enlist`; beam_hit.wgsl, sample_hit.wgsl, miss_hit.wgsl, light_miss_hit.wgsl):
/// its dispatch, laid as `MISS_ARGS_LIST`'s, no row until one is listed, then its count
/// (`LEFT_CNT`). Every list, these and the walk again's, raises the same row word.
pub const LEFT_EMPTY: [u32; 4] = [MISS_ACROSS, 0, 1, 0];
pub const LEFT_CNT: u32 = 3;
const _: () = assert!(
    MISS_ARGS_LIST == 0,
    "a list's rows are its dispatch's second word"
);

/// The eye's walk again's words past those: the args what lies about the
/// faces its hits met is asked by, once a square of them (pxl-render
/// hit_lead.wgsl `hit_lead_cs`, face.wgsl `face_cs`), laid as the boxes' are
/// (`raster::ARGS_WORDS`: their count within their room, their dispatch,
/// where their asks lie, and their count as they are listed), at a storage binding's alignment, 256 bytes on every device, so
/// the faces' pass binds them alone as it binds the boxes'. Set each frame
/// the cast runs, as the words before them are.
pub const MISS_ARGS_ROOM: u32 = 64;
const _: () = assert!(MISS_ARGS_WORDS as u32 <= MISS_ARGS_ROOM);

/// The side of the pixels' pass's workgroup, a square of pixels (pxl-render cast.wgsl
/// `cast_cs`).
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
