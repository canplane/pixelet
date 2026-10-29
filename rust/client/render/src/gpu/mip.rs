// pxl-render/gpu/mip.rs

//! A texture's mip chain, each level drawn from the one above it
//! (mip.wgsl): for the texture pack, the cloud sheets' tile, an image and
//! the marker sheet alike.

use crate::frame::shader::record::PIPELINE_MIP;
use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::gpu::api::{pass_desc, release, sampler_entry, view_entry, Format, Gpu, Load};
use crate::gpu::pipeline::Pipelines;

pub fn mip_levels(width: u32, height: u32) -> u32 {
    32 - width.max(height).max(1).leading_zeros()
}

pub struct MipPass {
    pipeline: u32,
    layout: u32,
    sampler: u32,
}

impl MipPass {
    /// A pipeline draws into the format it was built for, so each format needs its own.
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, format: Format) -> MipPass {
        let built = pipelines.build(g, PIPELINE_MIP, format);
        let sampler = g.sampler(
            FilterMode::linear,
            FilterMode::linear,
            MipmapFilterMode::nearest,
            AddressMode::clamp_to_edge,
            None,
            1,
        );
        MipPass {
            pipeline: built.pipeline,
            layout: built.layouts[0],
            sampler,
        }
    }

    pub fn build(&self, g: &Gpu, texture: u32, levels: u32, first_layer: u32, layers: u32) {
        let encoder = g.encoder();
        for layer in first_layer..first_layer + layers {
            for level in 1..levels {
                let above = g.view(texture, false, level - 1, 1, layer, 1);
                let target = g.view(texture, false, level, 1, layer, 1);
                let group = g.bind_group(
                    self.layout,
                    &[view_entry(0, above), sampler_entry(1, self.sampler)],
                );
                let pass = g.pass(
                    encoder,
                    &pass_desc(target, Load::Clear, 0, Load::Clear, 0.0, None),
                );
                g.set_pipeline(pass, self.pipeline);
                g.set_bind_group(pass, 0, group, &[]);
                g.draw(pass, 3, 1, 0, 0);
                g.end(pass);
                // The encoder keeps what it recorded; letting these go now only frees the handles.
                for id in [pass, group, target, above] {
                    release(id);
                }
            }
        }
        g.submit(encoder);
    }

    pub fn release(self) {
        release(self.sampler);
    }
}
