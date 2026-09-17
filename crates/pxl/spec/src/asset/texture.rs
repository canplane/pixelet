// pxl-spec/asset/texture.rs

//! The texture pack this build ships: which tile of the pack's sheet a cube
//! type wears on each face.
//!
//! A pack decorates the cube catalog; it does not define it. A cube's identity
//! and colour are spec/cell's (spec/cell/color.rs), and the pack is one image
//! per type and face on top of that, so a type with no image here still draws
//! — as its colour — and a pack can be swapped without touching an id. Which
//! cube face a tile is, the ingest reads off the file's name and writes into
//! ingested.rs; the sheet's bucket key is the shell's.

pub use super::ingested::CubeFace;
use super::ingested::{Sheet, SHEET};
use crate::cell::PlaceableCube;

/// The pack as one image, for the layer array: the sheet's geometry, and
/// where a type's face sits on it.
pub const SHIPPED_SHEET: &Sheet = &SHEET;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileOrigin {
    pub x: u32,
    pub y: u32,
}

/// The tile of the sheet a cube type wears on a face — its pixel origin on
/// the sheet — or None when the pack has no image for it; the caller decides
/// whether a side falls back to the top or the type to its colour.
pub fn shipped_texture_tile(cube: PlaceableCube, face: CubeFace) -> Option<TileOrigin> {
    let &(_, _, n) = SHEET.tiles.iter().find(|(c, f, _)| *c == cube && *f == face)?;
    Some(TileOrigin {
        x: (n % SHEET.columns) * SHEET.cell,
        y: (n / SHEET.columns) * SHEET.cell,
    })
}
