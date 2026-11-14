// pxl-render/gpu/mip.rs

//! Builds a texture's mip chain, drawing each level from the one above it (mip.wgsl). The
//! texture pack, the cloud sheets' tile, an image and the marker atlas all use it.

use crate::frame::shader::record::PIPELINE_MIP;
use xpute_gpu_link::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::gpu::api::{pass_desc, release, sampler_entry, view_entry, Format, Gpu, Layout, Load};
use crate::gpu::pipeline::PipelineCache;

pub fn mip_cnt(width: u32, height: u32) -> u32 {
    32 - width.max(height).max(1).leading_zeros()
}

pub struct MipPass {
    pipeline: u32,
    layout: Layout,
    sampler: u32,
}

impl MipPass {
    /// A pipeline draws into the format it was built for, so each format needs its own.
    pub fn new(g: &Gpu, pipelines: &mut PipelineCache, format: Format) -> MipPass {
        let pipeline = pipelines.build(g, PIPELINE_MIP, format);
        let sampler = g.sampler(
            FilterMode::Linear,
            FilterMode::Linear,
            MipmapFilterMode::Nearest,
            AddressMode::ClampToEdge,
            None,
            1,
        );
        MipPass {
            pipeline: pipeline.id,
            layout: pipeline.layouts[0],
            sampler,
        }
    }

    pub fn build(&self, g: &Gpu, texture: u32, mip_cnt: u32, first_layer: u32, layer_cnt: u32) {
        let encoder = g.encoder();
        for layer in first_layer..first_layer + layer_cnt {
            for level in 1..mip_cnt {
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
                // The encoder keeps what it recorded, so letting these go now frees only
                // their handles.
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
