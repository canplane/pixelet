// pxl-render/gpu/resolve.rs

//! The world's color gathered over frames: each frame's pixels sample the
//! world off their middles by a jitter of their own (pxl-spec-core
//! `eye::JITTER`), a sequence that covers the pixel in `JITTERS` frames, and
//! each is blended into what the frames before left (screen_resolve.wgsl),
//! two of the world surface's `history` in turn, the one written this frame
//! what the finish encodes (gpu/finish.rs); a frame that draws a region
//! alone writes it back into the last, which the finish then encodes.
//! Each square of the screen keeps its age, the frames since it last
//! changed (`ages`): where only the jitter moved there since, nothing the
//! frames before left is stale, and each frame takes its even share of the
//! sequence; where it changed this frame, what the frames before left is
//! moved to where this frame's eye sees it and held within what this
//! frame's pixels about it hold. A square is drawn again until it has run
//! through the sequence (`gathering_rect`); once every one has, the jitter
//! holds, and the frame stands.

use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::world;

use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::frame::shader::record::PIPELINE_SCREEN_RESOLVE;
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, pass_desc, place, release, sampler_entry, view_entry,
    word_bytes, Gpu, Load, RenderContext,
};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::span_spec::GpuSpanName;
use crate::gpu::surface::{Region, Surface, WORLD_COLOR};

/// The frames the jitter's sequence covers a pixel in.
const JITTERS: u32 = 8;

/// The least share a frame takes where what the frames before left is
/// held to it: how long a trail what moved leaves, and how still an edge
/// stands.
const BLEND_LEAST: f32 = 0.1;

/// Its uniform's floats (screen_resolve.wgsl `Resolve`).
const UNIFORM_FLOATS: usize = 20;

/// A square's drawn pixels a side, whose age the resolve keeps
/// (screen_resolve.wgsl `RESOLVE_SIDE`).
pub const SIDE: u32 = 32;

const _: () = assert!(
    JITTERS == 8 && SIDE == 32,
    "screen_resolve.wgsl's `RESOLVE_JITTERS` and `RESOLVE_SIDE` are these"
);

pub struct Resolve {
    built: Built,
    sampler: u32,
    uniform: u32,
    /// A group a history written, reading the other, and what each was
    /// made over: made again exactly when one is another.
    groups: [u32; 2],
    bound: [[u32; 5]; 2],
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
    /// Each square's frames since it last changed, a row after another
    /// (screen_resolve.wgsl `ages`), the squares across and down, and the
    /// buffer the resolve reads them from.
    ages: Vec<u32>,
    squares: [u32; 2],
    ages_buf: u32,
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
            bound: [[0; 5]; 2],
            turn: 0,
            histories: [0; 2],
            gathered: 0,
            view: (mat4::new(), [0.0; 3]),
            last: (mat4::new(), [0.0; 3]),
            phase: 0,
            jitter: [0.0; 2],
            ages: Vec::new(),
            squares: [0; 2],
            ages_buf: 0,
        }
    }

    /// Each square's age as this frame draws it, of a world `size` drawn
    /// pixels across and down: where only the region's draws moved
    /// (`partial`), the squares `changed` covers 0 and the rest older;
    /// else each one older where only the jitter moved (`quiet`), every one
    /// 0 where anything else did. Every square 0 where the squares are new.
    pub fn age(
        &mut self,
        g: &Gpu,
        size: [u32; 2],
        partial: bool,
        quiet: bool,
        changed: Option<Region>,
    ) {
        let squares = size.map(|v| v.div_ceil(SIDE));
        if squares != self.squares {
            self.squares = squares;
            self.ages = vec![0; (squares[0] * squares[1]) as usize];
            release(self.ages_buf);
            self.ages_buf = g.buf(
                word_bytes(self.ages.len()),
                buffer_usage::STORAGE | buffer_usage::COPY_DST,
            );
        } else if !partial && !quiet {
            self.ages.fill(0);
        } else {
            for a in &mut self.ages {
                *a = a.saturating_add(1);
            }
        }
        if let Some(r) = changed.filter(|_| partial) {
            let lo = r.at.map(|v| v / SIDE);
            let hi: [u32; 2] =
                core::array::from_fn(|k| (r.at[k] + r.size[k]).div_ceil(SIDE).min(squares[k]));
            for y in lo[1]..hi[1] {
                for x in lo[0]..hi[0] {
                    self.ages[(y * squares[0] + x) as usize] = 0;
                }
            }
        }
        g.write_buf(self.ages_buf, 0, bytes(&self.ages));
    }

    /// The squares that have not yet run through the jitter's sequence since
    /// they last changed, of a world `size` drawn pixels across and down,
    /// the least region holding them; none where every one has, or the
    /// squares are not this size's.
    pub fn gathering_rect(&self, size: [u32; 2]) -> Option<Region> {
        let squares = size.map(|v| v.div_ceil(SIDE));
        if squares != self.squares {
            return None;
        }
        let mut b: Option<[u32; 4]> = None;
        for y in 0..squares[1] {
            for x in 0..squares[0] {
                if self.ages[(y * squares[0] + x) as usize] < JITTERS - 1 {
                    let e = b.get_or_insert([x, y, x, y]);
                    *e = [e[0].min(x), e[1].min(y), e[2].max(x), e[3].max(y)];
                }
            }
        }
        b.map(|[x0, y0, x1, y1]| {
            let at = [x0 * SIDE, y0 * SIDE];
            let end = [
                ((x1 + 1) * SIDE).min(size[0]),
                ((y1 + 1) * SIDE).min(size[1]),
            ];
            Region {
                at,
                size: [end[0] - at[0], end[1] - at[1]],
            }
        })
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
    /// left, into the history its turn writes, each square by its age
    /// (`age`). Where the frame draws a `region` again alone, only it,
    /// written back into the last history,
    /// which stays the one read. A pass of its own, by the frame words in
    /// `frame`, its time counted in the span `resolve`; the history the finish
    /// encodes.
    pub fn draw(
        &mut self,
        ctx: &mut RenderContext,
        frame: u32,
        s: &Surface,
        region: Option<Region>,
    ) -> u32 {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::resolve);
        if s.history_view != self.histories {
            (self.histories, self.gathered, self.turn) = (s.history_view, 0, 0);
            self.ages.fill(0);
            g.write_buf(self.ages_buf, 0, bytes(&self.ages));
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
        u[17] = f32::from_bits(self.squares[0]);
        g.write_buf(self.uniform, 0, bytes(&u));
        let bound = [
            frame,
            s.color_view,
            s.depth_view,
            s.history_view[was],
            self.ages_buf,
        ];
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
                    buf_entry(6, self.ages_buf),
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
        let at = [r.at[0], r.at[1], 0];
        g.copy_texture(
            encoder,
            &place(s.history[k], at),
            &place(s.history[was], at),
            [r.size[0], r.size[1], 1],
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
    /// and the jitter moves on unless it holds; `partial` where it drew a
    /// region alone.
    pub fn on_drawn(&mut self, partial: bool) {
        if !partial {
            self.turn = 1 - self.turn;
        }
        self.gathered = self.gathered.saturating_add(1);
        self.last = self.view;
        if self.gathering() {
            self.phase = (self.phase + 1) % JITTERS;
        }
    }

    /// Whether the jitter has places of its sequence left to cover in a
    /// square since it last changed (`ages`), or none was aged yet.
    pub fn gathering(&self) -> bool {
        self.ages.is_empty() || self.ages.iter().any(|&a| a < JITTERS - 1)
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
