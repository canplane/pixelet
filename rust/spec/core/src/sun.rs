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
/// `corner`, X then Y, Y counted down the rows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sun {
    pub first: [f32; 3],
    pub reach: f32,
    pub across: [f32; 3],
    pub down: [f32; 3],
    pub dir: [f32; 3],
    pub bias: [f32; 3],
    pub corner: [i32; 2],
}

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
    /// `corner` is the world texel index of the box's first texel.
    pub fn of([x, y, z, w]: [[f32; 3]; 4], size: u32, corner: [i32; 2]) -> Sun {
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
        }
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
        }
    }

    /// Its words, as `eye::SUN` lays them.
    pub fn words(&self) -> [u32; crate::eye::SUN_WORDS] {
        let v = |a: [f32; 3], w: u32| [a[0].to_bits(), a[1].to_bits(), a[2].to_bits(), w];
        let c = [
            v(self.first, self.reach.to_bits()),
            v(self.across, self.corner[0] as u32),
            v(self.down, self.corner[1] as u32),
            v(self.dir, 0),
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

/// A slot's depth (`Sun::word`): a share of the box's depth, the stop's
/// own pushed back (`depth`), so no nearer than it.
pub fn depth_of(word: u32) -> f32 {
    (word >> ZOOM_BITS) as f32 / DEPTH_ONE as f32
}

/// What the sun's cast is handed beside the eye's words, a storage buffer: a
/// head of `MOVED_HEAD` vec4s, whether the cast is counted
/// (`MOVED_COUNTED`), then the rays' starts (`Starts`, from `STARTS_AT`).
pub const MOVED_HEAD: usize = STARTS_AT + STARTS_VEC4S;
pub const MOVED_WORDS: usize = 4 * MOVED_HEAD;
pub const MOVED_COUNTED: usize = 1;

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
        // [naga] each axis by itself, not a loop (caves.rs `node_seed`).
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

/// The sun's beam (pxl-shader sun.rs `sun_beam_cs`, `sun_cells_cs`; doc/minutes/20261022-a-cell-casts-a-beam.md,
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

/// The list's buffer's words: the beam's, the samples, the misses.
pub const fn list_words(size: u32) -> u32 {
    misses_at(size) + size * size
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
            ..down()
        };
        let w = s.words();
        assert_eq!(CORNER.map(|k| w[k] as i32), s.corner);
        assert_eq!(f32::from_bits(w[3]), s.reach);
        assert_eq!(f32::from_bits(w[4]), s.across[0]);
        assert_eq!(f32::from_bits(w[18]), s.bias[2]);
        assert_eq!(Sun::of_words(&w), s);
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
        );
        // World texel 0 across, -3 + 3, lies in slot 0; 5 down, in slot 5.
        assert_eq!(s.texel(8, [0, 5]), [3, 0]);
        // The box moved a texel on across: the slot holds the next texel.
        s.corner[0] += 1;
        assert_eq!(s.texel(8, [0, 5]), [2, 0]);
    }
}
