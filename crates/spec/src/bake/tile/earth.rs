// pxl-spec/bake/tile/earth.rs

//! The Earth's picture as a layer of the pyramid, at its top zoom alone, a
//! ground cell a texel: per cell, Blue Marble's reflectance coded as
//! sRGB, as a texture reads it, and in alpha the square root of the Black
//! Marble's lights, so a village keeps its few steps of brightness. Row by row from the
//! tile's south edge, in the ground lattice's cell order. One XTP tree, as every
//! other baked tile is; a tile with no object is answered by the global picture.

use crate::bake::malformed;
use crate::space::coord::zoom::Zoom;
use crate::space::layer::lattice::demand_reach;
use crate::space::layer::lattice::{GROUND, TILE_CELLS};
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

/// The Earth's one zoom: the ground's top. Its tiles are the ground's lattice,
/// a ground cell a texel, so the tile follows the grid as the ground does.
pub const EARTH_ZOOM: Zoom = GROUND.top();

/// How far from the eye the tiles are drawn, before the global picture: the
/// ground's demand at that zoom.
pub const EARTH_REACH: f64 = demand_reach(GROUND, EARTH_ZOOM);

pub const EARTH_BYTES: usize = (TILE_CELLS * TILE_CELLS * 4) as usize;

pub fn encode(rgba: &[u8]) -> Result<Vec<u8>, MarshalError> {
    if rgba.len() != EARTH_BYTES {
        return Err(malformed());
    }
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.u8_array(Some(rgba));
    }));
    encoder().encode(&t, TreeEncoderOptions::default())
}

/// The tile's texels, read in place.
pub fn decode(bytes: &[u8]) -> Result<&[u8], MarshalError> {
    let rgba = TreeReader::new(bytes)?.read_branch()?.at(0)?.get_array::<u8>()?;
    if rgba.len() != EARTH_BYTES {
        return Err(malformed());
    }
    Ok(rgba)
}
