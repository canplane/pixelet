// pxl-render/gpu/propagate.rs

//! Propagates the shadow map's changed tiles to the screen's tiles (screen_propagate.wgsl), as
//! the light's own propagation takes the screen's receivers to the map's tiles
//! (light_propagate.wgsl). It runs on the GPU in a frame where the casters that changed marked
//! tiles of the map (gpu/tiles.rs), after the marks and before anything reads the screen's
//! words, timed in the span `propagate`. Each screen tile whose pixels' shadow taps read a
//! marked map tile is set to have changed this frame.
//!
//! It reads the world's depth as the last frame left it. That holds because the eye stands
//! still, as it does in every frame through the gate. The CPU does not read back what it
//! marked. It runs only in a frame whose changed casters marked the map, and those casters also
//! mark the screen tiles they are drawn in, so the CPU counts that frame as marked
//! (`Tiles::marked_in`) and takes the tiles to gather from it, as the light decides from what
//! it handed whether to cast.
//!
//! The depth it reads is a bound: the least and most depth of each square of `BOUND_STEP`
//! pixels a side, with a workgroup for each screen tile and a thread for each square. The bound
//! stays on the GPU and is set after each world pass in the tiles that pass drew
//! (screen_bound.wgsl, `set_bounds`). So the pass reads no pixel's depth except where a
//! square's bound spans more than a few of the map's tiles. It also passes over what lies
//! outside the map tiles the frame's marks cover, which the GPU keeps
//! (`Tiles::new_damage_rect`).

use pxl_engine::param::light;
use pxl_spec::space::coord::mat4::{self, Mat4};

use crate::frame::shader::floor::MAX_COMPUTE_INVOCATIONS_PER_WORKGROUP;
use crate::frame::shader::record::{binds, COMPUTE, DEPTH, STORAGE, STORAGE_RW, UNIFORM};
use crate::frame::shader::record::{COMPUTE_SCREEN_BOUND, COMPUTE_SCREEN_PROPAGATE};
use crate::frame::shader::word::{BOUND_SQUARES, BOUND_STEP};
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, release, view_entry, word_bytes, Gpu, Group, RenderContext,
};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::span_spec::GpuSpanName;
use crate::gpu::tiles::Tiles;

/// What the propagate pass binds (screen_propagate.wgsl): the world's depth, the pass's
/// uniform, the map's tile words and uniform, the screen's tile words and uniform, the depth's
/// bound, and the map's tiles the frame's marks cover.
const GROUP: [u32; 24] = [
    0, COMPUTE, DEPTH, 1, COMPUTE, UNIFORM, 2, COMPUTE, STORAGE, 3, COMPUTE, UNIFORM, 4, COMPUTE,
    STORAGE_RW, 5, COMPUTE, UNIFORM, 6, COMPUTE, STORAGE, 7, COMPUTE, STORAGE,
];

/// What the bound's pass binds (screen_bound.wgsl): the world's depth, the
/// bound, the screen's words and the uniform it draws by.
const BOUNDS_GROUP: [u32; 12] = [
    0, COMPUTE, DEPTH, 1, COMPUTE, STORAGE_RW, 30, COMPUTE, STORAGE, 31, COMPUTE, UNIFORM,
];

// Each compute group here must lay out what its shader reads, as the shader declares it
// (build.rs `BINDS`).
const _: () = assert!(
    binds(&[COMPUTE_SCREEN_PROPAGATE], &GROUP) && binds(&[COMPUTE_SCREEN_BOUND], &BOUNDS_GROUP),
    "a compute group lays out other than its shader reads (record.rs `binds`)"
);

const _: () = assert!(
    BOUND_SQUARES * BOUND_SQUARES <= MAX_COMPUTE_INVOCATIONS_PER_WORKGROUP,
    "a screen tile's squares are a workgroup every device takes"
);

/// The floats of the pass's uniform (screen_propagate.wgsl `Propagate`).
const UNIFORM_FLOATS: usize = 36;

pub struct Propagate {
    pipeline: Pipeline,
    uniform: u32,
    /// The depth's bound (screen_bound.wgsl `bounds`), two floats a square; the squares across
    /// and down it was made for (`size`); and the bound's pass.
    bounds: u32,
    bounds_size: [u32; 2],
    bounds_pipeline: Pipeline,
    /// The groups of the propagate pass and of the bound's pass (`set_groups`).
    group: Group,
    bounds_group: Group,
}

impl Propagate {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines) -> Propagate {
        Propagate {
            pipeline: pipelines.build_compute(g, COMPUTE_SCREEN_PROPAGATE, &GROUP),
            uniform: g.buf(
                word_bytes(UNIFORM_FLOATS),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            bounds: 0,
            bounds_size: [0; 2],
            bounds_pipeline: pipelines.build_compute(g, COMPUTE_SCREEN_BOUND, &BOUNDS_GROUP),
            group: Group::default(),
            bounds_group: Group::default(),
        }
    }

    pub fn release(mut self) {
        for id in [self.uniform, self.bounds] {
            release(id);
        }
        self.group.release();
        self.bounds_group.release();
    }

    /// Writes this frame's uniform: the eye's view-projection `view_proj` and the light's
    /// `light_view_proj`, both taking units measured from the eye, and the reach of a
    /// receiver's taps, `spread` texels each way on a map `side` texels a side.
    pub fn prepare(
        &self,
        g: &Gpu,
        view_proj: &Mat4,
        light_view_proj: &Mat4,
        spread: f32,
        side: u32,
    ) {
        let mut inv = mat4::new();
        mat4::invert(&mut inv, view_proj);
        let mut u = [0.0f32; UNIFORM_FLOATS];
        for (to, from) in u[..16].iter_mut().zip(inv) {
            *to = from as f32;
        }
        for (to, from) in u[16..32].iter_mut().zip(light_view_proj) {
            *to = *from as f32;
        }
        u[32] = spread;
        u[33] = side as f32;
        g.write_buf(self.uniform, 0, bytes(&u));
    }

    /// Makes the depth's bound for a world depth of `size` pixels across and down, before the
    /// frame's commands. It is made anew only for a new size, and every tile is drawn in that
    /// frame.
    pub fn size(&mut self, g: &Gpu, size: [u32; 2]) {
        let squares = size.map(|v| v.div_ceil(BOUND_STEP));
        if squares == self.bounds_size {
            return;
        }
        release(self.bounds);
        self.bounds = g.buf(
            word_bytes(2 * (squares[0] * squares[1]) as usize),
            buffer_usage::STORAGE,
        );
        self.bounds_size = squares;
    }

    /// Sets both passes' groups before the frame's commands, once the bound for the world's
    /// size is made (`size`): over the world's depth `depth`, the map's `light` tiles and the
    /// `screen`'s tiles.
    pub fn set_groups(&mut self, g: &Gpu, depth: u32, light: &Tiles, screen: &Tiles) {
        let (l, s) = (light.this_frame(), screen.this_frame());
        self.bounds_group.set(
            g,
            self.bounds_pipeline.layouts[0],
            &[
                view_entry(0, depth),
                buf_entry(1, self.bounds),
                buf_entry(30, s.0),
                buf_entry(31, s.1),
            ],
        );
        self.group.set(
            g,
            self.pipeline.layouts[0],
            &[
                view_entry(0, depth),
                buf_entry(1, self.uniform),
                buf_entry(2, l.0),
                buf_entry(3, l.1),
                buf_entry(4, s.0),
                buf_entry(5, s.1),
                buf_entry(6, self.bounds),
                buf_entry(7, light.new_damage_rect()),
            ],
        );
    }

    /// Sets the depth's bound in the tiles the world's pass drew this frame, which are the
    /// screen's claimed tiles (`set_groups`), `tiles` across and down. Its time is counted with
    /// the resolve, which follows it.
    pub fn set_bounds(&self, ctx: &mut RenderContext, tiles: [u32; 2]) {
        let g = ctx.g;
        let pass = g.compute_pass(ctx.encoder, ctx.stamp(GpuSpanName::resolve).pass());
        g.set_bind_group(pass, 0, self.bounds_group.id(), &[]);
        g.set_pipeline(pass, self.bounds_pipeline.id);
        g.dispatch(pass, tiles[0], tiles[1], 1);
        g.end(pass);
    }

    /// Propagates the map's tiles that changed this frame to the screen's, the ones its groups
    /// bind (`set_groups`), with the screen's `tiles` across and down. It reads the depth's
    /// bound, and the world's depth where a square's bound spans more than a few map tiles,
    /// both as the last frame left them. It passes over what lies outside the tiles the frame's
    /// marks cover (`Tiles::new_damage_rect`).
    pub fn dispatch(&self, ctx: &mut RenderContext, tiles: [u32; 2]) {
        let g = ctx.g;
        let pass = g.compute_pass(ctx.encoder, ctx.stamp(GpuSpanName::propagate).pass());
        g.set_bind_group(pass, 0, self.group.id(), &[]);
        g.set_pipeline(pass, self.pipeline.id);
        g.dispatch(pass, tiles[0], tiles[1], 1);
        g.end(pass);
    }
}

/// The texels a receiver's taps reach each way from its own texel, for the filter and the
/// normal's offset, with `margin` texels of room (shadow.wgsl `shadow_visibility`).
pub fn spread(margin: f32) -> f32 {
    light::SPREAD + margin
}
