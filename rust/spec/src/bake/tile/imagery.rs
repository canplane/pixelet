// pxl-spec/bake/tile/imagery.rs

//! The imagery as a layer of the pyramid, at its top zoom alone, a
//! ground cell a texel: per cell, Blue Marble's reflectance coded as
//! sRGB, as a texture reads it, and in alpha the square root of the Black
//! Marble's lights, so a village keeps its few steps of brightness. Row by row from the
//! tile's south edge, in the ground lattice's cell order. One XTP tree, as every
//! other baked tile is; a tile with no object is answered by the global image.

use crate::bake::malformed;
use crate::space::coord::zoom::{MIN_ZOOM, Zoom};
use crate::space::layer::lattice::{GROUND, TILE_CELLS, demand_dist};
use xpute_kit::status::error::XputeError;
use xpute_kit::wire::xtp::shape;

/// The imagery's one zoom: the ground's top. Its tiles are the ground's lattice,
/// a ground cell a texel, so the tile follows the grid as the ground does.
pub const IMAGERY_ZOOM: Zoom = MIN_ZOOM;

/// How far from the eye the tiles are drawn, before the global image: the
/// ground's demand at that zoom.
pub const IMAGERY_DIST: f64 = demand_dist(GROUND, IMAGERY_ZOOM);

pub const TILE_SZ: usize = (TILE_CELLS * TILE_CELLS * 4) as usize;

/// The global image's texels, across and down, what is drawn past the
/// tiles: the whole Earth equirectangular, as pxl-geo-bake bin/imagery.rs
/// writes it.
pub const GLOBAL_SIZE: (u32, u32) = (2048, 1024);

/// Every packet's length: its texels, as `encode` lays them.
pub(crate) const PKT_SZ: u32 =
    (super::pkt::HDR + super::pkt::branch(1) + super::pkt::seq(TILE_SZ)) as u32;

type Packet = (Vec<u8>,);

pub fn encode(rgba: &[u8]) -> Result<Vec<u8>, XputeError> {
    if rgba.len() != TILE_SZ {
        return Err(malformed());
    }
    shape::encode::<Packet>((rgba,), PKT_SZ)
}

/// The tile's texels, read in place.
pub fn decode(pkt: &[u8]) -> Result<&[u8], XputeError> {
    let (rgba,) = shape::get_deep::<Packet>(pkt)?;
    if rgba.len() != TILE_SZ {
        return Err(malformed());
    }
    Ok(rgba)
}
