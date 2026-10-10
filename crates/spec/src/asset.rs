// pxl-spec/asset.rs

//! The shipped assets, and the texture pack: which of its tiles a cube type
//! wears on each face. A type with no image still draws as its catalog color.

#[path = "asset/ingested.spec.rs"]
pub mod ingested;

use crate::cell::object::PlaceableCube;
use ingested::{CubeFace, PACK};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileOrigin {
    pub x: u32,
    pub y: u32,
}

/// The pack tile a face wears; None where the pack has no image.
fn tile_of(cube: PlaceableCube, face: CubeFace) -> Option<u32> {
    PACK.tiles.iter().find(|(c, f, _)| *c == cube && *f == face).map(|&(_, _, n)| n)
}

/// The tile's mean color in linear light, seen from far enough that the tile
/// is gone.
pub fn shipped_tile_mean(cube: PlaceableCube, face: CubeFace) -> Option<[u8; 3]> {
    PACK.means.get(tile_of(cube, face)? as usize).copied()
}

/// The tile's pixel origin in the pack's image.
pub fn shipped_texture_tile(cube: PlaceableCube, face: CubeFace) -> Option<TileOrigin> {
    let n = tile_of(cube, face)?;
    Some(TileOrigin {
        x: (n % PACK.columns) * PACK.cell,
        y: (n / PACK.columns) * PACK.cell,
    })
}
