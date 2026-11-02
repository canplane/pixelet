// pxl-engine/screen.rs

//! The screen's tiles, which a frame not drawn whole draws again by, as the
//! light's map is cast by its own (light.rs): each `TILE` drawn pixels a side,
//! its word the number of the frame it last changed in (`changed_at`), as a
//! light tile's names a cast, the GPU's; its age the frames since (`age`);
//! claimed while its history has not run through the jitter's `JITTERS`
//! frames (`CLAIMED_FOR`)
//! (pxl-render gpu/resolve.rs; doc/minutes/20261029-the-screen-keeps-its-tiles-as-the-light-does.md).

/// A tile's drawn pixels a side.
pub const TILE: u32 = 32;

/// The frames the jitter's sequence covers a pixel in.
pub const JITTERS: u32 = 8;

/// The frames a tile stays claimed once changed (`claimed_for`): the frame
/// it changed in, its color held to its pixels, then `JITTERS - 1` still
/// frames, the first taking half of what stands and each after it its even
/// share (pxl-render screen_resolve.wgsl), each frame at its own place of the
/// jitter's sequence.
pub const CLAIMED_FOR: u32 = JITTERS;

/// The words of the uniform a pass draws the tiles by: the tiles across and
/// down, the frames a tile stays claimed once changed (`claimed_for`, all of
/// them where every one is drawn), this frame's number; the target's pixels
/// across and down, how far a quad is grown past its tile, and a tile's
/// side, f32 bits.
pub const TILES_WORDS: usize = 8;

/// The frames since a tile last changed, by its word `changed_at`, in the
/// frame numbered `now`. With `claimed`, the one rule a tile is claimed by:
/// pxl-shader's cast calls it, and pxl-render's build.rs writes it as the
/// hand-written passes' `tiles_age` and `tiles_claimed` (tile_word.wgsl).
pub const fn age(now: u32, changed_at: u32) -> u32 {
    now.wrapping_sub(changed_at)
}

/// Whether a tile `age` frames old is claimed, where a tile stays claimed
/// `claimed_for` frames once changed.
pub const fn claimed(age: u32, claimed_for: u32) -> bool {
    age < claimed_for
}
