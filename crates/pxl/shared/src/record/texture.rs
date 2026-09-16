// pxl_shared/record/texture.rs

//! The texture pack this build ships: which image a cube type wears on
//! each face, by bucket key.
//!
//! A pack decorates the palette; it does not define it. A cube's identity
//! and colour are the palette row's (spec/cell/color.rs), and the pack is
//! one image per type and face on top of that, so a type with no image
//! here still draws — as its colour — and a pack can be swapped without
//! touching an id. Which image is which is the file's name under
//! asset/cube: `<channel>_<type>.png` for a top, `side_<type>.png` for a
//! land side, `block_<colour>.png` for a placed block; the bytes and keys
//! come from ingested.rs.

use std::collections::HashMap;
use std::sync::LazyLock;

use super::ingested::{Sheet, INGESTED, SHEET};

/// Which of a box's images this is. The split is the renderer's own: a face
/// is lateral at `abs(normal.z) < 0.5`, so a bottom face wears the top image
/// just as the top face does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CubeFace {
    Top,
    Side,
}

fn path_of(channel: &str, type_name: &str, face: CubeFace) -> Option<String> {
    let r#type = type_name.to_lowercase();
    if channel == "block" {
        return if face == CubeFace::Top { Some(format!("cube/block/block_{type}.png")) } else { None };
    }
    Some(match face {
        CubeFace::Top => format!("cube/terrain/{channel}_{type}.png"),
        CubeFace::Side => format!("cube/terrain/side_{type}.png"),
    })
}

/// The bucket key of the image a cube type wears on a face, or None when
/// the pack has none for it — the caller decides whether a side falls back
/// to the top or the type to its colour.
pub fn shipped_texture_key(channel: &str, type_name: &str, face: CubeFace) -> Option<&'static str> {
    path_of(channel, type_name, face).and_then(|path| INGESTED.get(path.as_str()).map(|row| row.key))
}

/// The pack as one image, for the layer array: the sheet's key and
/// geometry, and where a type's face sits on it.
pub const SHIPPED_SHEET: &Sheet = &SHEET;

static TILE_INDEX: LazyLock<HashMap<&'static str, u32>> = LazyLock::new(|| SHEET.tiles.iter().enumerate().map(|(n, path)| (*path, n as u32)).collect());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileOrigin {
    pub x: u32,
    pub y: u32,
}

/// The tile of the sheet a cube type wears on a face — its pixel origin on
/// the sheet — or None when the pack has no image for it.
pub fn shipped_texture_tile(channel: &str, type_name: &str, face: CubeFace) -> Option<TileOrigin> {
    let path = path_of(channel, type_name, face)?;
    let n = *TILE_INDEX.get(path.as_str())?;
    Some(TileOrigin {
        x: (n % SHEET.columns) * SHEET.cell,
        y: (n / SHEET.columns) * SHEET.cell,
    })
}

#[cfg(test)]
#[path = "texture.test.rs"]
mod test;
