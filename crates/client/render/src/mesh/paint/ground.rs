// pxl-render/mesh/paint/ground.rs

//! What a block that fills is drawn as, by its value alone: its swatch and
//! texture layer as the catalog's painter teaches them (object.rs), a ground
//! look's and a placed cube of that look's one entry.

use pxl_spec::block::color;
use pxl_spec::block::cube::sentinel;

use crate::frame::shader::words::NO_LAYER;
use crate::look::color::linear;
use crate::mesh::paint::object::PAINTER;
use crate::mesh::paint::palette::{layer_mean, rgb255};

/// The ground's bytes, which every value under them is one of.
const BYTES: usize = u8::MAX as usize + 1;

/// Each value's look as a ray's pixel paints it (frame/shader/screen_rays.wgsl
/// `drawn`), by value alone, whichever source holds it
/// (doc/minutes/20261005-the-ground-is-a-carto-palette.md, "One value
/// space"), four words, its colours in linear light, which a pixel lights
/// in, worked out here once a look rather than at every pixel: its swatch's
/// red and green, its blue with its layer over it, then what it reads as
/// from afar (`layer_mean`), red and green, then blue; each channel 16 bits
/// of the unit (WGSL `unpack2x16unorm`), which darks keep without bands. A
/// cube the catalog teaches is its
/// painter's (object.rs `look_of`), a ground look's the cube of that look; a
/// value none is, the bedrock's `unknown` and the bytes no look names among
/// them, is drawn as `unknown`.
pub fn look_words() -> Vec<[u32; 4]> {
    let unknown = (color::sentinel(sentinel::unknown).map_or([0; 3], |s| rgb255(s.rgb)), NO_LAYER as u8);
    let unit = |v: u8| (linear(v) * f64::from(u16::MAX)).round_ties_even() as u32;
    let pair = |a: u8, b: u8| unit(a) | unit(b) << 16;
    let words = |(rgb, layer): ([u8; 3], u8)| {
        let far = layer_mean(u32::from(layer), rgb);
        [pair(rgb[0], rgb[1]), unit(rgb[2]) | u32::from(layer) << 16, pair(far[0], far[1]), unit(far[2])]
    };
    (0..PAINTER.states().max(BYTES)).map(|v| PAINTER.look_of(v as u32).unwrap_or(unknown)).map(words).collect()
}
