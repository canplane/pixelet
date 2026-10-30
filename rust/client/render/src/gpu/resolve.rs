// pxl-render/gpu/resolve.rs

//! The world's color gathered over frames: each frame's pixels sample the
//! world off their middles by a jitter of their own (pxl-spec-core
//! `eye::JITTER`), a sequence that covers the pixel in `JITTERS` frames, and
//! each is blended into what the frames before left (screen_resolve.wgsl),
//! two of the world surface's `history` in turn, the one written this frame
//! what the finish encodes (gpu/finish.rs); a frame drawn through the gate
//! writes its claimed tiles back into the last, which the finish then
//! encodes. Each tile of the screen keeps its age, the frames since it last
//! changed: where only the jitter moved there since, nothing the
//! frames before left is stale, and each frame takes its even share of the
//! sequence; where it changed this frame, what the frames before left is
//! moved to where this frame's eye sees it and held within what this
//! frame's pixels about it hold. A tile is claimed, drawn again, until it
//! has run through the sequence (screen_tiles.wgsl); once every one has,
//! the jitter holds, and the frame stands. The tiles' words are the GPU's,
//! as the light's are (gpu/tiles.rs): each the number of the frame it last
//! changed in, which the frame's marks set; the host hands the marks, and
//! knows whether any tile gathers from what it handed. The resolve and the
//! write-back draw the claimed tiles alone, a quad each (screen_tiles.wgsl).

use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::world;

use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::frame::shader::record::{PIPELINE_SCREEN_RESOLVE, PIPELINE_SCREEN_WRITE_BACK};
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, pass_desc, release, sampler_entry, view_entry, Gpu, Load,
    RenderContext,
};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::span_spec::GpuSpanName;
use crate::gpu::surface::{Surface, WORLD_COLOR};
use crate::gpu::tiles::{Tiles, SIDE};
use pxl_spec_core::screen::JITTERS;

/// The least share a frame takes where what the frames before left is
/// held to it: how long a trail what moved leaves, and how still an edge
/// stands.
const BLEND_LEAST: f32 = 0.1;

/// Its uniform's floats (screen_resolve.wgsl `Resolve`).
const UNIFORM_FLOATS: usize = 20;

const _: () = assert!(
    JITTERS == 8,
    "screen_tiles.wgsl's `TILE_JITTERS` is pxl-spec-core `screen`'s"
);

pub struct Resolve {
    built: Built,
    sampler: u32,
    uniform: u32,
    /// A group a history written, reading the other, and what each was
    /// made over: made again exactly when one is another.
    groups: [u32; 2],
    bound: [[u32; 5]; 2],
    /// The write-back, and its group a history read, as `groups`.
    back: Built,
    back_groups: [u32; 2],
    back_bound: [[u32; 2]; 2],
    /// The history this frame writes, the other the last frame's.
    turn: usize,
    /// The surface's histories it gathered into, and the frames gathered
    /// since they were made: none kept before.
    histories: [u32; 2],
    gathered: u32,
    /// This frame's view, its view-projection about the eye and the eye's
    /// place (`lay_out`), and the last drawn frame's.
    view: (Mat4, [f64; 3]),
    last: (Mat4, [f64; 3]),
    /// Where the jitter is in its sequence, and this frame's, in drawn
    /// pixels.
    phase: u32,
    jitter: [f32; 2],
    /// The screen's tiles, claimed until each has gathered the jitter's
    /// sequence since it changed (gpu/tiles.rs): what the gate, the
    /// pixels' cast, the resolve, the write-back and the finish draw by.
    pub tiles: Tiles,
}

impl Resolve {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines) -> Resolve {
        Resolve {
            built: pipelines.build_world(g, PIPELINE_SCREEN_RESOLVE),
            sampler: g.sampler(
                FilterMode::linear,
                FilterMode::linear,
                MipmapFilterMode::nearest,
                AddressMode::clamp_to_edge,
                None,
                1,
            ),
            uniform: g.buf(
                (4 * UNIFORM_FLOATS) as u32,
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            groups: [0; 2],
            bound: [[0; 5]; 2],
            back: pipelines.build(g, PIPELINE_SCREEN_WRITE_BACK, WORLD_COLOR),
            back_groups: [0; 2],
            back_bound: [[0; 2]; 2],
            turn: 0,
            histories: [0; 2],
            gathered: 0,
            view: (mat4::new(), [0.0; 3]),
            last: (mat4::new(), [0.0; 3]),
            phase: 0,
            jitter: [0.0; 2],
            tiles: Tiles::new(g, pipelines, SIDE, JITTERS),
        }
    }

    /// This frame's view: its view-projection about the eye, and the eye's
    /// place.
    pub fn lay_out(&mut self, view_proj: &Mat4, eye: [f64; 3]) {
        self.view = (*view_proj, eye);
    }

    /// This frame's jitter, in drawn pixels, X right and Y down: the
    /// sequence's next, or where it holds once it ran through with nothing
    /// else moving: the last frame anything else moved in and the
    /// `JITTERS - 1` after it, each at its own place of the sequence.
    pub fn jitter(&mut self) -> [f32; 2] {
        if self.gathering() {
            let k = self.phase + 1;
            self.jitter = [halton(k, 2) - 0.5, halton(k, 3) - 0.5];
        }
        self.jitter
    }

    /// The frame's world color, `s`'s, blended into what the frames before
    /// left, into the history its turn writes, each tile by its age
    /// (`age`). Where the frame is drawn through the gate (`gated`), only the
    /// claimed tiles, written back into the last history, which stays the
    /// one read. Passes of their own, by the frame words in `frame`, their
    /// time counted in the span `resolve`; the history the finish encodes.
    pub fn draw(&mut self, ctx: &mut RenderContext, frame: u32, s: &Surface, gated: bool) -> u32 {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::resolve);
        if s.history_view != self.histories {
            (self.histories, self.gathered, self.turn) = (s.history_view, 0, 0);
        }
        let (k, was) = (self.turn, 1 - self.turn);
        // A square that changed this frame takes its share held no less than
        // `BLEND_LEAST`; one where only the jitter moved since, its even share
        // of the sequence's, off its age (screen_resolve.wgsl).
        let blend = match self.gathered {
            0 => 1.0,
            n => (1.0 / (n + 1) as f32).max(BLEND_LEAST),
        };
        let mut u = [0.0f32; UNIFORM_FLOATS];
        let m = self.reproject();
        for (to, from) in u[..16].iter_mut().zip(m) {
            *to = from as f32;
        }
        u[16] = blend;
        g.write_buf(self.uniform, 0, bytes(&u));
        let bound = [
            frame,
            s.color_view,
            s.depth_only_view,
            s.history_view[was],
            self.tiles.changed_at(),
        ];
        if bound != self.bound[k] {
            release(self.groups[k]);
            self.groups[k] = g.bind_group(
                self.built.layouts[0],
                &[
                    buf_entry(0, frame),
                    view_entry(1, s.color_view),
                    view_entry(2, s.depth_only_view),
                    view_entry(3, s.history_view[was]),
                    sampler_entry(4, self.sampler),
                    buf_entry(5, self.uniform),
                    buf_entry(30, self.tiles.bound().0),
                    buf_entry(31, self.tiles.bound().1),
                ],
            );
            self.bound[k] = bound;
        }
        let [across, down] = self.tiles.squares();
        let tiles = across * down;
        let load = if gated { Load::Load } else { Load::Clear };
        let pass = g.pass(
            encoder,
            &pass_desc(s.history_view[k], load, 0, Load::Clear, 0.0, stamp.pass()),
        );
        g.set_pipeline(pass, self.built.pipeline);
        g.set_bind_group(pass, 0, self.groups[k], &[]);
        g.draw(pass, 6, tiles, 0, 0);
        g.end(pass);
        if !gated {
            return s.history_view[k];
        }
        let back_bound = [s.history_view[k], self.tiles.changed_at()];
        if back_bound != self.back_bound[k] {
            release(self.back_groups[k]);
            self.back_groups[k] = g.bind_group(
                self.back.layouts[0],
                &[
                    view_entry(1, s.history_view[k]),
                    buf_entry(30, self.tiles.bound().0),
                    buf_entry(31, self.tiles.bound().1),
                ],
            );
            self.back_bound[k] = back_bound;
        }
        let pass = g.pass(
            encoder,
            &pass_desc(
                s.history_view[was],
                Load::Load,
                0,
                Load::Clear,
                0.0,
                stamp.pass(),
            ),
        );
        g.set_pipeline(pass, self.back.pipeline);
        g.set_bind_group(pass, 0, self.back_groups[k], &[]);
        g.draw(pass, 6, tiles, 0, 0);
        g.end(pass);
        s.history_view[was]
    }

    /// A pixel's middle and depth in this frame's view to its place in the
    /// last drawn frame's, in f64: the eye's move between them taken off
    /// its places, the nearest copy of the last's about this one's.
    fn reproject(&self) -> Mat4 {
        let ((now, eye), (last, was)) = (&self.view, &self.last);
        let moved = [
            world::nearest(eye[0], was[0]) - was[0],
            eye[1] - was[1],
            eye[2] - was[2],
        ];
        let (mut inv, mut t, mut a, mut m) = (mat4::new(), mat4::new(), mat4::new(), mat4::new());
        mat4::invert(&mut inv, now);
        mat4::translation(&mut t, moved[0], moved[1], moved[2]);
        mat4::mul(&mut a, &t, &inv);
        mat4::mul(&mut m, last, &a);
        m
    }

    /// The frame was drawn: its history is the next's to read, unless it was
    /// drawn through the gate (`gated`), written back into the last; its
    /// view the last, and the jitter moves on unless it holds.
    pub fn on_drawn(&mut self, gated: bool) {
        if !gated {
            self.turn = 1 - self.turn;
        }
        self.gathered = self.gathered.saturating_add(1);
        self.last = self.view;
        if self.gathering() {
            self.phase = (self.phase + 1) % JITTERS;
        }
    }

    /// Whether the jitter has places of its sequence left to cover in a
    /// tile since it last changed, as far as the host knows (`Tiles::claiming`).
    pub fn gathering(&self) -> bool {
        self.tiles.claiming()
    }
}

/// `i`'s radical inverse in `base`: the Halton sequence's `i`th.
fn halton(mut i: u32, base: u32) -> f32 {
    let (mut f, mut r) = (1.0f32, 0.0f32);
    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }
    r
}
