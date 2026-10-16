// pxl-render/gpu/image_map.rs

//! A picture the sky draws from, the Moon's face or the Earth's, in a texture
//! with its mip chain: without it a 512-texel face read at thirty pixels
//! sparkles as the camera moves. Until it arrives the shaders draw its mean.

use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::gpu::api::{release, texture_usage, Format, Gpu};

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
        let usage = texture_usage::TEXTURE_BINDING | texture_usage::COPY_DST | texture_usage::RENDER_ATTACHMENT;
        let texture = g.texture(width, height, 1, Format::rgba8unorm_srgb, levels, usage);
        let view = g.view(texture, true, 0, levels, 0, 1);
        let sampler = g.sampler(FilterMode::linear, FilterMode::linear, MipmapFilterMode::linear, AddressMode::clamp_to_edge, None, 1);
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
        let face = g.image_texture(image);
        let encoder = g.encoder();
        g.copy_texture(encoder, face, 0, 0, self.texture, 0, 0, 0, self.size.0, self.size.1);
        g.submit(encoder);
        release(face);
        mips.build(g, self.texture, self.levels, 0, 1);
        self.loaded = true;
    }
}
