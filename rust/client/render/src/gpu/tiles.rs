// pxl-render/gpu/tiles.rs

//! Tiles a frame not drawn whole draws again by, the screen's and the light's
//! map's alike (pxl-spec-core `screen`; doc/minutes/20261029-the-screen-keeps-
//! its-tiles-as-the-light-does.md): `side` pixels a side, each a word, the
//! number of the frame it last changed in (`changed_at`), the GPU's, as a
//! light tile's word names the cast that read it. The host hands each frame
//! what changed, a mark each (`mark_of`), and a pass sets the words of the
//! tiles they cover, a thread a tile of each mark's, one mark of every tile
//! where the frame is drawn whole (screen_mark.wgsl), before anything reads
//! them. A tile is claimed while it
//! changed fewer than `claimed_for` frames ago, each by its role: the screen
//! until its history has gathered the jitter's sequence, the map the frame
//! it changed alone; every one where the frame is drawn whole. What draws
//! them reads the words and the uniform (`Tiles`, screen_tiles.wgsl).

use pxl_spec_core::screen::{self, TILES_WORDS};

use crate::frame::shader::part::SHADER_SCREEN_MARK;
use crate::frame::shader::record::{COMPUTE, STORAGE, STORAGE_RW, UNIFORM};
use crate::gpu::api::{buf_entry, buffer_usage, bytes, release, word_bytes, Gpu, RenderContext};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::span_spec::GpuSpanName;
/// The screen's tiles' pixels a side (pxl-spec-core `screen::TILE`).
pub use pxl_spec_core::screen::TILE as SIDE;

/// The pixels a mark is grown by each way on the screen: the resolve's
/// pixels beside it and a face's growth (pxl-spec-core param.rs
/// `raster::GROWN`), with room.
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

/// What the marks' pass binds (screen_mark.wgsl): its head, the marks' tiles
/// and the tiles' words.
const MARK_GROUP: [u32; 9] = [
    0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE_RW,
];

/// The marks its buffer first has room for, each a vec4's tiles.
const MARK_ROOM: usize = 64;

pub struct Tiles {
    /// A tile's pixels a side, and the frames a tile stays claimed once it
    /// changed, in a frame through the gate.
    side: u32,
    claimed_for: u32,
    /// The uniform the frame draws them by, the claimed alone or every one
    /// (`mark`); and one that draws every tile (`every`).
    uniform: u32,
    every: u32,
    /// Each tile's word, a row after another, the GPU's; the tiles across
    /// and down; this frame's number, and that of the last frame a mark was
    /// handed in, every tile where it was drawn whole.
    changed_at: u32,
    squares: [u32; 2],
    now: u32,
    marked_at: u32,
    /// The marks' pass, its group over the head, the marks' tiles (room for
    /// `mark_room`) and the words, made again where one is another; and
    /// whether this frame hands any.
    mark: Built,
    mark_group: u32,
    mark_bound: [u32; 2],
    mark_head: u32,
    marks: u32,
    mark_room: usize,
    marking: bool,
    /// The marks this frame hands, those whose words the marks' pass sets
    /// first, and the most tiles one of those holds; the tiles each of the
    /// rest holds, whose words a pass beside sets.
    mark_cnt: u32,
    set_cnt: u32,
    mark_most: u32,
    beside_tiles: Vec<u32>,
    /// The tiles all of them cover, from the first to past the last.
    marks_box: [u32; 4],
    /// Whether a pass beside may have set tiles the host has not seen yet
    /// (gpu/reach.rs).
    unseen: bool,
}

impl Tiles {
    /// Tiles `side` pixels a side, claimed for `claimed_for` frames once
    /// changed.
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, side: u32, claimed_for: u32) -> Tiles {
        let uniform = || {
            g.buf(
                word_bytes(TILES_WORDS),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            )
        };
        Tiles {
            side,
            claimed_for,
            uniform: uniform(),
            every: uniform(),
            changed_at: 0,
            squares: [0; 2],
            now: 0,
            marked_at: 0,
            mark: pipelines.build_compute(g, SHADER_SCREEN_MARK, &MARK_GROUP),
            mark_group: 0,
            mark_bound: [0; 2],
            mark_head: g.buf(
                word_bytes(4),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            marks: g.buf(
                word_bytes(4 * MARK_ROOM),
                buffer_usage::STORAGE | buffer_usage::COPY_DST,
            ),
            mark_room: MARK_ROOM,
            marking: false,
            mark_cnt: 0,
            set_cnt: 0,
            mark_most: 0,
            beside_tiles: Vec::new(),
            marks_box: [0; 4],
            unseen: false,
        }
    }

    pub fn release(self) {
        for id in [
            self.uniform,
            self.every,
            self.changed_at,
            self.mark_group,
            self.mark_head,
            self.marks,
        ] {
            release(id);
        }
    }

    /// The tiles of a target `size` pixels across and down, their words made
    /// anew where they were another size's, a frame that moves every one;
    /// before anything this frame binds them (`bound`, `every`).
    pub fn size(&mut self, g: &Gpu, size: [u32; 2]) {
        let squares = size.map(|v| v.div_ceil(self.side));
        if squares == self.squares {
            return;
        }
        self.squares = squares;
        release(self.changed_at);
        self.changed_at = g.buf(
            word_bytes((squares[0] * squares[1]) as usize),
            buffer_usage::STORAGE,
        );
        let words = self.words(size, 0.0, u32::MAX);
        g.write_buf(self.every, 0, bytes(&words));
    }

    /// The marks this frame hands, of a target `size` pixels across and
    /// down, the frame numbered anew: where it is drawn through the gate
    /// (`gated`), the tiles each of `marks` covers (`mark_of`), and those
    /// of `beside`, which a pass beside sets the words of and the frame draws
    /// the claimed of (`beside`); else none where nothing it draws moved
    /// (`quiet`), one of every tile where anything did.
    /// The tiles the frame draws: the claimed alone where it is gated, else
    /// every one. Set on the GPU, before anything reads them, by
    /// `dispatch_marks`.
    pub fn mark(
        &mut self,
        g: &Gpu,
        size: [u32; 2],
        (gated, quiet): (bool, bool),
        marks: &[Mark],
        beside: &[Mark],
    ) {
        let squares = self.squares;
        self.now = self.now.wrapping_add(1);
        let whole = !gated && !quiet;
        let every = [0, 0, squares[0], squares[1]];
        let side = self.side;
        let tiles_of = |marks: &[Mark]| -> Vec<[u32; 4]> {
            marks
                .iter()
                .filter(|_| gated)
                .map(|m| {
                    let lo = [m[0], m[1]].map(|v| v / side);
                    let hi: [u32; 2] =
                        core::array::from_fn(|k| (m[k] + m[k + 2]).div_ceil(side).min(squares[k]));
                    [lo[0], lo[1], hi[0], hi[1]]
                })
                .filter(|t| t[2] > t[0] && t[3] > t[1])
                .collect()
        };
        let mut tiles = tiles_of(marks);
        if whole {
            tiles = vec![every];
        }
        self.marking = !tiles.is_empty();
        self.set_cnt = tiles.len() as u32;
        self.mark_most = tiles
            .iter()
            .map(|t| (t[2] - t[0]) * (t[3] - t[1]))
            .max()
            .unwrap_or(0);
        let beside = if whole { Vec::new() } else { tiles_of(beside) };
        self.beside_tiles.clear();
        self.beside_tiles
            .extend(beside.iter().map(|t| (t[2] - t[0]) * (t[3] - t[1])));
        tiles.extend(beside);
        self.mark_cnt = tiles.len() as u32;
        self.marks_box = tiles.iter().fold([u32::MAX, u32::MAX, 0, 0], |b, t| {
            [
                b[0].min(t[0]),
                b[1].min(t[1]),
                b[2].max(t[2]),
                b[3].max(t[3]),
            ]
        });
        if self.marking {
            self.marked_at = self.now;
        }
        if tiles.len() > self.mark_room {
            release(self.marks);
            self.mark_room = tiles.len().next_power_of_two();
            self.marks = g.buf(
                word_bytes(4 * self.mark_room),
                buffer_usage::STORAGE | buffer_usage::COPY_DST,
            );
        }
        if !tiles.is_empty() {
            g.write_buf(self.marks, 0, bytes(tiles.as_flattened()));
        }
        let head = [self.set_cnt, self.now, squares[0], 0];
        g.write_buf(self.mark_head, 0, bytes(&head));
        let claimed_for = if gated { self.claimed_for } else { u32::MAX };
        let words = self.words(size, 0.0, claimed_for);
        g.write_buf(self.uniform, 0, bytes(&words));
    }

    /// The marks `mark` handed set into the tiles' words, a workgroup row a
    /// mark and a thread a tile of its tiles, first in the frame, timed in
    /// the span `mark`; none where it handed none.
    pub fn dispatch_marks(&mut self, ctx: &mut RenderContext) {
        if !self.marking {
            return;
        }
        let g = ctx.g;
        if [self.marks, self.changed_at] != self.mark_bound {
            release(self.mark_group);
            self.mark_group = g.bind_group(
                self.mark.layouts[0],
                &[
                    buf_entry(0, self.mark_head),
                    buf_entry(1, self.marks),
                    buf_entry(2, self.changed_at),
                ],
            );
            self.mark_bound = [self.marks, self.changed_at];
        }
        let pass = g.compute_pass(ctx.encoder, ctx.stamp(GpuSpanName::mark).pass());
        g.set_bind_group(pass, 0, self.mark_group, &[]);
        g.set_pipeline(pass, self.mark.pipeline);
        g.dispatch(pass, self.mark_most.div_ceil(64), self.set_cnt, 1);
        g.end(pass);
    }

    /// The words and the uniform as this frame draws them, the claimed alone
    /// or every one (`mark`).
    pub fn bound(&self) -> (u32, u32) {
        (self.changed_at, self.uniform)
    }

    /// The words and the uniform that draws every one.
    pub fn every(&self) -> (u32, u32) {
        (self.changed_at, self.every)
    }

    /// The words alone, as a pass drawing the tiles by a uniform of its own
    /// binds (`words`).
    pub fn changed_at(&self) -> u32 {
        self.changed_at
    }

    /// The marks this frame handed, each its tiles from the first to past
    /// the last (screen_tiles.wgsl `marks`), and how many of them the frame
    /// claims the tiles of exactly: what a pass that draws the tiles changed
    /// this frame draws, a quad a mark, with `beside`.
    pub fn marks(&self) -> (u32, u32) {
        (self.marks, self.set_cnt)
    }

    /// The marks past those, whose words a pass beside sets: the first
    /// instance and the instances a draw of each takes, a tile of its tiles
    /// each, the claimed alone drawn (screen_tiles.wgsl `mark_clip`).
    pub fn beside(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        let every = self.squares[0] * self.squares[1];
        (self.set_cnt..self.mark_cnt)
            .zip(&self.beside_tiles)
            .map(move |(m, &n)| (every * (m + 1), n))
    }

    /// The tiles this frame's marks cover, from the first across and down to
    /// past the last; none changed off them (empty where it handed none).
    pub fn marks_box(&self) -> [u32; 4] {
        self.marks_box
    }

    /// The tiles across and down.
    pub fn squares(&self) -> [u32; 2] {
        self.squares
    }

    /// The uniform's words (pxl-spec-core `screen::TILES_WORDS`) for a
    /// target `size` pixels across and down, a quad grown by `grow` of them,
    /// in this frame, a tile claimed for `claimed_for` frames once changed,
    /// and a tile's side.
    pub fn words(&self, size: [u32; 2], grow: f32, claimed_for: u32) -> [u32; TILES_WORDS] {
        let mut t = [0u32; TILES_WORDS];
        t[..4].copy_from_slice(&[self.squares[0], self.squares[1], claimed_for, self.now]);
        t[4] = (size[0] as f32).to_bits();
        t[5] = (size[1] as f32).to_bits();
        t[6] = grow.to_bits();
        t[7] = (self.side as f32).to_bits();
        t
    }

    /// The frames a tile stays claimed once changed, in a frame through the
    /// gate.
    pub fn claimed_for(&self) -> u32 {
        self.claimed_for
    }

    /// Whether a tile changed so lately it has frames left of its claim but
    /// this one, as far as the host knows: a mark was handed, or read back
    /// as set beside (`marked_in`), fewer than `claimed_for - 1` frames ago;
    /// or one set beside is not read back yet (`unseen`).
    pub fn claiming(&self) -> bool {
        self.unseen || screen::age(self.now, self.marked_at) < self.claimed_for.saturating_sub(1)
    }

    /// This frame's number.
    pub fn now(&self) -> u32 {
        self.now
    }

    /// A mark a pass beside set, read back, in the frame numbered `at`
    /// (gpu/reach.rs): the tiles gather from it as from one handed, where
    /// it is later than the last.
    pub fn marked_in(&mut self, at: u32) {
        if screen::age(self.now, at) < screen::age(self.now, self.marked_at) {
            self.marked_at = at;
        }
    }

    /// Whether a pass beside may have set tiles not read back yet.
    pub fn unseen(&mut self, unseen: bool) {
        self.unseen = unseen;
    }
}
