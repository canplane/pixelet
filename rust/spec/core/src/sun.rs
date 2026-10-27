// pxl-spec-core/sun.rs

//! gpu-portable: the world's depth from the sun, a ray a texel of the sun's
//! shadow map (pxl-shader `sun_cs`): each texel's ray from where it
//! lies on the light box's near plane, along the sun's rays to the far
//! plane, walked through the one descent the eye's rays take, every node
//! asked at the zoom the eye's rays ask it (`cast::Detail::eye`); and the
//! depth it leaves, as a raster caster's would be (pxl-render gpu/shadow.rs,
//! where the decor and the actors are drawn into the same map).
//!
//! The world's depth is a clipmap: the light box's texels lie on a grid
//! fixed in the world (`aim`'s snap), and each world texel is held in the
//! slot its index names modulo the map's side. A slot holds its depth and
//! the zoom its stop was asked at (`Sun::word`). Every slot is cast again
//! each frame the map is, as the eye's rays are (`SAMPLE`): nothing a cast
//! established is kept for the next, so no policy says what still stands.

use crate::cast::{Detail, Ends, Met, Ray};
use crate::{axis, FRAC_W};

/// The sun's cast as the eye's words hold it (`eye::SUN`), a frame's
/// constants worked out once off the light box (`Sun::of`), each a vec4's
/// first three words: where texel (0, 0)'s ray starts, from the eye, in
/// units, and how far every ray reaches (the fourth word); the step to the
/// next texel across and down; the rays' one direction; and how far a stop
/// on a face across each axis is pushed back in depth. Their fourth words
/// hold the clipmap's: the world texel index of the box's texel (0, 0),
/// `corner`, X then Y, Y counted down the rows; and the share of the box's
/// half-width a receiver reads the map within, `rim` (`Sun::read`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sun {
    pub first: [f32; 3],
    pub reach: f32,
    pub across: [f32; 3],
    pub down: [f32; 3],
    pub dir: [f32; 3],
    pub bias: [f32; 3],
    pub corner: [i32; 2],
    pub rim: f32,
}

/// The word of `eye::SUN` the rim lies in.
pub const RIM: usize = 15;

/// How far past the disk a receiver reads the map reaches, in texels: its
/// taps a texel and a half out, filtered a texel more, its normal offset a
/// texel (pxl-render shadow.wgsl `sun_lit`), and the box's snap under one.
pub const READ_PAD: f32 = 4.0;

/// The words of `eye::SUN` the corner lies in, X then Y, which the map is
/// laid in off.
pub const CORNER: [usize; 2] = [7, 11];

/// A slot's word: its depth, a 24-bit share of the box's depth as a
/// depth24 map holds it, over the zoom its stop was asked at.
pub const DEPTH_ONE: u32 = (1 << 24) - 1;
pub const ZOOM_BITS: u32 = 8;
/// A slot whose walk stopped at nothing: no zoom asked.
const NO_ZOOM: u32 = (1 << ZOOM_BITS) - 1;

fn length(v: [f32; 3]) -> f32 {
    libm::sqrtf(dot(v, v))
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// How much a plane's depth in the map changes a texel across and a texel
/// down it, as a share of the box's depth: the plane's normal `n`, the
/// step `across` and `down` a texel takes in the world, and `deep`, the
/// world's step the box's whole depth along the sun's rays. The plane
/// holds `n`'s dot with each step it takes, so the share of the depth `t`
/// that keeps it on the plane after a step `s` across is
/// `−(n·s) / (n·deep)`. A caster's push is its greatest, times
/// `eye::SLOPE_BIAS` (`Sun::of`), and a receiver's taps each compare its
/// depth carried along its own plane to where the tap lies (pxl-render
/// shadow.wgsl `sun_slope`, off the light box's rows): the one statement of
/// a face's slope in the map, which both take any normal to. None for a
/// face along the rays, which no ray enters across.
pub fn slope(n: [f32; 3], across: [f32; 3], down: [f32; 3], deep: [f32; 3]) -> [f32; 2] {
    let d = dot(n, deep);
    if d == 0.0 {
        return [0.0, 0.0];
    }
    [-dot(n, across) / d, -dot(n, down) / d]
}

impl Sun {
    /// Off the light box's inverse (pxl-render gpu/shadow.rs `aim`), from
    /// the map's NDC to units from the eye, by its columns: a texel's NDC
    /// `(x, y)` lies at `w + x·x + y·y` on the near plane, and `z` runs
    /// from the near plane to the far; the map `size` texels a side, row 0
    /// at the top, as the lit passes read it. A stop's push is a raster
    /// caster's polygon offset (`eye::SLOPE_BIAS`): the face's greatest
    /// change in depth a texel across the map (`slope`), times it.
    /// `corner` is the world texel index of the box's first texel; `rim`
    /// the share of its half-width a receiver reads within.
    pub fn of([x, y, z, w]: [[f32; 3]; 4], size: u32, corner: [i32; 2], rim: f32) -> Sun {
        let texel = 2.0 / size as f32;
        let (ax, ay) = (1.0 - texel / 2.0, texel / 2.0 - 1.0);
        let reach = length(z);
        let dir = [z[0] / reach, z[1] / reach, z[2] / reach];
        let across = [x[0] * texel, x[1] * texel, x[2] * texel];
        let down = [-y[0] * texel, -y[1] * texel, -y[2] * texel];
        // A face's normal is its axis.
        let bias = |a: u32| {
            let on = |k: u32| if a == k { 1.0 } else { 0.0 };
            let n = [on(0), on(1), on(2)];
            let [i, j] = slope(n, across, down, z);
            let (i, j) = (libm::fabsf(i), libm::fabsf(j));
            crate::eye::SLOPE_BIAS * if i > j { i } else { j }
        };
        Sun {
            first: [
                w[0] - ax * x[0] - ay * y[0],
                w[1] - ax * x[1] - ay * y[1],
                w[2] - ax * x[2] - ay * y[2],
            ],
            reach,
            across,
            down,
            dir,
            bias: [bias(0), bias(1), bias(2)],
            corner,
            rim,
        }
    }

    /// Where a point `v` units from the eye lies on the box, in texels across
    /// and down from texel (0, 0)'s middle: its step from that texel's start
    /// along each of the box's axes, over that axis's texel. The light box
    /// is orthographic, its axes square to the sun's rays, so depth along
    /// them counts for nothing.
    pub fn texel_at(&self, v: [f32; 3]) -> [f32; 2] {
        let d = [
            v[0] - self.first[0],
            v[1] - self.first[1],
            v[2] - self.first[2],
        ];
        [
            dot(d, self.across) / dot(self.across, self.across),
            dot(d, self.down) / dot(self.down, self.down),
        ]
    }

    /// The tile of the box a receiver standing `v` units from the eye
    /// stands on (pxl-shader sun.rs `sun_mark_cs`, `sun_mark_drawn_cs`).
    pub fn tile_of(&self, v: [f32; 3]) -> [i32; 2] {
        let (at, tile) = (self.texel_at(v), TILE as f32);
        [
            libm::floorf((at[0] + 0.5) / tile) as i32,
            libm::floorf((at[1] + 0.5) / tile) as i32,
        ]
    }

    /// How many tiles either side of its own a drawn receiver stands on
    /// (pxl-shader sun.rs `sun_mark_drawn_cs`): as many as half the widest
    /// decor (`scatter::WIDEST`) spans, at least one, as its tile is one a
    /// point of it lies in.
    pub fn ring(&self) -> i32 {
        let across = TILE as f32 * libm::sqrtf(dot(self.across, self.across));
        let half = crate::scatter::WIDEST as f32 / 2.0;
        (libm::ceilf(half / across) as i32).max(1)
    }

    /// The way up the tallest decor (`scatter::TALLEST`) on the box, in
    /// tiles across and down, and the steps it takes a tile apart at most:
    /// a receiver standing on a tile reads the box up it, decor and the
    /// actors no taller (pxl-shader sun.rs `sun_reach_cs`). The box is
    /// orthographic, so it is the same from every tile.
    pub fn up(&self) -> ([f32; 2], u32) {
        let (low, top, tile) = (
            self.texel_at([0.0; 3]),
            self.texel_at([0.0, 0.0, crate::scatter::TALLEST as f32]),
            TILE as f32,
        );
        let way = [(top[0] - low[0]) / tile, (top[1] - low[1]) / tile];
        let long = libm::fabsf(way[0]).max(libm::fabsf(way[1]));
        (way, libm::ceilf(long) as u32)
    }

    /// The `k`-th step of the way `up`, whole tiles from the tile stood on,
    /// rounded down: a point that far up lies this many tiles over or one
    /// more, as the point stood on lies anywhere in its tile.
    pub fn up_at((way, steps): ([f32; 2], u32), k: u32) -> [i32; 2] {
        let f = if steps == 0 {
            0.0
        } else {
            k as f32 / steps as f32
        };
        [
            libm::floorf(way[0] * f) as i32,
            libm::floorf(way[1] * f) as i32,
        ]
    }

    /// Whether box texel `p` of the map `size` a side lies where a receiver
    /// may read it, grown by `pad` texels: within `rim` of the box's
    /// half-width about its middle, where the eye stands, as past it a
    /// shadow has faded to nothing (pxl-render shadow.wgsl `sun_lit`). The
    /// box is snapped by under a texel, which `READ_PAD` holds.
    pub fn read(&self, size: u32, p: [i32; 2], pad: f32) -> bool {
        let half = size as f32 / 2.0;
        let (x, y) = (p[0] as f32 + 0.5 - half, p[1] as f32 + 0.5 - half);
        let r = self.rim * half + pad;
        x * x + y * y <= r * r
    }

    /// `detail`, the eye's zooms at every point as the eye asks them
    /// (`cast::Detail::eye`), so the map holds the blocks the eye draws; its
    /// cone the map's texel, which passes what a texel cannot tell apart
    /// (`cast::Cone`), and its beam rays `BEAM` of them apart.
    pub fn detail(&self, mut detail: Detail) -> Detail {
        detail.cone = crate::cast::Cone {
            angle: 0.0,
            across: length(self.across),
        };
        detail.apart = BEAM as f32;
        detail
    }

    /// The box's texel a slot of the map holds, `size` a power of two its
    /// side: the slot's world texel, less the box's corner. The sun's raster
    /// (`SAMPLE`) is drawn in the box's order, as the map is laid, and a
    /// slot reads its pick at this texel: drawn in the slots' order, a box
    /// would split at the seam where the slots wrap.
    pub fn texel(&self, size: u32, slot: [u32; 2]) -> [u32; 2] {
        let at = |k: usize| (slot[k] as i32).wrapping_sub(self.corner[k]) as u32 & (size - 1);
        [at(0), at(1)]
    }

    /// The zoom the eye asks at a stop `q` of the box's depth along a
    /// texel's ray, `detail.eye` where the eye stands from its start
    /// (`ray`); none where it stopped at nothing.
    fn asked(&self, detail: Detail, q: u32) -> u32 {
        if q >= DEPTH_ONE {
            return NO_ZOOM;
        }
        let t = q as f32 / DEPTH_ONE as f32 * self.reach;
        let e = detail.eye;
        let v = [
            e[0] - self.dir[0] * t,
            e[1] - self.dir[1] * t,
            e[2] - self.dir[2] * t,
        ];
        detail.at(v[0] * v[0] + v[1] * v[1] + v[2] * v[2])
    }

    /// A slot's word for depth `d` (`depth`), the zoom asked at it under
    /// `detail`.
    pub fn word(&self, detail: Detail, d: f32) -> u32 {
        let q = (d * DEPTH_ONE as f32 + 0.5) as u32;
        let q = if q < DEPTH_ONE { q } else { DEPTH_ONE };
        q << ZOOM_BITS | self.asked(detail, q)
    }

    /// The sun its words lay (`words`).
    pub fn of_words(w: &[u32; crate::eye::SUN_WORDS]) -> Sun {
        let v = |k: usize| {
            [
                f32::from_bits(w[k]),
                f32::from_bits(w[k + 1]),
                f32::from_bits(w[k + 2]),
            ]
        };
        Sun {
            first: v(0),
            reach: f32::from_bits(w[3]),
            across: v(4),
            down: v(8),
            dir: v(12),
            bias: v(16),
            corner: CORNER.map(|k| w[k] as i32),
            rim: f32::from_bits(w[RIM]),
        }
    }

    /// Its words, as `eye::SUN` lays them.
    pub fn words(&self) -> [u32; crate::eye::SUN_WORDS] {
        let v = |a: [f32; 3], w: u32| [a[0].to_bits(), a[1].to_bits(), a[2].to_bits(), w];
        let c = [
            v(self.first, self.reach.to_bits()),
            v(self.across, self.corner[0] as u32),
            v(self.down, self.corner[1] as u32),
            v(self.dir, self.rim.to_bits()),
            v(self.bias, 0),
        ];
        let mut out = [0; crate::eye::SUN_WORDS];
        let mut k = 0;
        while k < out.len() {
            out[k] = c[k / 4][k % 4];
            k += 1;
        }
        out
    }
}

/// What the sun's cast is handed beside the eye's words, a storage buffer: a
/// head of `MOVED_HEAD` vec4s, whether the cast is counted
/// (`MOVED_COUNTED`), then the rays' starts (`Starts`, from `STARTS_AT`),
/// then the actors (`ACTORS_AT`).
pub const MOVED_HEAD: usize = ACTORS_AT + 1 + ACTORS;
pub const MOVED_WORDS: usize = 4 * MOVED_HEAD;
/// The cast's number, counted up from one each cast, which a tile stood on
/// and read in it is marked with (`stands_at`, `reads_at`).
pub const MOVED_CAST: usize = 0;
pub const MOVED_COUNTED: usize = 1;
/// The number of the cast since which only what says which tiles are read
/// moved: what a cast in it or after left in a tile stands, as the eye's
/// place, the light and the world it was cast for still do (`reads_at`).
pub const MOVED_EPOCH: usize = 2;

/// Where a texel's ray starts, the same point of the world in every frame
/// cast in one light frame (pxl-render gpu/shadow.rs `aim`): off anchors
/// `ANCHOR` texels apart on the world's texel grid, each a start on the near
/// plane worked out once on the host, its Q3 position and what rounding to
/// it left (`frac`), and a texel's step across and down in Q3 steps, so a
/// start is the anchor's and the steps from it rounded, an f32 sum of the
/// same numbers each frame (`start`). Worked out off the eye each frame
/// instead, its float's rounding moved as the eye did, and a texel's depth
/// a Q3 step with it: an edge's shadow stirred as the eye moved. `base` is the world texel index of
/// anchor `at[0]`; `at[1]` is the next across, `at[2]` the next down,
/// `at[3]` both, the box lying in the four (`ANCHOR` its side at least).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Starts {
    pub base: [i32; 2],
    pub across: [f32; 3],
    pub down: [f32; 3],
    pub at: [[i32; 3]; 4],
    pub frac: [[f32; 3]; 4],
}

pub const ANCHOR: i32 = 2048;

/// The starts' words in the moved nodes' buffer's head (`MOVED_HEAD`), a
/// vec4 each from `STARTS_AT`: the base, the steps across and down, the
/// anchors, their remainders.
pub const STARTS_AT: usize = 2;
pub const STARTS_VEC4S: usize = 11;

/// The actors the host draws, receivers of the sun's map as decor is
/// (pxl-shader sun.rs `sun_mark_drawn_cs`), past the starts: how many, the
/// first word of a vec4, then each where it stands, units from the eye, a
/// vec4 each, as many as `ACTORS`; any past them marks nothing.
pub const ACTORS_AT: usize = STARTS_AT + STARTS_VEC4S;
pub const ACTORS: usize = 32;

impl Starts {
    /// The start of the ray of world texel `w`, X across and Y down the
    /// rows: its anchor's (`base` and `ANCHOR`) and the steps from it.
    pub fn start(&self, w: [i32; 2]) -> [i32; 3] {
        let (dx, dy) = (
            w[0].wrapping_sub(self.base[0]),
            w[1].wrapping_sub(self.base[1]),
        );
        let (kx, ky) = (dx >= ANCHOR, dy >= ANCHOR);
        let (i, j) = (
            (dx - if kx { ANCHOR } else { 0 }) as f32,
            (dy - if ky { ANCHOR } else { 0 }) as f32,
        );
        // [naga] each anchor by name, not an array a dynamic index reads.
        let (at, frac) = match (kx, ky) {
            (false, false) => (self.at[0], self.frac[0]),
            (true, false) => (self.at[1], self.frac[1]),
            (false, true) => (self.at[2], self.frac[2]),
            (true, true) => (self.at[3], self.frac[3]),
        };
        let step = |k: usize| libm::rintf(frac[k] + i * self.across[k] + j * self.down[k]) as i32;
        // [naga] each axis by itself, not a loop (cave.rs `node_seed`).
        [
            at[0].wrapping_add(step(0)),
            at[1].wrapping_add(step(1)),
            at[2].wrapping_add(step(2)),
        ]
    }

    /// Its words (`STARTS_VEC4S` vec4s), as `of_words` reads them.
    pub fn words(&self) -> [u32; 4 * STARTS_VEC4S] {
        let f = |v: [f32; 3]| [v[0].to_bits(), v[1].to_bits(), v[2].to_bits(), 0];
        let i = |v: [i32; 3]| [v[0] as u32, v[1] as u32, v[2] as u32, 0];
        let rows = [
            [self.base[0] as u32, self.base[1] as u32, 0, 0],
            f(self.across),
            f(self.down),
            i(self.at[0]),
            i(self.at[1]),
            i(self.at[2]),
            i(self.at[3]),
            f(self.frac[0]),
            f(self.frac[1]),
            f(self.frac[2]),
            f(self.frac[3]),
        ];
        let mut out = [0; 4 * STARTS_VEC4S];
        let mut k = 0;
        while k < out.len() {
            out[k] = rows[k / 4][k % 4];
            k += 1;
        }
        out
    }

    /// The starts its vec4s `w` lay, `w(k)` the `k`-th, as `words` lays
    /// them.
    pub fn of_words(w: impl Fn(usize) -> [i32; 4]) -> Starts {
        let f = |v: [i32; 4]| {
            [
                f32::from_bits(v[0] as u32),
                f32::from_bits(v[1] as u32),
                f32::from_bits(v[2] as u32),
            ]
        };
        let i = |v: [i32; 4]| [v[0], v[1], v[2]];
        let b = w(0);
        Starts {
            base: [b[0], b[1]],
            across: f(w(1)),
            down: f(w(2)),
            at: [i(w(3)), i(w(4)), i(w(5)), i(w(6))],
            frac: [f(w(7)), f(w(8)), f(w(9)), f(w(10))],
        }
    }
}

/// The clipmap's tiles, `TILE` slots a side, a workgroup each of the
/// slots' pass (pxl-shader `sun_cs`), dispatched over the whole map.
pub const TILE: u32 = 8;

/// The sun's beam (pxl-shader sun.rs `sun_beam_cs`, `sun_cell_cs`; doc/minutes/20261022-a-cell-casts-a-beam.md,
/// the eye's): a beam ray at every `BEAM`-th texel of the box across and
/// down, `BEAM_WINDOW` past it on each side, each walked from the world's
/// root along the sun's rays until a block that fills or lacks or a split
/// narrower than `2√2` beam spacings (`Sun::detail`, `cast::Detail::apart`), leaving its
/// line and how far it got less `margin`; each `BEAM`-square cell of the
/// box starts at the least of the beam rays within `BEAM_WINDOW` of its
/// corners, and its texels resume from that ray's line there.
///
/// Why that is a texel's root walk, as the eye's proof has it with the
/// rays parallel: take `X`, the block a texel's ray ends at, `e` its entry,
/// and `Y` the smallest cube holding `e` at least `√2` beam spacings wide.
/// Along the sun `Y` covers a disk half its side across, which holds a beam
/// ray; a beam ray through `Y` stops where it enters `Y` or sooner, as `Y`
/// is a split narrower than `2√2` spacings or lies in `X`; two parallel
/// rays through `Y` enter it no more than its diagonal, under `2√6`
/// spacings, apart in distance from the near plane both start on; and that
/// beam ray lies no more than that diagonal across from the texel's, so
/// within `BEAM_WINDOW` spacings of its cell's corners.
pub const BEAM: u32 = TILE;
pub const BEAM_WINDOW: u32 = 5;
const _: () = assert!(
    BEAM_WINDOW as f32 >= 4.899,
    "a beam ray through `Y` lies within 2√6 spacings"
);

/// What a beam ray leaves, in words: the eye's beam rays' line
/// (`beam::LINE_T`, `beam::LINE_NODE`, `beam::LINE_AT`), how far a cell's
/// texels may start from it in its first word, then its origin
/// (`LINE_ORIGIN`), which its nodes are held from (`cast::Node::moved`), as
/// every sun ray starts apart.
pub const LINE_ORIGIN: u32 = crate::beam::LINE_WORDS;
pub const LINE_WORDS: u32 = LINE_ORIGIN + 3;
/// What a cell hands its texels: their start, f32 bits, and the beam ray
/// whose line they resume from, by its index (`line`).
pub const CELL_T: u32 = 0;
pub const CELL_RAY: u32 = 1;
pub const CELL_WORDS: u32 = 2;

/// The beam rays across the map `size` texels a side, and down.
pub const fn beam_across(size: u32) -> u32 {
    size / BEAM + 1 + 2 * BEAM_WINDOW
}

/// The cells across the map, and down.
pub const fn cells_across(size: u32) -> u32 {
    size / BEAM
}

/// Where the beam's words lie in the list's buffer, its first: the lines,
/// a beam ray each, then the cells' starts; and all its words.
pub const fn lines_at(_size: u32) -> u32 {
    0
}

pub const fn line(size: u32, ray: u32) -> u32 {
    lines_at(size) + ray * LINE_WORDS
}

pub const fn cells_at(size: u32) -> u32 {
    lines_at(size) + beam_across(size) * beam_across(size) * LINE_WORDS
}

pub const fn cell(size: u32, [i, j]: [u32; 2]) -> u32 {
    cells_at(size) + (j * cells_across(size) + i) * CELL_WORDS
}

pub const fn beam_words(size: u32) -> u32 {
    cells_at(size) + cells_across(size) * cells_across(size) * CELL_WORDS
}

/// How far short of where a beam ray stopped a cell's texels start: the
/// diagonal of a cube `2√2` beam spacings `width` units wide, `2√6 ·
/// width`, and a Q3 step's rounding of each ray's start on the near plane,
/// two.
pub fn margin(width: f32) -> f32 {
    2.0 * libm::sqrtf(6.0) * width + 2.0 / (1u32 << FRAC_W) as f32
}

/// The sun's map as the eye's rays are cast, every slot every frame
/// (pxl-spec-core `raster`): a sample every `SAMPLE` slots across and down
/// walked (pxl-shader `sun_sample_cs`), the blocks they stopped at written
/// as boxes by the eye's writer (pxl-shader raster.rs `boxed`) and drawn
/// from the sun by the eye's raster, orthographic (pxl-render raster.wgsl),
/// in the box's order (`Sun::texel`), each texel keeping the nearest its own
/// ray enters; and a slot whose ray enters its pick, tested again with its
/// own ray as the eye's pixels test theirs (`cast::entry`), takes it
/// (`sun_cs`); the rest are listed and walked apart (`misses_at`). A block
/// no sample stopped at may stand in front of the one picked, as in the
/// eye's. The raster's `EYED` case has no
/// sun's: every ray starts on the near plane, before every box.
///
/// The samples' spacing; their words, the eye's samples' own
/// (`beam::SAMPLE_WORDS`), past the beam's in the list's buffer
/// (`samples_at`), a sample `size / SAMPLE` across and down, by its index
/// `j · across + i`, which its box is drawn with (`raster::id_of`).
pub const SAMPLE: u32 = 4;

pub const fn samples_at(size: u32) -> u32 {
    beam_words(size)
}

pub const fn sample(size: u32, index: u32) -> u32 {
    samples_at(size) + index * crate::beam::SAMPLE_WORDS
}

pub const fn sample_words(size: u32) -> u32 {
    sample(size, (size / SAMPLE) * (size / SAMPLE))
}

/// The slots whose ray took no pick, listed for their walk (pxl-shader
/// `sun_miss_cs`) apart from the slots that took theirs (`sun_cs`), as the
/// eye's missed pixels are walked apart from the pixels that took their
/// pick, by the same listing and dispatch (pxl-shader cast.rs `listed`,
/// `ray::MISS_ARGS_WORDS`): a walk holds its workgroup as long as its
/// slowest lane. Past the samples: how many, copied in once the slots are
/// done, as the eye's count is (`observed::MARKS_COUNT`), then a slot each,
/// its X and its Y over 16 bits.
pub const fn misses_count(size: u32) -> u32 {
    sample_words(size)
}

pub const fn misses_at(size: u32) -> u32 {
    misses_count(size) + 1
}

/// The tiles of the box a receiver stands on in a cast, past the misses: a
/// word a tile (`TILE` texels a side), `tiles` a row, the number of the
/// last cast a receiver stood on it in (`MOVED_CAST`; pxl-shader sun.rs
/// `sun_mark_cs`, `sun_mark_drawn_cs`): where the eye's pixels meet the
/// world, and where the decor and the actors drawn stand.
pub const fn stands_at(size: u32) -> u32 {
    misses_at(size) + size * size
}

/// The tiles a receiver reads, past the stands, as many: the number of the
/// cast that first read one in its epoch (`MOVED_EPOCH`), read where it lies
/// within its taps of the way up from a tile one stands on (`Sun::up`;
/// pxl-shader sun.rs `sun_reach_cs`). A tile this cast's number names is
/// the cast's to cast; one an earlier cast of its epoch names holds what
/// that cast left; no receiver reads any other.
pub const fn reads_at(size: u32) -> u32 {
    stands_at(size) + tiles(size) * tiles(size)
}

pub const fn tiles(size: u32) -> u32 {
    size / TILE
}

/// The drawn pixels a thread of the mark reads, a side (pxl-shader sun.rs
/// `sun_mark_cs`): every pixel is read, as a run of them can span many
/// tiles at an edge or a grazing face, and a run on one tile marks it once.
pub const MARK: u32 = 4;

/// The reads by the square of `COARSE` tiles a side, past them, `coarse`
/// a row: the cast's number where a tile in the square is the cast's to
/// cast, read in place of the tiles where a span of them is asked
/// (pxl-shader sun.rs `Map::needs`).
pub const COARSE: u32 = 8;

pub const fn coarse(size: u32) -> u32 {
    tiles(size) / COARSE
}

pub const fn coarse_at(size: u32) -> u32 {
    reads_at(size) + tiles(size) * tiles(size)
}

/// The word of the square tile `[i, j]` lies in, among the squares'
/// (`coarse_at`); the map's tiles a whole number of squares a side.
pub const fn coarse_square(size: u32, [i, j]: [u32; 2]) -> u32 {
    coarse_at(size) + (j / COARSE) * coarse(size) + i / COARSE
}

/// The list's buffer's words: the beam's, the samples, the misses, the
/// stands, the reads, their squares.
pub const fn list_words(size: u32) -> u32 {
    coarse_at(size) + coarse(size) * coarse(size)
}

/// Box texel `p`'s ray: its origin, where its world texel's middle lies on
/// the near plane, rounded to a Q3 step as its anchor lays it (`Starts`);
/// the ray; and where the eye, at Q3 `eye`, stands from that origin, in
/// units, for the zooms it asks (`cast::Detail::eye`).
pub fn ray(eye: [i32; 3], s: &Sun, starts: &Starts, p: [u32; 2]) -> ([i32; 3], Ray, [f32; 3]) {
    let o = starts.start([
        s.corner[0].wrapping_add(p[0] as i32),
        s.corner[1].wrapping_add(p[1] as i32),
    ]);
    let step = (1u32 << FRAC_W) as f32;
    // The eye from the start, X the short way round as an i32 wraps.
    let from = |k: usize| eye[k].wrapping_sub(o[k]) as f32 / step;
    (o, Ray::new(s.dir, s.reach), [from(0), from(1), from(2)])
}
/// The depth a texel's walk `met` leaves in the map: where it stopped
/// along its ray, as a share of the box's depth, pushed back as its face's
/// axis is (`Sun::bias`); the far plane, 1, where it stopped at nothing.
pub fn depth<A>(s: &Sun, met: &Met<A>) -> f32 {
    if met.ends != Ends::Stop {
        return 1.0;
    }
    let d = met.stop.t / s.reach + axis(s.bias, met.stop.axis);
    if d < 1.0 {
        d
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Starts as a host lays them for the eye at Q3 `eye`, off the sun's
    /// words alone: each anchor where its texel lies off the eye, rounded.
    fn starts_of(eye: [i32; 3], s: &Sun) -> Starts {
        let base = [
            s.corner[0].div_euclid(ANCHOR) * ANCHOR,
            s.corner[1].div_euclid(ANCHOR) * ANCHOR,
        ];
        let q3 = (1u32 << FRAC_W) as f32;
        let anchor = |k: usize| {
            let w = [
                base[0] + (k % 2) as i32 * ANCHOR,
                base[1] + (k / 2) as i32 * ANCHOR,
            ];
            let (i, j) = ((w[0] - s.corner[0]) as f32, (w[1] - s.corner[1]) as f32);
            let v = |a: usize| (s.first[a] + i * s.across[a] + j * s.down[a]) * q3;
            let r = |a: usize| libm::rintf(v(a));
            (
                [0, 1, 2].map(|a| eye[a].wrapping_add(r(a) as i32)),
                [0, 1, 2].map(|a| v(a) - r(a)),
            )
        };
        let [a0, a1, a2, a3] = [0, 1, 2, 3].map(anchor);
        Starts {
            base,
            across: s.across.map(|v| v * q3),
            down: s.down.map(|v| v * q3),
            at: [a0.0, a1.0, a2.0, a3.0],
            frac: [a0.1, a1.1, a2.1, a3.1],
        }
    }

    /// The starts' words read back are the starts.
    #[test]
    fn starts_are_their_words() {
        let s = down();
        let st = starts_of([7, -9, 11], &s);
        let w = st.words();
        let back = Starts::of_words(|k| [0, 1, 2, 3].map(|j| w[4 * k + j] as i32));
        assert_eq!(back, st);
    }

    /// A box 4 units a side and 8 deep, looking straight down from 4 up,
    /// two texels a side.
    fn down() -> Sun {
        Sun::of(
            [
                [2.0, 0.0, 0.0],
                [0.0, 2.0, 0.0],
                [0.0, 0.0, -8.0],
                [0.0, 0.0, 4.0],
            ],
            2,
            [0, 0],
            1.0,
        )
    }

    /// A walk that stopped `t` along its ray on a face across `axis`, or went on.
    fn met(ends: Ends, t: f32, axis: u32) -> Met<()> {
        let node = crate::cast::Node::ROOT;
        Met {
            ends,
            stop: crate::cast::Stop {
                t,
                axis,
                node,
                at: (),
                block: [0; 2],
            },
            visits: 0,
        }
    }

    #[test]
    fn a_texel_starts_where_it_lies_on_the_near_plane() {
        let s = down();
        let (o, ray, from) = ray([100, 200, 300], &s, &starts_of([100, 200, 300], &s), [0, 0]);
        // The top-left texel's middle: half the box across to the left, up.
        assert_eq!(o, [100 - 256, 200 + 256, 300 + 4 * 256]);
        assert_eq!(ray.direction, [0.0, 0.0, -1.0]);
        assert_eq!(ray.reach, 8.0);
        assert_eq!(from, [1.0, -1.0, -4.0]);
        // The next across and the next down, a texel's side on.
        assert_eq!(
            super::ray([0; 3], &s, &starts_of([0; 3], &s), [1, 1]).0,
            [256, -256, 4 * 256]
        );
    }

    #[test]
    fn a_face_square_to_the_sun_takes_no_slope() {
        let s = down();
        assert_eq!(depth(&s, &met(Ends::Stop, 2.0, 2)), 0.25);
        assert_eq!(depth(&s, &met(Ends::On, 8.0, 0)), 1.0);
        assert_eq!(depth(&s, &met(Ends::Lacks, 2.0, 2)), 1.0);
    }

    #[test]
    fn a_leaning_face_is_pushed_back_by_its_slope() {
        // The sun 45 degrees over x: a top face changes a texel's side in
        // depth across a texel, which is pushed back `SLOPE_BIAS` times.
        let r = core::f32::consts::FRAC_1_SQRT_2;
        let s = Sun::of(
            [
                [2.0 * r, 0.0, 2.0 * r],
                [0.0, 2.0, 0.0],
                [8.0 * r, 0.0, -8.0 * r],
                [0.0, 0.0, 4.0],
            ],
            2048,
            [0, 0],
            1.0,
        );
        let texel = 4.0 / 2048.0;
        let d = depth(&s, &met(Ends::Stop, 2.0, 2));
        assert!(
            (d - (0.25 + crate::eye::SLOPE_BIAS * texel / 8.0)).abs() < 1e-6,
            "{d}"
        );
    }

    #[test]
    fn a_plane_keeps_its_depth_along_itself() {
        // The same box: a top face's depth deepens a texel's side a texel
        // across, toward the sun's going, and not at all down.
        let r = core::f32::consts::FRAC_1_SQRT_2;
        let (across, down, deep) = (
            [4.0 * r / 2048.0, 0.0, 4.0 * r / 2048.0],
            [0.0, -4.0 / 2048.0, 0.0],
            [8.0 * r, 0.0, -8.0 * r],
        );
        let [i, j] = slope([0.0, 0.0, 1.0], across, down, deep);
        assert!((i - 4.0 / 2048.0 / 8.0).abs() < 1e-9 && j == 0.0, "{i} {j}");
        // A step along any plane, back down its slope, is on it again: any
        // normal, as decor's and the actors' are.
        let n = [0.3, -0.5, 0.81];
        let [i, j] = slope(n, across, down, deep);
        let step = [
            across[0] - down[0] * 2.0 + deep[0] * (i - 2.0 * j),
            across[1] - down[1] * 2.0 + deep[1] * (i - 2.0 * j),
            across[2] - down[2] * 2.0 + deep[2] * (i - 2.0 * j),
        ];
        assert!(dot(n, step).abs() < 1e-7, "{}", dot(n, step));
        // A face along the rays has none.
        assert_eq!(slope([r, 0.0, r], across, down, deep), [0.0, 0.0]);
    }

    #[test]
    fn its_words_are_its_fields_a_vec4_each() {
        let s = Sun {
            corner: [-7, 9],
            rim: 0.75,
            ..down()
        };
        let w = s.words();
        assert_eq!(CORNER.map(|k| w[k] as i32), s.corner);
        assert_eq!(f32::from_bits(w[RIM]), s.rim);
        assert_eq!(f32::from_bits(w[3]), s.reach);
        assert_eq!(f32::from_bits(w[4]), s.across[0]);
        assert_eq!(f32::from_bits(w[18]), s.bias[2]);
        assert_eq!(Sun::of_words(&w), s);
    }

    /// A point lies on the texel whose ray starts above it.
    #[test]
    fn a_point_lies_on_the_texel_over_it() {
        let s = down();
        let (_, _, from) = ray([0; 3], &s, &starts_of([0; 3], &s), [1, 0]);
        // Texel (1, 0)'s start, from the eye, and a point straight under it.
        let at = s.texel_at([-from[0], -from[1], -from[2] - 3.0]);
        assert!((at[0] - 1.0).abs() < 1e-5 && at[1].abs() < 1e-5, "{at:?}");
    }

    /// A point along a texel's ray lies on that texel, whatever the sun's
    /// slant, the box's corner and its anchors' rounding.
    #[test]
    fn a_point_on_a_texels_ray_lies_on_it() {
        let r = core::f32::consts::FRAC_1_SQRT_2;
        let s = Sun::of(
            [
                [2.0 * r, 0.0, 2.0 * r],
                [0.0, 2.0, 0.0],
                [8.0 * r, 0.0, -8.0 * r],
                [0.3, -0.2, 4.0],
            ],
            64,
            [-37, 91],
            1.0,
        );
        let eye = [1_234_567, -7_654_321, 98_765];
        let st = starts_of(eye, &s);
        let q3 = (1u32 << FRAC_W) as f32;
        for p in [[0, 0], [5, 17], [63, 40]] {
            let (o, ray, _) = super::ray(eye, &s, &st, p);
            let v = |k: usize, t: f32| o[k].wrapping_sub(eye[k]) as f32 / q3 + ray.direction[k] * t;
            let at = s.texel_at([v(0, 3.0), v(1, 3.0), v(2, 3.0)]);
            assert!(
                // The starts' Q3 rounding, far under a tile.
                (at[0] - p[0] as f32).abs() < 0.05 && (at[1] - p[1] as f32).abs() < 0.05,
                "{p:?} {at:?}"
            );
        }
    }

    /// The way up runs a tile a step, along the sun's slant across the box.
    #[test]
    fn the_way_up_runs_a_tile_a_step() {
        // The sun 45 degrees over x, a box 4 units wide in 16 texels: a
        // unit up moves a point a texel's worth of units across, 4 texels.
        let r = core::f32::consts::FRAC_1_SQRT_2;
        let s = Sun::of(
            [
                [2.0 * r, 0.0, 2.0 * r],
                [0.0, 2.0, 0.0],
                [8.0 * r, 0.0, -8.0 * r],
                [0.0, 0.0, 4.0],
            ],
            16,
            [0, 0],
            1.0,
        );
        let up = s.up();
        let rise = crate::scatter::TALLEST as f32 * 4.0 / TILE as f32 * r;
        assert!(
            (up.0[0] - rise).abs() < 1e-4 && up.0[1].abs() < 1e-6,
            "{up:?}"
        );
        assert_eq!(up.1, libm::ceilf(rise) as u32);
        assert_eq!(Sun::up_at(up, 0), [0, 0]);
        assert_eq!(Sun::up_at(up, up.1), [libm::floorf(rise) as i32, 0]);
        // A point on the near plane under texel 9's middle stands on tile 1.
        assert_eq!(s.tile_of([0.0; 3])[1], s.tile_of([0.0, 0.0, -1.0])[1]);
    }

    /// Every texel a receiver's taps read, at any height up to the tallest
    /// decor over a point stood on, lies in a tile the reach reads from the
    /// tile stood on (pxl-shader sun.rs `sun_reach_cs`: a tile `t` is read
    /// where one stood on lies at `t − up_at(k) + [−2, 1]²`), whatever the
    /// sun's slant.
    #[test]
    fn the_reach_reads_every_tile_a_receiver_up_the_way_reads() {
        for (sx, sz) in [
            (0.0f32, 1.0f32),
            (0.3, 0.95),
            (0.7, 0.7),
            (0.95, 0.31),
            (-0.9, 0.44),
        ] {
            let n = libm::sqrtf(sx * sx + sz * sz);
            let dir = [sx / n, 0.0, -sz / n];
            // The box's axes square to the rays: across in x and z, down y.
            let across = [sz / n * 2.0, 0.0, sx / n * 2.0];
            let s = Sun::of(
                [
                    across,
                    [0.0, 2.0, 0.0],
                    dir.map(|v| v * 8.0),
                    [0.0, 0.0, 4.0],
                ],
                64,
                [0, 0],
                1.0,
            );
            let up = s.up();
            let texel = libm::sqrtf(dot(s.across, s.across));
            for base in [[0.1, 0.2, -3.0], [0.37, -0.81, -2.5], [-0.66, 0.45, -3.3]] {
                let stood = s.tile_of(base);
                for h in [0.0, 0.7, 1.9, 3.1, crate::scatter::TALLEST as f32] {
                    for (a, b) in [(0.0, 0.0), (3.5, 0.0), (-3.5, 0.0), (0.0, 3.5), (2.5, -2.5)] {
                        // A tap `(a, b)` texels off the point `h` up.
                        let p = [
                            base[0] + s.across[0] / texel * a * texel,
                            base[1] + s.down[1] / texel * b * texel,
                            base[2] + h + s.across[2] / texel * a * texel,
                        ];
                        let t = s.tile_of(p);
                        let reached = (0..=up.1).any(|k| {
                            let o = Sun::up_at(up, k);
                            let d = [t[0] - o[0] - stood[0], t[1] - o[1] - stood[1]];
                            (-1..=2).contains(&d[0]) && (-1..=2).contains(&d[1])
                        });
                        assert!(reached, "sun {sx} {sz} base {base:?} h {h} tap {a} {b}");
                    }
                }
            }
        }
    }

    /// A receiver reads the disk of `rim` about the box's middle, and a
    /// pass past it its pad.
    #[test]
    fn a_receiver_reads_the_disk_within_the_rim() {
        let s = Sun { rim: 0.5, ..down() };
        // A box 16 texels a side: its middle is 8, the disk 4 texels out.
        assert!(s.read(16, [8, 8], 0.0));
        assert!(s.read(16, [11, 7], 0.0));
        assert!(!s.read(16, [13, 7], 0.0));
        assert!(s.read(16, [13, 7], 2.0));
        assert!(!s.read(16, [0, 0], 2.0));
    }

    #[test]
    fn a_slot_holds_its_world_texel_wherever_the_box_lies() {
        let mut s = Sun::of(
            [
                [2.0, 0.0, 0.0],
                [0.0, 2.0, 0.0],
                [0.0, 0.0, -8.0],
                [0.0, 0.0, 4.0],
            ],
            8,
            [-3, 5],
            1.0,
        );
        // World texel 0 across, -3 + 3, lies in slot 0; 5 down, in slot 5.
        assert_eq!(s.texel(8, [0, 5]), [3, 0]);
        // The box moved a texel on across: the slot holds the next texel.
        s.corner[0] += 1;
        assert_eq!(s.texel(8, [0, 5]), [2, 0]);
    }
}
