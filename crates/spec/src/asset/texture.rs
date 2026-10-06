// pxl-spec/asset/texture.rs

//! The shipped texture pack: which sheet tile a cube type wears on each face.
//! A type with no image still draws as its catalog color.

pub use super::ingested::CubeFace;
use super::ingested::{Sheet, SHEET};
use crate::cell::object::PlaceableCube;

/// The pack's sheet geometry and where each type's face sits on it.
pub const SHIPPED_SHEET: &Sheet = &SHEET;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileOrigin {
    pub x: u32,
    pub y: u32,
}

/// The tile's mean color in linear light, seen from far enough that the tile
/// is gone; None where the pack has no image.
pub fn shipped_tile_mean(cube: PlaceableCube, face: CubeFace) -> Option<[u8; 3]> {
    let &(_, _, n) = SHEET.tiles.iter().find(|(c, f, _)| *c == cube && *f == face)?;
    SHEET.means.get(n as usize).copied()
}

/// The tile's pixel origin on the sheet, or None when the pack has no image.
pub fn shipped_texture_tile(cube: PlaceableCube, face: CubeFace) -> Option<TileOrigin> {
    let &(_, _, n) = SHEET.tiles.iter().find(|(c, f, _)| *c == cube && *f == face)?;
    Some(TileOrigin {
        x: (n % SHEET.columns) * SHEET.cell,
        y: (n / SHEET.columns) * SHEET.cell,
    })
}
