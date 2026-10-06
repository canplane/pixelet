// pxl-spec/bake/earth_tile.rs

//! The Earth's picture as a layer of the pyramid, at its top zoom alone, a
//! ground cell a texel: per cell, Blue Marble's reflectance coded as
//! sRGB, as a texture reads it, and in alpha the square root of the Black
//! Marble's lights, so a village keeps its few steps of brightness. Row by row from the
//! tile's south edge, in the ground lattice's cell order. One XTP tree, as every
//! other baked tile is; a tile with no object is answered by the global picture.

use crate::bake::malformed;
use crate::space::camera::grid::split_distance;
use crate::space::coord::zoom::{units, Zoom};
use crate::space::layer::lattice::{Lattice, GROUND};
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

/// The Earth's one zoom: its lattice's top.
pub const EARTH_ZOOM: Zoom = EARTH.top();

/// The picture's texels, a ground cell each; the camera grid's cell holds two
/// of them a side where its tile is drawn, so the tile follows the grid in proportion.
pub const EARTH: Lattice = Lattice::new(GROUND.cell()).folding(1);

/// How far from the eye the tiles are drawn, before the global picture.
pub const EARTH_REACH: f64 = split_distance(EARTH.grid_cell(EARTH_ZOOM - 1));

/// The tiles about the eye within the reach, a side and in all.
pub const EARTH_ACROSS: u32 = (2.0 * EARTH_REACH / units(EARTH_ZOOM)) as u32 + 2;
pub const EARTH_HELD: u32 = EARTH_ACROSS * EARTH_ACROSS;

/// A tile's cells on a side, each a texel.
pub const EARTH_TEXELS: u32 = GROUND.tile_cells();

pub const EARTH_BYTES: usize = (EARTH_TEXELS * EARTH_TEXELS * 4) as usize;

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
