// pxl-spec/bake/earth_page.rs

//! The Earth's picture as a layer of the pyramid, at its top zoom alone, a
//! ground cell a texel: per cell, Blue Marble's reflectance coded as
//! sRGB, as a texture reads it, and in alpha the square root of the Black
//! Marble's lights, so a village keeps its few steps of brightness. Row by row from the
//! page's south edge, in the ground lattice's cell order. One XTP tree, as every
//! other baked page is; a page with no object is answered by the global picture.

use crate::bake::malformed;
use crate::space::pyramid::{GROUND, TOP_Z};
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

pub const EARTH_Z: u32 = TOP_Z;

/// A page's cells on a side, each a texel.
pub const EARTH_TEXELS: u32 = GROUND.page_cells();

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

/// The page's texels, read in place.
pub fn decode(bytes: &[u8]) -> Result<&[u8], MarshalError> {
    let rgba = TreeReader::new(bytes)?.read_branch()?.at(0)?.get_array::<u8>()?;
    if rgba.len() != EARTH_BYTES {
        return Err(malformed());
    }
    Ok(rgba)
}
