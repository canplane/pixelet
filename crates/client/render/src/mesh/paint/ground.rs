// pxl-render/mesh/paint/ground.rs

//! What a block that fills is drawn as, by its value alone: its swatch and
//! texture layer as the catalog's painter teaches them (object.rs), a ground
//! look's and a placed cube of that look's one entry.

use pxl_spec::block::color;
use pxl_spec::block::cube::sentinel;

use crate::mesh::paint::object::PAINTER;
use crate::mesh::paint::palette::{layer_mean, rgb255};
use crate::mesh::word::NO_LAYER;

/// The ground's bytes, which every value under them is one of.
const BYTES: usize = u8::MAX as usize + 1;

/// Each value's look as a ray's pixel paints it (frame/shader/screen_rays.wgsl
/// `drawn`), by value alone, whichever source holds it
/// (doc/minutes/20261005-the-ground-is-a-carto-palette.md, "One value
/// space"), two words: its swatch with its layer in the top byte, then what
/// it reads as from afar (`layer_mean`). A cube the catalog teaches is its
/// painter's (object.rs `look_of`), a ground look's the cube of that look; a
/// value none is, the bedrock's `unknown` and the bytes no look names among
/// them, is drawn as `unknown`.
pub fn look_words() -> Vec<[u32; 2]> {
    let unknown = (color::sentinel(sentinel::unknown).map_or([0; 3], |s| rgb255(s.rgb)), NO_LAYER as u8);
    let words = |(rgb, layer): ([u8; 3], u8)| {
        let far = layer_mean(u32::from(layer), rgb);
        [u32::from_le_bytes([rgb[0], rgb[1], rgb[2], layer]), u32::from_le_bytes([far[0], far[1], far[2], 0])]
    };
    (0..PAINTER.swatches().len().max(BYTES)).map(|v| PAINTER.look_of(v as u32).unwrap_or(unknown)).map(words).collect()
}
