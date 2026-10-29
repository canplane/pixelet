// pxl-render/gpu/depth_bounds.rs

//! Where the world stands under the screen: the world's depth a frame drew,
//! the least and the most of each square of it `SIDE` drawn pixels a side
//! (screen_depth_bounds.wgsl), copied and mapped back as the counts are
//! (gpu/ray/tally.rs). A frame drawn whole takes every square, a region's
//! frame those it drew; what landed holds while only the region's draws
//! moved since it was taken (`kept`), drawn whole or not, as only those
//! moving leaves the view and the world under it as they were.

use xpute_guest::mem::off_of;

use crate::frame::shader::part::SHADER_DEPTH_BOUNDS;
use crate::frame::shader::record::{COMPUTE, DEPTH, STORAGE_RW, UNIFORM};
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, release, view_entry, word_bytes, Gpu, RenderContext, DONE,
};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::span_spec::GpuSpanName;
use crate::gpu::surface::Region;

/// A square's drawn pixels a side (screen_depth_bounds.wgsl `SIDE`).
pub const SIDE: u32 = 32;

/// What its pass binds: the depth, a square's two floats, the squares it
/// takes.
const GROUP: [u32; 9] = [
    0, COMPUTE, DEPTH, 1, COMPUTE, STORAGE_RW, 2, COMPUTE, UNIFORM,
];

pub struct DepthBounds {
    built: Built,
    group: u32,
    bound: [u32; 2],
    within: u32,
    bounds: u32,
    read: u32,
    landed: Box<[f32]>,
    mapping: Box<u32>,
    /// The squares across and down, as the buffers were made for.
    pub squares: [u32; 2],
    taken: u32,
    whole: u32,
    copied: u32,
    mapped: u32,
    landed_at: u32,
}

impl DepthBounds {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines) -> DepthBounds {
        DepthBounds {
            built: pipelines.build_compute(g, SHADER_DEPTH_BOUNDS, &GROUP),
            group: 0,
            bound: [0; 2],
            within: g.buf(
                word_bytes(4),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            bounds: 0,
            read: 0,
            landed: Box::new([]),
            mapping: Box::new(DONE),
            squares: [0; 2],
            taken: 0,
            whole: 0,
            copied: 0,
            mapped: 0,
            landed_at: 0,
        }
    }

    pub fn release(self) {
        for id in [self.group, self.within, self.bounds, self.read] {
            release(id);
        }
    }

    /// The squares under `region`, or every one, of the world's depth
    /// `depth` drawn `size` pixels across and down, taken after the world's
    /// pass, timed with the resolve; `kept` where only the region's draws
    /// moved since the last frame, so what landed before still holds.
    pub fn take(
        &mut self,
        ctx: &mut RenderContext,
        depth: u32,
        size: [u32; 2],
        region: Option<Region>,
        kept: bool,
    ) {
        let g = ctx.g;
        let squares = size.map(|v| v.div_ceil(SIDE));
        if squares != self.squares {
            release(self.bounds);
            release(self.read);
            let n = (squares[0] * squares[1]) as usize;
            self.bounds = g.buf(
                word_bytes(2 * n),
                buffer_usage::STORAGE | buffer_usage::COPY_SRC,
            );
            self.read = g.buf(
                word_bytes(2 * n),
                buffer_usage::MAP_READ | buffer_usage::COPY_DST,
            );
            self.landed = vec![0.0; 2 * n].into_boxed_slice();
            self.squares = squares;
            (self.copied, self.landed_at) = (0, 0);
        }
        if [depth, self.bounds] != self.bound {
            release(self.group);
            self.group = g.bind_group(
                self.built.layouts[0],
                &[
                    view_entry(0, depth),
                    buf_entry(1, self.bounds),
                    buf_entry(2, self.within),
                ],
            );
            self.bound = [depth, self.bounds];
        }
        let (at, n) = region.map_or(([0; 2], squares), |r| {
            let lo = r.at.map(|v| v / SIDE);
            let hi: [u32; 2] =
                core::array::from_fn(|k| (r.at[k] + r.size[k]).div_ceil(SIDE).min(squares[k]));
            (lo, [hi[0] - lo[0], hi[1] - lo[1]])
        });
        g.write_buf(self.within, 0, bytes(&[at[0], at[1], n[0], n[1]]));
        let pass = g.compute_pass(ctx.encoder, ctx.stamp(GpuSpanName::resolve).pass());
        g.set_bind_group(pass, 0, self.group, &[]);
        g.set_pipeline(pass, self.built.pipeline);
        g.dispatch(pass, n[0].div_ceil(8), n[1].div_ceil(8), 1);
        g.end(pass);
        self.taken = self.taken.wrapping_add(1).max(1);
        if !kept {
            self.whole = self.taken;
        }
    }

    /// What was taken copied where none is out, after the frame's passes.
    pub fn copy(&mut self, g: &Gpu, encoder: u32) {
        if self.landed.is_empty()
            || self.copied != 0
            || self.mapped != 0
            || self.taken == self.landed_at
        {
            return;
        }
        g.copy_buf(
            encoder,
            (self.bounds, 0),
            (self.read, 0),
            word_bytes(self.landed.len()),
        );
        self.copied = self.taken;
    }

    /// What was copied last frame mapped back, after the frame that copied
    /// it was submitted.
    pub fn back(&mut self, g: &Gpu) {
        if self.mapped != 0 && *self.mapping & DONE != 0 {
            (self.landed_at, self.mapped) = (self.mapped, 0);
        }
        if self.copied != 0 && self.mapped == 0 && *self.mapping & DONE != 0 {
            let addr = off_of(self.landed.as_ptr()) as u32;
            g.map_read(
                self.read,
                0,
                word_bytes(self.landed.len()),
                addr,
                &mut self.mapping,
                1,
            );
            (self.mapped, self.copied) = (self.copied, 0);
        }
    }

    /// Each square's least and most depth, reversed, as landed, where only
    /// the region's draws moved since it was taken.
    pub fn landed(&self) -> Option<&[f32]> {
        (self.landed_at != 0 && self.landed_at >= self.whole).then_some(&*self.landed)
    }

    /// The take what landed is of, 0 before any.
    pub fn landed_at(&self) -> u32 {
        self.landed_at
    }

    /// The frame its copy was encoded in was dropped.
    pub fn on_dropped(&mut self) {
        self.copied = 0;
    }
}
