// pxl-render/gpu/tile.rs

//! The tiles by which a frame that is not drawn whole draws again only what changed, on the
//! screen and on the light's map alike (word.rs `screen`). A tile is `side` pixels a side. Its
//! word, which the GPU holds, is the number of the frame it last changed in. Each frame, a
//! pass sets the words of the tiles that the one mark the CPU hands covers (`damage`), before
//! anything reads them, and the draws that changed set the words of the tiles they cover
//! (pass/mark.rs). A tile is damaged while it changed fewer than `damage_frames` frames
//! ago, and every tile is damaged in a frame drawn whole. The screen keeps a tile damaged until
//! its history has gathered the jitter's whole sequence; the map damages a tile only in the
//! frame it changed. A pass that draws the tiles reads the words and the uniform
//! (screen_tile.wgsl) and draws only the damaged ones. A target drawn through its gate takes
//! the number its gate writes into the stencil from here (`mask`).

use crate::frame::shader::group::{
    DAMAGE_RECT_GROUP, LIGHT_RECT_ARGS, LIGHT_RECT_BINDS, RECT_ARGS_GROUP, SCREEN_MARK_GROUP,
};

use crate::frame::shader::record::{COMPUTE_DAMAGE_RECT, COMPUTE_RECT_ARGS, COMPUTE_SCREEN_MARK};
/// The side of a screen tile, in pixels.
pub use crate::frame::shader::word::screen::TILE as SIDE;
use crate::frame::shader::word::screen::{self, TILES_WORDS};
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, release, word_bytes, Gpu, Group, PassDesc, RenderContext,
};
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::gpu::span_spec::GpuSpanName;

/// How many pixels a mark on the screen is grown by each way. The margin covers the pixels
/// beside the mark that the resolve reads and a face's growth (pxl-engine param.rs
/// `raster::GROWTH`), with room to spare.
pub const DAMAGE_MARGIN: f32 = 4.0;

/// A mark: the pixels `[x, y, width, height]` that a change covers. A frame drawn through the
/// gate damages the tiles they lie in (`TileGrid::damage`).
pub type Damage = [u32; 4];

/// The mark of the clip-space box `[x0, y0, x1, y1]`, Y up, on a target `width` by `height`
/// pixels, grown by `margin` pixels each way and cut to the target. It is none where the box
/// covers no pixel.
pub fn damage_of(
    [x0, y0, x1, y1]: [f32; 4],
    width: u32,
    height: u32,
    margin: f32,
) -> Option<Damage> {
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

/// The box of damaged tiles as it is set before its pass runs: an empty box, whose first tile
/// lies past its last.
const NO_DAMAGE_RECT: [u32; 4] = [u32::MAX, u32::MAX, 0, 0];

/// How many numbers a frame drawn through the gate can write into its damaged tiles'
/// stencil: 1 to 255, every value of the stencil's eight bits but 0, which a frame drawn whole
/// leaves. When the numbers come round to 1, the frame clears the stencil first, so no tile
/// holds this frame's number but those it damages (gpu/api.rs `Stencil`).
const MASK_NUMBERS: u32 = 255;

pub struct TileGrid {
    /// A tile's side in pixels, and how many frames a tile stays damaged once it changed, in a
    /// frame drawn through the gate.
    side: u32,
    damage_frames: u32,
    /// The span the marks' passes are timed in: the screen's (`dispatch_damage`) or the
    /// light's (`dispatch_rect_args`).
    span: GpuSpanName,
    /// The uniform by which a frame draws the tiles. It covers the damaged tiles alone or
    /// every tile (`damage`).
    uniform: u32,
    /// Each tile's word, row after row, which the GPU holds; the target's size in pixels
    /// across and down; and this frame's number.
    changed_at: u32,
    size: [u32; 2],
    now: u32,
    /// The number of the last frame whose mark asks for the frames of its damage (`asking`).
    /// A mark asks unless the scene's animation alone made it, and a frame drawn whole marks
    /// every tile and asks.
    asked_at: u32,
    /// The pass that sets the handed mark into the words, and its uniform, which holds the
    /// mark's tiles and a head. `damage_cnt` is how many tiles this frame's mark sets, 0
    /// where it sets none.
    damage_pipeline: Pipeline,
    damage_uniform: u32,
    damage_cnt: u32,
    /// The group of that pass, over its uniform and the words. `size` sets it when it makes
    /// the words.
    damage_group: Group,
    /// The box of tiles the frame's marks cover, from the first tile to past the last, in four
    /// words the GPU holds. It starts as the handed mark, and the draws that mark widen it
    /// (tile_mark.wgsl). The draws that mark bind it whether or not anything reads it.
    new_damage_rect: u32,
    /// The indirect args that draw a quad for each tile of `new_damage_rect`, with the pass
    /// and the group that set them from the box on the GPU once the marks are made. They exist
    /// only where a tile is damaged in the frame it changed alone, as only then does the box
    /// hold every damaged tile.
    rect_args: Option<(u32, Pipeline, u32)>,
    /// The number the last frame drawn through the gate wrote into the damaged tiles'
    /// stencil, or 0 where the target was drawn whole (`mask`).
    mask_number: u32,
    /// The box of the damaged tiles, from the first across and down to the last, in four
    /// words the GPU holds, and the pass that sets it (`dispatch_damage_rect`).
    damage_rect: u32,
    damage_rect_pipeline: Pipeline,
}

impl TileGrid {
    /// Makes tiles `side` pixels a side, which stay damaged for `damage_frames` frames once
    /// they changed, with their marks' passes timed in `span`.
    pub fn new(
        g: &Gpu,
        pipelines: &mut PipelineCache,
        side: u32,
        damage_frames: u32,
        span: GpuSpanName,
    ) -> TileGrid {
        let new_damage_rect = g.buf(
            word_bytes(4),
            buffer_usage::STORAGE | buffer_usage::COPY_DST,
        );
        let uniform = g.buf(
            word_bytes(TILES_WORDS),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let rect_args = (damage_frames == 1).then(|| {
            let args = g.buf(
                word_bytes(4),
                buffer_usage::STORAGE | buffer_usage::INDIRECT,
            );
            let pass = pipelines.build_compute(g, COMPUTE_RECT_ARGS, &RECT_ARGS_GROUP);
            let group = g.frame_group(
                pass.layouts[0],
                &LIGHT_RECT_BINDS,
                |b| if b == LIGHT_RECT_ARGS { args } else { 0 },
                &[buf_entry(0, new_damage_rect), buf_entry(2, uniform)],
            );
            (args, pass, group)
        });
        TileGrid {
            side,
            damage_frames,
            span,
            uniform,
            changed_at: 0,
            size: [0; 2],
            now: 0,
            asked_at: 0,
            damage_pipeline: pipelines.build_compute(g, COMPUTE_SCREEN_MARK, &SCREEN_MARK_GROUP),
            damage_uniform: g.buf(
                word_bytes(8),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            damage_cnt: 0,
            damage_group: Group::default(),
            new_damage_rect,
            rect_args,
            mask_number: 0,
            damage_rect: g.buf(
                word_bytes(4),
                buffer_usage::STORAGE | buffer_usage::COPY_DST,
            ),
            damage_rect_pipeline: pipelines.build_compute(
                g,
                COMPUTE_DAMAGE_RECT,
                &DAMAGE_RECT_GROUP,
            ),
        }
    }

    /// Sizes the tiles for a target `size` pixels across and down, before anything this frame
    /// binds the words (`this_frame`). Where the size changed, it makes the words anew and
    /// keeps nothing of them, as a frame that resizes the target changes every tile.
    pub fn size(&mut self, g: &Gpu, size: [u32; 2]) {
        if size == self.size {
            return;
        }
        self.size = size;
        let [across, down] = self.squares();
        release(self.changed_at);
        self.changed_at = g.buf(word_bytes((across * down) as usize), buffer_usage::STORAGE);
        let entries = [
            buf_entry(0, self.damage_uniform),
            buf_entry(1, self.changed_at),
        ];
        self.damage_group
            .set(g, self.damage_pipeline.layouts[0], &entries);
    }

    /// Numbers a new frame and hands its mark. A frame drawn through the gate (`masked`) marks
    /// the tiles `damage` covers (`damage_of`). Any other frame marks every tile, or none where
    /// nothing it draws moved (`quiet`). A gated mark in a frame where the scene's animation
    /// stepped (`animated`) is the animation's own, and it asks for no frame of its damage
    /// (`asking`). The uniform covers the damaged tiles alone in a gated frame, and every tile
    /// in any other. `dispatch_damage` later sets the mark into the words on the GPU. Where a
    /// tile is damaged only in the frame it changed (`damage_frames` 1, the map's), a frame
    /// that is not gated sets nothing: it damages every tile by its uniform, and a later gated
    /// frame damages only the tiles set in that frame, so no frame would read what it set.
    pub fn damage(
        &mut self,
        g: &Gpu,
        (masked, quiet, animated): (bool, bool, bool),
        damage: Option<Damage>,
    ) {
        let squares = self.squares();
        self.now = self.now.wrapping_add(1);
        let whole = !masked && !quiet;
        let side = self.side;
        let tiles = if whole {
            Some([0, 0, squares[0], squares[1]])
        } else {
            damage
                .filter(|_| masked)
                .map(|m| {
                    let lo = [m[0], m[1]].map(|v| v / side);
                    let hi: [u32; 2] =
                        core::array::from_fn(|k| (m[k] + m[k + 2]).div_ceil(side).min(squares[k]));
                    [lo[0], lo[1], hi[0], hi[1]]
                })
                .filter(|t| t[2] > t[0] && t[3] > t[1])
        };
        if tiles.is_some() && !(masked && animated) {
            self.asked_at = self.now;
        }
        let stamped = tiles.filter(|_| masked || self.damage_frames > 1);
        self.damage_cnt = stamped.map_or(0, |t| (t[2] - t[0]) * (t[3] - t[1]));
        if let Some(t) = stamped {
            let words = [t[0], t[1], t[2], t[3], self.now, squares[0], 0, 0];
            g.write_buf(self.damage_uniform, 0, bytes(&words));
        }
        if self.rect_args.is_some() {
            // The box starts as the handed mark, and the draws that mark widen it. An empty box
            // starts at the greatest i32, as the propagation reads the box as signed
            // (screen_propagate.wgsl).
            let none = i32::MAX as u32;
            let new_damage_rect = tiles.unwrap_or([none, none, 0, 0]);
            g.write_buf(self.new_damage_rect, 0, bytes(&new_damage_rect));
        }
        let damage_frames = if masked { self.damage_frames } else { u32::MAX };
        let words = self.words(0.0, damage_frames);
        g.write_buf(self.uniform, 0, bytes(&words));
    }

    /// Sets the mark `damage` handed into the tiles' words, a thread a tile (screen_mark.wgsl),
    /// timed in the tiles' span. It runs before anything reads the words, and does nothing
    /// where the mark sets no tile.
    pub fn dispatch_damage(&self, ctx: &mut RenderContext) {
        if self.damage_cnt == 0 {
            return;
        }
        let g = ctx.g;
        let pass = g.compute_pass(ctx.encoder, ctx.stamp(self.span).pass());
        g.set_bind_group(pass, 0, self.damage_group.id(), &[]);
        g.set_pipeline(pass, self.damage_pipeline.id);
        g.dispatch(pass, self.damage_cnt.div_ceil(64), 1, 1);
        g.end(pass);
    }

    /// Sets the box of the tiles this frame damages, a thread a tile (damage_rect.wgsl), once the
    /// frame's marks are made, and returns its buffer of four words. It is timed in the marks'
    /// span.
    pub fn dispatch_damage_rect(&self, ctx: &mut RenderContext) -> u32 {
        let g = ctx.g;
        g.write_buf(self.damage_rect, 0, bytes(&NO_DAMAGE_RECT));
        let group = g.bind_group(
            self.damage_rect_pipeline.layouts[0],
            &[
                buf_entry(0, self.damage_rect),
                buf_entry(30, self.changed_at),
                buf_entry(31, self.uniform),
            ],
        );
        let [across, down] = self.squares();
        let pass = g.compute_pass(ctx.encoder, ctx.stamp(self.span).pass());
        g.set_bind_group(pass, 0, group, &[]);
        g.set_pipeline(pass, self.damage_rect_pipeline.id);
        g.dispatch(pass, (across * down).div_ceil(64), 1, 1);
        g.end(pass);
        release(group);
        self.damage_rect
    }

    /// The span the marks' passes are timed in. A pass that reads the marks beside them, such
    /// as the decor's cull (gpu/generator.rs `Generator::cull`), is timed in it too.
    pub fn span(&self) -> GpuSpanName {
        self.span
    }

    /// The words and the uniform by which this frame draws the tiles: the damaged tiles alone,
    /// or every tile (`damage`).
    pub fn this_frame(&self) -> (u32, u32) {
        (self.changed_at, self.uniform)
    }

    /// The words alone, for a pass that draws the tiles by a uniform of its own (`words`).
    pub fn changed_at(&self) -> u32 {
        self.changed_at
    }

    /// The box of tiles this frame's marks cover, from the first across and down to past the
    /// last, in four words on the GPU. It starts as the handed mark and the draws that mark
    /// widen it (tile_mark.wgsl), so no tile outside it changed this frame. It is kept only
    /// where a tile is damaged in the frame it changed alone (`rect_args`).
    pub fn new_damage_rect(&self) -> u32 {
        self.new_damage_rect
    }

    /// Sets the indirect args of the marks' box from the box, on the GPU (rect_args.wgsl). It
    /// runs after every pass that marks this frame and before a draw reads the args, timed in
    /// the tiles' span. It does nothing where the tiles keep no box.
    pub fn dispatch_rect_args(&self, ctx: &mut RenderContext) {
        let Some((_, pipeline, group)) = &self.rect_args else {
            return;
        };
        let g = ctx.g;
        let pass = g.compute_pass(ctx.encoder, ctx.stamp(self.span).pass());
        g.set_bind_group(pass, 0, *group, &[]);
        g.set_pipeline(pass, pipeline.id);
        g.dispatch(pass, 1, 1, 1);
        g.end(pass);
    }

    /// The indirect args that draw a quad for each tile of the marks' box (screen_tile.wgsl
    /// `rect_clip`), as `dispatch_rect_args` set them. They draw the damaged tiles alone where
    /// a tile is damaged only in the frame it changed, as the map's is. They are none where a
    /// tile stays damaged longer.
    pub fn rect_args(&self) -> Option<u32> {
        self.rect_args.as_ref().map(|b| b.0)
    }

    /// Prepares the pass `desc` that draws the target these tiles cover, through its gate
    /// where `masked`. It returns the pass and the number the gate writes into the damaged
    /// tiles' stencil, which counts from 1 and comes round after `MASK_NUMBERS`. The pass
    /// keeps the stencil the last gated frame left, except when the numbers come round, and
    /// then it clears the stencil. The number is none where the target is drawn whole, which
    /// clears the stencil.
    pub fn mask(&mut self, desc: PassDesc, masked: bool) -> (PassDesc, Option<u32>) {
        let number = masked.then(|| self.mask_number % MASK_NUMBERS + 1);
        self.mask_number = number.unwrap_or(0);
        let desc = if number.is_some_and(|n| n > 1) {
            desc.load_stencil()
        } else {
            desc
        };
        (desc, number)
    }

    /// Called when the frame was dropped. The stencil holds what the last drawn frame left, so
    /// the next gated frame clears it.
    pub fn on_dropped(&mut self) {
        self.mask_number = MASK_NUMBERS;
    }

    /// The tiles across and down.
    pub fn squares(&self) -> [u32; 2] {
        self.size.map(|v| v.div_ceil(self.side))
    }

    /// The uniform's words for this frame, laid out as build.rs's `TileGrid` reads them
    /// (tile_word.wgsl). A quad is grown by `grow` pixels each way past its tile, and a tile
    /// stays damaged for `damage_frames` frames once it changed.
    pub fn words(&self, grow: f32, damage_frames: u32) -> [u32; TILES_WORDS] {
        let [across, down] = self.squares();
        let mut t = [0u32; TILES_WORDS];
        t[..4].copy_from_slice(&[across, down, damage_frames, self.now]);
        t[4] = (self.size[0] as f32).to_bits();
        t[5] = (self.size[1] as f32).to_bits();
        t[6] = grow.to_bits();
        t[7] = (self.side as f32).to_bits();
        t
    }

    /// How many frames a tile stays damaged once it changed, in a frame drawn through the gate.
    pub fn damage_frames(&self) -> u32 {
        self.damage_frames
    }

    /// Whether a mark that asks for the frames of its damage changed a tile so lately that the
    /// tile stays damaged in a frame after this one. The mark may be handed (`damage`) or
    /// drawn (`marked_in`). A mark made by the scene's animation alone asks for none.
    pub fn asking(&self) -> bool {
        screen::in_damage(
            screen::age(self.now, self.asked_at),
            self.damage_frames.saturating_sub(1),
        )
    }

    /// This frame's number.
    pub fn now(&self) -> u32 {
        self.now
    }

    /// Takes note of a mark that another pass set in the frame numbered `at`: the marks of the
    /// draws that changed (pass/mark.rs), or the propagation's in their frame
    /// (pass/propagate.rs). Unless the scene's animation made it (`animated`), the mark asks for
    /// the frames of its damage as a handed mark does, where it is later than the last that
    /// asked.
    pub fn marked_in(&mut self, at: u32, animated: bool) {
        if !animated && screen::age(self.now, at) < screen::age(self.now, self.asked_at) {
            self.asked_at = at;
        }
    }
}
