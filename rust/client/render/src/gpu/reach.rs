// pxl-render/gpu/reach.rs

//! The light's map's tiles that changed reached across to the screen's
//! (screen_reach.wgsl), as the light's reach takes the screen's receivers to
//! the light's tiles (pxl-shader light.rs `light_reach_cs`): in a frame
//! where the casters that changed marked tiles of the map (gpu/tiles.rs),
//! each screen tile whose pixels' shadow's taps read one of them is set to
//! have changed this frame, on the GPU, after the marks and before anything
//! reads the screen's words, timed with them in the span `mark`. It reads
//! the world's depth as the last frame left it, which holds while the eye
//! stands, as it does in every frame through the gate. What it marked the
//! host does not see: the pass keeps the number of the last frame it marked
//! a tile in (`reached`), copied and mapped back as the light's counts are
//! (gpu/ray/tally.rs), and until a frame it ran in has landed the screen's
//! tiles are taken to gather (`Tiles::unseen`). The depth it reads is a
//! bound, each square of 4 pixels a side its least and most depth, kept on
//! the GPU and set after each world's pass in the tiles it drew
//! (screen_bound.wgsl, `set_bounds`), so a frame it runs in reads no pixel's
//! depth but where a square's bound spans past a few of the map's tiles; and
//! it passes by what lies off the map's tiles the frame's marks cover, which
//! the GPU keeps (`Tiles::marks_box`).

use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec_core::param::light;
use xpute_guest::mem::off_of;

use crate::frame::shader::part::{SHADER_SCREEN_BOUND, SHADER_SCREEN_REACH};
use crate::frame::shader::record::{COMPUTE, DEPTH, STORAGE, STORAGE_RW, UNIFORM};
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, release, view_entry, word_bytes, Gpu, RenderContext, DONE,
};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::span_spec::GpuSpanName;
use crate::gpu::tiles::Tiles;

/// What its pass binds (screen_reach.wgsl): the world's depth, its uniform,
/// the map's words and uniform, the screen's words and uniform, the number
/// it keeps, the depth's bound, the map's tiles the frame's marks cover.
const GROUP: [u32; 27] = [
    0, COMPUTE, DEPTH, 1, COMPUTE, UNIFORM, 2, COMPUTE, STORAGE, 3, COMPUTE, UNIFORM, 4, COMPUTE,
    STORAGE_RW, 5, COMPUTE, UNIFORM, 6, COMPUTE, STORAGE_RW, 7, COMPUTE, STORAGE, 8, COMPUTE,
    STORAGE,
];

/// What the bound's pass binds (screen_bound.wgsl): the world's depth, the
/// bound, the screen's words and the uniform it draws by.
const BOUNDS_GROUP: [u32; 12] = [
    0, COMPUTE, DEPTH, 1, COMPUTE, STORAGE_RW, 30, COMPUTE, STORAGE, 31, COMPUTE, UNIFORM,
];

/// A square of the depth's bound's pixels a side (screen_bound.wgsl
/// `BOUND_STEP`, screen_reach.wgsl `REACH_STEP`).
const BOUND_STEP: u32 = 4;

/// Its uniform's floats (screen_reach.wgsl `Reach`).
const UNIFORM_FLOATS: usize = 36;

pub struct Reach {
    built: Built,
    uniform: u32,
    /// Its group and what it was made over, made again where one is another.
    group: u32,
    bound: [u32; 7],
    /// The depth's bound (screen_bound.wgsl `bounds`), two floats a square,
    /// the squares across and down it was made for, and its pass and group,
    /// made again where what it binds is another.
    bounds: u32,
    bounds_size: [u32; 2],
    bounds_built: Built,
    bounds_group: u32,
    bounds_bound: [u32; 4],
    /// The number of the last frame it marked a tile in (screen_reach.wgsl
    /// `reached`), a word, and the buffer it is copied into and mapped
    /// from, into `landed`, `mapping` its map's word.
    reached: u32,
    read: u32,
    landed: Box<[u32]>,
    mapping: Box<u32>,
    /// The screen's frame it last ran in, and the one before it, which a
    /// frame dropped puts back; the frame whose copy is encoded, whose map
    /// is out, and whose map last landed, each 0 for none.
    ran_in: u32,
    ran_before: u32,
    copied: u32,
    mapped: u32,
    landed_in: u32,
}

impl Reach {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines) -> Reach {
        Reach {
            built: pipelines.build_compute(g, SHADER_SCREEN_REACH, &GROUP),
            uniform: g.buf(
                word_bytes(UNIFORM_FLOATS),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            group: 0,
            bound: [0; 7],
            bounds: 0,
            bounds_size: [0; 2],
            bounds_built: pipelines.build_compute(g, SHADER_SCREEN_BOUND, &BOUNDS_GROUP),
            bounds_group: 0,
            bounds_bound: [0; 4],
            reached: g.buf(
                word_bytes(1),
                buffer_usage::STORAGE | buffer_usage::COPY_SRC,
            ),
            read: g.buf(
                word_bytes(1),
                buffer_usage::MAP_READ | buffer_usage::COPY_DST,
            ),
            landed: Box::new([0]),
            mapping: Box::new(DONE),
            ran_in: 0,
            ran_before: 0,
            copied: 0,
            mapped: 0,
            landed_in: 0,
        }
    }

    pub fn release(self) {
        for id in [
            self.uniform,
            self.group,
            self.reached,
            self.read,
            self.bounds,
            self.bounds_group,
        ] {
            release(id);
        }
    }

    /// This frame's views: the eye's view-projection `view_proj` and the
    /// light's `light_view_proj`, both off units from the eye; a receiver's
    /// taps `spread` texels each way of a map `side` texels a side.
    pub fn lay_out(
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

    /// The depth's bound set in the tiles the world's pass drew this frame,
    /// the words and uniform `drawn` (`Tiles::bound`, or `Tiles::every`
    /// where the pass draws every tile), off the world's depth `depth`, `size`
    /// pixels across and down; made anew for a new size, where every tile is
    /// drawn. Timed with the resolve, which follows it.
    pub fn set_bounds(
        &mut self,
        ctx: &mut RenderContext,
        depth: u32,
        size: [u32; 2],
        drawn: (u32, u32),
        tiles: [u32; 2],
    ) {
        let g = ctx.g;
        let squares = size.map(|v| v.div_ceil(BOUND_STEP));
        if squares != self.bounds_size {
            release(self.bounds);
            self.bounds = g.buf(
                word_bytes(2 * (squares[0] * squares[1]) as usize),
                buffer_usage::STORAGE,
            );
            self.bounds_size = squares;
        }
        let bound = [depth, self.bounds, drawn.0, drawn.1];
        if bound != self.bounds_bound {
            release(self.bounds_group);
            self.bounds_group = g.bind_group(
                self.bounds_built.layouts[0],
                &[
                    view_entry(0, depth),
                    buf_entry(1, self.bounds),
                    buf_entry(30, drawn.0),
                    buf_entry(31, drawn.1),
                ],
            );
            self.bounds_bound = bound;
        }
        let pass = g.compute_pass(ctx.encoder, ctx.stamp(GpuSpanName::resolve).pass());
        g.set_bind_group(pass, 0, self.bounds_group, &[]);
        g.set_pipeline(pass, self.bounds_built.pipeline);
        g.dispatch(pass, tiles[0], tiles[1], 1);
        g.end(pass);
    }

    /// The map's `light` tiles that changed this frame reached across to the
    /// `screen`'s, over the depth's bound and, where a square's spans past a
    /// few tiles, the world's depth `depth`, both as the last frame left
    /// them, off the tiles the frame's marks cover (`Tiles::marks_box`);
    /// what it keeps copied where no map of it is out.
    pub fn dispatch(&mut self, ctx: &mut RenderContext, depth: u32, light: &Tiles, screen: &Tiles) {
        let g = ctx.g;
        let (l, s) = (light.bound(), screen.bound());
        let bound = [depth, l.0, l.1, s.0, s.1, self.bounds, light.marks_box()];
        if bound != self.bound {
            release(self.group);
            self.group = g.bind_group(
                self.built.layouts[0],
                &[
                    view_entry(0, depth),
                    buf_entry(1, self.uniform),
                    buf_entry(2, l.0),
                    buf_entry(3, l.1),
                    buf_entry(4, s.0),
                    buf_entry(5, s.1),
                    buf_entry(6, self.reached),
                    buf_entry(7, self.bounds),
                    buf_entry(8, light.marks_box()),
                ],
            );
            self.bound = bound;
        }
        let [across, down] = screen.squares();
        let pass = g.compute_pass(ctx.encoder, ctx.stamp(GpuSpanName::mark).pass());
        g.set_bind_group(pass, 0, self.group, &[]);
        g.set_pipeline(pass, self.built.pipeline);
        g.dispatch(pass, across, down, 1);
        g.end(pass);
        (self.ran_before, self.ran_in) = (self.ran_in, screen.now());
        if self.copied == 0 && self.mapped == 0 && *self.mapping & DONE != 0 {
            g.copy_buf(
                ctx.encoder,
                (self.reached, 0),
                (self.read, 0),
                word_bytes(1),
            );
            self.copied = self.ran_in;
        }
    }

    /// What was copied last frame mapped back, after the frame that copied
    /// it was submitted, and what landed handed to the `screen`'s tiles: the
    /// last frame the reach marked in, and whether a frame it ran in has not
    /// landed yet (`Tiles::unseen`).
    pub fn back(&mut self, g: &Gpu, screen: &mut Tiles) {
        if self.mapped != 0 && *self.mapping & DONE != 0 {
            (self.landed_in, self.mapped) = (self.mapped, 0);
            screen.marked_in(self.landed[0]);
        }
        if self.copied != 0 && self.mapped == 0 && *self.mapping & DONE != 0 {
            let addr = off_of(self.landed.as_ptr()) as u32;
            g.map_read(self.read, 0, word_bytes(1), addr, &mut self.mapping, 1);
            (self.mapped, self.copied) = (self.copied, 0);
        }
        screen.unseen(self.ran_in.wrapping_sub(self.landed_in) as i32 > 0);
    }

    /// The frame it ran and copied in was dropped: neither happened.
    pub fn on_dropped(&mut self, now: u32) {
        if self.ran_in == now {
            self.ran_in = self.ran_before;
        }
        if self.copied == now {
            self.copied = 0;
        }
    }
}

/// The texels a receiver's taps read each way of its own, the filter's and
/// the normal's offset, with `margin` texels of room (shadow.wgsl
/// `light_lit`).
pub fn spread(margin: f32) -> f32 {
    light::SPREAD + margin
}
