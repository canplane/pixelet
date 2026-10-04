// pxl-render/gpu/text.rs

//! Text is laid out and drawn by the platform, which owns the fonts, fallback
//! and color glyphs and shapes a line whole; the renderer gets pixels.

use crate::gpu::surface::DPR_MAX;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Font {
    pub weight: u32,
    pub px: f32,
}

/// RGBA, top row first, not premultiplied; `origin` is the line's start at its
/// em box's middle.
#[derive(Clone, Copy, Debug)]
pub struct Raster<'a> {
    pub pixels: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub origin: [f32; 2],
    pub advance: f32,
}

/// None until the platform has drawn it.
pub type TextOf = fn(Font, u32, &str) -> Option<Raster<'static>>;

/// Half an em either side and two ems tall, for glyphs that reach past their
/// advance and em box.
pub fn raster_box(font: Font, advance: f32) -> ([u32; 2], [f32; 2]) {
    let em = font.px * DPR_MAX as f32;
    let margin = (em / 2.0).ceil();
    let width = (advance * DPR_MAX as f32 + 2.0 * margin).ceil() as u32;
    let height = (2.0 * em).ceil() as u32;
    ([width, height], [margin, height as f32 / 2.0])
}
