// pxl-render/gpu/propagate.rs

//! The light's map's tiles that changed propagated to the screen's
//! (screen_propagate.wgsl), as the light's propagation takes the screen's
//! receivers to the light's tiles
//! (light_propagate.wgsl): in a frame
//! where the casters that changed marked tiles of the map (gpu/tiles.rs),
//! each screen tile whose pixels' shadow's taps read one of them is set to
//! have changed this frame, on the GPU, after the marks and before anything
//! reads the screen's words, timed in the span `propagate`. It reads
//! the world's depth as the last frame left it, which holds while the eye
//! stands, as it does in every frame through the gate. What it marked the
//! CPU does not read back: it runs only in a frame whose casters that
//! changed marked, which mark the screen's tiles drawn as well, so the CPU
//! counts that frame as marked (`Tiles::marked_in`) and takes the tiles to
//! gather from it, as the light decides whether to cast from what it handed.
//! The depth it reads is a bound, each square of `BOUND_STEP` pixels a side
//! its least and most depth, a thread a square of the workgroup a screen
//! tile, kept on the GPU and set after each world's pass in the tiles it drew
//! (screen_bound.wgsl, `set_bounds`), so a frame it runs in reads no pixel's
//! depth but where a square's bound spans past a few of the map's tiles; and
//! it passes by what lies off the map's tiles the frame's marks cover, which
//! the GPU keeps (`Tiles::marks_box`).

use pxl_engine::param::light;
use pxl_spec::space::coord::mat4::{self, Mat4};

use crate::frame::shader::floor::MAX_COMPUTE_INVOCATIONS_PER_WORKGROUP;
use crate::frame::shader::part::{SHADER_SCREEN_BOUND, SHADER_SCREEN_PROPAGATE};
use crate::frame::shader::record::{binds, COMPUTE, DEPTH, STORAGE, STORAGE_RW, UNIFORM};
use crate::frame::shader::word::{BOUND_SQUARES, BOUND_STEP};
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, release, view_entry, word_bytes, Gpu, RenderContext,
};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::span_spec::GpuSpanName;
use crate::gpu::tiles::Tiles;

/// What its pass binds (screen_propagate.wgsl): the world's depth, its uniform,
/// the map's words and uniform, the screen's words and uniform, the depth's
/// bound, the map's tiles the frame's marks cover.
const GROUP: [u32; 24] = [
    0, COMPUTE, DEPTH, 1, COMPUTE, UNIFORM, 2, COMPUTE, STORAGE, 3, COMPUTE, UNIFORM, 4, COMPUTE,
    STORAGE_RW, 5, COMPUTE, UNIFORM, 6, COMPUTE, STORAGE, 7, COMPUTE, STORAGE,
];

/// What the bound's pass binds (screen_bound.wgsl): the world's depth, the
/// bound, the screen's words and the uniform it draws by.
const BOUNDS_GROUP: [u32; 12] = [
    0, COMPUTE, DEPTH, 1, COMPUTE, STORAGE_RW, 30, COMPUTE, STORAGE, 31, COMPUTE, UNIFORM,
];

// Each compute group here against what its shader reads, as its declarations take it
// (build.rs `BINDS`).
const _: () = assert!(
    binds(&[SHADER_SCREEN_PROPAGATE], &GROUP) && binds(&[SHADER_SCREEN_BOUND], &BOUNDS_GROUP),
    "a compute group lays out other than its shader reads (record.rs `binds`)"
);

const _: () = assert!(
    BOUND_SQUARES * BOUND_SQUARES <= MAX_COMPUTE_INVOCATIONS_PER_WORKGROUP,
    "a screen tile's squares are a workgroup every device takes"
);

/// Its uniform's floats (screen_propagate.wgsl `Propagate`).
const UNIFORM_FLOATS: usize = 36;

pub struct Propagate {
    pipeline: Pipeline,
    uniform: u32,
    /// The depth's bound (screen_bound.wgsl `bounds`), two floats a square,
    /// the squares across and down it was made for (`size`), and its pass.
    bounds: u32,
    bounds_size: [u32; 2],
    bounds_built: Pipeline,
}

impl Propagate {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines) -> Propagate {
        Propagate {
            pipeline: pipelines.build_compute(g, SHADER_SCREEN_PROPAGATE, &GROUP),
            uniform: g.buf(
                word_bytes(UNIFORM_FLOATS),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            bounds: 0,
            bounds_size: [0; 2],
            bounds_built: pipelines.build_compute(g, SHADER_SCREEN_BOUND, &BOUNDS_GROUP),
        }
    }

    pub fn release(self) {
        for id in [self.uniform, self.bounds] {
            release(id);
        }
    }

    /// This frame's views: the eye's view-projection `view_proj` and the
    /// light's `light_view_proj`, both off units from the eye; a receiver's
    /// taps `spread` texels each way of a map `side` texels a side.
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

    /// The depth's bound for a world's depth `size` pixels across and down,
    /// made anew for a new size, where every tile is drawn; before the
    /// frame's commands.
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

    /// The depth's bound set in the tiles the world's pass drew this frame,
    /// the words and uniform `drawn` (`Tiles::this_frame`), off the world's
    /// depth `depth`, `tiles` across and down. Timed with the resolve, which
    /// follows it.
    pub fn set_bounds(
        &self,
        ctx: &mut RenderContext,
        depth: u32,
        drawn: (u32, u32),
        tiles: [u32; 2],
    ) {
        let g = ctx.g;
        let group = g.bind_group(
            self.bounds_built.layouts[0],
            &[
                view_entry(0, depth),
                buf_entry(1, self.bounds),
                buf_entry(30, drawn.0),
                buf_entry(31, drawn.1),
            ],
        );
        let pass = g.compute_pass(ctx.encoder, ctx.stamp(GpuSpanName::resolve).pass());
        g.set_bind_group(pass, 0, group, &[]);
        g.set_pipeline(pass, self.bounds_built.id);
        g.dispatch(pass, tiles[0], tiles[1], 1);
        g.end(pass);
        release(group);
    }

    /// The map's `light` tiles that changed this frame propagated to the
    /// `screen`'s, over the depth's bound and, where a square's spans past a
    /// few tiles, the world's depth `depth`, both as the last frame left
    /// them, off the tiles the frame's marks cover (`Tiles::marks_box`).
    pub fn dispatch(&self, ctx: &mut RenderContext, depth: u32, light: &Tiles, screen: &Tiles) {
        let g = ctx.g;
        let (l, s) = (light.this_frame(), screen.this_frame());
        let group = g.bind_group(
            self.pipeline.layouts[0],
            &[
                view_entry(0, depth),
                buf_entry(1, self.uniform),
                buf_entry(2, l.0),
                buf_entry(3, l.1),
                buf_entry(4, s.0),
                buf_entry(5, s.1),
                buf_entry(6, self.bounds),
                buf_entry(7, light.marks_box()),
            ],
        );
        let [across, down] = screen.squares();
        let pass = g.compute_pass(ctx.encoder, ctx.stamp(GpuSpanName::propagate).pass());
        g.set_bind_group(pass, 0, group, &[]);
        g.set_pipeline(pass, self.pipeline.id);
        g.dispatch(pass, across, down, 1);
        g.end(pass);
        release(group);
    }
}

/// The texels a receiver's taps read each way of its own, the filter's and
/// the normal's offset, with `margin` texels of room (shadow.wgsl
/// `shadow_visibility`).
pub fn spread(margin: f32) -> f32 {
    light::SPREAD + margin
}
