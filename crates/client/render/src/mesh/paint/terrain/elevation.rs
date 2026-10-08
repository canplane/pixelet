// pxl-render/mesh/paint/terrain/elevation.rs

//! Tile elevation in cells (1 cell = 1 texel). Prefixes: c chunk index,
//! d loop delta, t texel, s padded source sample.

use pxl_spec::bake::surface::bicubic;

/// Signed because the padded raster reaches ELE_PAD samples past the tile, so a
/// chunk at the edge can read its neighbor's ground.
pub fn sample_ele(tile_ele: &[i16], tx: i32, ty: i32) -> i16 {
    bicubic(tile_ele, tx as f64 + 0.5, ty as f64 + 0.5)
}

/// The ground's top face, in cells, at a point in texels: the stored height h
/// means cells up to h − 1 are ground and the surface is at z = h. Every look
/// stands there; what is painted never moves it.
pub fn ground_ele_at(tile_ele: &[i16], tx: f64, ty: f64) -> i16 {
    bicubic(tile_ele, tx, ty)
}

#[cfg(test)]
#[path = "elevation.test.rs"]
mod test;
