// pxl-spec/asset/cube.rs

//! The cube kind (`spec/spec/asset/cube.json`): the ground and the cubes,
//! one table with the GPU's (pxl-spec-core), and a cube as the catalog
//! gives it out to be placed.

use std::sync::LazyLock;

pub use pxl_spec_core::cube::*;

use crate::asset::color;
use crate::asset::id::AssetId;

/// A cube of the catalog (`asset/cube.json`): a look of the ground's palette,
/// or a color only an object takes; a player may place either.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cube {
    Ground(Ground),
    Object(Object),
}

/// The asset id a placement stores.
pub fn cube_id(cube: Cube) -> AssetId {
    match cube {
        Cube::Ground(g) => g as u32,
        Cube::Object(b) => b as u32,
    }
}

/// None for VOID, UNKNOWN, AIR or a decor kind.
pub fn cube_of(num_id: AssetId) -> Option<Cube> {
    match u8::try_from(num_id) {
        Ok(byte) => Ground::of(byte).map(Cube::Ground),
        Err(_) => u16::try_from(num_id)
            .ok()
            .and_then(Object::of)
            .map(Cube::Object),
    }
}

pub fn cube_rgb(cube: Cube) -> Option<u32> {
    match cube {
        Cube::Ground(g) => color::ground(g as u8),
        Cube::Object(b) => color::object(b as u16),
    }
    .map(|s| s.rgb)
}

/// The placed colors first, then the palette in paint order.
pub static ALL_CUBES: LazyLock<Vec<Cube>> = LazyLock::new(|| {
    Object::ALL
        .into_iter()
        .map(Cube::Object)
        .chain(Ground::ALL.into_iter().map(Cube::Ground))
        .collect()
});

#[cfg(test)]
#[path = "cube.test.rs"]
mod test;
