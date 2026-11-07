// pxl-render/gpu/tiles.rs

//! Tiles a frame not drawn whole draws again by, the screen's and the light's
//! map's alike (word.rs `screen`): `side` pixels a side, each a word, the
//! number of the frame it last changed in (`changed_at`), the GPU's, as a
//! light tile's word names the cast that read it. The CPU hands each frame
//! at most one mark (`mark_of`), the clouds' sheets on the screen, and a pass
//! sets the words of the tiles it covers, a thread a tile, one mark of every
//! tile where the frame is drawn whole (screen_mark.wgsl), before anything
//! reads them; the draws that changed set theirs drawn (gpu/pass/mark.rs),
//! each tile once. A tile is claimed while it
//! changed fewer than `claimed_for` frames ago, each by its role: the screen
//! until its history has gathered the jitter's sequence, the map the frame
//! it changed alone; every one where the frame is drawn whole. What draws
//! them reads the words and the uniform (`Tiles`, screen_tiles.wgsl): a quad
//! a tile of every one, or, where a tile is claimed the frame it changed
//! alone, of the box the frame's marks cover, its count the GPU's
//! (`box_args`). A target drawn through its gate numbers its stencil here
//! (`gate`).

use crate::frame::shader::group::{BOX_ARGS_GROUP, CLAIMED_BOX_GROUP, SCREEN_MARK_GROUP};

use crate::frame::shader::record::{COMPUTE_BOX_ARGS, COMPUTE_CLAIMED_BOX, COMPUTE_SCREEN_MARK};
/// The screen's tiles' pixels a side (word.rs `screen::TILE`).
pub use crate::frame::shader::word::screen::TILE as SIDE;
use crate::frame::shader::word::screen::{self, TILES_WORDS};
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, release, word_bytes, Gpu, Group, PassDesc, RenderContext,
};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::span_spec::GpuSpanName;

/// The pixels a mark is grown by each way on the screen: the resolve's
/// pixels beside it and a face's growth (pxl-engine param.rs
/// `raster::GROWTH`), with room.
pub const MARK_MARGIN: f32 = 4.0;

/// What a draw that changed marks: pixels, `[x, y, width, height]`, whose
/// tiles a frame through the gate claims (`Tiles::mark`).
pub type Mark = [u32; 4];

/// The mark under clip `[x0, y0, x1, y1]`, Y up, of a target `width` by
/// `height`, grown by `margin` pixels; none where it holds nothing.
pub fn mark_of([x0, y0, x1, y1]: [f32; 4], width: u32, height: u32, margin: f32) -> Option<Mark> {
    let (w, h) = (width as f32, height as f32);
    let lo = [
        ((x0 * 0.5 + 0.5) * w - margin).max(0.0),
        ((0.5 - y1 * 0.5) * h - margin).max(0.0),
    ];
    let hi = [
        ((x1 * 0.5 + 0.5) * w + margin).min(w),
        ((0.5 - y0 * 0.5) * h + margin).min(h),
    ];
    (hi[0] > lo[0] && hi[1] > lo[1]).then(|| {
        let at = lo.map(|v| v as u32);
        [
            at[0],
            at[1],
            hi[0].ceil() as u32 - at[0],
            hi[1].ceil() as u32 - at[1],
        ]
    })
}

/// The claimed tiles' box before its pass: none, its first past its last.
const NO_CLAIMED_BOX: [u32; 4] = [u32::MAX, u32::MAX, 0, 0];

/// The numbers a frame through the gate writes into its claimed tiles'
/// stencil, 1 on, the stencil's eight bits but 0, which a frame drawn whole
/// leaves: one coming round clears it first, so no tile holds this frame's
/// number but those it claims (gpu/api.rs `Stencil`).
const GATE_NUMBERS: u32 = 255;

pub struct Tiles {
    /// A tile's pixels a side, and the frames a tile stays claimed once it
    /// changed, in a frame through the gate.
    side: u32,
    claimed_for: u32,
    /// The span their marks' passes are timed in (`dispatch_marks`,
    /// `dispatch_box_args`): the screen's or the light's.
    span: GpuSpanName,
    /// The uniform the frame draws them by, the claimed alone or every one
    /// (`mark`).
    uniform: u32,
    /// Each tile's word, a row after another, the GPU's; the target's
    /// pixels across and down; this frame's number.
    changed_at: u32,
    size: [u32; 2],
    now: u32,
    /// The last frame a mark was handed or drawn in that asks the frames of its claim: one
    /// not made by what the scene animates alone, every tile where it was drawn whole
    /// (`asking`).
    asked_at: u32,
    /// The marks' pass; its uniform, the mark's tiles and its head; and the
    /// tiles this frame's mark sets, 0 where it sets none.
    mark: Pipeline,
    mark_uniform: u32,
    mark_cnt: u32,
    /// The marks' pass's group over its uniform and the words, set with the
    /// words (`size`).
    mark_group: Group,
    /// The tiles the frame's marks cover, from the first to past the last,
    /// four words the GPU's: the one handed, widened by the draws that mark
    /// (tile_mark.wgsl), which bind it whatever reads it.
    marks_box: u32,
    /// The indirect args of a quad a tile of `marks_box`, set from it on the
    /// GPU once the marks are made, by a pass over its group (`box_args`);
    /// made where a tile is claimed the frame it changed alone, so the box
    /// holds every tile claimed.
    box_args: Option<(u32, Pipeline, u32)>,
    /// The number the last frame through the gate wrote into the claimed
    /// tiles' stencil, 0 where the target was drawn whole (`gate`).
    gate_number: u32,
    /// The box of the tiles claimed, from the first across and down to the last, four words
    /// the GPU's, and the pass that sets it (`dispatch_claimed_box`).
    claimed_box: u32,
    claimed_box_pass: Pipeline,
}

impl Tiles {
    /// Tiles `side` pixels a side, claimed for `claimed_for` frames once
    /// changed, their marks timed in `span`.
    pub fn new(
        g: &Gpu,
        pipelines: &mut Pipelines,
        side: u32,
        claimed_for: u32,
        span: GpuSpanName,
    ) -> Tiles {
        let marks_box = g.buf(
            word_bytes(4),
            buffer_usage::STORAGE | buffer_usage::COPY_DST,
        );
        let uniform = g.buf(
            word_bytes(TILES_WORDS),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let box_args = (claimed_for == 1).then(|| {
            let args = g.buf(
                word_bytes(4),
                buffer_usage::STORAGE | buffer_usage::INDIRECT,
            );
            let pass = pipelines.build_compute(g, COMPUTE_BOX_ARGS, &BOX_ARGS_GROUP);
            let group = g.bind_group(
                pass.layouts[0],
                &[
                    buf_entry(0, marks_box),
                    buf_entry(1, args),
                    buf_entry(2, uniform),
                ],
            );
            (args, pass, group)
        });
        Tiles {
            side,
            claimed_for,
            span,
            uniform,
            changed_at: 0,
            size: [0; 2],
            now: 0,
            asked_at: 0,
            mark: pipelines.build_compute(g, COMPUTE_SCREEN_MARK, &SCREEN_MARK_GROUP),
            mark_uniform: g.buf(
                word_bytes(8),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            mark_cnt: 0,
            mark_group: Group::default(),
            marks_box,
            box_args,
            gate_number: 0,
            claimed_box: g.buf(
                word_bytes(4),
                buffer_usage::STORAGE | buffer_usage::COPY_DST,
            ),
            claimed_box_pass: pipelines.build_compute(g, COMPUTE_CLAIMED_BOX, &CLAIMED_BOX_GROUP),
        }
    }

    pub fn release(mut self) {
        self.mark_group.release();
        let (args, group) = self.box_args.map_or((0, 0), |(a, _, g)| (a, g));
        for id in [
            self.uniform,
            self.changed_at,
            self.mark_uniform,
            self.marks_box,
            self.claimed_box,
            args,
            group,
        ] {
            release(id);
        }
    }

    /// The tiles of a target `size` pixels across and down, their words made
    /// anew where the target was another size, a frame that moves every
    /// one; before anything this frame binds them (`this_frame`).
    pub fn size(&mut self, g: &Gpu, size: [u32; 2]) {
        if size == self.size {
            return;
        }
        self.size = size;
        let [across, down] = self.squares();
        release(self.changed_at);
        self.changed_at = g.buf(word_bytes((across * down) as usize), buffer_usage::STORAGE);
        let entries = [
            buf_entry(0, self.mark_uniform),
            buf_entry(1, self.changed_at),
        ];
        self.mark_group.set(g, self.mark.layouts[0], &entries);
    }

    /// The mark this frame hands, the frame numbered anew: where it is
    /// drawn through the gate (`gated`), the tiles `mark` covers
    /// (`mark_of`); else none where nothing it draws moved (`quiet`), one of
    /// every tile where anything did. A mark through the gate in a turn the
    /// scene's animation stepped (`animated`) is that animation's, and asks no
    /// frame of its claim (`asking`). The tiles the frame draws: the claimed alone where it is gated,
    /// else every one. Set on the GPU, before anything reads them, by
    /// `dispatch_marks`; but a whole frame's stamp where a tile is claimed
    /// the frame it changed alone (`claimed_for` 1, the map's): that frame
    /// claims every tile by its uniform, and a later one through the gate
    /// only those set in it, so no frame reads the stamp.
    pub fn mark(
        &mut self,
        g: &Gpu,
        (gated, quiet, animated): (bool, bool, bool),
        mark: Option<Mark>,
    ) {
        let squares = self.squares();
        self.now = self.now.wrapping_add(1);
        let whole = !gated && !quiet;
        let side = self.side;
        let tiles = if whole {
            Some([0, 0, squares[0], squares[1]])
        } else {
            mark.filter(|_| gated)
                .map(|m| {
                    let lo = [m[0], m[1]].map(|v| v / side);
                    let hi: [u32; 2] =
                        core::array::from_fn(|k| (m[k] + m[k + 2]).div_ceil(side).min(squares[k]));
                    [lo[0], lo[1], hi[0], hi[1]]
                })
                .filter(|t| t[2] > t[0] && t[3] > t[1])
        };
        if tiles.is_some() && !(gated && animated) {
            self.asked_at = self.now;
        }
        let stamped = tiles.filter(|_| gated || self.claimed_for > 1);
        self.mark_cnt = stamped.map_or(0, |t| (t[2] - t[0]) * (t[3] - t[1]));
        if let Some(t) = stamped {
            let words = [t[0], t[1], t[2], t[3], self.now, squares[0], 0, 0];
            g.write_buf(self.mark_uniform, 0, bytes(&words));
        }
        if self.box_args.is_some() {
            // The one handed, which the draws that mark widen; none an i32's
            // greatest, as the propagation reads it (screen_propagate.wgsl).
            let none = i32::MAX as u32;
            let marks_box = tiles.unwrap_or([none, none, 0, 0]);
            g.write_buf(self.marks_box, 0, bytes(&marks_box));
        }
        let claimed_for = if gated { self.claimed_for } else { u32::MAX };
        let words = self.words(0.0, claimed_for);
        g.write_buf(self.uniform, 0, bytes(&words));
    }

    /// The mark `mark` handed set into the tiles' words, a thread a tile of
    /// its tiles, before anything reads them, timed in the tiles' span; none
    /// where it handed none.
    pub fn dispatch_marks(&self, ctx: &mut RenderContext) {
        if self.mark_cnt == 0 {
            return;
        }
        let g = ctx.g;
        let pass = g.compute_pass(ctx.encoder, ctx.stamp(self.span).pass());
        g.set_bind_group(pass, 0, self.mark_group.id(), &[]);
        g.set_pipeline(pass, self.mark.id);
        g.dispatch(pass, self.mark_cnt.div_ceil(64), 1, 1);
        g.end(pass);
    }

    /// The box of the tiles this frame claims, once its marks are made, set by a thread a tile
    /// (claimed_box.wgsl): four words the GPU's, timed with the marks.
    pub fn dispatch_claimed_box(&self, ctx: &mut RenderContext) -> u32 {
        let g = ctx.g;
        g.write_buf(self.claimed_box, 0, bytes(&NO_CLAIMED_BOX));
        let group = g.bind_group(
            self.claimed_box_pass.layouts[0],
            &[
                buf_entry(0, self.claimed_box),
                buf_entry(30, self.changed_at),
                buf_entry(31, self.uniform),
            ],
        );
        let [across, down] = self.squares();
        let pass = g.compute_pass(ctx.encoder, ctx.stamp(self.span).pass());
        g.set_bind_group(pass, 0, group, &[]);
        g.set_pipeline(pass, self.claimed_box_pass.id);
        g.dispatch(pass, (across * down).div_ceil(64), 1, 1);
        g.end(pass);
        release(group);
        self.claimed_box
    }

    /// The span the marks' passes are timed in, and what reads the marks beside them.
    pub fn span(&self) -> GpuSpanName {
        self.span
    }

    /// The words and the uniform as this frame draws them, the claimed alone
    /// or every one (`mark`).
    pub fn this_frame(&self) -> (u32, u32) {
        (self.changed_at, self.uniform)
    }

    /// The words alone, as a pass drawing the tiles by a uniform of its own
    /// binds (`words`).
    pub fn changed_at(&self) -> u32 {
        self.changed_at
    }

    /// The tiles this frame's marks cover, from the first across and down to
    /// past the last, four words on the GPU: the one handed, widened by the
    /// draws that mark (tile_mark.wgsl), none changed off them; kept where a
    /// tile is claimed the frame it changed alone (`box_args`).
    pub fn marks_box(&self) -> u32 {
        self.marks_box
    }

    /// The marks' box's args set from it on the GPU, after every pass that
    /// marks this frame and before a draw reads them, timed in the tiles'
    /// span (box_args.wgsl); none where the tiles keep no box.
    pub fn dispatch_box_args(&self, ctx: &mut RenderContext) {
        let Some((_, pipeline, group)) = &self.box_args else {
            return;
        };
        let g = ctx.g;
        let pass = g.compute_pass(ctx.encoder, ctx.stamp(self.span).pass());
        g.set_bind_group(pass, 0, *group, &[]);
        g.set_pipeline(pass, pipeline.id);
        g.dispatch(pass, 1, 1, 1);
        g.end(pass);
    }

    /// The indirect args of a quad a tile of the marks' box
    /// (screen_tiles.wgsl `box_clip`), as `dispatch_box_args` set them: the
    /// claimed tiles alone drawn, where a tile is claimed the frame it
    /// changed alone, as the map's is; none where it is claimed longer.
    pub fn box_args(&self) -> Option<u32> {
        self.box_args.as_ref().map(|b| b.0)
    }

    /// The pass `desc` drawing the target these tiles cover, through its
    /// gate where `gating`, and the number its gate writes into the claimed
    /// tiles' stencil: 1 on, coming round after `GATE_NUMBERS`, the stencil
    /// kept from the last frame through the gate but where the numbers come
    /// round, cleared then; none where the target is drawn whole, which
    /// clears it.
    pub fn gate(&mut self, desc: PassDesc, gating: bool) -> (PassDesc, Option<u32>) {
        let number = gating.then(|| self.gate_number % GATE_NUMBERS + 1);
        self.gate_number = number.unwrap_or(0);
        let desc = if number.is_some_and(|n| n > 1) {
            desc.load_stencil()
        } else {
            desc
        };
        (desc, number)
    }

    /// The frame was dropped: the stencil holds what the last frame drawn
    /// left, so the next through the gate clears it.
    pub fn on_dropped(&mut self) {
        self.gate_number = GATE_NUMBERS;
    }

    /// The tiles across and down.
    pub fn squares(&self) -> [u32; 2] {
        self.size.map(|v| v.div_ceil(self.side))
    }

    /// The uniform's words (word.rs `screen::TILES_WORDS`) for the
    /// target, a quad grown by `grow` of its pixels, in this frame, a tile
    /// claimed for `claimed_for` frames once changed,
    /// and a tile's side, as build.rs's `Tiles` reads them (tile_word.wgsl).
    pub fn words(&self, grow: f32, claimed_for: u32) -> [u32; TILES_WORDS] {
        let [across, down] = self.squares();
        let mut t = [0u32; TILES_WORDS];
        t[..4].copy_from_slice(&[across, down, claimed_for, self.now]);
        t[4] = (self.size[0] as f32).to_bits();
        t[5] = (self.size[1] as f32).to_bits();
        t[6] = grow.to_bits();
        t[7] = (self.side as f32).to_bits();
        t
    }

    /// The frames a tile stays claimed once changed, in a frame through the
    /// gate.
    pub fn claimed_for(&self) -> u32 {
        self.claimed_for
    }

    /// Whether a tile changed, by a mark that asks the frames of its claim, so lately it has
    /// frames left of its claim but this one: handed or drawn (`marked_in`), so lately its
    /// tile is claimed under a claim a frame shorter (`screen::claimed`). A tile changed by
    /// what the scene animates alone asks none.
    pub fn asking(&self) -> bool {
        screen::claimed(
            screen::age(self.now, self.asked_at),
            self.claimed_for.saturating_sub(1),
        )
    }

    /// This frame's number.
    pub fn now(&self) -> u32 {
        self.now
    }

    /// A mark a pass beside set in the frame numbered `at`, the draws that
    /// changed drawn and the propagation their frame's (gpu/pass/mark.rs,
    /// gpu/propagate.rs): the tiles gather from it as from one handed, where it
    /// is later than the last, and ask its frames unless the scene's animation
    /// made it (`animated`).
    pub fn marked_in(&mut self, at: u32, animated: bool) {
        if !animated && screen::age(self.now, at) < screen::age(self.now, self.asked_at) {
            self.asked_at = at;
        }
    }
}
