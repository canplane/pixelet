// pxl-spec-core/ray.rs

//! gpu-portable: a ray's words as the GPU's cast leaves them at its drawn
//! pixel and the screen pass reads them there (pxl-render
//! frame/shader/screen_ray.wgsl), a layer of the world surface's rays a
//! word: what it stopped on, through which face, at which zoom and the
//! share of its cube it keeps (`first_word`); how far, f32 bits; the four looks the ground's block is
//! drawn by (`ground::word`, none for any other), or in the steps view the
//! nodes its descent asked; and what lies about the face it entered
//! (`faces::word`), the world's own answer about the block, the same for
//! every pixel of it. A block is painted from these, its own, alone, by its
//! value whichever source holds it. Where it met nothing, the first two
//! are zero: a stop's value is never void.

use crate::Zoom;

/// A ray's words, and the word of what lies about its face.
pub const WORDS: u32 = 4;
pub const FACES: u32 = 3;

/// The first word's fields: the value, one of the catalog's whichever
/// source holds it (doc/minutes/20261005-the-ground-is-a-carto-palette.md,
/// "One value space"); the face it entered through, 2·axis plus one where
/// its outward normal points down the axis; the zoom of the block it
/// stopped on, six bits; the eighths of its cube the block keeps, 0 for all
/// eight (`answer::kept`), so the screen pass draws the box it is
/// (`answer::side`).
pub use crate::answer::VALUE_BITS;
pub const FACE_BITS: u32 = 3;
pub use crate::answer::KEPT_BITS;
pub use crate::ZOOM_BITS;
pub const FACE_SHIFT: u32 = VALUE_BITS;
pub const ZOOM_SHIFT: u32 = FACE_SHIFT + FACE_BITS;
pub const KEPT_SHIFT: u32 = ZOOM_SHIFT + ZOOM_BITS;
const _: () = assert!(KEPT_SHIFT + KEPT_BITS < 32);

/// The first word the cast's reading walk leaves a pixel it could not
/// finish from the observed pyramid alone, for the walk that writes it to
/// find (pxl-shader `cast_cs`, `miss_cs`): no stop's, as
/// `first_word` sets no bit past the kept share's, and not the zero of one that
/// met nothing. It lives within the cast's one pass: the walk again runs
/// after the reading walk in that pass and overwrites every pixel so
/// marked, so no reader past it, the screen pass's or any frame's, ever
/// sees it.
pub const MISS: u32 = u32::MAX;
const _: () = assert!(MISS >> (KEPT_SHIFT + KEPT_BITS) != 0);

/// The walk again's dispatches, as the reading walk counts what it marks
/// (pxl-shader `cast_cs`): over the marked pixels listed
/// (`observed::MARKS_AT`), `MISS_LANES` a workgroup, `MISS_ACROSS`
/// workgroups a row and a row each `MISS_LANES * MISS_ACROSS` listed
/// (`MISS_ARGS_LIST`); over every pixel, 8 × 8 a workgroup, where any
/// marked pixel found the list full, else none (`MISS_ARGS_WHOLE`); and the
/// count listed or not (`MISS_ARGS_COUNT`); then what its pixels, or the
/// sun's slots, counted where the `ray_counts` setting asks (`tally::RAYS`
/// lanes from `tally::PICKED`, `MISS_ARGS_TALLY`). Each frame's start
/// (`MISS_ARGS_EMPTY`) dispatches neither and counts none.
pub const MISS_LANES: u32 = 64;
pub const MISS_ACROSS: u32 = 64;
pub const MISS_ARGS_LIST: u32 = 0;
pub const MISS_ARGS_WHOLE: u32 = 3;
pub const MISS_ARGS_COUNT: u32 = 6;
pub const MISS_ARGS_TALLY: u32 = 7;
pub const MISS_ARGS_WORDS: usize = (MISS_ARGS_TALLY + crate::tally::RAYS) as usize;
pub const MISS_ARGS_EMPTY: [u32; MISS_ARGS_WORDS] = [MISS_ACROSS, 0, 1, 0, 0, 1, 0, 0, 0, 0];

/// The first word of a stop at `block`, a block's word (`answer`): its
/// value and the share it keeps, the face and the zoom.
pub const fn first_word(block: u32, face: u32, zoom: Zoom) -> u32 {
    let kept = block >> crate::answer::KEPT_SHIFT & ((1 << KEPT_BITS) - 1);
    crate::answer::value(block) | face << FACE_SHIFT | zoom << ZOOM_SHIFT | kept << KEPT_SHIFT
}
