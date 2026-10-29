// pxl-render/gpu/resolve.rs

//! The world's color gathered over frames: each frame's pixels sample the
//! world off their middles by a jitter of their own (pxl-spec-core
//! `eye::JITTER`), a sequence that covers the pixel in `JITTERS` frames, and
//! each is blended into what the frames before left (screen_resolve.wgsl),
//! two of the world surface's `history` in turn, the one written this frame
//! what the finish encodes (gpu/finish.rs); a frame that draws a region
//! alone writes it back into the last, which the finish then encodes.
//! Where only the jitter moved,
//! nothing the frames before left is stale, and each frame takes its even
//! share of the sequence; once it has run through, the jitter holds, and
//! the frame stands. Else what the frames before left is moved to where
//! this frame's eye sees it and held within what this frame's pixels about
//! it hold.

use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::world;

use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::frame::shader::record::PIPELINE_SCREEN_RESOLVE;
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, pass_desc, release, sampler_entry, view_entry, Gpu, Load, Stamp,
};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::surface::{Region, Surface, WORLD_COLOR};

/// The frames the jitter's sequence covers a pixel in.
const JITTERS: u32 = 8;

/// The least share a frame takes where what the frames before left is
/// held to it: how long a trail what moved leaves, and how still an edge
/// stands.
const BLEND_LEAST: f32 = 0.1;

/// Its uniform's floats (screen_resolve.wgsl `Resolve`).
const UNIFORM_FLOATS: usize = 20;

pub struct Resolve {
    built: Built,
    sampler: u32,
    uniform: u32,
    /// A group a history written, reading the other, and what each was
    /// made over: made again exactly when one is another.
    groups: [u32; 2],
    bound: [[u32; 4]; 2],
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
    /// Where the jitter is in its sequence, the frames since anything but
    /// it moved, and this frame's, in drawn pixels.
    phase: u32,
    quiet: u32,
    jitter: [f32; 2],
}

impl Resolve {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines) -> Resolve {
        Resolve {
            built: pipelines.build(g, PIPELINE_SCREEN_RESOLVE, WORLD_COLOR),
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
            bound: [[0; 4]; 2],
            turn: 0,
            histories: [0; 2],
            gathered: 0,
            view: (mat4::new(), [0.0; 3]),
            last: (mat4::new(), [0.0; 3]),
            phase: 0,
            quiet: 0,
            jitter: [0.0; 2],
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
    /// left, into the history its turn writes: `quiet` where nothing but
    /// the jitter moved since the last drawn frame. Where the frame draws a
    /// `region` again alone, only it, written back into the last history,
    /// which stays the one read. A pass of its own, by the frame words in
    /// `frame`, its time counted in `stamp`'s span; the history the finish
    /// encodes.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        g: &Gpu,
        encoder: u32,
        frame: u32,
        s: &Surface,
        quiet: bool,
        region: Option<Region>,
        stamp: &mut Stamp,
    ) -> u32 {
        if s.history_view != self.histories {
            (self.histories, self.gathered, self.turn) = (s.history_view, 0, 0);
        }
        let (k, was) = (self.turn, 1 - self.turn);
        // A frame where only the jitter moved takes its even share of the
        // sequence's, and what the frames before left stands whole; else
        // its share is held no less than `BLEND_LEAST`.
        let blend = match (self.gathered, quiet) {
            (0, _) => 1.0,
            (_, true) => 1.0 / (self.quiet.min(JITTERS - 2) + 2) as f32,
            (n, false) => (1.0 / (n + 1) as f32).max(BLEND_LEAST),
        };
        let mut u = [0.0f32; UNIFORM_FLOATS];
        let m = self.reproject();
        for (to, from) in u[..16].iter_mut().zip(m) {
            *to = from as f32;
        }
        u[16] = blend;
        u[17] = if quiet { 0.0 } else { 1.0 };
        g.write_buf(self.uniform, 0, bytes(&u));
        let bound = [frame, s.color_view, s.depth_view, s.history_view[was]];
        if bound != self.bound[k] {
            release(self.groups[k]);
            self.groups[k] = g.bind_group(
                self.built.layouts[0],
                &[
                    buf_entry(0, frame),
                    view_entry(1, s.color_view),
                    view_entry(2, s.depth_view),
                    view_entry(3, s.history_view[was]),
                    sampler_entry(4, self.sampler),
                    buf_entry(5, self.uniform),
                ],
            );
            self.bound[k] = bound;
        }
        let load = if region.is_some() {
            Load::Load
        } else {
            Load::Clear
        };
        let pass = g.pass(
            encoder,
            &pass_desc(s.history_view[k], load, 0, Load::Clear, 0.0, stamp.pass()),
        );
        if let Some(r) = region {
            g.set_scissor(pass, r.rect());
        }
        g.set_pipeline(pass, self.built.pipeline);
        g.set_bind_group(pass, 0, self.groups[k], &[]);
        g.draw(pass, 3, 1, 0, 0);
        g.end(pass);
        let Some(r) = region else {
            return s.history_view[k];
        };
        g.copy_texture(
            encoder,
            s.history[k],
            r.at[0],
            r.at[1],
            s.history[was],
            r.at[0],
            r.at[1],
            0,
            r.size[0],
            r.size[1],
        );
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

    /// The frame was drawn: its history is the next's to read, unless it
    /// drew a region alone, written back into the last; its view the last,
    /// and the jitter moves on unless it holds; `quiet` and `partial` as
    /// `draw` was handed them.
    pub fn on_drawn(&mut self, quiet: bool, partial: bool) {
        if !partial {
            self.turn = 1 - self.turn;
        }
        self.gathered = self.gathered.saturating_add(1);
        self.last = self.view;
        if self.gathering() {
            self.phase = (self.phase + 1) % JITTERS;
        }
        self.quiet = if quiet { self.quiet + 1 } else { 0 };
    }

    /// Whether the jitter has places of its sequence left to cover since
    /// anything else last moved.
    pub fn gathering(&self) -> bool {
        self.quiet < JITTERS - 1
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
