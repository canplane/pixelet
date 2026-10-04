// pxl-render/gpu/image_map.rs

//! A picture the sky draws from, the Moon's face or the Earth's, in a texture
//! with its mip chain: without it a 512-texel face read at thirty pixels
//! sparkles as the camera moves. Until it arrives the shaders draw its mean.

use crate::gpu::api::{object, release, texture_usage, Gpu};
use crate::gpu::api_spec::{address, compare, dimension, filter, format};
use crate::gpu::layers::{mip_levels, Mips};

pub struct ImageMap {
    texture: u32,
    pub view: u32,
    pub sampler: u32,
    size: (u32, u32),
    levels: u32,
    pub loaded: bool,
}

impl ImageMap {
    pub fn new(g: &Gpu, width: u32, height: u32) -> ImageMap {
        let levels = mip_levels(width, height);
        let texture = object();
        let usage = texture_usage::TEXTURE_BINDING | texture_usage::COPY_DST | texture_usage::RENDER_ATTACHMENT;
        g.texture(texture, width, height, 1, format::RGBA8_SRGB, levels, usage);
        let view = object();
        g.view(view, texture, dimension::D2_ARRAY, 0, levels, 0, 1);
        let sampler = object();
        g.sampler(sampler, filter::LINEAR, filter::LINEAR, filter::LINEAR, address::CLAMP, compare::NONE, 1);
        ImageMap {
            texture,
            view,
            sampler,
            size: (width, height),
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
        g.copy_texture(encoder, face, 0, 0, self.texture, 0, 0, 0, self.size.0, self.size.1);
        g.submit(encoder);
        release(encoder);
        release(face);
        mips.build(g, self.texture, self.levels, 0, 1);
        self.loaded = true;
    }
}
