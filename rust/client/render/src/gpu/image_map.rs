// pxl-render/gpu/image_map.rs

//! An image the sky draws from, the moon's face or the imagery's global
//! image, in a texture with its mip chain. Until it arrives the shaders draw
//! its mean.

use pxl_gpu_link::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::gpu::api::{Format, Gpu, Image, place, release, texture_usage};

use crate::gpu::mip::{MipPass, mip_cnt};

pub struct ImageMap {
    texture: u32,
    pub view: u32,
    pub sampler: u32,
    size: (u32, u32),
    mip_cnt: u32,
    pub loaded: bool,
}

impl ImageMap {
    pub fn new(g: &Gpu, width: u32, height: u32) -> ImageMap {
        let mip_cnt = mip_cnt(width, height);
        let usage = texture_usage::TEXTURE_BINDING
            | texture_usage::COPY_DST
            | texture_usage::RENDER_ATTACHMENT;
        let texture = g.texture(width, height, 1, Format::Rgba8unormSrgb, mip_cnt, usage);
        let view = g.view(texture, true, 0, mip_cnt, 0, 1);
        let sampler = g.sampler(
            FilterMode::Linear,
            FilterMode::Linear,
            MipmapFilterMode::Linear,
            AddressMode::ClampToEdge,
            None,
            1,
        );
        ImageMap {
            texture,
            view,
            sampler,
            size: (width, height),
            mip_cnt,
            loaded: false,
        }
    }

    /// Copies the image in and builds its mips the first time it arrives. Returns whether it
    /// arrived this frame.
    pub fn step(&mut self, g: &Gpu, mip: &MipPass, image: Option<Image>) -> bool {
        if self.loaded {
            return false;
        }
        let Some(face) = image.and_then(|(image, file)| g.image(image, file)) else {
            return false;
        };
        let encoder = g.encoder();
        g.copy_texture(
            encoder,
            &place(face, [0, 0, 0]),
            &place(self.texture, [0, 0, 0]),
            [self.size.0, self.size.1, 1],
        );
        g.submit(encoder);
        release(face);
        mip.build(g, self.texture, self.mip_cnt, 0, 1);
        self.loaded = true;
        true
    }
}
