// pxl-spec/asset.rs

//! The shipped assets, and the texture pack: which of its tiles a cube
//! wears on each face. A type with no image still draws as its catalog color.

#[path = "asset/ingested.spec.rs"]
pub mod ingested;
#[path = "asset/symbol.spec.rs"]
pub mod symbol;
pub mod star;

use crate::block::object::Cube;
use ingested::{CubeFace, PACK};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileOrigin {
    pub x: u32,
    pub y: u32,
}

/// The pack tile a face wears; None where the pack has no image.
fn tile_of(cube: Cube, face: CubeFace) -> Option<u32> {
    PACK.tiles.iter().find(|(c, f, _)| *c == cube && *f == face).map(|&(_, _, n)| n)
}

/// The tile's mean color in linear light, seen from far enough that the tile
/// is gone.
pub fn shipped_tile_mean(cube: Cube, face: CubeFace) -> Option<[u8; 3]> {
    PACK.means.get(tile_of(cube, face)? as usize).copied()
}

/// The tile's pixel origin in the pack's image.
pub fn shipped_texture_tile(cube: Cube, face: CubeFace) -> Option<TileOrigin> {
    let n = tile_of(cube, face)?;
    Some(TileOrigin {
        x: (n % PACK.columns) * PACK.cell,
        y: (n / PACK.columns) * PACK.cell,
    })
}

/// The width and height a PNG's or a WebP's header gives; none for any other file.
pub fn image_size(file: &[u8]) -> Option<(u32, u32)> {
    let be = |at: usize| Some(u32::from_be_bytes(file.get(at..at + 4)?.try_into().ok()?));
    let le24 = |at: usize| Some(u32::from_le_bytes([*file.get(at)?, *file.get(at + 1)?, *file.get(at + 2)?, 0]));
    let le16 = |at: usize| Some(u32::from(u16::from_le_bytes(file.get(at..at + 2)?.try_into().ok()?)));
    if file.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some((be(16)?, be(20)?));
    }
    if !(file.starts_with(b"RIFF") && file.get(8..12) == Some(b"WEBP")) {
        return None;
    }
    match file.get(12..16)? {
        // Lossy: the frame's 14-bit width and height after its start code.
        b"VP8 " => Some((le16(26)? & 0x3fff, le16(28)? & 0x3fff)),
        // Lossless: 14 bits each, less one, after the signature byte.
        b"VP8L" => {
            let bits = u32::from_le_bytes(file.get(21..25)?.try_into().ok()?);
            Some(((bits & 0x3fff) + 1, (bits >> 14 & 0x3fff) + 1))
        }
        // Extended: the canvas's 24-bit width and height, less one.
        b"VP8X" => Some((le24(24)? + 1, le24(27)? + 1)),
        _ => None,
    }
}
