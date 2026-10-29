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
//! slot its index names modulo the map's side, so the box follows the eye
//! a texel at a time and no held texel moves. A slot holds its depth and
//! the zoom its stop was asked at (`Sun::word`), and is cast again only
//! where its world texel was outside the box last cast (`Sun::kept`), the
//! eye's zoom at its stop has changed (`Sun::stands`), or its ray passes
//! over a node the world's query answers otherwise since (`over`): what a
//! walk established is never walked again.

use crate::beam;
use crate::cast;
use crate::cast::Spacing;
use crate::cast::{Detail, End, Met, Ray};
use crate::eye;
use crate::param::sun;
use crate::param::sun::{SAMPLE, SPREAD};
use crate::scatter;
use crate::{axis, FRAC_W};
use core::f32::consts::SQRT_2;

/// The sun's cast as the eye's words hold it (`eye::SUN`), a frame's
/// constants worked out once off the light box (`Sun::of`), each a vec4's
/// first three words: where texel (0, 0)'s ray starts, from the eye, in
/// units, and how far every ray reaches (the fourth word); the step to the
/// next texel across and down; the rays' one direction; and how far a stop
/// on a face across each axis is pushed back in depth. Their fourth words
/// hold the clipmap's: the world texel index of the box's texel (0, 0),
/// `corner`, X then Y, Y counted down the rows; and the corner the map was
/// last cast at, `kept`, which a box no texel of this one's lies in names
/// where nothing is kept (`Sun::none`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sun {
    pub first: [f32; 3],
    pub reach: f32,
    pub across: [f32; 3],
    pub down: [f32; 3],
    pub dir: [f32; 3],
    pub bias: [f32; 3],
    pub corner: [i32; 2],
    pub kept: [i32; 2],
}

/// The word of `eye::SUN` the rim lies in.
pub const RIM: usize = 15;

/// The words of `eye::SUN` the corner lies in, X then Y, which the map is
/// laid in off; and the kept corner's, which the renderer writes as it
/// decides what the map keeps.
pub const CORNER: [usize; 2] = [7, 11];
pub const KEPT: [usize; 2] = [15, 19];

/// A slot's word: its depth, a 24-bit share of the box's depth as a
/// depth24 map holds it, over the zoom its stop was asked at.
pub const DEPTH_ONE: u32 = (1 << 24) - 1;
pub const ZOOM_BITS: u32 = 8;
/// A slot whose walk stopped at nothing: no zoom asked, so no eye's move
/// casts it again.
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
/// `param::sun::SLOPE_BIAS` (`Sun::of`), and a receiver's taps each compare its
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
    /// caster's polygon offset (`param::sun::SLOPE_BIAS`): the face's greatest
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
            sun::SLOPE_BIAS * if i > j { i } else { j }
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
            kept: Sun::none(corner, size),
        }
    }

    /// The finest zoom its rays ask the world at: the finest whose block
    /// spans `BLOCK_SAMPLES` samples (`SAMPLE`) of the map. The eye's blocks
    /// span several of its samples, and the raster hands a block to the
    /// pixels about the samples that met it; the sun's texel follows no
    /// zoom, and asked as finely as the eye asks near it, a block was a
    /// texel or less, met by no sample about most texels.
    pub fn finest(&self) -> Zoom {
        let side = BLOCK_SAMPLES as f32 * SAMPLE as f32 * length(self.across);
        let steps = side * (1u32 << FRAC_W) as f32;
        // The zoom whose side, `2^(FINEST_ZOOM - z)` steps, is `steps` or more.
        let mut z = FINEST_ZOOM;
        let mut side = 1.0f32;
        while z > 0 && side < steps {
            z -= 1;
            side *= 2.0;
        }
        z
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
        let half = scatter::WIDEST as f32 / 2.0;
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
            self.texel_at([0.0, 0.0, scatter::TALLEST as f32]),
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
    /// shadow has faded to nothing (pxl-render shadow.wgsl `sun_lit`).
    pub fn read(&self, size: u32, p: [i32; 2], pad: f32) -> bool {
        let half = size as f32 / 2.0;
        let (x, y) = (p[0] as f32 + 0.5 - half, p[1] as f32 + 0.5 - half);
        let r = self.rim * half + pad;
        x * x + y * y <= r * r
    }

    /// `detail`, the eye's zooms at every point as the eye asks them
    /// (`cast::Detail::eye`), so the map holds the cells the eye draws; its
    /// beam rays `BEAM` of the map's texels apart (`cast::Spacing`).
    pub fn detail(&self, mut detail: Detail) -> Detail {
        detail.spacing = Spacing {
            angle: 0.0,
            across: BEAM as f32 * length(self.across),
        };
        detail
    }

    /// The box's texel a slot of the map holds, `size` a power of two its
    /// side: the slot's world texel, less the box's corner. The sun's raster
    /// (`SAMPLE`) is drawn in the box's order, as the map is laid, and a
    /// slot reads its visible sample at this texel: drawn in the slots' order,
    /// a box would split at the seam where the slots wrap.
    pub fn texel(&self, size: u32, slot: [u32; 2]) -> [u32; 2] {
        let at = |k: usize| (slot[k] as i32).wrapping_sub(self.corner[k]) as u32 & (size - 1);
        [at(0), at(1)]
    }

    /// Whether box texel `p`'s world texel lay in the box the map was last
    /// cast at, so its slot holds it.
    pub fn kept(&self, size: u32, p: [u32; 2]) -> bool {
        let within = |k: usize| {
            (self.corner[k]
                .wrapping_add(p[k] as i32)
                .wrapping_sub(self.kept[k]) as u32)
                < size
        };
        within(0) && within(1)
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

    /// Whether a kept slot's word stands under `detail`: the eye asks its
    /// stop at the zoom it was cast at, so a walk now would meet the same.
    pub fn stands(&self, detail: Detail, word: u32) -> bool {
        self.asked(detail, word >> ZOOM_BITS) == word & NO_ZOOM
    }

    /// The box texel of a tile's first slot on each axis (`TILE`), and
    /// whether the tile's slots hold a run of the box's texels from it:
    /// one that straddles the box's edge holds its two ends.
    pub fn tile_texel(&self, size: u32, tile: [u32; 2]) -> ([u32; 2], bool) {
        let p = self.texel(size, [tile[0] * TILE, tile[1] * TILE]);
        (p, p[0] <= size - TILE && p[1] <= size - TILE)
    }

    /// Whether every slot of a tile whose run starts at box texel `p`
    /// holds its world texel (`kept`): the run lies in the kept box on
    /// each axis.
    pub fn tile_kept(&self, size: u32, p: [u32; 2]) -> bool {
        let within = |k: usize| {
            (self.corner[k]
                .wrapping_add(p[k] as i32)
                .wrapping_sub(self.kept[k]) as u32)
                <= size - TILE
        };
        within(0) && within(1)
    }

    /// Whether every slot of a tile whose run starts at box texel `p` and
    /// whose words are `held` stands under `detail` (`stands`): the eye
    /// asks the zoom they were all cast at wherever in the box of their
    /// stops it looks, nearest to farthest. Held to the slots' own
    /// judgement, a stop lies off where its texel lies on the near plane by
    /// its start's rounding to the Q3 grid at most, a step each way.
    pub fn tile_stands(&self, detail: Detail, p: [u32; 2], held: Held) -> bool {
        if held.zoom == NO_ZOOM {
            return true;
        }
        if held.zoom == MIXED {
            return false;
        }
        let side = (TILE - 1) as f32;
        let (i, j) = (p[0] as f32, p[1] as f32);
        let t = [
            held.near as f32 / DEPTH_ONE as f32 * self.reach,
            held.far as f32 / DEPTH_ONE as f32 * self.reach,
        ];
        let pad = 1.0 / (1u32 << FRAC_W) as f32;
        // The stops' box along each of the box's own axes, through the eye:
        // how near and how far it lies along each, squared.
        let along = |axis: [f32; 3]| {
            let l = length(axis);
            let u = [axis[0] / l, axis[1] / l, axis[2] / l];
            let base = dot(self.first, u) + i * dot(self.across, u) + j * dot(self.down, u);
            let (a, b, w) = (
                side * dot(self.across, u),
                side * dot(self.down, u),
                dot(self.dir, u),
            );
            let (c, d) = (t[0] * w, t[1] * w);
            let lo = base + least(a) + least(b) + least(c - d) + d;
            let hi = base + most(a) + most(b) + most(c - d) + d;
            let slack = pad + (libm::fabsf(lo) + libm::fabsf(hi)) * 1e-5;
            let (lo, hi) = (lo - slack, hi + slack);
            let near = if lo > 0.0 {
                lo
            } else if hi < 0.0 {
                -hi
            } else {
                0.0
            };
            let far = if -lo > hi { -lo } else { hi };
            (near * near, far * far)
        };
        let (x, y, z) = (along(self.across), along(self.down), along(self.dir));
        let (near2, far2) = (x.0 + y.0 + z.0, x.1 + y.1 + z.1);
        detail.at(near2) == held.zoom && detail.at(far2) == held.zoom
    }
}

/// A slot whose stop the eye asked at another zoom than another slot's of
/// its tile (`Held`).
const MIXED: u32 = u32::MAX;

/// A step's share toward less, and toward more: a run's ends off its
/// start, the step taken or not.
fn least(d: f32) -> f32 {
    if d < 0.0 {
        d
    } else {
        0.0
    }
}

fn most(d: f32) -> f32 {
    if d > 0.0 {
        d
    } else {
        0.0
    }
}

/// What a tile's slots hold, off their words (`Sun::word`), as the mark
/// judges the tile whole (pxl-shader `sun_mark_cs`): the nearest and the
/// farthest stop, the zoom they were all asked at (none where no ray
/// stopped, `MIXED` where they differ), how deep the deepest ray reaches,
/// one that stopped at nothing to the far plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Held {
    pub near: u32,
    pub far: u32,
    pub zoom: u32,
    pub reach: u32,
}

impl Held {
    pub const NONE: Held = Held {
        near: DEPTH_ONE,
        far: 0,
        zoom: NO_ZOOM,
        reach: 0,
    };

    pub fn with(self, word: u32) -> Held {
        let (q, z) = (word >> ZOOM_BITS, word & NO_ZOOM);
        let reach = if q > self.reach { q } else { self.reach };
        if z == NO_ZOOM {
            return Held { reach, ..self };
        }
        Held {
            near: if q < self.near { q } else { self.near },
            far: if q > self.far { q } else { self.far },
            zoom: if self.zoom == NO_ZOOM || self.zoom == z {
                z
            } else {
                MIXED
            },
            reach,
        }
    }
}

impl Sun {
    /// A tile's rays as one, for whether any passes over a node
    /// (`over_any`): the ray of its run's first texel `p`, and how far the
    /// others' starts lie off its on the world's X and Y, least and most,
    /// a step's rounding wider.
    pub fn tile_rays(&self, eye: [i32; 3], p: [u32; 2]) -> ([i32; 3], [[f32; 2]; 2]) {
        let (o, _, _) = ray(eye, self, p);
        let side = (TILE - 1) as f32;
        let pad = 1.0 / (1u32 << FRAC_W) as f32;
        let off = |k: usize| {
            let (a, b) = (side * self.across[k], side * self.down[k]);
            [least(a) + least(b) - pad, most(a) + most(b) + pad]
        };
        (o, [off(0), off(1)])
    }

    /// The sun its words lay (`words`).
    pub fn of_words(w: &[u32; eye::SUN_WORDS]) -> Sun {
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
            kept: KEPT.map(|k| w[k] as i32),
        }
    }

    /// Its words, as `eye::SUN` lays them.
    pub fn words(&self) -> [u32; eye::SUN_WORDS] {
        let v = |a: [f32; 3], w: u32| [a[0].to_bits(), a[1].to_bits(), a[2].to_bits(), w];
        let c = [
            v(self.first, self.reach.to_bits()),
            v(self.across, self.corner[0] as u32),
            v(self.down, self.corner[1] as u32),
            v(self.dir, self.kept[0] as u32),
            v(self.bias, self.kept[1] as u32),
        ];
        let mut out = [0; eye::SUN_WORDS];
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
/// The map's side, which the slots' lattice is laid by. A pass the eye's
/// rays and the shadow map share is handed the eye's own head, all zero, in
/// its place (pxl-shader either.rs `Either`): a head naming no map is the
/// eye's.
pub const MOVED_SIZE: usize = 3;

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

/// The actors the host draws, receivers of the shadow map as decor is
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
pub const LIST_ACROSS: u32 = 256;
pub const LIST_ROWS: usize = 1;
pub const LIST_COUNT: usize = 3;
pub const LIST_AT: usize = 4;
/// The list's head as each frame starts it: no tile listed.
pub const LIST_EMPTY: [u32; LIST_AT] = [LIST_ACROSS, 0, 1, 0];

/// The sun's beam (pxl-shader beam.rs `beam_cs`, `cell_cs`, the eye's entries;
/// doc/minutes/20261022-a-cell-casts-a-beam.md): a beam ray at every `BEAM`-th
/// texel of the box across and down, `BEAM_WINDOW` past it on each side, each
/// walked from the world's root along the sun's rays until a cell that fills or
/// lacks or a split narrower than `2√2` beam spacings (`Sun::detail`,
/// `cast::Spacing`), leaving its line and how far it got less `margin`;
/// each `BEAM`-square cell of the box starts at the least of the beam rays
/// within `BEAM_WINDOW` of its corners, and its texels resume from that ray's
/// line there.
///
/// Why that is a texel's root walk, as the eye's proof has it with the
/// rays parallel: take `X`, the cell a texel's ray ends at, `e` its entry,
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
    BEAM_WINDOW as f32 >= 2.0 * cast::SQRT_6,
    "a beam ray through `Y` lies within 2√6 spacings"
);

/// What a beam ray leaves, in words: the eye's beam rays' line
/// (`beam::LINE_T`, `beam::LINE_NODE`, `beam::LINE_HANDED`), how far a cell's
/// texels may start from it in its first word, then its origin
/// (`LINE_ORIGIN`), which its nodes are held from (`cast::Node::moved`), as
/// every sun ray starts apart.
pub const LINE_ORIGIN: u32 = beam::LINE_WORDS;

/// How far past the disk a receiver reads the map reaches, in texels, at
/// worst (pxl-render shadow.wgsl `sun_lit`): its corner tap `SPREAD` each
/// way, and past it the neighbor its linear compare filters in, a texel
/// each way, so both along a diagonal. The rim is measured at the
/// receiver's offset point, so its normal offset adds none; and the fade
/// and `Sun::read` both measure from the snapped box's middle, its one
/// matrix (pxl-render gpu/shadow.rs `aim`), so its snap adds none.
pub const READ_PAD: f32 = (SPREAD + FILTER) * SQRT_2;
const FILTER: f32 = 1.0;

/// The tiles a receiver that reads a slot may stand in, across and down from
/// the slot's tile, from the first to the last: a tap reads that far
/// (pxl-shader sun.rs `sun_reach_cs`).
pub const READ_TILES: [i32; 2] = [-2, 1];
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
    2.0 * cast::SQRT_6 * width + 2.0 / (1u32 << FRAC_W) as f32
}

/// The shadow map as the eye's rays are cast, every slot every frame
/// (pxl-spec-core `raster`): a sample every `SAMPLE` slots across and down
/// walked (pxl-shader `sun_sample_cs`), the cells they stopped at written
/// as boxes by the eye's writer (pxl-shader raster.rs `boxed`) and drawn
/// from the sun by the eye's raster, orthographic (pxl-render raster.wgsl),
/// in the box's order (`Sun::texel`), each texel keeping the nearest its own
/// ray enters; and a slot whose ray enters its visible sample, tested again
/// with its own ray as the eye's pixels test theirs (`cast::entry`), takes it
/// (`sun_cs`); the rest are listed and walked apart (`misses_at`). A cell
/// no sample stopped at may stand in front of the visible one, as in the
/// eye's. The raster's `EYED` case has no
/// sun's: every ray starts on the near plane, before every box. Its rays ask
/// the world no finer than `Sun::finest`, so a block spans the samples as
/// the eye's blocks span its.
///
/// The samples' words, the eye's samples' own (`beam::SAMPLE_WORDS`), past
/// the beam's in the list's buffer, a sample `size / SAMPLE` across and
/// down, by its index `j · across + i`, which its box is drawn with
/// (`raster::id_of`).
pub const fn samples_at(size: u32) -> u32 {
    beam_words(size)
}

pub const fn sample(size: u32, idx: u32) -> u32 {
    samples_at(size) + idx * beam::SAMPLE_WORDS
}

pub const fn sample_words(size: u32) -> u32 {
    sample(size, (size / SAMPLE) * (size / SAMPLE))
}

/// The slots whose ray took no visible sample, listed for their walk
/// (pxl-shader `sun_miss_cs`) apart from the slots that took theirs
/// (`sun_cs`), as the eye's missed pixels are walked apart from the pixels
/// that took their visible sample, by the same listing and dispatch
/// (pxl-shader cast.rs `enlist`, `ray::MISS_ARGS_WORDS`): a walk holds its
/// workgroup as long as its slowest lane. Past the samples: how many, copied
/// in once the slots are done, as the eye's count is
/// (`observed::MARKS_COUNT`), then a slot each, its X and its Y over 16 bits.
pub const fn misses_cnt(size: u32) -> u32 {
    sample_words(size)
}

pub const fn misses_at(size: u32) -> u32 {
    misses_cnt(size) + 1
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
    let side = (1u32 << shift) as f32 / step;
    // The share of the ray within the node across axis `k`.
    let span = |k: usize, n: i32| {
        let lo = (n.wrapping_shl(shift).wrapping_sub(o[k])) as f32 / step;
        let (lo, hi) = (lo - off[k][1], lo + side - off[k][0]);
        let d = dir[k] * t;
        if d == 0.0 {
            if lo <= 0.0 && 0.0 < hi {
                (0.0, 1.0)
            } else {
                (1.0, 0.0)
            }
        } else if d > 0.0 {
            (lo / d, hi / d)
        } else {
            (hi / d, lo / d)
        }
    };
    let (a, b) = (span(0, x), span(1, y));
    let from = if a.0 > b.0 { a.0 } else { b.0 };
    let to = if a.1 < b.1 { a.1 } else { b.1 };
    (if from > 0.0 { from } else { 0.0 }) < (if to < 1.0 { to } else { 1.0 })
}

/// Texel `p`'s ray: its origin, the eye's Q3 `eye` and the steps to where
/// the texel's middle lies on the near plane; the ray; and where the eye
/// stands from that origin, in units, for the zooms it asks
/// (`cast::Detail::eye`).
pub fn ray(eye: [i32; 3], s: &Sun, p: [u32; 2]) -> ([i32; 3], Ray, [f32; 3]) {
    let (i, j) = (p[0] as f32, p[1] as f32);
    let step = (1u32 << FRAC_W) as f32;
    let steps =
        |k: usize| libm::rintf((s.first[k] + i * s.across[k] + j * s.down[k]) * step) as i32;
    let q = [steps(0), steps(1), steps(2)];
    let from = [
        -(q[0] as f32) / step,
        -(q[1] as f32) / step,
        -(q[2] as f32) / step,
    ];
    (
        [
            eye[0].wrapping_add(q[0]),
            eye[1].wrapping_add(q[1]),
            eye[2].wrapping_add(q[2]),
        ],
        Ray::new(s.dir, s.reach),
        from,
    )
}

/// The depth a texel's walk `met` leaves in the map: where it stopped
/// along its ray, as a share of the box's depth, pushed back as its face's
/// axis is (`Sun::bias`); the far plane, 1, where it stopped at nothing.
pub fn depth<A>(s: &Sun, met: &Met<A>) -> f32 {
    if met.ends != End::Stop {
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
    use crate::cast::Node;
    use crate::cast::Stop;
    use crate::param::sun;
    use crate::scatter;

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
    fn met(ends: End, t: f32, axis: u32) -> Met<()> {
        let node = Node::ROOT;
        Met {
            ends,
            stop: Stop {
                t,
                axis,
                node,
                at: (),
                asset: [0; 2],
            },
            visits: 0,
        }
    }

    #[test]
    fn texel_starts_on_near_plane() {
        let s = down();
        let (o, ray, from) = ray([100, 200, 300], &s, [0, 0]);
        // The top-left texel's middle: half the box across to the left, up.
        assert_eq!(o, [100 - 256, 200 + 256, 300 + 4 * 256]);
        assert_eq!(ray.dir, [0.0, 0.0, -1.0]);
        assert_eq!(ray.reach, 8.0);
        assert_eq!(from, [1.0, -1.0, -4.0]);
        // The next across and the next down, a texel's side on.
        assert_eq!(super::ray([0; 3], &s, [1, 1]).0, [256, -256, 4 * 256]);
    }

    #[test]
    fn square_face_no_slope() {
        let s = down();
        assert_eq!(depth(&s, &met(End::Stop, 2.0, 2)), 0.25);
        assert_eq!(depth(&s, &met(End::On, 8.0, 0)), 1.0);
        assert_eq!(depth(&s, &met(End::Lacks, 2.0, 2)), 1.0);
    }

    #[test]
    fn leaning_face_pushed_back() {
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
        let d = depth(&s, &met(End::Stop, 2.0, 2));
        assert!(
            (d - (0.25 + sun::SLOPE_BIAS * texel / 8.0)).abs() < 1e-6,
            "{d}"
        );
    }

    #[test]
    fn plane_keeps_depth() {
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
    fn words_are_fields() {
        let s = Sun {
            corner: [-7, 9],
            kept: [3, -4],
            ..down()
        };
        let w = s.words();
        assert_eq!(CORNER.map(|k| w[k] as i32), s.corner);
        assert_eq!(KEPT.map(|k| w[k] as i32), s.kept);
        assert_eq!(f32::from_bits(w[3]), s.reach);
        assert_eq!(f32::from_bits(w[4]), s.across[0]);
        assert_eq!(f32::from_bits(w[18]), s.bias[2]);
        assert_eq!(Sun::of_words(&w), s);
    }

    /// A point lies on the texel whose ray starts above it.
    #[test]
    fn point_on_texel() {
        let s = down();
        let (_, _, from) = ray([0; 3], &s, &starts_of([0; 3], &s), [1, 0]);
        // Texel (1, 0)'s start, from the eye, and a point straight under it.
        let at = s.texel_at([-from[0], -from[1], -from[2] - 3.0]);
        assert!((at[0] - 1.0).abs() < 1e-5 && at[1].abs() < 1e-5, "{at:?}");
    }

    /// A point along a texel's ray lies on that texel, whatever the sun's
    /// slant, the box's corner and its anchors' rounding.
    #[test]
    fn point_on_texel_ray() {
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
            let (o, ray, _) = ray(eye, &s, &st, p);
            let v = |k: usize, t: f32| o[k].wrapping_sub(eye[k]) as f32 / q3 + ray.dir[k] * t;
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
    fn way_up_tile_steps() {
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
        let rise = scatter::TALLEST as f32 * 4.0 / TILE as f32 * r;
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
    fn reach_covers_receivers() {
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
                for h in [0.0, 0.7, 1.9, 3.1, scatter::TALLEST as f32] {
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
                            let read = -READ_TILES[1]..=-READ_TILES[0];
                            read.contains(&d[0]) && read.contains(&d[1])
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
    fn receiver_reads_disk() {
        let s = Sun { rim: 0.5, ..down() };
        // A box 16 texels a side: its middle is 8, the disk 4 texels out.
        assert!(s.read(16, [8, 8], 0.0));
        assert!(s.read(16, [11, 7], 0.0));
        assert!(!s.read(16, [13, 7], 0.0));
        assert!(s.read(16, [13, 7], 2.0));
        assert!(!s.read(16, [0, 0], 2.0));
    }

    #[test]
    fn slot_holds_texel() {
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
        assert!(!s.kept(8, [3, 0]), "nothing is kept at first");
        // The box moved a texel on across: all but its last column were in the last.
        s.kept = s.corner;
        s.corner[0] += 1;
        assert!(s.kept(8, [6, 7]) && !s.kept(8, [7, 0]));
        // Moved back past where it was: its first column is new.
        s.corner[0] -= 2;
        assert!(!s.kept(8, [0, 3]) && s.kept(8, [1, 3]));
    }

    #[test]
    fn a_slot_stands_while_the_eye_asks_its_stop_at_its_zoom() {
        let s = down();
        let far = Detail {
            tau: 1.0,
            range: [0, crate::FINEST_ZOOM],
            eye: [0.0, 0.0, -400.0],
        };
        let w = s.word(far, 0.25);
        assert!(s.stands(far, w));
        // The eye come up to the stop asks it finer.
        let near = Detail {
            eye: [0.0, 0.0, 2.0],
            ..far
        };
        assert!(!s.stands(near, w));
        // A walk that stopped at nothing stands wherever the eye is.
        assert!(s.stands(near, s.word(far, 1.0)));
    }

    #[test]
    fn a_ray_passes_over_the_nodes_its_shadow_falls_through() {
        // A ray down and across x from (0, 0), 4 units on x by its stop.
        let (o, dir) = ([0, 0, 0], [0.8, 0.0, -0.6]);
        let unit = crate::UNIT_ZOOM;
        assert!(over(o, dir, 5.0, [3, 0], unit), "the unit 3 to 4 across");
        assert!(!over(o, dir, 5.0, [5, 0], unit), "past its stop");
        assert!(!over(o, dir, 5.0, [-1, 0], unit), "behind its start");
        assert!(!over(o, dir, 5.0, [2, 1], unit), "beside it");
        // Straight down, only the node under it.
        assert!(
            over(o, [0.0, 0.0, -1.0], 5.0, [0, 0], unit)
                && !over(o, [0.0, 0.0, -1.0], 5.0, [1, 0], unit)
        );
        // A node as wide as half the world holds it wherever it is.
        assert!(over([i32::MAX - 5, 0, 0], dir, 5.0, [1, 0], 1));
    }

    #[test]
    fn a_tile_judged_whole_walks_wherever_a_slot_of_it_would() {
        // A box 64 units a side, the sun 40 degrees off straight down and
        // turned, 64 texels a side; the eye's zooms asked finely enough that
        // their rings cross the tiles.
        let (c, s) = (libm::cosf(0.7), libm::sinf(0.7));
        let (ca, sa) = (libm::cosf(0.4), libm::sinf(0.4));
        let x = [32.0 * ca, 32.0 * sa, 0.0];
        let y = [-32.0 * sa * c, 32.0 * ca * c, 32.0 * s];
        let z = [-sa * s * 160.0, ca * s * 160.0, c * -160.0];
        let size = 64;
        let mut seed = 7u32;
        let mut next = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1 << 24) as f32
        };
        let (mut judged, mut whole, mut met) = (0, 0, 0);
        for trial in 0..400 {
            let w = [
                next() * 40.0 - 20.0,
                next() * 40.0 - 20.0,
                60.0 + next() * 40.0,
            ];
            let mut sun = Sun::of([x, y, z, w], size, [3, -5]);
            sun.kept = sun.corner;
            let detail = Detail {
                tau: 0.5 + next() * 3.0,
                range: [0, crate::FINEST_ZOOM],
                eye: [0.0; 3],
            };
            let eye = [1000, -2000, 300];
            let tile = [(next() * 8.0) as u32 % 8, (next() * 8.0) as u32 % 8];
            let (p, run) = sun.tile_texel(size, tile);
            if !run {
                continue;
            }
            // Each slot cast where the eye stood a little off, so some stand and some do not.
            let mut held = Held::NONE;
            let mut words = [0u32; (TILE * TILE) as usize];
            let shift = [next() * 2.0 - 1.0, next() * 2.0 - 1.0, next() * 2.0 - 1.0];
            let d0 = 0.2 + next() * 0.6;
            for k in 0..TILE * TILE {
                let q = [p[0] + k % TILE, p[1] + k / TILE];
                let (_, _, from) = ray(eye, &sun, q);
                let at = Detail {
                    eye: [from[0] + shift[0], from[1] + shift[1], from[2] + shift[2]],
                    ..detail
                };
                let d = if trial % 7 == 0 && k % 5 == 0 {
                    1.0
                } else {
                    d0 + next() * 0.02
                };
                words[k as usize] = sun.word(at, d);
                held = held.with(words[k as usize]);
            }
            let node = [
                (next() * 80.0 - 40.0) as i32 + 1000 / 256,
                (next() * 80.0 - 40.0) as i32 - 2000 / 256,
            ];
            let stands = sun.tile_stands(detail, p, held);
            let (o, off) = sun.tile_rays(eye, p);
            let reach = held.reach as f32 / DEPTH_ONE as f32 * sun.reach;
            let passes = over_any(o, off, sun.dir, reach, node, crate::UNIT_ZOOM);
            for k in 0..TILE * TILE {
                let q = [p[0] + k % TILE, p[1] + k / TILE];
                let (o, _, from) = ray(eye, &sun, q);
                let at = Detail {
                    eye: from,
                    ..detail
                };
                let word = words[k as usize];
                if stands {
                    assert!(
                        sun.stands(at, word),
                        "trial {trial}: slot {k} walks in a tile judged to stand"
                    );
                }
                if over(
                    o,
                    sun.dir,
                    depth_of(word) * sun.reach,
                    node,
                    crate::UNIT_ZOOM,
                ) {
                    assert!(
                        passes,
                        "trial {trial}: slot {k} passes over a node its tile's rays miss"
                    );
                    met += 1;
                }
            }
            judged += 1;
            whole += u32::from(stands);
        }
        // Enough of either kind to have judged the bound.
        assert!(
            judged > 200 && whole > 20 && whole < judged - 20 && met > 50,
            "{judged} {whole} {met}"
        );
        // A tile at the box's edge straddles it; one in the kept box is kept.
        let mut sun = Sun::of([x, y, z, [0.0; 3]], size, [3, -5]);
        assert!(!sun.tile_texel(size, [0, 0]).1);
        sun.kept = sun.corner;
        assert!(sun.tile_kept(size, [8, 16]));
        sun.corner[0] += 1;
        assert!(sun.tile_kept(size, [7, 16]) && !sun.tile_kept(size, [size - TILE, 16]));
    }
}
