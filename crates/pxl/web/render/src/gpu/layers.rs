// pxl-render/gpu/layers.rs

//! The texture pack as two 2D array textures on the device — a top and a side
//! face per layer, one layer per ground type and cube colour, indexed by the
//! layer indices the mesher bakes into a vertex's taps (mesh/paint/palette/shader_meta.rs). Every
//! layer starts as its flat colour, and the pack's sheet replaces them tile by
//! tile once it arrives; a side with no tile of its own wears the top's, the
//! rule the shader assumes.
//!
//! Each array keeps its mip chain: a tile a cell minifies to a few texels at
//! the far tiers, and without the chain every camera movement resamples a
//! different texel and the ground crawls. WebGPU has no generateMipmap, so the
//! chain is drawn on the device, a level from the one above (Mips).

use pxl_spec::asset::texture::{shipped_texture_tile, CubeFace, SHIPPED_SHEET};
use pxl_spec::cell::object::{block_cubes, placeable_cube_rgb, terrain_style_cube_for, PlaceableCube};

use crate::frame::shader::record::PIPELINE_MIPS;
use crate::gpu::api::{object, texture_usage, GpuApi};
use crate::gpu::api_spec::{address, compare, dimension, filter, format, load, resource};
use crate::gpu::paint::Paint;
use crate::gpu::pipeline::Pipelines;
use crate::mesh::paint::palette::shader_meta::{rgb255, GROUND_LAYER_META};
use crate::mesh::paint::palette::tex_array::OBJECT_LAYER_IDX;

/// The tile edge in texels (pxl-asset-bake/pack.rs's TILE_SIZE).
pub const LAYER_TEX_SZ: u32 = 256;

/// Levels a texture of `width` by `height` has, down to one texel.
pub fn mip_levels(width: u32, height: u32) -> u32 {
    32 - width.max(height).max(1).leading_zeros()
}

pub struct Mips {
    pipeline: u32,
    layout: u32,
    sampler: u32,
}

impl Mips {
    pub fn new(g: &GpuApi, pipelines: &mut Pipelines) -> Mips {
        let built = pipelines.build(g, PIPELINE_MIPS, format::RGBA8_SRGB, 1);
        let sampler = object();
        (g.sampler)(sampler, filter::LINEAR, filter::LINEAR, filter::NEAREST, address::CLAMP, compare::NONE, 1);
        Mips {
            pipeline: built.pipeline,
            layout: built.layouts[0],
            sampler,
        }
    }

    /// Draws every level of `texture` below the first, for `layers` array
    /// layers from `first_layer`, each box-filtered from the one above.
    pub fn build(&self, g: &GpuApi, texture: u32, levels: u32, first_layer: u32, layers: u32) {
        let encoder = object();
        (g.encoder)(encoder);
        for layer in first_layer..first_layer + layers {
            for level in 1..levels {
                let above = object();
                (g.view)(above, texture, dimension::D2, level - 1, 1, layer, 1);
                let target = object();
                (g.view)(target, texture, dimension::D2, level, 1, layer, 1);
                let group = object();
                (g.bind_group)(group, self.layout, &[0, resource::VIEW, above, 0, 0, 1, resource::SAMPLER, self.sampler, 0, 0]);
                let pass = object();
                (g.pass)(
                    pass,
                    encoder,
                    &crate::gpu::api::pass_desc(target, 0, load::CLEAR, 0, load::CLEAR, 0.0, 0, crate::gpu::api::NONE, crate::gpu::api::NONE),
                );
                (g.set_pipeline)(pass, self.pipeline);
                (g.set_bind_group)(pass, 0, group, &[]);
                (g.draw)(pass, 3, 1, 0, 0);
                (g.end)(pass);
                // The encoder keeps what it recorded: a view, a group or a pass
                // let go now is gone from the table, not from the commands.
                for id in [pass, group, target, above] {
                    crate::gpu::api::release(id);
                }
            }
        }
        (g.submit)(encoder);
        crate::gpu::api::release(encoder);
    }
}

/// Every layer: the cube it is, where one stands for it, its index, and its
/// colour. A ground type no cube stands in for (common's) has no tile.
fn each_layer(mut f: impl FnMut(Option<PlaceableCube>, u32, u32)) {
    for m in GROUND_LAYER_META.iter() {
        f(terrain_style_cube_for(m.ch_key, m.type_id as u32), m.layer_index, m.rgb);
    }
    for block in block_cubes() {
        let cube = PlaceableCube::Block(block);
        if let (Some(layer), Some(rgb)) = (OBJECT_LAYER_IDX(block), placeable_cube_rgb(cube)) {
            f(Some(cube), layer, rgb);
        }
    }
}

pub struct Layers {
    top: u32,
    side: u32,
    pub top_view: u32,
    pub side_view: u32,
    pub sampler: u32,
    pub bytes: u32,
    count: u32,
    levels: u32,
    /// The pack's sheet drawn into both arrays.
    loaded: bool,
}

impl Layers {
    /// Both arrays, each layer its flat colour — laid out in `fill`, a tile's
    /// texels — with their mip chains.
    pub fn new(g: &GpuApi, mips: &Mips, fill: &mut [u8]) -> Layers {
        let count = (GROUND_LAYER_META.len() + block_cubes().len()) as u32;
        let levels = mip_levels(LAYER_TEX_SZ, LAYER_TEX_SZ);
        let usage = texture_usage::TEXTURE_BINDING | texture_usage::COPY_DST | texture_usage::RENDER_ATTACHMENT;
        let make = || {
            let tex = object();
            (g.texture)(tex, LAYER_TEX_SZ, LAYER_TEX_SZ, count, format::RGBA8_SRGB, levels, 1, usage);
            let view = object();
            (g.view)(view, tex, dimension::D2_ARRAY, 0, levels, 0, count);
            (tex, view)
        };
        let ((top, top_view), (side, side_view)) = (make(), make());
        each_layer(|_, layer, rgb| {
            let [r, gr, b] = rgb255(rgb);
            for px in fill.as_chunks_mut::<4>().0 {
                px.copy_from_slice(&[r, gr, b, 255]);
            }
            for tex in [top, side] {
                (g.write_texture)(tex, layer, LAYER_TEX_SZ, LAYER_TEX_SZ, fill);
            }
        });
        mips.build(g, top, levels, 0, count);
        mips.build(g, side, levels, 0, count);
        // Anisotropic at the four three's arrays were filtered at: the ground
        // is mostly seen at a grazing angle, where an isotropic mip blurs the
        // tile along the view.
        let sampler = object();
        (g.sampler)(sampler, filter::LINEAR, filter::LINEAR, filter::LINEAR, address::REPEAT, compare::NONE, 4);
        // A chain is the sum of its levels' texels: under four thirds of the first.
        let bytes = (0..levels).map(|level| (LAYER_TEX_SZ >> level).pow(2) * 4 * count * 2).sum();
        Layers {
            top,
            side,
            top_view,
            side_view,
            sampler,
            bytes,
            count,
            levels,
            loaded: false,
        }
    }

    /// Draws the pack's sheet into both arrays once it has arrived: `sheet`
    /// asks for its image, until it has been drawn.
    pub fn step(&mut self, g: &GpuApi, mips: &Mips, sheet: fn() -> Option<u32>) {
        if self.loaded {
            return;
        }
        let Some(image) = sheet() else { return };
        let (cell, size) = (SHIPPED_SHEET.cell as f32, LAYER_TEX_SZ as f32);
        for (tex, face) in [(self.top, CubeFace::Top), (self.side, CubeFace::Side)] {
            each_layer(|cube, layer, _| {
                let Some(tile) = cube.and_then(|cube| shipped_texture_tile(cube, face).or_else(|| shipped_texture_tile(cube, CubeFace::Top))) else {
                    return;
                };
                let mut p = Paint::new();
                p.size(LAYER_TEX_SZ, LAYER_TEX_SZ);
                p.image(image, tile.x as f32, tile.y as f32, cell, cell, 0.0, 0.0, size, size);
                p.run(g);
                (g.paint_copy)(tex, 0, 0, layer, LAYER_TEX_SZ, LAYER_TEX_SZ);
            });
            mips.build(g, tex, self.levels, 0, self.count);
        }
        self.loaded = true;
    }
}
