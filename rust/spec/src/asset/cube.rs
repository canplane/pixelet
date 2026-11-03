// pxl-spec/asset/cube.rs

//! The cube kind (`spec/spec/asset/cube.json`): the ground and the cubes,
//! one table with the GPU's (pxl-content), and a cube as the catalog
//! gives it out to be placed.

use std::sync::LazyLock;

pub use pxl_content::cube::*;

use crate::asset::color;
use crate::asset::id::AssetId;

/// A cube of the catalog (`asset/cube.json`) by its number, the shell's too: a
/// palette entry of the ground's, a byte, or past a byte one that is only ever
/// placed; a player may place either.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cube(u16);

impl Cube {
    pub const fn ground(g: Ground) -> Cube {
        Cube(g as u16)
    }

    pub const fn object(o: Object) -> Cube {
        Cube(o as u16)
    }
}

/// The asset id a placement stores.
pub fn cube_id(cube: Cube) -> AssetId {
    u32::from(cube.0)
}

/// None for VOID, UNKNOWN, AIR or a decor kind.
pub fn cube_of(num_id: AssetId) -> Option<Cube> {
    match u8::try_from(num_id) {
        Ok(byte) => Ground::of(byte).map(Cube::ground),
        Err(_) => u16::try_from(num_id)
            .ok()
            .and_then(Object::of)
            .map(Cube::object),
    }
}

pub fn cube_rgb(cube: Cube) -> Option<u32> {
    match u8::try_from(cube.0) {
        Ok(byte) => color::ground(byte),
        Err(_) => color::object(cube.0),
    }
    .map(|s| s.rgb)
}

/// The placed colors first, then the palette in paint order.
pub static ALL_CUBES: LazyLock<Vec<Cube>> = LazyLock::new(|| {
    Object::ALL
        .into_iter()
        .map(Cube::object)
        .chain(Ground::ALL.into_iter().map(Cube::ground))
        .collect()
});

#[cfg(test)]
#[path = "cube.test.rs"]
mod test;
