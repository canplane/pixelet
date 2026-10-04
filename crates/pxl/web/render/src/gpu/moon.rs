// pxl-render/gpu/moon.rs

//! The Moon's face (from NASA SVS's CGI Moon Kit) in a texture with its mip
//! chain: without it a 512-texel face read at thirty pixels sparkles as the
//! camera moves. Until it arrives the sky draws the face at its mean.

use crate::gpu::api::{object, release, texture_usage, Gpu};
use crate::gpu::api_spec::{address, compare, dimension, filter, format};
use crate::gpu::layers::{mip_levels, Mips};

const MOON_TEX_SZ: u32 = 512;

pub struct MoonMap {
    texture: u32,
    pub view: u32,
    pub sampler: u32,
    levels: u32,
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

    pub fn step(&mut self, g: &Gpu, mips: &Mips, image: fn() -> Option<u32>) {
        if self.loaded {
            return;
        }
        let Some(image) = image() else { return };
        let face = object();
        g.image_texture(face, image);
        let encoder = object();
        g.encoder(encoder);
        g.copy_texture(encoder, face, 0, 0, self.texture, 0, 0, 0, MOON_TEX_SZ, MOON_TEX_SZ);
        g.submit(encoder);
        release(encoder);
        release(face);
        mips.build(g, self.texture, self.levels, 0, 1);
        self.loaded = true;
    }
}
