// pxl-render/gpu/resolve.rs

//! Gathers the world's color over frames. Each frame samples every pixel off its middle by a
//! jitter (pxl-engine `eye::JITTER`) taken from a sequence that covers the pixel in `JITTERS`
//! frames, and blends it into what the frames before left (screen_resolve.wgsl). The world
//! surface has two histories, written in turn, and the one written this frame is what the
//! finish encodes (gpu/finish.rs). A frame drawn through the gate writes its claimed tiles back
//! into the last history, which the finish then encodes.
//!
//! Each screen tile keeps its age, the frames since it last changed. Where only the jitter has
//! moved since, nothing the frames before left is stale, and each frame takes its even share of
//! the sequence. Where the tile changed this frame, what the frames before left is moved to
//! where this frame's eye sees it, and held within what this frame's pixels about it hold. A
//! tile is claimed, and drawn again, until it has run through the sequence (word.rs
//! `screen::DAMAGE_FRAMES`, screen_tiles.wgsl). Once every tile has, the jitter holds and the
//! frame stands still.
//!
//! The tiles' words live on the GPU, as the light's do (gpu/tiles.rs). Each holds the number of
//! the frame its tile last changed in, which the frame's marks set. The CPU hands the marks, so
//! it knows whether any tile gathers from what it handed. The resolve and the write-back draw
//! the claimed tiles alone, a quad each (screen_tiles.wgsl).

use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::world;

use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::frame::shader::record::{PIPELINE_SCREEN_RESOLVE, PIPELINE_SCREEN_WRITE_BACK};
use crate::frame::shader::word::screen::{DAMAGE_FRAMES, JITTERS};
use crate::gpu::api::WORLD_COLOR;
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, pass_desc, sampler_entry, view_entry, Gpu, Group, Load,
    RenderContext,
};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::span_spec::GpuSpanName;
use crate::gpu::surface::Surface;
use crate::gpu::tiles::{Tiles, SIDE};

/// The least share a frame takes where what the frames before left is held to this frame's
/// pixels. It trades how long a trail a moving thing leaves against how still an edge stands.
const BLEND_LEAST: f32 = 0.1;

/// The floats of the resolve's uniform (screen_resolve.wgsl `Resolve`).
const UNIFORM_FLOATS: usize = 20;

pub struct Resolve {
    pipeline: Pipeline,
    sampler: u32,
    uniform: u32,
    /// The write-back, which writes a gated frame's claimed tiles into the last history.
    back: Pipeline,
    /// The groups `set_groups` sets, one for each of the surface's histories. A resolve group
    /// reads its history as the last frame's, and a write-back group reads its history as this
    /// frame's.
    groups: [Group; 2],
    back_groups: [Group; 2],
    /// The history this frame writes. The other is the last frame's.
    turn: usize,
    /// The surface's histories the resolve gathered into, and the frames gathered since they
    /// were made. New histories hold nothing from before.
    histories: [u32; 2],
    gathered: u32,
    /// This frame's view, as its view-projection about the eye and the eye's place
    /// (`prepare`), and the last drawn frame's view.
    view: (Mat4, [f64; 3]),
    last: (Mat4, [f64; 3]),
    /// Where the jitter is in its sequence, and this frame's jitter in drawn pixels.
    phase: u32,
    jitter: [f32; 2],
    /// The screen's tiles, each claimed until it has gathered the jitter's whole sequence since
    /// it changed (gpu/tiles.rs). The gate, the pixels' cast, the resolve, the write-back and
    /// the finish all draw by them.
    pub tiles: Tiles,
}

impl Resolve {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines) -> Resolve {
        Resolve {
            pipeline: pipelines.build_world(g, PIPELINE_SCREEN_RESOLVE),
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
            back: pipelines.build(g, PIPELINE_SCREEN_WRITE_BACK, WORLD_COLOR),
            groups: [Group::default(), Group::default()],
            back_groups: [Group::default(), Group::default()],
            turn: 0,
            histories: [0; 2],
            gathered: 0,
            view: (mat4::new(), [0.0; 3]),
            last: (mat4::new(), [0.0; 3]),
            phase: 0,
            jitter: [0.0; 2],
            tiles: Tiles::new(g, pipelines, SIDE, DAMAGE_FRAMES, GpuSpanName::mark),
        }
    }

    /// Takes this frame's view: its view-projection about the eye, and the eye's place.
    pub fn prepare(&mut self, view_proj: &Mat4, eye: [f64; 3]) {
        self.view = (*view_proj, eye);
    }

    /// This frame's jitter, in drawn pixels, X right and Y down. While a change that is not
    /// the scene's animation settles (`asks`), it is the sequence's next. Otherwise it holds
    /// where it stands: once the sequence has run through, which takes the last frame such a
    /// change came in and the `JITTERS - 1` frames after it, each at its own place of the
    /// sequence; and in a frame drawn for anything else, such as an animation's step.
    pub fn jitter(&mut self) -> [f32; 2] {
        if self.asks() {
            let k = self.phase + 1;
            self.jitter = [halton(k, 2) - 0.5, halton(k, 3) - 0.5];
        }
        self.jitter
    }

    /// Sets the groups over `s`, the frame words in `frame` and the tiles' words and uniform,
    /// before the frame's commands, once the tiles are sized for `s` (`Tiles::size`).
    pub fn set_groups(&mut self, g: &Gpu, frame: u32, s: &Surface) {
        let (words, uniform) = self.tiles.this_frame();
        for k in 0..2 {
            self.groups[k].set(
                g,
                self.pipeline.layouts[0],
                &[
                    buf_entry(0, frame),
                    view_entry(1, s.color_view),
                    view_entry(2, s.depth_only_view),
                    view_entry(3, s.history_view[k]),
                    sampler_entry(4, self.sampler),
                    buf_entry(5, self.uniform),
                    buf_entry(30, words),
                    buf_entry(31, uniform),
                ],
            );
            self.back_groups[k].set(
                g,
                self.back.layouts[0],
                &[
                    view_entry(1, s.history_view[k]),
                    buf_entry(30, words),
                    buf_entry(31, uniform),
                ],
            );
        }
    }

    /// Blends the frame's world color, `s`'s, into what the frames before left, writing the
    /// history this turn writes, each tile by its age. When the frame is drawn through the gate
    /// (`masked`), only the claimed tiles are blended, and they are written back into the last
    /// history, which stays the one read. The passes are their own, over the groups set for
    /// `s` (`set_groups`), and their time is counted in the span `resolve`. Returns the history
    /// the finish encodes.
    pub fn draw(&mut self, ctx: &mut RenderContext, s: &Surface, masked: bool) -> u32 {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::resolve);
        if s.history_view != self.histories {
            (self.histories, self.gathered, self.turn) = (s.history_view, 0, 0);
        }
        let (k, was) = (self.turn, 1 - self.turn);
        // A square that changed this frame takes a share of no less than `BLEND_LEAST`. A
        // square where only the jitter has moved since takes its even share of the sequence,
        // found from its age (screen_resolve.wgsl).
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
        let [across, down] = self.tiles.squares();
        let tiles = across * down;
        let load = if masked { Load::Load } else { Load::Clear };
        let pass = g.pass(
            encoder,
            &pass_desc(s.history_view[k], load, 0, Load::Clear, 0.0, stamp.pass()),
        );
        g.set_pipeline(pass, self.pipeline.id);
        g.set_bind_group(pass, 0, self.groups[was].id(), &[]);
        g.draw(pass, 6, tiles, 0, 0);
        g.end(pass);
        if !masked {
            return s.history_view[k];
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
        g.set_pipeline(pass, self.back.id);
        g.set_bind_group(pass, 0, self.back_groups[k].id(), &[]);
        g.draw(pass, 6, tiles, 0, 0);
        g.end(pass);
        s.history_view[was]
    }

    /// Maps a pixel's middle and depth in this frame's view to its place in the last drawn
    /// frame's view, in f64. The eye's move between the frames is taken from its two places,
    /// using the copy of the last place nearest this one, as the world wraps in X.
    fn reproject(&self) -> Mat4 {
        let ((now, eye), (last, was)) = (&self.view, &self.last);
        let shift = [
            world::nearest(eye[0], was[0]) - was[0],
            eye[1] - was[1],
            eye[2] - was[2],
        ];
        let (mut inv, mut t, mut a, mut m) = (mat4::new(), mat4::new(), mat4::new(), mat4::new());
        mat4::invert(&mut inv, now);
        mat4::translation(&mut t, shift[0], shift[1], shift[2]);
        mat4::mul(&mut a, &t, &inv);
        mat4::mul(&mut m, last, &a);
        m
    }

    /// Called once the frame is drawn. Its history becomes the next frame's to read, unless it
    /// was drawn through the gate (`masked`) and written back into the last. Its view becomes
    /// the last view, and the jitter moves on when the frame took a place of the sequence
    /// (`asks`, as `jitter` read it).
    pub fn on_drawn(&mut self, masked: bool) {
        if !masked {
            self.turn = 1 - self.turn;
        }
        self.gathered = self.gathered.saturating_add(1);
        self.last = self.view;
        if self.asks() {
            self.phase = (self.phase + 1) % JITTERS;
        }
    }

    /// Whether the frames left of the jitter's sequence are asked for, with the jitter taking
    /// the sequence's next place in each. They are while the screen gathers after a change that
    /// is not the scene's animation alone (`Tiles::asking`). The jitter is the world's
    /// sequence, and moving it casts every pixel again (gpu/ray/schedule.rs `FrameKind::px`).
    /// So an animation's step holds the jitter: the tiles it marks are drawn through the gate
    /// with the jitter where it stands, and no pixel is cast for it. The animation's own
    /// antialiasing converges no further while only it moves.
    pub fn asks(&self) -> bool {
        self.tiles.asking()
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
