// pxl-spec/bake/tile/earth.rs

//! The Earth's picture as a layer of the pyramid, at its top zoom alone, a
//! ground cell a texel: per cell, Blue Marble's reflectance coded as
//! sRGB, as a texture reads it, and in alpha the square root of the Black
//! Marble's lights, so a village keeps its few steps of brightness. Row by row from the
//! tile's south edge, in the ground lattice's cell order. One XTP tree, as every
//! other baked tile is; a tile with no object is answered by the global picture.

use crate::bake::malformed;
use crate::space::coord::zoom::Zoom;
use crate::space::layer::lattice::demand_dist;
use crate::space::layer::lattice::{GROUND, TILE_CELLS};
use crate::space::layer::tile::TOP_ZOOM;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::shape;

/// The Earth's one zoom: the ground's top. Its tiles are the ground's lattice,
/// a ground cell a texel, so the tile follows the grid as the ground does.
pub const EARTH_ZOOM: Zoom = TOP_ZOOM;

/// How far from the eye the tiles are drawn, before the global picture: the
/// ground's demand at that zoom.
pub const EARTH_DIST: f64 = demand_dist(GROUND, EARTH_ZOOM);

pub const EARTH_BYTES: usize = (TILE_CELLS * TILE_CELLS * 4) as usize;

/// The global picture's texels, across and down, what is drawn past the
/// tiles: the whole Earth equirectangular, as pxl-geo-bake bake_earth.rs
/// writes it.
pub const PICTURE: (u32, u32) = (2048, 1024);

/// Every packet's length: its texels, as `encode` lays them.
pub(crate) const PKT_BYTES: usize =
    super::pkt::HEADER + super::pkt::branch(1) + super::pkt::seq(EARTH_BYTES);

type Packet = (Vec<u8>,);

pub fn encode(rgba: &[u8]) -> Result<Vec<u8>, MarshalError> {
    if rgba.len() != EARTH_BYTES {
        return Err(malformed());
    }
    shape::encode::<Packet>((rgba,), PKT_BYTES as u32)
}

/// The tile's texels, read in place.
pub fn decode(bytes: &[u8]) -> Result<&[u8], MarshalError> {
    let (rgba,) = shape::get_deep::<Packet>(bytes)?;
    if rgba.len() != EARTH_BYTES {
        return Err(malformed());
    }
    Ok(rgba)
}
