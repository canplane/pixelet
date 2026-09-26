// pxl-render/gpu/text.rs

//! Text as the renderer draws it: a line in a font and a color, laid out and
//! drawn by the platform — the kernel asks for it (pxl-kernel text.rs) — and
//! handed to the sheet as pixels. The platform's text engine is the one thing
//! here the renderer does not own: it has the fonts, the fallback for every
//! script and the color glyphs, and it shapes a line whole, so a line is
//! drawn as a line and never assembled from glyphs.

use crate::gpu::surface::DPR_MAX;

/// A font as the platform's own family at a weight and a size: the family is
/// the platform's, the one the app's own text is set in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Font {
    pub weight: u32,
    pub px: f32,
}

/// A line drawn at the sheet's scale: its pixels, RGBA a byte a channel, top
/// row first, alpha not premultiplied; `origin`, where the line starts and
/// its em box's middle lies, in its pixels; `advance`, how far the line runs
/// from there.
#[derive(Clone, Copy, Debug)]
pub struct Raster<'a> {
    pub pixels: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub origin: [f32; 2],
    pub advance: f32,
}

/// What the kernel gives the renderer for a line of `text` in a font and a
/// color (0xRRGGBBAA): its raster, once the platform has drawn it.
pub type TextOf = fn(Font, u32, &str) -> Option<Raster<'static>>;

/// The box a line of `advance` logical pixels is drawn in, at the sheet's
/// scale: its size and its origin. Half an em either side of the line, for
/// what a glyph reaches past its advance; two ems tall about the em box's
/// middle, for what reaches past the em box above and below it.
pub fn raster_box(font: Font, advance: f32) -> ([u32; 2], [f32; 2]) {
    let em = font.px * DPR_MAX as f32;
    let margin = (em / 2.0).ceil();
    let width = (advance * DPR_MAX as f32 + 2.0 * margin).ceil() as u32;
    let height = (2.0 * em).ceil() as u32;
    ([width, height], [margin, height as f32 / 2.0])
}
