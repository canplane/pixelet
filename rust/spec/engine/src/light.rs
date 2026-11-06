// pxl-engine/light.rs

//! gpu-portable: the world's depth from the light, a ray a texel of the light's
//! shadow map (pxl-render light.wgsl `light_cs`): each texel's ray from where it
//! lies on the light box's near plane, along the light's rays to the far
//! plane, walked through the one descent the eye's rays take, every node
//! asked at its tile's band, the zoom the eye's rays ask as near as the
//! tile's column comes to the eye (`Light::band`); and the depth it leaves,
//! as a raster caster's would be (pxl-render gpu/shadow.rs, where the decor
//! and the actors are drawn into the same map).
//!
//! The world's depth is a clipmap: the light box's texels lie on a grid
//! fixed in the world (`aim`'s snap), and each world texel is held in the
//! slot its index names modulo the map's side. A slot holds its depth
//! (`word`). A tile of slots cast in an epoch holds until the box slides it
//! off or into another band (`Light::keeps`, pxl-render
//! light_slide.wgsl).

use crate::beam;
use crate::cast;
use crate::cast::Spacing;
use crate::cast::{End, Ray, Trace};
use crate::eye;
use crate::grid;
use crate::param::light;
use crate::param::light::{SAMPLE, SPREAD};
use crate::ray;
use crate::{axis, Zoom, FINEST_ZOOM, FRAC_W, UNIT_ZOOM};
use core::f32::consts::SQRT_2;

/// The light's cast as the eye's words hold it (`eye::LIGHT`), a frame's
/// constants worked out once off the light box (`Light::of`), each a vec4's
/// first three words: where texel (0, 0)'s ray starts, from the eye, in
/// units, and how far every ray reaches (the fourth word); the step to the
/// next texel across and down; the rays' one direction; and how far a hit
/// on a face across each axis is pushed back in depth. Their fourth words
/// hold the clipmap's: the world texel index of the box's texel (0, 0),
/// `corner`, X then Y, Y counted down the rows; the share of the box's
/// half-width a receiver reads the map within, `reach_ndc`
/// (`Light::read`); and a texel's width (`width`). Past them, the reciprocals
/// a rule both the CPU and a GPU decide multiplies by in place of a division,
/// each rounded once by the host (`words`): the rays' direction's (`inverse`)
/// and the reach's (`t_max_inverse`), a vec4; then the squares of the steps
/// across and down (`across2_inverse`, `down2_inverse`) and the width squared over the split
/// distance squared (`band_scale`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub first: [f32; 3],
    pub t_max: f32,
    pub across: [f32; 3],
    pub down: [f32; 3],
    pub dir: [f32; 3],
    pub bias: [f32; 3],
    pub corner: [i32; 2],
    pub reach_ndc: f32,
}

/// The word of `eye::LIGHT` the shadow's radius lies in.
pub const REACH_NDC: usize = 15;

/// The words of `eye::LIGHT` the corner lies in, X then Y, which the map is
/// laid in off.
pub const CORNER: [usize; 2] = [7, 11];

/// A slot's word: its depth, a 24-bit share of the box's depth as a
/// depth24 map holds it (`word`).
pub const DEPTH_ONE: u32 = (1 << 24) - 1;

/// A slot's word for depth `d` (`depth`), a share of the box's.
pub fn word(d: f32) -> u32 {
    let q = (d * DEPTH_ONE as f32 + 0.5) as u32;
    if q < DEPTH_ONE {
        q
    } else {
        DEPTH_ONE
    }
}

fn length(v: [f32; 3]) -> f32 {
    libm::sqrtf(dot(v, v))
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// How much a plane's depth in the map changes a texel across and a texel
/// down it, as a share of the box's depth: the plane's normal `n`, the
/// step `across` and `down` a texel takes in the world, and `deep`, the
/// world's step the box's whole depth along the light's rays. The plane
/// holds `n`'s dot with each step it takes, so the share of the depth `t`
/// that keeps it on the plane after a step `s` across is
/// `−(n·s) / (n·deep)`. A caster's push is its greatest, times
/// `param::light::SLOPE_BIAS` (`Light::of`), and a receiver's taps each compare its
/// depth carried along its own plane to where the tap lies (pxl-render
/// shadow.wgsl `light_slope`, off the light box's rows): the one statement of
/// a face's slope in the map, which both take any normal to. None for a
/// face along the rays, which no ray enters across.
pub fn slope(n: [f32; 3], across: [f32; 3], down: [f32; 3], deep: [f32; 3]) -> [f32; 2] {
    let d = dot(n, deep);
    if d == 0.0 {
        return [0.0, 0.0];
    }
    [-dot(n, across) / d, -dot(n, down) / d]
}

impl Light {
    /// Off the light box's inverse (pxl-render gpu/shadow.rs `aim`), from
    /// the map's NDC to units from the eye, by its columns: a texel's NDC
    /// `(x, y)` lies at `w + x·x + y·y` on the near plane, and `z` runs
    /// from the near plane to the far; the map `size` texels a side, row 0
    /// at the top, as the lit passes read it. A hit's push is a raster
    /// caster's polygon offset (`param::light::SLOPE_BIAS`): the face's greatest
    /// change in depth a texel across the map (`slope`), times it.
    /// `corner` is the world texel index of the box's first texel;
    /// `reach_ndc` the share of its half-width a receiver reads
    /// within.
    pub fn of([x, y, z, w]: [[f32; 3]; 4], size: u32, corner: [i32; 2], reach_ndc: f32) -> Light {
        let texel = 2.0 / size as f32;
        let (ax, ay) = (1.0 - texel / 2.0, texel / 2.0 - 1.0);
        let t_max = length(z);
        let dir = [z[0] / t_max, z[1] / t_max, z[2] / t_max];
        let across = [x[0] * texel, x[1] * texel, x[2] * texel];
        let down = [-y[0] * texel, -y[1] * texel, -y[2] * texel];
        // A face's normal is its axis.
        let bias = |a: u32| {
            let on = |k: u32| if a == k { 1.0 } else { 0.0 };
            let n = [on(0), on(1), on(2)];
            let [i, j] = slope(n, across, down, z);
            let (i, j) = (libm::fabsf(i), libm::fabsf(j));
            light::SLOPE_BIAS * if i > j { i } else { j }
        };
        Light {
            first: [
                w[0] - ax * x[0] - ay * y[0],
                w[1] - ax * x[1] - ay * y[1],
                w[2] - ax * x[2] - ay * y[2],
            ],
            t_max,
            across,
            down,
            dir,
            bias: [bias(0), bias(1), bias(2)],
            corner,
            reach_ndc,
        }
    }

    /// Where a point `v` units from the eye lies on the box, in texels across
    /// and down from texel (0, 0)'s middle: its step from that texel's start
    /// along each of the box's axes, over that axis's texel. The light box
    /// is orthographic, its axes square to the light's rays, so depth along
    /// them counts for nothing.
    pub(crate) fn texel_at(&self, v: [f32; 3]) -> [f32; 2] {
        let d = [
            v[0] - self.first[0],
            v[1] - self.first[1],
            v[2] - self.first[2],
        ];
        [
            dot(d, self.across) * self.across2_inverse(),
            dot(d, self.down) * self.down2_inverse(),
        ]
    }

    /// The tile of the box a receiver standing `v` units from the eye
    /// stands on (pxl-render light_mark.wgsl, light_mark_draws.wgsl).
    pub fn tile_of(&self, v: [f32; 3]) -> [i32; 2] {
        let (at, tile) = (self.texel_at(v), TILE as f32);
        [
            libm::floorf((at[0] + 0.5) / tile) as i32,
            libm::floorf((at[1] + 0.5) / tile) as i32,
        ]
    }

    /// Whether box texel `p` of the map `size` a side lies where a receiver
    /// may read it, grown by `pad` texels: within `reach_ndc` of the
    /// box's half-width about its middle, where the eye stands, as past it a
    /// shadow has faded to nothing (pxl-render shadow.wgsl
    /// `shadow_visibility`).
    pub fn read(&self, size: u32, p: [i32; 2], pad: f32) -> bool {
        let half = size as f32 / 2.0;
        let (x, y) = (p[0] as f32 + 0.5 - half, p[1] as f32 + 0.5 - half);
        let r = self.reach_ndc * half + pad;
        x * x + y * y <= r * r
    }

    /// A texel's width in units, across and down alike, the box's texels
    /// square: `across`'s length, which the host rounds and hands the
    /// shaders in a word (`W_WIDTH`).
    pub fn width(&self) -> f32 {
        length(self.across)
    }

    /// The reciprocals the rules multiply by, each rounded once here, on
    /// the host, and handed the shaders in a word (`words`): the reach's, which scales a depth
    /// along a ray to its share of the box's (`W_T_MAX_INVERSE`, `depth`); and the
    /// squares of the steps across and down, which scale a step's dot with
    /// each to texels (`W_ACROSS2_INVERSE`, `W_DOWN2_INVERSE`, `texel_at`).
    pub fn t_max_inverse(&self) -> f32 {
        1.0 / self.t_max
    }

    pub fn across2_inverse(&self) -> f32 {
        1.0 / dot(self.across, self.across)
    }

    pub fn down2_inverse(&self) -> f32 {
        1.0 / dot(self.down, self.down)
    }

    /// The reciprocal of the rays' one direction on each axis, as every
    /// light ray takes it (`Ray::inverse`), rounded once here and handed the
    /// shaders in a word (`W_INVERSE`).
    pub fn inverse(&self) -> [f32; 3] {
        Ray::new(self.dir, self.t_max).inverse()
    }

    /// How far apart its beam rays lie, `BEAM` of the map's texels
    /// (`cast::Spacing`).
    pub fn spacing(&self) -> Spacing {
        Spacing {
            angle: 0.0,
            across: BEAM as f32 * self.width(),
        }
    }

    /// The zoom every walk over box tile `tile` asks, its band, the map
    /// `size` texels a side: the camera grid's cell zoom
    /// (`grid::cell_zoom_at`) at the tile column's nearest point to the eye,
    /// from the box's middle (`read`), in units, held to the eye's zooms
    /// `range` (`eye::RANGE`). The tile's place in the box alone, the same
    /// wherever the eye stands in it, and as fine as the eye asks anywhere in
    /// the column.
    ///
    /// Every pass that reads a tile's band must give it the same one: a slot
    /// takes a sample walked at its own band, a cell resumes from beam rays
    /// walked at its, a slide keeps a tile cast at its. So the band is exact:
    /// the distance squared is a whole number of texels squared
    /// (`band_texels2`) times `band_scale`, one product, which no device
    /// fuses or reorders.
    pub fn band(&self, size: u32, range: [Zoom; 2], tile: [i32; 2]) -> Zoom {
        let held = cast::Detail {
            range,
            spacing: Spacing::NONE,
        };
        held.hold(grid::cell_zoom_in_splits(
            band_texels2(size, tile) as f32 * self.band_scale(),
        ))
    }

    /// A texel's width squared over the split distance squared, the scale of
    /// a band's texels squared (`band`), rounded once by the host and handed
    /// the shaders in a word (`W_BAND_SCALE`).
    pub fn band_scale(&self) -> f32 {
        self.width() * self.width() * grid::INV_SPLIT2
    }

    /// Whether box tile `tile`, the map `size` texels a side, keeps what a
    /// cast left in it where the box slid to its corner from `was`, the
    /// eye's zooms `range`: the tile lay in the box under `was` too, and its
    /// band there is its band here (`band`). The one rule of what a slide
    /// keeps (pxl-render light_slide.wgsl).
    pub fn keeps(&self, size: u32, range: [Zoom; 2], tile: [u32; 2], was: [i32; 2]) -> bool {
        let (n, t) = (tiles(size) as i32, TILE as i32);
        let from = [
            tile[0] as i32 + self.corner[0].wrapping_sub(was[0]) / t,
            tile[1] as i32 + self.corner[1].wrapping_sub(was[1]) / t,
        ];
        let here = [tile[0] as i32, tile[1] as i32];
        from[0] >= 0
            && from[1] >= 0
            && from[0] < n
            && from[1] < n
            && self.band(size, range, from) == self.band(size, range, here)
    }

    /// Whether the rays of box tile `tile`, the map `size` texels a side, asking at its band
    /// under the eye's zooms `range` (`band`), may meet a cell over `column`: the world's
    /// columns from `[x0, y0]` to `[x1, y1]`, units from the eye (`column_of`), at every
    /// height, where what the world's query reads changed. The one rule of what a world's
    /// change keeps (pxl-render light_slide.wgsl).
    ///
    /// A ray's answer is that of its band's cells along it: a split's box and a node of air
    /// pass over cells of air alone, and a node not held is a split, its cells at the band the
    /// lack (`ground::node`); a cell's answer reads the columns under it alone, the bricks
    /// that hold them and what is placed or cut over them (pxl-render forget.wgsl). So a
    /// tile whose rays meet no cell of its band over the column answers as before. A cell
    /// over the column lies within it grown by a cell's side, grown by a texel more for the
    /// rounding of the rays' starts (`Starts`). The tile's rays, seen from above, sweep the
    /// parallelogram of its texels on the near plane along the light to their reach: a
    /// hexagon of three edges each twice, which the grown column misses where one of five
    /// axes parts them, X, Y and across each edge.
    pub fn reaches(&self, size: u32, range: [Zoom; 2], tile: [u32; 2], column: [f32; 4]) -> bool {
        let band = self.band(size, range, [tile[0] as i32, tile[1] as i32]);
        let grow = libm::ldexpf(1.0, UNIT_ZOOM as i32 - band as i32) + self.width();
        let lo = [column[0] - grow, column[1] - grow];
        let hi = [column[2] + grow, column[3] + grow];
        let t = TILE as f32;
        let (i, j) = (tile[0] as f32 * t - 0.5, tile[1] as f32 * t - 0.5);
        let edges = [
            [self.across[0] * t, self.across[1] * t],
            [self.down[0] * t, self.down[1] * t],
            [self.dir[0] * self.t_max, self.dir[1] * self.t_max],
        ];
        let mid = [0, 1].map(|k| {
            self.first[k]
                + i * self.across[k]
                + j * self.down[k]
                + 0.5 * (edges[0][k] + edges[1][k] + edges[2][k])
        });
        let at = [0, 1].map(|k| 0.5 * (lo[k] + hi[k]) - mid[k]);
        let half = [0, 1].map(|k| 0.5 * (hi[k] - lo[k]));
        let parts = |n: [f32; 2]| {
            let swept: f32 = edges
                .iter()
                .map(|e| libm::fabsf(e[0] * n[0] + e[1] * n[1]))
                .sum();
            let column = libm::fabsf(half[0] * n[0]) + libm::fabsf(half[1] * n[1]);
            libm::fabsf(at[0] * n[0] + at[1] * n[1]) > 0.5 * swept + column
        };
        !(parts([1.0, 0.0]) || parts([0.0, 1.0]) || edges.iter().any(|e| parts([-e[1], e[0]])))
    }

    /// The box's texel a slot of the map holds, `size` a power of two its
    /// side: the slot's world texel, less the box's corner. The light's raster
    /// (`SAMPLE`) is drawn in the box's order, as the map is laid, and a
    /// slot reads its visible sample at this texel: drawn in the slots' order,
    /// a box would split at the seam where the slots wrap.
    pub fn texel(&self, size: u32, slot: [u32; 2]) -> [u32; 2] {
        let at = |k: usize| (slot[k] as i32).wrapping_sub(self.corner[k]) as u32 & (size - 1);
        [at(0), at(1)]
    }

    /// The light its words lay (`words`).
    pub fn of_words(w: &[u32; eye::LIGHT_WORDS]) -> Light {
        let v = |k: usize| {
            [
                f32::from_bits(w[k]),
                f32::from_bits(w[k + 1]),
                f32::from_bits(w[k + 2]),
            ]
        };
        Light {
            first: v(0),
            t_max: f32::from_bits(w[W_T_MAX]),
            across: v(W_ACROSS),
            down: v(W_DOWN),
            dir: v(W_DIR),
            bias: v(W_BIAS),
            corner: CORNER.map(|k| w[k] as i32),
            reach_ndc: f32::from_bits(w[REACH_NDC]),
        }
    }

    /// Its words, as `eye::LIGHT` lays them.
    pub fn words(&self) -> [u32; eye::LIGHT_WORDS] {
        let v = |a: [f32; 3], w: u32| [a[0].to_bits(), a[1].to_bits(), a[2].to_bits(), w];
        let c = [
            v(self.first, self.t_max.to_bits()),
            v(self.across, self.corner[0] as u32),
            v(self.down, self.corner[1] as u32),
            v(self.dir, self.reach_ndc.to_bits()),
            v(self.bias, self.width().to_bits()),
            v(self.inverse(), self.t_max_inverse().to_bits()),
            [
                self.across2_inverse().to_bits(),
                self.down2_inverse().to_bits(),
                self.band_scale().to_bits(),
                // Past the light's words (`eye::LIGHT_WORDS`), so laid nowhere.
                0,
            ],
        ];
        let mut out = [0; eye::LIGHT_WORDS];
        let mut k = 0;
        while k < out.len() {
            out[k] = c[k / 4][k % 4];
            k += 1;
        }
        out
    }
}

/// A `Light`'s words (`words`), each from `eye::LIGHT`, as a shader reads them.
pub const W_T_MAX: usize = 3;
pub const W_ACROSS: usize = 4;
pub const W_DOWN: usize = 8;
pub const W_DIR: usize = 12;
pub const W_BIAS: usize = 16;
pub const W_WIDTH: usize = 19;
pub const W_INVERSE: usize = 20;
pub const W_T_MAX_INVERSE: usize = 23;
pub const W_ACROSS2_INVERSE: usize = 24;
pub const W_DOWN2_INVERSE: usize = 25;
pub const W_BAND_SCALE: usize = 26;

/// The texels squared from the middle of a box `size` a side to the nearest
/// point of box tile `tile`'s column (`Light::band`), each axis's gap a whole
/// number of texels, as the middle and the tiles' edges lie on texels' edges.
/// A side of at most 4096 and the beam rays' window past it keep a gap under
/// 2^11.5, so the sum stays under 2^24 and is a float exactly: twice `ANCHOR`,
/// the host's side at most.
pub fn band_texels2(size: u32, tile: [i32; 2]) -> u32 {
    let (half, t) = ((size / 2) as i32, TILE as i32);
    let gap = |k: i32| (k * t - half).max(half - (k + 1) * t).max(0).unsigned_abs();
    let (x, y) = (gap(tile[0]), gap(tile[1]));
    x * x + y * y
}

/// The world's columns of the nodes at `zoom` from `lo` to `hi`, X then Y, `hi` past the
/// last (`Node::idx`), as `Light::reaches` reads them: units from `origin`, the eye's steps
/// (`eye::ORIGIN`), `[x0, y0, x1, y1]`; X the short way round the world, and the whole of it
/// where they span it, as far as a float's sum stays finite.
pub fn column_of(origin: [i32; 3], zoom: Zoom, lo: [i32; 2], hi: [i32; 2]) -> [f32; 4] {
    let s = FINEST_ZOOM - zoom;
    let unit = 1.0 / f64::from(1u32 << FRAC_W);
    let across = i64::from(hi[0] - lo[0]) << s;
    let (x0, x1) = if across >= 1i64 << 32 {
        (-1.0e30 / unit, 1.0e30 / unit)
    } else {
        let from = ((i64::from(lo[0]) << s) as u32).wrapping_sub(origin[0] as u32) as i32;
        (f64::from(from), f64::from(from) + across as f64)
    };
    let y = |v: i32| ((i64::from(v) << s) - i64::from(origin[1])) as f64;
    [x0, y(lo[1]), x1, y(hi[1])].map(|v| (v * unit) as f32)
}

/// How many changed columns a cast is handed (`COLUMNS_AT`); a change of more casts the map
/// whole.
pub const COLUMNS: usize = 64;

/// What the light's cast is handed beside the eye's words, a uniform: a head
/// of `HEAD_VEC4S` vec4s, the cast's number, whether it is counted, its
/// epoch and the map's side (`HEAD_CAST` to `HEAD_SIZE`), the corner of the
/// cast before (`HEAD_CORNER_WAS`), whether its marks take each pixel's
/// footprint (`HEAD_FOOTPRINT`), then the rays' starts (`Starts`, from
/// `STARTS_AT`), then what the host draws that reads the map (`STANDS_AT`), then the list's places for the passes by hand
/// (`HEAD_CELLS`), then the world's columns that changed (`COLUMNS_AT`).
pub const HEAD_VEC4S: usize = COLUMNS_AT + 1 + COLUMNS;
pub const HEAD_WORDS: usize = 4 * HEAD_VEC4S;
/// The cast's number, counted up from one each cast, which a tile it lists to cast is marked
/// with (`listed_marks_at`) and a tile it claims named by (`reads_at`).
pub const HEAD_CAST: usize = 0;
pub const HEAD_COUNTED: usize = 1;
/// The number of the cast since which only what says which tiles are read
/// moved, and the box: what a cast in it or after left in a tile stands, as
/// the light, the eye's zooms and the world it was cast for still do, where
/// the box kept it (`reads_at`, `Light::keeps`).
pub const HEAD_EPOCH: usize = 2;
/// The map's side, which the slots' lattice is laid by. A pass the eye's
/// rays and the shadow map share is handed the eye's own head, all zero, in
/// its place: a head naming no map is the eye's.
pub const HEAD_SIZE: usize = 3;
/// The clipmap's corner the cast before was cast under, X and Y, the first
/// two words of the second vec4: how many tiles the box slid since, its
/// corners `TILE`-aligned, which the reads move by (pxl-render
/// light_slide.wgsl).
pub const HEAD_CORNER_WAS: usize = 4;
/// Whether the cast's marks take each pixel's footprint (`FOOTPRINT_TILES`),
/// 1 or 0, the second vec4's third word: not while the eye moves, as the next
/// frame's rays mark again; in the first frame it stands, which the jitter's
/// frames after it stand on, and in each cast of its rolled lattice
/// (`beam::ROLLS`), whose samples find what the casts before did not
/// (pxl-render light_mark.wgsl).
pub const HEAD_FOOTPRINT: usize = 6;
/// Where the slots' misses' count lies in the list (`misses_cnt`), the second vec4's last word.
pub const HEAD_MISSES_CNT: usize = 7;
/// Where a pass finds the list's words and the args' marks, and how many a row, as
/// `head_layout` lays them for the map's side: words of their own past the actors.
pub const HEAD_CELLS: usize = 4 * (STANDS_AT + 1 + 2 * STANDS);
pub const HEAD_CELLS_ACROSS: usize = HEAD_CELLS + 1;
pub const HEAD_SAMPLES: usize = HEAD_CELLS + 2;
pub const HEAD_SAMPLES_ACROSS: usize = HEAD_CELLS + 3;
pub const HEAD_LEFTS: usize = HEAD_CELLS + 4;
pub const HEAD_READS: usize = HEAD_CELLS + 5;
pub const HEAD_TILES: usize = HEAD_CELLS + 6;
pub const HEAD_HANDS: usize = HEAD_CELLS + 7;
/// What a cast keeps for the next (`kept_at`).
pub const HEAD_KEPT: usize = HEAD_CELLS + 8;
pub const HEAD_BEAM_ACROSS: usize = HEAD_CELLS + 9;
pub const HEAD_MISSES: usize = HEAD_CELLS + 10;
/// Each list's marks (`listed_marks_at`), then each list (`listed_at`), a word each in the
/// lists' order (`LISTED_CORNERS`).
pub const HEAD_MARKS: usize = HEAD_CELLS + 11;
pub const HEAD_LISTED: usize = HEAD_MARKS + LISTS as usize;
/// The beam rays the walk over the node cache left (`corners_left_at`).
pub const HEAD_CORNERS_LEFT: usize = HEAD_LISTED + LISTS as usize;
/// The slide's two words (`slide_at`).
pub const HEAD_SLIDE: usize = HEAD_CORNERS_LEFT + 1;
/// Past the words the map's side lays, two the host sets each cast: the number of the cast
/// the world's receivers were last stood in, this one's where it stands them again
/// (`LISTED_STOOD`; pxl-render light_mark.wgsl); and the number of the last cast before this
/// one that owed a slide, the box slid or a column changed (`slide_at`; light_slide.wgsl).
pub const HEAD_WORLD: usize = HEAD_SLIDE + 1;
pub const HEAD_SLIDE_OWED: usize = HEAD_WORLD + 1;
/// Whether the world's receivers are stood this frame, 1 or 0, which the host writes every
/// frame, a light cast's head or not, so the eye's draw stands the boxes it draws
/// only where the light's cast stands them (pxl-render record_draw.wgsl `record_draw_cs`).
pub const HEAD_STANDS_WORLD: usize = HEAD_SLIDE_OWED + 1;
/// The places (`places_at`), the holes' region (`list_words`) and the rings (`rings_at`), laid
/// by the map's side.
pub const HEAD_PLACES: usize = HEAD_STANDS_WORLD + 1;
pub const HEAD_REGION: usize = HEAD_PLACES + 1;
pub const HEAD_RINGS: usize = HEAD_REGION + 1;
/// Whether an owed tile's parked rays are resumed, 1 or 0, the host's each cast: where its
/// cast held the light's content, the box did not slide, no changed column reached it and the
/// node cache forgot nothing since the cast before (`READ_OWED`; pxl-render
/// light_propagate.wgsl `light_claim_cs`).
pub const HEAD_CARRIES: usize = HEAD_RINGS + 1;
/// The holes' stops (`hits_at`), laid by the map's side.
pub const HEAD_HITS: usize = HEAD_CARRIES + 1;
const HEAD_LAID_VEC4S: usize = 10;
/// The world's columns that changed since the cast before, where they did and the epoch goes
/// on (`column_of`, `Light::reaches`): how many, the first word of a vec4, then each a vec4
/// of four floats, as many as `COLUMNS`.
pub const COLUMNS_AT: usize = STANDS_AT + 1 + 2 * STANDS + HEAD_LAID_VEC4S;
const _: () = assert!(
    HEAD_HITS < HEAD_CELLS + 4 * HEAD_LAID_VEC4S,
    "the head holds the words it lays"
);

/// The words of the head the map's side lays: each word and what it holds.
pub const fn head_layout(size: u32) -> [(usize, u32); 30] {
    [
        (HEAD_SIZE, size),
        (HEAD_MISSES_CNT, misses_cnt(size)),
        (HEAD_CELLS, cell(size, [0, 0])),
        (HEAD_CELLS_ACROSS, cells_across(size)),
        (HEAD_SAMPLES, sample(size, 0)),
        (HEAD_SAMPLES_ACROSS, size / SAMPLE),
        (HEAD_LEFTS, lefts_at(size)),
        (HEAD_READS, reads_at(size)),
        (HEAD_TILES, tiles(size)),
        (HEAD_HANDS, hand_at(size, 0)),
        (HEAD_KEPT, kept_at(size)),
        (HEAD_BEAM_ACROSS, beam_across(size)),
        (HEAD_MISSES, misses_at(size)),
        (HEAD_CORNERS_LEFT, corners_left_at(size)),
        (HEAD_SLIDE, slide_at(size)),
        (HEAD_PLACES, places_at(size)),
        (HEAD_REGION, list_words(size)),
        (HEAD_RINGS, rings_at(size)),
        (HEAD_HITS, hits_at(size)),
        (
            HEAD_MARKS + LISTED_CORNERS as usize,
            listed_marks_at(size, LISTED_CORNERS),
        ),
        (
            HEAD_MARKS + LISTED_FED as usize,
            listed_marks_at(size, LISTED_FED),
        ),
        (
            HEAD_MARKS + LISTED_CLAIMED as usize,
            listed_marks_at(size, LISTED_CLAIMED),
        ),
        (
            HEAD_LISTED + LISTED_CORNERS as usize,
            listed_at(size, LISTED_CORNERS),
        ),
        (
            HEAD_LISTED + LISTED_FED as usize,
            listed_at(size, LISTED_FED),
        ),
        (
            HEAD_LISTED + LISTED_CLAIMED as usize,
            listed_at(size, LISTED_CLAIMED),
        ),
        (
            HEAD_MARKS + LISTED_STOOD as usize,
            listed_marks_at(size, LISTED_STOOD),
        ),
        (
            HEAD_MARKS + LISTED_READ as usize,
            listed_marks_at(size, LISTED_READ),
        ),
        (
            HEAD_LISTED + LISTED_STOOD as usize,
            listed_at(size, LISTED_STOOD),
        ),
        (
            HEAD_LISTED + LISTED_READ as usize,
            listed_at(size, LISTED_READ),
        ),
        (
            HEAD_LISTED + LISTED_WANTED as usize,
            listed_at(size, LISTED_WANTED),
        ),
    ]
}

/// Where a texel's ray starts, the same point of the world in every frame
/// cast in one light frame (pxl-render gpu/shadow.rs `aim`): off anchors
/// `ANCHOR` texels apart on the world's texel grid, each a start on the near
/// plane worked out once on the host, its Q3 position and what rounding to
/// it left (`frac`), and a texel's step across and down in Q3 steps, so a
/// start is the anchor's and the steps from it rounded, the same sum each
/// frame (`start`). `base` is the world texel index of anchor `at[0]`;
/// `at[1]` is the next across, `at[2]` the next down, `at[3]` both, the box
/// lying in the four (`ANCHOR` its side at least). The steps and the
/// remainders are fixed point, `STEP_BITS` fractional bits of a Q3 step
/// (`fixed`), so a start is summed in integers, exactly, and a GPU that
/// fuses, reorders or simplifies a float sum (Metal's fast math) takes the
/// same one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Starts {
    pub base: [i32; 2],
    pub across: [i32; 3],
    pub down: [i32; 3],
    pub at: [[i32; 3]; 4],
    pub frac: [[i32; 3]; 4],
}

pub const ANCHOR: i32 = 2048;
const _: () = assert!(
    2 * (ANCHOR as u32 + (BEAM_WINDOW + 1) * TILE).pow(2) < 1 << 24,
    "a band's texels squared are a float exactly (`band_texels2`)"
);

/// The fractional bits of a Q3 step a start's steps and remainders hold
/// (`Starts`), where `start` splits each step: its whole steps, under 2^14
/// a texel (64 units), times a whole number under `ANCHOR`, and its fraction
/// times one, each sum within an i32.
pub const STEP_BITS: u32 = 16;
const _: () = assert!((ANCHOR as u32).trailing_zeros() + STEP_BITS + 2 <= 31);

/// `v` Q3 steps in fixed point, `STEP_BITS` fractional bits, to the nearest.
pub fn fixed(v: f32) -> i32 {
    libm::rintf(libm::ldexpf(v, STEP_BITS as i32)) as i32
}

/// The starts' words in the cast's head (`HEAD_VEC4S`), a
/// vec4 each from `STARTS_AT`: the base, the steps across and down, the
/// anchors, their remainders.
pub const STARTS_AT: usize = 2;
/// `Starts`'s vec4s from `STARTS_AT`, as a shader reads them (`Starts::words`).
pub const STARTS_ACROSS: usize = 1;
pub const STARTS_DOWN: usize = 2;
pub const STARTS_ANCHORS: usize = 3;
pub const STARTS_FRACS: usize = 7;
pub const STARTS_VEC4S: usize = 11;

/// What the host draws that reads the shadow map, receivers as decor is (pxl-render
/// frame/draw.rs `DrawList`, light_mark_draws.wgsl), past the starts: how many, the first word
/// of a vec4, then each the box it stands within, units from the eye, its lowest corner then
/// its highest, a vec4 each, as many as `STANDS`; the host takes the boxes past them into the
/// last, so none stands on nothing.
pub const STANDS_AT: usize = STARTS_AT + STARTS_VEC4S;
pub const STANDS: usize = 64;

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
            dx - if kx { ANCHOR } else { 0 },
            dy - if ky { ANCHOR } else { 0 },
        );
        // [naga] each anchor by name, not an array a dynamic index reads.
        let (at, frac) = match (kx, ky) {
            (false, false) => (self.at[0], self.frac[0]),
            (true, false) => (self.at[1], self.frac[1]),
            (false, true) => (self.at[2], self.frac[2]),
            (true, true) => (self.at[3], self.frac[3]),
        };
        // Each step split into its whole steps and its fraction, the fractions' products
        // summed with the remainder and the half that rounds to the nearest, the whole steps'
        // apart, so no sum leaves an i32; the fractions' whole steps, a floor, added.
        let m = (1 << STEP_BITS) - 1;
        let step = |k: usize| {
            let (a, d) = (self.across[k], self.down[k]);
            let low = i * (a & m) + j * (d & m) + frac[k] + (1 << (STEP_BITS - 1));
            let high = i * (a >> STEP_BITS) + j * (d >> STEP_BITS);
            high + (low >> STEP_BITS)
        };
        // [naga] each axis by itself, not a loop (seed.rs `node_seed`).
        [
            at[0].wrapping_add(step(0)),
            at[1].wrapping_add(step(1)),
            at[2].wrapping_add(step(2)),
        ]
    }

    /// Its words (`STARTS_VEC4S` vec4s), as `of_words` reads them.
    pub fn words(&self) -> [u32; 4 * STARTS_VEC4S] {
        let i = |v: [i32; 3]| [v[0] as u32, v[1] as u32, v[2] as u32, 0];
        let rows = [
            [self.base[0] as u32, self.base[1] as u32, 0, 0],
            i(self.across),
            i(self.down),
            i(self.at[0]),
            i(self.at[1]),
            i(self.at[2]),
            i(self.at[3]),
            i(self.frac[0]),
            i(self.frac[1]),
            i(self.frac[2]),
            i(self.frac[3]),
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
        let i = |v: [i32; 4]| [v[0], v[1], v[2]];
        let b = w(0);
        Starts {
            base: [b[0], b[1]],
            across: i(w(1)),
            down: i(w(2)),
            at: [i(w(3)), i(w(4)), i(w(5)), i(w(6))],
            frac: [i(w(7)), i(w(8)), i(w(9)), i(w(10))],
        }
    }
}

/// The clipmap's tiles, `TILE` slots a side, a workgroup each of the
/// slots' pass (pxl-render light.wgsl `light_cs`), dispatched over the whole map.
pub const TILE: u32 = 8;

/// The tiles a cast claims at most, nearest the eye first (`claims`, pxl-render
/// light_propagate.wgsl `light_claim_cs`): of the tiles it wants, those no cast of the epoch
/// claimed and those owed (`READ_OWED`), by ring about the box's middle, so a new light frame,
/// a slide or a changed column is cast over several frames, never more a frame. A tile wanted
/// past it is wanted again by the next cast, which the work it leaves owed makes due where
/// nothing else moves (`node_cache::OWED_AT`); until it is claimed the map holds it lit, or as its
/// last cast left it where owed (light_lay.wgsl).
pub const CLAIM_BUDGET: u32 = 4096;

/// The light's beam (pxl-render beam_hit.wgsl, beam.wgsl, beam.light.wgsl, light_cell.wgsl
/// `light_cell_cs`):
/// a beam ray at every `BEAM`-th texel of the box across and down,
/// `BEAM_WINDOW` past it on each side, each walked from the world's root along
/// the light's rays until a cell that fills or lacks or a split narrower than
/// `2√2` beam spacings (`Light::spacing`), at the band of the tile it lies on
/// (`corner_at`, `Light::band`) as a slot asks its own, leaving its line and
/// how far it got less `margin`; each `BEAM`-square cell of the box starts at
/// the least of the beam rays within `BEAM_WINDOW` of its corners, and its
/// texels resume from that ray's line there where every beam ray of the
/// window asked their band, and walk from the root where one asked another
/// (`cast::Detail::alike`, `beam::CELL_RESUMES`), as the proof below holds
/// for walks alike.
///
/// Why that is a texel's root walk, as the eye's proof has it with the
/// rays parallel: take `X`, the cell a texel's ray ends at, `e` its entry,
/// and `Y` the smallest cube holding `e` at least `√2` beam spacings wide.
/// Along the light `Y` covers a disk half its side across, which holds a beam
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
/// (`beam::LINE_NODE`, `beam::LINE_INHERITANCE`), then its origin
/// (`LINE_ORIGIN`), which its nodes are held from (`cast::Node::moved`), as
/// every light ray starts apart.
pub const LINE_ORIGIN: u32 = beam::LINE_WORDS;

/// The box texel beam ray `c` starts over, across and down: every `BEAM`-th,
/// from `BEAM_WINDOW` of them before the box's first (pxl-render light_list.wgsl
/// `corner`).
pub const fn corner_at(c: [u32; 2]) -> [i32; 2] {
    let (b, w) = (BEAM as i32, BEAM_WINDOW as i32);
    [(c[0] as i32 - w) * b, (c[1] as i32 - w) * b]
}

/// How far past the disk a receiver reads the map reaches, in texels, at
/// worst (pxl-render shadow.wgsl `shadow_visibility`): its corner tap `SPREAD`
/// each way, and past it the neighbor its linear compare filters in, a texel
/// each way, so both along a diagonal. The distance is measured at the
/// receiver's offset point, so its normal offset adds none; and the fade and
/// `Light::read` both measure from the snapped box's middle, its one matrix
/// (pxl-render gpu/shadow.rs `aim`), so its snap adds none.
pub const READ_PAD: f32 = (SPREAD + FILTER) * SQRT_2;
const FILTER: f32 = 1.0;

/// How far past the disk a receiver reads a tile's middle may lie and the tile still hold a
/// slot it reads (`Light::reads_tile`): `READ_PAD` and half the tile's diagonal.
pub const TILE_READ_PAD: f32 = READ_PAD + TILE as f32 * core::f32::consts::FRAC_1_SQRT_2;

/// The tiles a receiver that reads a slot may stand in, across and down from
/// the slot's tile, from the first to the last: a tap reads that far
/// (pxl-render light_propagate.wgsl).
pub const READ_TILES: [i32; 2] = [-2, 1];
pub const LINE_WORDS: u32 = LINE_ORIGIN + 3;

/// The beam rays across the map `size` texels a side, and down.
pub const fn beam_across(size: u32) -> u32 {
    size / BEAM + 1 + 2 * BEAM_WINDOW
}

/// The cells across the map, and down.
pub const fn cells_across(size: u32) -> u32 {
    size / BEAM
}

/// Where the beam's words lie in the list's buffer, its first: the lines,
/// a beam ray each, then how far a cell's texels may start from each, a word
/// a beam ray in their order (`hand_at`), as the eye's (`beam::Grid::hand_at`),
/// then the cells' starts; and all its words.
pub(crate) const fn lines_at(_size: u32) -> u32 {
    0
}

pub const fn line(size: u32, ray: u32) -> u32 {
    lines_at(size) + ray * LINE_WORDS
}

pub const fn hand_at(size: u32, ray: u32) -> u32 {
    lines_at(size) + beam_across(size) * beam_across(size) * LINE_WORDS + ray
}

pub(crate) const fn cells_at(size: u32) -> u32 {
    lines_at(size) + beam_across(size) * beam_across(size) * (LINE_WORDS + 1)
}

pub const fn cell(size: u32, [i, j]: [u32; 2]) -> u32 {
    cells_at(size) + (j * cells_across(size) + i) * beam::CELL_WORDS
}

pub(crate) const fn beam_words(size: u32) -> u32 {
    cells_at(size) + cells_across(size) * cells_across(size) * beam::CELL_WORDS
}

/// How far short of where a beam ray stopped a cell's texels start: the
/// diagonal of a cube `2√2` beam spacings `width` units wide, `2√6 ·
/// width`, and a Q3 step's rounding of each ray's start on the near plane,
/// two.
pub fn margin(width: f32) -> f32 {
    2.0 * cast::SQRT_6 * width + 2.0 / (1u32 << FRAC_W) as f32
}

/// The shadow map as the eye's rays are cast, every slot every frame
/// (pxl-engine `raster`): a sample every `SAMPLE` slots across and down
/// walked (pxl-render sample.wgsl `sample_cs`), the cells they stopped at written
/// as boxes by the eye's writer (pxl-render boxes.wgsl `write_box`) and drawn
/// from the light by the eye's raster, orthographic (pxl-render raster.wgsl),
/// in the box's order (`Light::texel`), each texel keeping the nearest its own
/// ray enters; and a slot whose ray enters its visible sample, tested again
/// with its own ray as the eye's pixels test theirs (`cast::entry`), takes it
/// (`light_cs`); the rest are listed and walked apart (`misses_at`). A cell
/// no sample stopped at may stand in front of the visible one, as in the
/// eye's. The raster's `EYED` case has no
/// light's: every ray starts on the near plane, before every box.
///
/// The samples' words, the eye's samples' own (`beam::SAMPLE_WORDS`), past
/// the beam's in the list's buffer, a sample `size / SAMPLE` across and
/// down, by its index `j · across + i`, which its box is drawn with
/// (`raster::id_of`).
pub(crate) const fn samples_at(size: u32) -> u32 {
    beam_words(size)
}

pub const fn sample(size: u32, idx: u32) -> u32 {
    samples_at(size) + idx * beam::SAMPLE_WORDS
}

/// The four samples about slot `slot`, box texel `p` of the map `size` a side, that a slot
/// takes the sky off where each went on to its reach (pxl-render light.wgsl `sky`): the sample
/// over the slot and the next across, down and both; none where they lie not about the texel,
/// within a sample of the box's edge.
pub fn samples_about(size: u32, slot: [u32; 2], p: [u32; 2]) -> Option<[[u32; 2]; 4]> {
    let d = SAMPLE;
    let r = [slot[0] % d, slot[1] % d];
    let about = p[0] >= r[0] && p[0] - r[0] + d < size && p[1] >= r[1] && p[1] - r[1] + d < size;
    let s = [slot[0] / d, slot[1] / d];
    about.then_some([s, [s[0] + 1, s[1]], [s[0], s[1] + 1], [s[0] + 1, s[1] + 1]])
}

pub(crate) const fn sample_words(size: u32) -> u32 {
    sample(size, (size / SAMPLE) * (size / SAMPLE))
}

/// The slots whose ray took no visible sample, listed for the holes' queue
/// (pxl-render queue.wgsl) apart from the slots that took theirs
/// (`light_cs`), as the eye's missed pixels are, by the same listing
/// (pxl-render list.wgsl `enlist`, `ray::MISS_ARGS_WORDS`). Past the samples: how many,
/// copied in once the slots are done, as the eye's count is (`ray::WALKED_CNT`), then a slot
/// each, its X and its Y over 16 bits; `size²` of them, a slot listed once a cast.
pub const fn misses_cnt(size: u32) -> u32 {
    sample_words(size)
}

pub const fn misses_at(size: u32) -> u32 {
    misses_cnt(size) + 1
}

/// The tiles a receiver reads, past the misses: a word a tile (`TILE` texels a side), `tiles`
/// a row, the number of the cast that last claimed it in its epoch (`HEAD_EPOCH`), read where
/// it lies within its taps of the way up from a tile a receiver stands on (`LISTED_STOOD`;
/// pxl-render light_propagate.wgsl `up`), and whether it is owed (`READ_OWED`). A tile this
/// cast's number names is the cast's to cast; one an earlier cast of the reads' epoch names
/// holds what that cast left (`reads_epoch`); no receiver reads any other. Its number is
/// written by the claim (light_propagate.wgsl `light_claim_cs`) and set to none by the slide
/// (light_slide.wgsl), and read under `READ_OWED` (`read_cast`).
pub const fn reads_at(size: u32) -> u32 {
    misses_at(size) + size * size
}

/// A tile owed: one cast that still holds a slot its queue's budget did not finish, carried
/// (pxl-render queue_last.wgsl `unreached`, which sets the bit; the claim clears it). An owed
/// tile keeps what its cast left, as the map lays it, and is wanted again (`LISTED_WANTED`),
/// claimed nearest first like a tile no cast of the epoch read (`claims`). Its parked rays
/// are held where the cast before claimed it exactly, and resumed where the claim follows it,
/// the light's content, the box and the node cache as they were (`HEAD_CARRIES`); else its
/// slots are cast again.
pub const READ_OWED: u32 = 1 << 31;

/// The cast a tile's read word names, owed or not (`READ_OWED`).
pub const fn read_cast(word: u32) -> u32 {
    word & !READ_OWED
}

pub const fn tiles(size: u32) -> u32 {
    size / TILE
}

/// The tiles each way past its own a receiver stands on where the eye stands
/// (`HEAD_FOOTPRINT`; `Light::box_tiles`): a jittered ray of the frames after lies within half
/// a pixel of an answered neighbor's, so where the surface runs on between them its hit lies
/// on the span of their tiles, within this many of either's; past it the two met apart, an
/// edge, and each stands on its own.
pub const FOOTPRINT_TILES: i32 = 3;

impl Light {
    /// How far from the eye a surface receives the shadow map, in units, on the map `size`
    /// texels a side: the share `reach_ndc` of the box's half-width, the effects' reach
    /// (pxl-kernel domain/scene/stage.rs `STAGE_AT::EFFECT_REACH`, effect.wgsl `effect_share`,
    /// 0 past it), and a texel past it, the normal offset a receiver reads the map at.
    pub fn reach(&self, size: u32) -> f32 {
        let width = self.width();
        self.reach_ndc * width * (size as f32 * 0.5) + width
    }

    /// Whether the box from `lo` to `hi`, units from the eye, comes within `reach` of it.
    pub fn within(lo: [f32; 3], hi: [f32; 3], reach: f32) -> bool {
        let gap = |k: usize| lo[k].max(-hi[k]).max(0.0);
        let (x, y, z) = (gap(0), gap(1), gap(2));
        x * x + y * y + z * z <= reach * reach
    }

    /// The tiles a receiver on the box from `lo` to `hi`, units from the eye, stands on, on
    /// the map `size` texels a side: the lowest and the highest of the tiles its eight corners
    /// lie on (`tile_of`), which hold every point of it, the light's rays parallel, and
    /// `FOOTPRINT_TILES` more each way where `footprint`; none where it lies past the reach
    /// (`reach`, `within`) or past the box. The one rule every receiver stands by: the box the
    /// eye's draw writes for a voxel in view (`cast::asset_box`; pxl-render
    /// record_draw.wgsl `record_draw_cs`, which stands it as it writes it), which every pixel
    /// that took that voxel hit on (visible.wgsl `visible_cell`); a walked pixel's hit, a box of one point (light_mark.wgsl); and the box
    /// a drawn receiver stands within (frame/draw.rs `Stand`, light_mark_draws.wgsl).
    pub fn box_tiles(
        &self,
        size: u32,
        lo: [f32; 3],
        hi: [f32; 3],
        footprint: bool,
    ) -> Option<[[i32; 2]; 2]> {
        if !Light::within(lo, hi, self.reach(size)) {
            return None;
        }
        let mut first = [i32::MAX; 2];
        let mut last = [i32::MIN; 2];
        // [naga] each corner by its bits, not an array a dynamic index reads.
        let mut k = 0;
        while k < 8 {
            let v = [
                if k & 1 == 0 { lo[0] } else { hi[0] },
                if k & 2 == 0 { lo[1] } else { hi[1] },
                if k & 4 == 0 { lo[2] } else { hi[2] },
            ];
            let t = self.tile_of(v);
            first = [first[0].min(t[0]), first[1].min(t[1])];
            last = [last[0].max(t[0]), last[1].max(t[1])];
            k += 1;
        }
        let grow = if footprint { FOOTPRINT_TILES } else { 0 };
        let n = tiles(size) as i32 - 1;
        let first = [(first[0] - grow).max(0), (first[1] - grow).max(0)];
        let last = [(last[0] + grow).min(n), (last[1] + grow).min(n)];
        (first[0] <= last[0] && first[1] <= last[1]).then_some([first, last])
    }
}

/// What a cast lists, a tile's `i | j << 16` each, in the list's buffer: what feeds a tile
/// it claimed at each level of the lattice, by the reach it feeds within (pxl-render
/// light_list.wgsl `light_needs`), each the box tile it lies on: the beam rays' corners
/// (`LISTED_CORNERS`), the tiles whose cells and samples feed one (`LISTED_FED`), and the
/// tiles claimed, whose slots it casts (`LISTED_CLAIMED`); the tiles a receiver stands on
/// (`LISTED_STOOD`), the world's first, then the drawn receivers' (pxl-render
/// light_mark.wgsl, light_mark_draws.wgsl); and the tiles a receiver reads, which the map is
/// laid in (`LISTED_READ`; light_propagate.wgsl, gpu/shadow.rs `Shadow::begin`); and the tiles
/// a receiver reads that no cast of the epoch claimed, or owed (`READ_OWED`), which the claim
/// takes nearest first (`LISTED_WANTED`, `claims`). Each list's
/// count, then its entries, the count copied in once propagation is done (`ARGS_COUNTS`), as
/// the eye's is (`ray::WALKED_CNT`); and a word for every place of each, in the args'
/// buffer (`listed_marks_at`): the last cast that listed it, so each is listed once a cast,
/// but the stood list's and the read list's, which name the cast the world's receivers were
/// last stood afresh in (`HEAD_WORLD`): a tile stood on, the world's receivers' or a drawn
/// one's, and a tile read, stays listed in every cast after it until they are stood afresh
/// again, each cast adding what it stands on and what that reads (`kept_at`). The passes that
/// cast tiles take their threads off the first
/// three (pxl-render light_list.wgsl `light_item`): `LIST_ACROSS` workgroups of
/// `LISTED_LANES` a row, an entry its side's square of them, a cast that begins an epoch as
/// any other (`CLAIM_BUDGET`); the propagation a thread a stood tile; the claim a thread a tile
/// wanted, as many rows (`CLAIM_DISPATCH`).
pub const LISTED_CORNERS: u32 = 0;
pub const LISTED_FED: u32 = 1;
pub const LISTED_CLAIMED: u32 = 2;
pub const LISTED_STOOD: u32 = 3;
pub const LISTED_READ: u32 = 4;
pub const LISTED_WANTED: u32 = 5;
pub const LISTS: u32 = 6;
pub const LIST_ACROSS: u32 = 64;
pub const LISTED_LANES: u32 = 64;
const _: () = assert!(
    LISTED_LANES == beam::SAMPLE_SIDE * beam::SAMPLE_SIDE,
    "a listed pass's workgroup is the samples' passes' square"
);

/// The places of list `which` on the map `size` texels a side: every beam
/// ray's corner, or every tile.
pub const fn listed_room(size: u32, which: u32) -> u32 {
    if which == LISTED_CORNERS {
        beam_across(size) * beam_across(size)
    } else {
        tiles(size) * tiles(size)
    }
}

/// The first of list `which`'s marks past the args' `ARGS_WORDS`, a word a place
/// (`LISTED_CORNERS`).
pub const fn listed_marks_at(size: u32, which: u32) -> u32 {
    let mut at = 0;
    let mut k = 0;
    while k < which {
        at += listed_room(size, k);
        k += 1;
    }
    at
}

/// List `which`'s count, its entries after it (`LISTED_CORNERS`).
pub const fn listed_at(size: u32, which: u32) -> u32 {
    let mut at = reads_at(size) + tiles(size) * tiles(size);
    let mut k = 0;
    while k < which {
        at += 1 + listed_room(size, k);
        k += 1;
    }
    at
}

/// The samples their walk over the node cache left, for the walk that asks the world
/// (pxl-render sample_hit.wgsl, sample.wgsl), past the lists, as the eye's are past its words
/// (`beam::Grid::lefts_at`): how many, copied in once it is done, then each, its X and its Y
/// over 16 bits.
pub const fn lefts_at(size: u32) -> u32 {
    listed_at(size, LISTS)
}

/// The beam rays the same walk left (pxl-render beam_hit.wgsl, beam.wgsl, beam.light.wgsl), past
/// the samples left, as the eye's are (`beam::Grid::corners_at`): how many, copied in once it is
/// done, then each, its X and its Y over 16 bits.
pub const fn corners_left_at(size: u32) -> u32 {
    lefts_at(size) + 1 + (size / SAMPLE) * (size / SAMPLE)
}

/// Past the beam rays left, two words kept from cast to cast: the number of the last cast its
/// slide ran in (`SLIDE_RAN`), set by the slide's first thread (pxl-render light_slide.wgsl);
/// and the number of the first cast whose reads hold where a cast that owed a slide stood no
/// receiver, so ran none (`READS_FROM`), set by the one thread that lists a cast's first stood
/// tile where the slide last ran before the last cast that owed one (`HEAD_SLIDE_OWED`;
/// light_stood.wgsl `stood_first`): the reads' epoch is the later of it and `HEAD_EPOCH`
/// (`reads_epoch`).
pub const fn slide_at(size: u32) -> u32 {
    corners_left_at(size) + 1 + beam_across(size) * beam_across(size)
}
pub const SLIDE_RAN: u32 = 0;
pub const READS_FROM: u32 = 1;

/// Past them, what a cast keeps for the next until the world's receivers are stood afresh
/// (`HEAD_WORLD`), copied out once its propagation is done and back in as the next begins
/// (gpu/ray/cast.rs): the stood list's args' first `STOOD_KEPT` words, then the read list's
/// count (`KEPT_READ`). The count kept is the stood tiles the casts before propagated, so a
/// cast propagates only those stood after them (pxl-render light_propagate.wgsl); none where
/// they are stood afresh.
pub const fn kept_at(size: u32) -> u32 {
    slide_at(size) + 2
}
pub const KEPT_READ: u32 = STOOD_KEPT;
pub const KEPT_WORDS: u32 = KEPT_READ + 1;

/// Past them, the place each tile holds the light's holes' parked rays at, a word a tile by its
/// slots' tile (`tiles` a row, the slots' own grid, which the box slides over), its number among
/// the cast's claims (`ARGS_PLACED`), as the cast that last claimed it laid it
/// (light_propagate.wgsl `light_claim_cs`, which writes it; pxl-render light_aim.wgsl
/// `light_place_at`, which the queue reads it by).
pub const fn places_at(size: u32) -> u32 {
    kept_at(size) + KEPT_WORDS
}

/// The list's buffer's words: the beam's, the samples, the misses, the
/// reads, their squares, the lists, the samples left, the beam
/// rays left, the slide's two words, what the world's receivers left and the places; the
/// holes' region past them (`region_words`).
pub const fn list_words(size: u32) -> u32 {
    places_at(size) + tiles(size) * tiles(size)
}

/// The slots a cast parks the light's holes' rays for: every slot of every tile it claims, at
/// most `CLAIM_BUDGET` tiles of `TILE`² slots (`claims`), a state each at its tile's place and
/// its slot within the tile (pxl-render light_aim.wgsl `light_state_at`).
pub const fn parked_room() -> u32 {
    CLAIM_BUDGET * TILE * TILE
}

/// The light's holes' region, past the list's words: a cast's parked states and the cast
/// before's, by the cast number's parity, so a claim takes an owed tile's rays from the one
/// while the queue writes the other (`ray::region_words`).
pub const fn region_words() -> u32 {
    ray::region_words(2 * parked_room())
}

/// The light's holes' stops, past its holes' region: a stop each of the most holes a cast
/// lists, every slot of the tiles it claims (`parked_room`), each `beam::HIT_WORDS`, kept by
/// pxl-render queue.wgsl `finish` and taken into the node cache's drawable entries after its
/// queue (record_stops.wgsl).
pub const fn hits_at(size: u32) -> u32 {
    list_words(size) + region_words()
}

/// The light's list, its holes' region and its stops with it: the buffer's words.
pub const fn buffer_words(size: u32) -> u32 {
    hits_at(size) + parked_room() * beam::HIT_WORDS
}

/// The rings of the box's tiles about its middle, a whole tile wide each: the order a cast
/// claims in (`claims`); fewer than the tiles across.
pub const fn rings(size: u32) -> u32 {
    tiles(size)
}

/// The ring box tile `t` of the map `size` texels a side lies in (`rings`): its middle's
/// distance from the box's, in whole tiles.
pub fn ring(size: u32, t: [i32; 2]) -> u32 {
    let half = tiles(size) as f32 * 0.5;
    let d = [t[0] as f32 + 0.5 - half, t[1] as f32 + 0.5 - half];
    libm::sqrtf(d[0] * d[0] + d[1] * d[1]) as u32
}

/// Whether a cast claims a tile it wants of a ring that `before` tiles it wants in the rings
/// within it and `own` in its own: every one of a ring all of whose tiles fit within
/// `CLAIM_BUDGET` with the nearer, and of the one ring that straddles it the first of its own
/// to take a place, `rank` the place it took among them; none past. So a cast claims nearest
/// first and `CLAIM_BUDGET` tiles at most (pxl-render light_propagate.wgsl `light_claim_cs`).
pub const fn claims(before: u32, own: u32, rank: u32) -> bool {
    before + own <= CLAIM_BUDGET || (before < CLAIM_BUDGET && rank < CLAIM_BUDGET - before)
}

/// The stood list's args, a buffer of their own (pxl-render light_mark.wgsl `Stood`), as a
/// list walked apart lays its own (`ray::LEFT_EMPTY`; list.wgsl `enlist`): the propagation's
/// dispatch over the stood tiles, a row as each `MISS_LANES · MISS_ACROSS` is listed
/// (`STOOD_LIST`), and their count (`STOOD_CNT`); then the slide's dispatch (`STOOD_SLIDE`):
/// its tiles' workgroups across once a tile is stood, the stood list's first setting it, and
/// down where the cast owes a slide, the host's, so a cast that owes one and stands nothing
/// slides nothing (`READS_FROM`). The first `STOOD_KEPT` are kept from cast to cast until the
/// world's receivers are stood afresh (`kept_at`); the host sets the rest each cast.
pub const STOOD_LIST: u32 = 0;
pub const STOOD_CNT: u32 = 3;
pub const STOOD_SLIDE: u32 = 4;
pub const STOOD_WORDS: u32 = 7;
pub const STOOD_KEPT: u32 = STOOD_SLIDE + 1;

/// The reads' epoch: the first cast whose reads hold, the later of the epoch the host began
/// (`HEAD_EPOCH`) and the one a slide unrun began (`READS_FROM`).
pub const fn reads_epoch(epoch: u32, reads_from: u32) -> u32 {
    if reads_from > epoch {
        reads_from
    } else {
        epoch
    }
}

/// The workgroups of the passes the light's cast takes off what it lists, a buffer of their
/// own: each pass's three words (x, y, z) from its `ARGS_*` word; the map's lay's four
/// (`ARGS_LAY`), a draw's; `ARGS_PASSES` in all; each list's count (`ARGS_COUNTS`). A cast
/// starts with none, `LIST_ACROSS` workgroups a row and no row, no list holding any; a tile
/// claimed as the cast's to cast (pxl-render light_claim.wgsl) opens the rows what it lists
/// takes (`listed_at`); a cast that claims none, as where the receivers moved only within the
/// tiles cast, casts none. The lay six vertices and an instance a tile read
/// (light_propagate.wgsl). So a cast whose receivers stand on nothing claims, lays and draws
/// no tile. Past `ARGS_WORDS`, which a cast writes, the lists' marks, which last from cast to
/// cast (`args_words`). The lay's instances are the read list's count, kept from cast to cast
/// with the list (`KEPT_READ`), set once the tiles wanted are listed (light_propagate.wgsl
/// `light_want_cs`). The passes that write these read no count of
/// theirs here: the drawn receivers' and the claim's dispatches are buffers of their own
/// (`DRAWS_DISPATCH_WORDS`, `CLAIM_DISPATCH`).
pub const ARGS_BEAM: u32 = 0;
pub const ARGS_CELLS: u32 = 3;
pub const ARGS_SAMPLE: u32 = 6;
pub const ARGS_SAMPLE_BOX: u32 = 9;
pub const ARGS_SLOT: u32 = 12;
pub const ARGS_LAY: u32 = 15;
/// The claims' count, each claim's place (`places_at`); the count of the straddling ring's
/// claims (`claims`); and the tiles wanted and left unclaimed, the first of the cast's owed
/// work (`node_cache::OWED_AT`); none at a cast's start.
pub const ARGS_PLACED: u32 = 19;
pub const ARGS_STRADDLE: u32 = 20;
pub const ARGS_LEFT: u32 = 21;
pub const ARGS_PASSES: u32 = 22;
/// The drawn receivers' dispatch, a buffer of its own, as many workgroups as they are, written
/// whole each cast by the pass that opens it (pxl-render light_mark_draws_open.wgsl) and read
/// by theirs, which writes the lists' marks here (light_mark_draws.wgsl).
pub const DRAWS_DISPATCH_WORDS: u32 = 3;
/// The claim's dispatch over the tiles wanted (`LISTED_WANTED`), a buffer of its own, as a
/// cast starts: `LIST_ACROSS` workgroups a row and no row; the pass that wants them raises its
/// rows as it lists them (pxl-render light_propagate.wgsl `want`), and the claim, which writes
/// these args, reads its count from it (light_claim.wgsl).
pub const CLAIM_DISPATCH: [u32; 3] = [LIST_ACROSS, 0, 1];
pub const ARGS_COUNTS: u32 = ARGS_PASSES;
pub const ARGS_WORDS: u32 = ARGS_COUNTS + LISTS;
/// The vertices of a tile the map is laid in, two triangles (pxl-render light_map.wgsl).
pub const LAY_VERTICES: u32 = 6;

/// The tiles a cast wants by ring (`rings`), past the lists' marks, counted as they are
/// wanted (light_propagate.wgsl `want`) and read by the claim, none at a cast's start.
pub const fn rings_at(size: u32) -> u32 {
    listed_marks_at(size, LISTS)
}

/// The args' buffer's words on the map `size` texels a side.
pub const fn args_words(size: u32) -> u32 {
    ARGS_WORDS + rings_at(size) + rings(size)
}

/// Box texel `p`'s ray: its origin, where its world texel's middle lies on
/// the near plane, rounded to a Q3 step as its anchor lays it (`Starts`);
/// and the ray.
pub fn ray(s: &Light, starts: &Starts, p: [u32; 2]) -> ([i32; 3], Ray) {
    let o = starts.start([
        s.corner[0].wrapping_add(p[0] as i32),
        s.corner[1].wrapping_add(p[1] as i32),
    ]);
    (o, Ray::new(s.dir, s.t_max))
}
/// The depth a texel's walk `met` leaves in the map: where it stopped
/// along its ray, as a share of the box's depth, its t times the reach's
/// reciprocal (`Light::t_max_inverse`), pushed back as its face's axis is
/// (`Light::bias`); the far plane, 1, where it stopped at nothing.
pub fn depth<A>(s: &Light, met: &Trace<A>) -> f32 {
    if met.ends != End::Hit {
        return 1.0;
    }
    let d = met.hit.t * s.t_max_inverse() + axis(s.bias, met.hit.axis);
    if d < 1.0 {
        d
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::answer;
    use crate::cast::Hit;
    use crate::cast::Node;
    use crate::content::Sample;
    use crate::overwrite::AIR;
    use crate::param::light;

    /// Starts as a host lays them for the eye at Q3 `eye`, off the light's
    /// words alone: each anchor where its texel lies off the eye, rounded.
    fn starts_of(eye: [i32; 3], s: &Light) -> Starts {
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
            across: s.across.map(|v| fixed(v * q3)),
            down: s.down.map(|v| fixed(v * q3)),
            at: [a0.0, a1.0, a2.0, a3.0],
            frac: [a0.1, a1.1, a2.1, a3.1].map(|f| f.map(fixed)),
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
    fn down() -> Light {
        Light::of(
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
    fn met(ends: End, t: f32, axis: u32) -> Trace<()> {
        let node = Node::ROOT;
        Trace {
            ends,
            hit: Hit {
                t,
                axis,
                node,
                at: (),
                asset: [0; 2],
            },
        }
    }

    #[test]
    fn texel_starts_on_near_plane() {
        let s = down();
        let (o, ray) = ray(&s, &starts_of([100, 200, 300], &s), [0, 0]);
        // The top-left texel's middle: half the box across to the left, up.
        assert_eq!(o, [100 - 256, 200 + 256, 300 + 4 * 256]);
        assert_eq!(ray.dir, [0.0, 0.0, -1.0]);
        assert_eq!(ray.t_max, 8.0);
        // The next across and the next down, a texel's side on.
        assert_eq!(
            super::ray(&s, &starts_of([0; 3], &s), [1, 1]).0,
            [256, -256, 4 * 256]
        );
    }

    #[test]
    fn square_face_no_slope() {
        let s = down();
        assert_eq!(depth(&s, &met(End::Hit, 2.0, 2)), 0.25);
        assert_eq!(depth(&s, &met(End::Miss, 8.0, 0)), 1.0);
        assert_eq!(depth(&s, &met(End::Lacks, 2.0, 2)), 1.0);
    }

    #[test]
    fn leaning_face_pushed_back() {
        // The light 45 degrees over x: a top face changes a texel's side in
        // depth across a texel, which is pushed back `SLOPE_BIAS` times.
        let r = core::f32::consts::FRAC_1_SQRT_2;
        let s = Light::of(
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
        let d = depth(&s, &met(End::Hit, 2.0, 2));
        assert!(
            (d - (0.25 + light::SLOPE_BIAS * texel / 8.0)).abs() < 1e-6,
            "{d}"
        );
    }

    #[test]
    fn plane_keeps_depth() {
        // The same box: a top face's depth deepens a texel's side a texel
        // across, toward the light's going, and not at all down.
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

    /// The light's words read back are the light.
    #[test]
    fn words_are_fields() {
        let s = Light {
            corner: [-7, 9],
            reach_ndc: 0.75,
            ..down()
        };
        assert_eq!(Light::of_words(&s.words()), s);
    }

    /// A point lies on the texel whose ray starts above it.
    #[test]
    fn point_on_texel() {
        let s = down();
        let (o, _) = ray(&s, &starts_of([0; 3], &s), [1, 0]);
        // Texel (1, 0)'s start, from the eye at 0, and a point straight under it.
        let v = o.map(|k| k as f32 / (1 << FRAC_W) as f32);
        let at = s.texel_at([v[0], v[1], v[2] - 3.0]);
        assert!((at[0] - 1.0).abs() < 1e-5 && at[1].abs() < 1e-5, "{at:?}");
    }

    /// A point along a texel's ray lies on that texel, whatever the light's
    /// slant, the box's corner and its anchors' rounding.
    #[test]
    fn point_on_texel_ray() {
        let r = core::f32::consts::FRAC_1_SQRT_2;
        let s = Light::of(
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
            let (o, ray) = ray(&s, &st, p);
            let v = |k: usize, t: f32| o[k].wrapping_sub(eye[k]) as f32 / q3 + ray.dir[k] * t;
            let at = s.texel_at([v(0, 3.0), v(1, 3.0), v(2, 3.0)]);
            assert!(
                // The starts' Q3 rounding, far under a tile.
                (at[0] - p[0] as f32).abs() < 0.05 && (at[1] - p[1] as f32).abs() < 0.05,
                "{p:?} {at:?}"
            );
        }
    }

    /// A receiver reads the disk of `reach_ndc` about the box's
    /// middle, and a pass past it its pad.
    #[test]
    fn receiver_reads_disk() {
        let s = Light {
            reach_ndc: 0.5,
            ..down()
        };
        // A box 16 texels a side: its middle is 8, the disk 4 texels out.
        assert!(s.read(16, [8, 8], 0.0));
        assert!(s.read(16, [11, 7], 0.0));
        assert!(!s.read(16, [13, 7], 0.0));
        assert!(s.read(16, [13, 7], 2.0));
        assert!(!s.read(16, [0, 0], 2.0));
    }

    #[test]
    fn slot_holds_texel() {
        let mut s = Light::of(
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

    /// Ground whose top is a few units of bumps, as each zoom's cells take it:
    /// a cell is stone where its floor lies under the top at its middle, so
    /// a coarser zoom's steps are wider; a node coarser than asked is a split
    /// where it reaches under the bumps' height, air over it.
    struct Bumps;

    const BUMPS: f32 = 6.0;

    fn top(x: f32, y: f32) -> f32 {
        3.0 * libm::sinf(x * 0.37) + 3.0 * libm::cosf(y * 0.29)
    }

    impl cast::Query for Bumps {
        type At = ();
        fn root(&mut self, _: [i32; 3]) {}
        fn child(&mut self, _: &(), _: Zoom, _: [i32; 3]) {}
        fn node(
            &mut self,
            _: &mut (),
            zoom: Zoom,
            c: [i32; 3],
            [_, near]: [Zoom; 2],
            _: f32,
        ) -> answer::In<[u32; 2]> {
            let side = libm::ldexpf(1.0, UNIT_ZOOM as i32 - zoom as i32);
            let floor = c[2] as f32 * side;
            let (kind, asset) = if zoom >= near {
                let mid = |k: usize| (c[k] as f32 + 0.5) * side;
                let filled = answer::fill_word(Sample::GROUND, answer::EIGHTHS);
                let under = floor < top(mid(0), mid(1));
                (answer::Kind::Cell, [if under { filled } else { AIR }, 0])
            } else if floor < BUMPS {
                (answer::Kind::Split, answer::BOX_WHOLE)
            } else {
                (answer::Kind::Cell, [AIR, 0])
            };
            answer::In { kind, asset }
        }
    }

    /// A box 512 units a side looking straight down over the bumps, 64
    /// texels a side, a tile 64 units: its bands run from the unit cells'
    /// about its middle to two zooms coarser at its edge.
    fn over_bumps(corner: [i32; 2]) -> Light {
        Light::of(
            [
                [256.0, 0.0, 0.0],
                [0.0, 256.0, 0.0],
                [0.0, 0.0, -128.0],
                [0.0, 0.0, 64.0],
            ],
            64,
            corner,
            1.0,
        )
    }

    /// The words a cast of box tile `tile` leaves, a slot each of its
    /// texels, every walk at the tile's band (`Light::band`) over the bumps.
    fn cast_tile(s: &Light, st: &Starts, range: [Zoom; 2], tile: [i32; 2]) -> std::vec::Vec<u32> {
        let b = s.band(64, range, tile);
        let detail = cast::Detail {
            range: [b, b],
            spacing: s.spacing(),
        };
        (0..TILE * TILE)
            .map(|k| {
                let p = [
                    tile[0] as u32 * TILE + k % TILE,
                    tile[1] as u32 * TILE + k / TILE,
                ];
                let (o, ray) = ray(s, st, p);
                let met = cast::first(&mut Bumps, o, &ray, detail, cast::Until::Opaque);
                word(depth(s, &met))
            })
            .collect()
    }

    /// A tile a slide of the box keeps (`Light::keeps`) holds what a fresh
    /// cast of it at its new place leaves, word for word; and a tile the
    /// slide moved into another band is cast again, as a fresh cast there
    /// leaves other words. The rays' starts are the light frame's, worked
    /// out once, as the host's anchors are (pxl-render gpu/shadow.rs
    /// `starts`).
    #[test]
    fn kept_tile_is_fresh_cast() {
        let (size, range) = (64, [UNIT_ZOOM - 3, UNIT_ZOOM]);
        let (n, t) = (tiles(size) as i32, TILE as i32);
        let was = over_bumps([512, 512]);
        let st = starts_of([0; 3], &was);
        let (mut whole, mut banded) = (0, 0);
        for (dx, dy) in [(1, 0), (0, -1), (2, 1), (-3, 2)] {
            let now = Light {
                corner: [was.corner[0] + dx * t, was.corner[1] + dy * t],
                ..was
            };
            for k in 0..n * n {
                let here = [k % n, k / n];
                let from = [here[0] + dx, here[1] + dy];
                let fresh = cast_tile(&now, &st, range, here);
                let tile = [here[0] as u32, here[1] as u32];
                if now.keeps(size, range, tile, was.corner) {
                    assert_eq!(
                        cast_tile(&was, &st, range, from),
                        fresh,
                        "{dx} {dy} {here:?}"
                    );
                    whole += 1;
                } else if (0..n).contains(&from[0]) && (0..n).contains(&from[1]) {
                    banded += u32::from(cast_tile(&was, &st, range, from) != fresh);
                }
            }
        }
        assert!(whole > 0 && banded > 0, "{whole} {banded}");
    }

    /// A box 64 texels a side, a unit each, over the eye, the light straight down, a receiver
    /// reading half of it: 8 tiles a side, and a reach of 17 units.
    fn over_eye() -> Light {
        Light::of(
            [
                [32.0, 0.0, 0.0],
                [0.0, 32.0, 0.0],
                [0.0, 0.0, -64.0],
                [0.0, 0.0, 32.0],
            ],
            64,
            [0, 0],
            0.5,
        )
    }

    /// The receivers' reach is the effects' share of the box's half-width and a texel past it.
    #[test]
    fn reach_is_effects_and_a_texel() {
        let s = over_eye();
        assert_eq!(s.width(), 1.0);
        assert_eq!(s.reach(64), 17.0);
    }

    /// A box stands on nothing past the reach, and on its tiles once any of it comes within.
    #[test]
    fn box_past_reach_stands_on_nothing() {
        let s = over_eye();
        assert_eq!(
            s.box_tiles(64, [17.5, -1.0, -1.0], [19.0, 1.0, 1.0], true),
            None
        );
        assert_eq!(
            s.box_tiles(64, [12.0, 12.0, 12.0], [13.0, 13.0, 13.0], true),
            None
        );
        assert!(s
            .box_tiles(64, [16.5, -1.0, -1.0], [19.0, 1.0, 1.0], false)
            .is_some());
        assert!(s
            .box_tiles(64, [-1.0, -1.0, -1.0], [1.0, 1.0, 1.0], false)
            .is_some());
    }

    /// Every point of a box within the reach stands on a tile its range holds, as every pixel
    /// that took a voxel hit on that voxel's box (pxl-render visible.wgsl `visible_entry`).
    #[test]
    fn box_tiles_hold_every_point() {
        let s = over_eye();
        let mut seed = 0x9e37_79b9u32;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as f32 / u32::MAX as f32
        };
        let mut boxes = 0;
        for _ in 0..2000 {
            let lo = [0, 1, 2].map(|_| next() * 60.0 - 30.0);
            let side = next() * 12.0;
            let hi = lo.map(|v| v + side);
            let within = Light::within(lo, hi, s.reach(64));
            let range = s.box_tiles(64, lo, hi, false);
            boxes += u32::from(range.is_some());
            for _ in 0..16 {
                let p = [0, 1, 2].map(|k| lo[k] + next() * (hi[k] - lo[k]));
                let t = s.tile_of(p);
                let on = (0..8).contains(&t[0]) && (0..8).contains(&t[1]);
                let held = range.is_some_and(|[first, last]| {
                    first[0] <= t[0] && t[0] <= last[0] && first[1] <= t[1] && t[1] <= last[1]
                });
                assert!(
                    !within || !on || held,
                    "{lo:?} {hi:?} {p:?} {t:?} {range:?}"
                );
            }
        }
        assert!(boxes > 100, "{boxes}");
    }

    /// A jittered ray between two answered neighbors' hits, on the line between them, where
    /// their tiles lie within `FOOTPRINT_TILES` of each other, stands on a tile either one's
    /// footprint holds (`box_tiles`), the tile of a point along the box's axes moving
    /// monotonically: a jittered walk between two voxels' pixels as between two walked ones.
    #[test]
    fn footprint_holds_the_jitter_between() {
        let s = over_eye();
        let mut spans = 0;
        for (a, b) in [
            ([0.5, 0.5, 0.0], [9.5, 3.0, -2.0]),
            ([-7.0, 2.0, 1.0], [-1.0, -9.0, 0.0]),
            ([3.0, 3.0, 0.0], [3.5, 3.2, 4.0]),
            ([-9.0, -9.0, 0.0], [12.0, 1.0, 0.0]),
        ] {
            let (ta, tb) = (s.tile_of(a), s.tile_of(b));
            if (ta[0] - tb[0]).abs() > FOOTPRINT_TILES || (ta[1] - tb[1]).abs() > FOOTPRINT_TILES {
                continue;
            }
            spans += 1;
            let range = s.box_tiles(64, a, a, true).unwrap();
            let held = |t: [i32; 2], [lo, hi]: [[i32; 2]; 2]| {
                lo[0] <= t[0] && t[0] <= hi[0] && lo[1] <= t[1] && t[1] <= hi[1]
            };
            assert!(held(tb, range), "{a:?} {b:?}");
            for k in 0..=64 {
                let f = k as f32 / 64.0;
                let p = [0, 1, 2].map(|i| a[i] + f * (b[i] - a[i]));
                let t = s.tile_of(p);
                assert!(held(t, range), "{a:?} {b:?} {p:?} {t:?}");
            }
        }
        assert!(spans >= 2, "{spans}");
    }

    /// The footprint grows a box's tiles `FOOTPRINT_TILES` each way, kept to the box.
    #[test]
    fn footprint_grows_by_its_tiles() {
        let s = over_eye();
        let v = [0.0, 0.0, 0.0];
        let [lo, hi] = s.box_tiles(64, v, v, false).unwrap();
        let [glo, ghi] = s.box_tiles(64, v, v, true).unwrap();
        assert_eq!(lo, hi);
        assert_eq!(
            glo,
            [lo[0] - FOOTPRINT_TILES, lo[1] - FOOTPRINT_TILES].map(|x| x.max(0))
        );
        assert_eq!(
            ghi,
            [hi[0] + FOOTPRINT_TILES, hi[1] + FOOTPRINT_TILES].map(|x| x.min(7))
        );
    }

    /// The reads' epoch is the later of the host's and the one a slide unrun began.
    #[test]
    fn reads_epoch_is_the_later() {
        assert_eq!(reads_epoch(5, 0), 5);
        assert_eq!(reads_epoch(5, 9), 9);
    }

    /// The words the host sets each cast, and the flag it sets every frame, lie in the head
    /// apart from each other and from every word the map's side lays (`head_layout`).
    #[test]
    fn host_words_lie_apart_in_the_head() {
        let host = [
            HEAD_CAST,
            HEAD_COUNTED,
            HEAD_EPOCH,
            HEAD_CORNER_WAS,
            HEAD_CORNER_WAS + 1,
            HEAD_FOOTPRINT,
            HEAD_WORLD,
            HEAD_SLIDE_OWED,
            HEAD_STANDS_WORLD,
        ];
        let laid: std::vec::Vec<usize> = head_layout(2048).iter().map(|&(k, _)| k).collect();
        for (i, a) in host.iter().enumerate() {
            assert!(*a < HEAD_WORDS);
            assert!(!laid.contains(a), "{a}");
            assert!(host[i + 1..].iter().all(|b| b != a), "{a}");
        }
    }

    /// Every box tile lies in a ring of its own count (`rings`), and a tile nearer the middle
    /// in no farther ring than one past it.
    #[test]
    fn rings_hold_every_tile_nearest_first() {
        let n = tiles(2048) as i32;
        let dist = |t: [i32; 2]| {
            let d = [t[0] - n / 2, t[1] - n / 2];
            d[0] * d[0] + d[1] * d[1]
        };
        for y in 0..n {
            for x in 0..n {
                assert!(ring(2048, [x, y]) < rings(2048), "{x} {y}");
            }
        }
        for (a, b) in [
            ([n / 2, n / 2], [n - 1, n - 1]),
            ([n / 2 + 3, n / 2], [0, n / 2]),
        ] {
            assert!(dist(a) < dist(b));
            assert!(ring(2048, a) <= ring(2048, b), "{a:?} {b:?}");
        }
    }

    /// Over any rings' counts, a cast claims every tile of each ring that fits with the nearer
    /// ones, as many of the ring that straddles the budget as fill it, none past, so
    /// `CLAIM_BUDGET` at most and every tile while the wanted fit (`claims`).
    #[test]
    fn claims_fill_the_budget_nearest_first() {
        for counts in [
            &[3u32, 9, 40][..],
            &[CLAIM_BUDGET][..],
            &[CLAIM_BUDGET - 1, 2, 7][..],
            &[100, CLAIM_BUDGET, 100][..],
            &[CLAIM_BUDGET / 2, CLAIM_BUDGET / 2, 1, 1][..],
        ] {
            let mut before = 0;
            let mut claimed = 0;
            for (r, &own) in counts.iter().enumerate() {
                let took = (0..own).filter(|&rank| claims(before, own, rank)).count() as u32;
                let fits = before + own <= CLAIM_BUDGET;
                let left = CLAIM_BUDGET.saturating_sub(before);
                assert_eq!(took, if fits { own } else { left }, "{counts:?} ring {r}");
                claimed += took;
                before += own;
            }
            let wanted: u32 = counts.iter().sum();
            assert_eq!(claimed, wanted.min(CLAIM_BUDGET), "{counts:?}");
        }
    }

    /// The light's holes' region holds a parked state for every slot of every tile a cast
    /// claims, for this cast and the one before (`parked_room`, `region_words`).
    #[test]
    fn region_parks_every_claimed_slot_of_two_casts() {
        assert_eq!(parked_room(), CLAIM_BUDGET * TILE * TILE);
        assert_eq!(region_words(), 2 * parked_room() * ray::STATE_WORDS);
    }
}
