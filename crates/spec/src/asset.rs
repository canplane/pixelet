// pxl-spec/asset.rs

//! The shipped assets, and the texture pack: which sheet tile a cube type
//! wears on each face. A type with no image still draws as its catalog color.

#[path = "asset/ingested.spec.rs"]
pub mod ingested;

use crate::cell::object::PlaceableCube;
use ingested::{CubeFace, SHEET};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileOrigin {
    pub x: u32,
    pub y: u32,
}

/// The sheet tile a face wears; None where the pack has no image.
fn tile_of(cube: PlaceableCube, face: CubeFace) -> Option<u32> {
    SHEET.tiles.iter().find(|(c, f, _)| *c == cube && *f == face).map(|&(_, _, n)| n)
}

/// The tile's mean color in linear light, seen from far enough that the tile
/// is gone.
pub fn shipped_tile_mean(cube: PlaceableCube, face: CubeFace) -> Option<[u8; 3]> {
    SHEET.means.get(tile_of(cube, face)? as usize).copied()
}

/// The tile's pixel origin on the sheet.
pub fn shipped_texture_tile(cube: PlaceableCube, face: CubeFace) -> Option<TileOrigin> {
    let n = tile_of(cube, face)?;
    Some(TileOrigin {
        x: (n % SHEET.columns) * SHEET.cell,
        y: (n / SHEET.columns) * SHEET.cell,
    })
}
