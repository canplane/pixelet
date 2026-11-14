// pxl-render/paint/ground.rs

//! How an asset that fills a cell is drawn, by its value alone: its swatch and texture layer
//! as the catalog's painter gives them (object.rs). A ground look and a placed cube of that
//! look share one entry.

use pxl_spec::asset::color;
use pxl_spec::asset::cube::Sentinel;
use pxl_spec::color::linear;

use crate::frame::shader::word::NO_LAYER;
use crate::paint::object::PAINTER;
use crate::paint::palette::{layer_mean, rgb255, sentinel_layer};

/// The number of values a ground's byte can take. Every value below it is one of them.
const BYTES: usize = u8::MAX as usize + 1;

/// Each value's look as a ray's pixel paints it (frame/shader/screen_ray.wgsl `look`). The look
/// depends on the value alone, whichever source holds it. It is four words, its colors in
/// linear light, which a pixel is lit in: the swatch's red and green; its blue, with the
/// texture layer above it; the color it reads as from afar (`layer_mean`), red and green; then
/// that color's blue. Each channel is 16 bits of the unit (WGSL `unpack2x16unorm`), which keeps
/// dark colors free of bands. A cube the catalog names takes its painter's look (object.rs
/// `look_of`), and a ground look takes the look of its cube. A value with no look, such as the
/// bedrock's `unknown` or a byte no look names, is drawn as `unknown`.
pub fn look_words() -> Vec<[u32; 4]> {
    let unknown = (
        color::sentinel(Sentinel::Unknown as u16).map_or([0; 3], |s| rgb255(s.rgb)),
        sentinel_layer(Sentinel::Unknown as u8).unwrap_or(NO_LAYER) as u8,
    );
    let unit = |v: u8| (linear(v) * f64::from(u16::MAX)).round_ties_even() as u32;
    let pair = |a: u8, b: u8| unit(a) | unit(b) << 16;
    let words = |(rgb, layer): ([u8; 3], u8)| {
        let far = layer_mean(u32::from(layer), rgb);
        [
            pair(rgb[0], rgb[1]),
            unit(rgb[2]) | u32::from(layer) << 16,
            pair(far[0], far[1]),
            unit(far[2]),
        ]
    };
    (0..PAINTER.state_cnt().max(BYTES))
        .map(|v| PAINTER.look_of(v as u32).unwrap_or(unknown))
        .map(words)
        .collect()
}
