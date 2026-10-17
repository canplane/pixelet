// pxl-render/frame/shader/words.rs

//! What the host lays out and the shaders read, stated once: compiled into
//! the crate, and read by build.rs, which hands each to every shader
//! (constant.wgsl's tail), a uniform's floats as its vec4s.

/// A draw's uniform (frame/draw.rs): its translation and what its pipeline
/// reads past it.
pub const DRAW_FLOATS: usize = 8;
/// The frame's (frame.wgsl, record.rs `FRAME_AT`).
pub const FRAME_FLOATS: usize = 48;
/// The sky's: the ray, the celestial pole, and whether the Moon's face has
/// arrived (sky_words.wgsl).
pub const SKY_FLOATS: usize = 16;
/// The air's (atmos_medium.wgsl, gpu/atmosphere.rs).
pub const ATMOS_FLOATS: usize = 52;
/// The cloud sheets' (clouds.wgsl, gpu/clouds.rs).
pub const CLOUDS_FLOATS: usize = 20;
/// The stars' (stars.wgsl, gpu/stars.rs).
pub const STARS_FLOATS: usize = 44;
/// The screen's occlusion's (ao_view.wgsl, gpu/ao.rs).
pub const AO_FLOATS: usize = 36;

/// The bits a held earth tile's layer takes in the air's window words
/// (gpu/atmosphere.rs `EARTH_WINDOW`).
pub const EARTH_TILE_BITS: u32 = 4;

/// The Moon's near side as download_moon cuts it, a side in texels; the
/// disc's radius is half of it.
pub const MOON_TEXELS: u32 = 512;

/// A tap past the last layer: the face's swatch alone.
pub const NO_LAYER: u32 = 255;

/// A face's top, side and bottom images, a slot of the pack each, in
/// `SLOT_BITS` each, in that order; `NO_SLOT` until an image arrives, past
/// any array WebGPU gives (maxTextureArrayLayers is 256).
pub const SLOT_BITS: u32 = 10;
pub const NO_SLOT: u32 = (1 << SLOT_BITS) - 1;
pub const IMAGE_TOP: u32 = 0;
pub const IMAGE_SIDE: u32 = 1;
pub const IMAGE_BOTTOM: u32 = 2;
