// pxl-render/gpu/text.rs

//! The platform lays out and draws text: it owns the fonts, their fallback and color glyphs,
//! and shapes a line whole. The renderer receives pixels.

use crate::look::marker::{Font, ATLAS_SCALE};

/// A line of text as the platform drew it: RGBA, top row first, not premultiplied.
/// `origin` is the line's start at its em box's middle, in the raster's pixels.
#[derive(Clone, Copy, Debug)]
pub struct Raster<'a> {
    pub px: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub origin: [f32; 2],
    pub advance: f32,
}

/// The raster of a line in a font and a color, none until the platform has drawn it.
pub type TextOf = fn(Font, u32, &str) -> Option<Raster<'static>>;

/// The raster's size and its origin for a line in `font` whose advance is `advance` logical
/// pixels. The raster reaches half an em past each end of the line and is two ems tall, so
/// glyphs that reach past their advance and em box stay whole.
pub fn raster_box(font: Font, advance: f32) -> ([u32; 2], [f32; 2]) {
    let em = font.px * ATLAS_SCALE as f32;
    let margin = (em / 2.0).ceil();
    let width = (advance * ATLAS_SCALE as f32 + 2.0 * margin).ceil() as u32;
    let height = (2.0 * em).ceil() as u32;
    ([width, height], [margin, height as f32 / 2.0])
}
