// pxl-render/gpu/moon.rs

//! The Moon's face on the device: the near side's reflectance
//! (asset/sky/moon.png, cut from the CGI Moon Kit of NASA's Scientific
//! Visualization Studio by pxl-tool/bin/download_moon.rs), painted once into
//! a texture with its mip chain for the sky pass to read
//! (frame/shader/sky.wgsl's moon_disc).
//!
//! The chain is what the texture is for: the disc is some thirty pixels
//! across on a phone, and a 512-texel face read at that without it would
//! sparkle as the camera moves.
//! Until the image has arrived, and if it never does, the sky draws the
//! face at its mean.
//!
//! What it costs: 1.4 MB on the device, a 512 × 512 level and its chain.

use crate::gpu::api::{object, texture_usage, Gpu};
use crate::gpu::api_spec::{address, compare, dimension, filter, format};
use crate::gpu::layers::{mip_levels, Mips};
use crate::gpu::paint::Paint;

/// The face's side in texels, the map's.
const MOON_TEX_SZ: u32 = 512;

pub struct MoonMap {
    texture: u32,
    pub view: u32,
    pub sampler: u32,
    levels: u32,
    /// The map painted in.
    pub loaded: bool,
}

impl MoonMap {
    pub fn new(g: &Gpu) -> MoonMap {
        let levels = mip_levels(MOON_TEX_SZ, MOON_TEX_SZ);
        let texture = object();
        let usage = texture_usage::TEXTURE_BINDING | texture_usage::COPY_DST | texture_usage::RENDER_ATTACHMENT;
        g.texture(texture, MOON_TEX_SZ, MOON_TEX_SZ, 1, format::RGBA8_SRGB, levels, usage);
        let view = object();
        g.view(view, texture, dimension::D2_ARRAY, 0, levels, 0, 1);
        let sampler = object();
        g.sampler(sampler, filter::LINEAR, filter::LINEAR, filter::LINEAR, address::CLAMP, compare::NONE, 1);
        MoonMap {
            texture,
            view,
            sampler,
            levels,
            loaded: false,
        }
    }

    /// Paints the face in once it has arrived: `image` asks for it, until it
    /// has been painted.
    pub fn step(&mut self, g: &Gpu, mips: &Mips, image: fn() -> Option<u32>) {
        if self.loaded {
            return;
        }
        let Some(image) = image() else { return };
        let size = MOON_TEX_SZ as f32;
        let mut p = Paint::new();
        p.size(MOON_TEX_SZ, MOON_TEX_SZ);
        p.image(image, 0.0, 0.0, size, size, 0.0, 0.0, size, size);
        p.run(g);
        g.paint_copy(self.texture, 0, 0, 0, MOON_TEX_SZ, MOON_TEX_SZ);
        mips.build(g, self.texture, self.levels, 0, 1);
        self.loaded = true;
    }
}
