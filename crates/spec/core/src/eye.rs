// pxl-spec-core/eye.rs

//! gpu-portable: the eye a cast is from, as the words the kernel writes
//! (pxl-kernel view/lens.rs `gpu_cast`, scene/cast.rs `eye`) and every GPU
//! reader takes (pxl-shader `Eye`, pxl-render gpu/renderer.rs and
//! gpu/grown.rs, the screen pass). Each is a word's place among them, one
//! layout written here alone.

/// The eye's position in Q3 steps, X, Y and Z.
pub const ORIGIN: usize = 0;
/// Whether each ray reports the nodes its descent asked, not what it met.
pub const STEPS: usize = 3;
/// The unit forward, X, Y and Z, then the reach, f32 bits.
pub const FORWARD: usize = 4;
pub const REACH: usize = 7;
/// The unit right, then τ, f32 bits.
pub const RIGHT: usize = 8;
pub const TAU: usize = 11;
/// The unit up, f32 bits, then a word no reader reads, so the next starts a
/// vec4.
pub const UP: usize = 12;
/// The tangents of the screen's half-angles across and up, f32 bits.
pub const TAN: usize = 16;
/// The zooms a block is asked at, coarsest then finest.
pub const RANGE: usize = 18;
/// A preview's cut to air and its left-out leaf, each X, Y, Z and the zoom,
/// the zoom `NO_CUT` for none (pxl-spec-core `overwrite::Cut`).
pub const CARVE: usize = 20;
pub const HIDE: usize = 24;
pub const CUT_WORDS: usize = 4;
/// Both cuts, carve then hide, as one run of words.
pub const CUTS: usize = 2 * CUT_WORDS;
pub const NO_CUT: u32 = crate::overwrite::Cut::NONE.zoom;
/// Every word, the cuts last.
pub const WORDS: usize = HIDE + CUT_WORDS;
const _: () = assert!(STEPS == ORIGIN + 3 && REACH == FORWARD + 3 && TAU == RIGHT + 3 && TAN == UP + 4 && RANGE == TAN + 2 && CARVE == RANGE + 2 && HIDE == CARVE + CUT_WORDS);
