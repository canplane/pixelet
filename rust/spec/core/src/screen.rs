// pxl-spec-core/screen.rs

//! The screen's tiles, which a frame not drawn whole draws again by, as the
//! light's map is cast by its own (light.rs): each `TILE` drawn pixels a side,
//! its word the number of the frame it last changed in (`changed_at`), as a
//! light tile's names a cast, the GPU's; its age the frames since (`age`);
//! claimed while its history has not run through the jitter's `JITTERS`
//! frames
//! (pxl-render gpu/resolve.rs; doc/minutes/20261029-the-screen-keeps-its-tiles-as-the-light-does.md).

/// A tile's drawn pixels a side.
pub const TILE: u32 = 32;

/// The frames the jitter's sequence covers a pixel in.
pub const JITTERS: u32 = 8;

/// The words of the uniform a pass draws the tiles by: the tiles across and
/// down, the frames a tile stays claimed once changed (`claimed_for`, all of
/// them where every one is drawn), this frame's number; the target's pixels
/// across and down, how far a quad is grown past its tile, and a tile's
/// side, f32 bits.
pub const TILES_WORDS: usize = 8;

/// The frames since a tile last changed, by its word `changed_at`, in the
/// frame numbered `now`.
pub const fn age(now: u32, changed_at: u32) -> u32 {
    now.wrapping_sub(changed_at)
}

/// Whether a tile `age` frames old is claimed, where a tile stays claimed
/// `claimed_for` frames once changed.
pub const fn claimed(age: u32, claimed_for: u32) -> bool {
    age < claimed_for
}
