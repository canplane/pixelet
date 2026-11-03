// pxl-spec/asset/cube.rs

//! The cube kind (`spec/spec/asset/cube.json`): the ground and the cubes,
//! one table with the GPU's (pxl-content), and a cube as the catalog
//! gives it out to be placed.

use std::sync::LazyLock;

pub use pxl_content::cube::*;

use crate::asset::color;
use crate::asset::id::AssetId;

/// A cube of the catalog (`asset/cube.json`) is its id: a palette entry of the
/// ground's, a byte, or past a byte one that is only ever placed; a player may
/// place either. None for VOID, UNKNOWN, AIR or a decor kind.
pub fn cube_of(id: AssetId) -> Option<AssetId> {
    let known = match u8::try_from(id) {
        Ok(byte) => Ground::of(byte).is_some(),
        Err(_) => u16::try_from(id).ok().and_then(Object::of).is_some(),
    };
    known.then_some(id)
}

pub fn cube_rgb(id: AssetId) -> Option<u32> {
    match u8::try_from(id) {
        Ok(byte) => color::ground(byte),
        Err(_) => color::object(u16::try_from(id).ok()?),
    }
    .map(|s| s.rgb)
}

/// The placed colors first, then the palette in paint order.
pub static ALL_CUBES: LazyLock<Vec<AssetId>> = LazyLock::new(|| {
    Object::ALL
        .into_iter()
        .map(|o| o as AssetId)
        .chain(Ground::ALL.into_iter().map(|g| g as AssetId))
        .collect()
});

#[cfg(test)]
#[path = "cube.test.rs"]
mod test;
