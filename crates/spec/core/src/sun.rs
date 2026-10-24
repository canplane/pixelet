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

use crate::cast::{Detail, Ends, Met, Ray};
use crate::{axis, Zoom, FINEST_ZOOM, FRAC_W};

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
    /// `corner` is the world texel index of the box's first texel, and
    /// nothing is kept.
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
            first: [w[0] - ax * x[0] - ay * y[0], w[1] - ax * x[1] - ay * y[1], w[2] - ax * x[2] - ay * y[2]],
            reach,
            across,
            down,
            dir,
            bias: [bias(0), bias(1), bias(2)],
            corner,
            kept: Sun::none(corner, size),
        }
    }

    /// The kept corner that keeps nothing: a box a side on from `corner`'s.
    pub fn none(corner: [i32; 2], size: u32) -> [i32; 2] {
        [corner[0].wrapping_add(size as i32), corner[1].wrapping_add(size as i32)]
    }

    /// The box's texel a slot of the map holds, `size` a power of two its
    /// side: the slot's world texel, less the box's corner.
    pub fn texel(&self, size: u32, slot: [u32; 2]) -> [u32; 2] {
        let at = |k: usize| (slot[k] as i32).wrapping_sub(self.corner[k]) as u32 & (size - 1);
        [at(0), at(1)]
    }

    /// Whether box texel `p`'s world texel lay in the box the map was last
    /// cast at, so its slot holds it.
    pub fn kept(&self, size: u32, p: [u32; 2]) -> bool {
        let within = |k: usize| (self.corner[k].wrapping_add(p[k] as i32).wrapping_sub(self.kept[k]) as u32) < size;
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
        let v = [e[0] - self.dir[0] * t, e[1] - self.dir[1] * t, e[2] - self.dir[2] * t];
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
        let within = |k: usize| (self.corner[k].wrapping_add(p[k] as i32).wrapping_sub(self.kept[k]) as u32) <= size - TILE;
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
        let t = [held.near as f32 / DEPTH_ONE as f32 * self.reach, held.far as f32 / DEPTH_ONE as f32 * self.reach];
        let pad = 1.0 / (1u32 << FRAC_W) as f32;
        // The stops' box along each of the box's own axes, through the eye:
        // how near and how far it lies along each, squared.
        let along = |axis: [f32; 3]| {
            let l = length(axis);
            let u = [axis[0] / l, axis[1] / l, axis[2] / l];
            let base = dot(self.first, u) + i * dot(self.across, u) + j * dot(self.down, u);
            let (a, b, w) = (side * dot(self.across, u), side * dot(self.down, u), dot(self.dir, u));
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
            zoom: if self.zoom == NO_ZOOM || self.zoom == z { z } else { MIXED },
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
    pub fn of_words(w: &[u32; crate::eye::SUN_WORDS]) -> Sun {
        let v = |k: usize| [f32::from_bits(w[k]), f32::from_bits(w[k + 1]), f32::from_bits(w[k + 2])];
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
    pub fn words(&self) -> [u32; crate::eye::SUN_WORDS] {
        let v = |a: [f32; 3], w: u32| [a[0].to_bits(), a[1].to_bits(), a[2].to_bits(), w];
        let c = [
            v(self.first, self.reach.to_bits()),
            v(self.across, self.corner[0] as u32),
            v(self.down, self.corner[1] as u32),
            v(self.dir, self.kept[0] as u32),
            v(self.bias, self.kept[1] as u32),
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

/// The nodes the world's query answers otherwise than when the map was
/// last cast, at most: the eye's windows' squares that moved or were
/// filled, a preview's cuts that moved, each X, Y and its zoom. Past them,
/// the world is cast again whole.
pub const MOVED: usize = 32;
/// Their words, as the sun's cast binds them: their count, then a vec4
/// each: X, Y and its zoom.
pub const MOVED_WORDS: usize = 4 + 4 * MOVED;

/// The clipmap's tiles, `TILE` slots a side, a workgroup each: the mark
/// lists the tiles a slot of which walks again (pxl-shader `sun_mark_cs`)
/// and the walk runs a workgroup a listed tile (`sun_cs`), so a slot that
/// stands holds no lane of the walk's. The list's words: the walk's
/// indirect dispatch, `LIST_ACROSS` workgroups across and a row of them
/// for each `LIST_ACROSS` tiles listed, then how many are, then each
/// tile, its X and its Y over 16 bits.
pub const TILE: u32 = 8;
pub const LIST_ACROSS: u32 = 256;
pub const LIST_ROWS: usize = 1;
pub const LIST_COUNT: usize = 3;
pub const LIST_AT: usize = 4;
/// The list's head as each frame starts it: no tile listed.
pub const LIST_EMPTY: [u32; LIST_AT] = [LIST_ACROSS, 0, 1, 0];

/// Whether a texel's ray, from its start `o` along `dir` to `t` units, its
/// stop, passes over node `[x, y]` of `zoom` on the world's X and Y: what a
/// walk to its stop met lies in the nodes it entered, so one wholly off the
/// node met nothing the node holds. X wraps round the world as a Q3
/// position does.
pub fn over(o: [i32; 3], dir: [f32; 3], t: f32, node: [i32; 2], zoom: Zoom) -> bool {
    over_any(o, [[0.0; 2]; 2], dir, t, node, zoom)
}

/// Whether any of the rays from `o` moved by up to `off` on the world's X
/// and Y, least and most, in units (`Sun::tile_rays`), passes over the
/// node (`over`) within `t` units along `dir`: one does where the ray from
/// `o` passes over the node widened by as much the other way.
pub fn over_any(o: [i32; 3], off: [[f32; 2]; 2], dir: [f32; 3], t: f32, [x, y]: [i32; 2], zoom: Zoom) -> bool {
    if zoom == 0 {
        return true;
    }
    let shift = FINEST_ZOOM - zoom;
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
    let steps = |k: usize| libm::rintf((s.first[k] + i * s.across[k] + j * s.down[k]) * step) as i32;
    let q = [steps(0), steps(1), steps(2)];
    let from = [-(q[0] as f32) / step, -(q[1] as f32) / step, -(q[2] as f32) / step];
    ([eye[0].wrapping_add(q[0]), eye[1].wrapping_add(q[1]), eye[2].wrapping_add(q[2])], Ray::new(s.dir, s.reach), from)
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

    /// A box 4 units a side and 8 deep, looking straight down from 4 up,
    /// two texels a side.
    fn down() -> Sun {
        Sun::of([[2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, -8.0], [0.0, 0.0, 4.0]], 2, [0, 0])
    }

    /// A walk that stopped `t` along its ray on a face across `axis`, or went on.
    fn met(ends: Ends, t: f32, axis: u32) -> Met<()> {
        let node = crate::cast::Node::ROOT;
        Met {
            ends,
            stop: crate::cast::Stop { t, axis, node, at: (), block: [0; 2] },
            visits: 0,
        }
    }

    #[test]
    fn a_texel_starts_where_it_lies_on_the_near_plane() {
        let s = down();
        let (o, ray, from) = ray([100, 200, 300], &s, [0, 0]);
        // The top-left texel's middle: half the box across to the left, up.
        assert_eq!(o, [100 - 256, 200 + 256, 300 + 4 * 256]);
        assert_eq!(ray.direction, [0.0, 0.0, -1.0]);
        assert_eq!(ray.reach, 8.0);
        assert_eq!(from, [1.0, -1.0, -4.0]);
        // The next across and the next down, a texel's side on.
        assert_eq!(super::ray([0; 3], &s, [1, 1]).0, [256, -256, 4 * 256]);
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
        let s = Sun::of([[2.0 * r, 0.0, 2.0 * r], [0.0, 2.0, 0.0], [8.0 * r, 0.0, -8.0 * r], [0.0, 0.0, 4.0]], 2048, [0, 0]);
        let texel = 4.0 / 2048.0;
        let d = depth(&s, &met(Ends::Stop, 2.0, 2));
        assert!((d - (0.25 + crate::eye::SLOPE_BIAS * texel / 8.0)).abs() < 1e-6, "{d}");
    }

    #[test]
    fn a_plane_keeps_its_depth_along_itself() {
        // The same box: a top face's depth deepens a texel's side a texel
        // across, toward the sun's going, and not at all down.
        let r = core::f32::consts::FRAC_1_SQRT_2;
        let (across, down, deep) = ([4.0 * r / 2048.0, 0.0, 4.0 * r / 2048.0], [0.0, -4.0 / 2048.0, 0.0], [8.0 * r, 0.0, -8.0 * r]);
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

    #[test]
    fn a_slot_holds_its_world_texel_wherever_the_box_lies() {
        let mut s = Sun::of([[2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, -8.0], [0.0, 0.0, 4.0]], 8, [-3, 5]);
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
        let near = Detail { eye: [0.0, 0.0, 2.0], ..far };
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
        assert!(over(o, [0.0, 0.0, -1.0], 5.0, [0, 0], unit) && !over(o, [0.0, 0.0, -1.0], 5.0, [1, 0], unit));
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
            let w = [next() * 40.0 - 20.0, next() * 40.0 - 20.0, 60.0 + next() * 40.0];
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
                let d = if trial % 7 == 0 && k % 5 == 0 { 1.0 } else { d0 + next() * 0.02 };
                words[k as usize] = sun.word(at, d);
                held = held.with(words[k as usize]);
            }
            let node = [(next() * 80.0 - 40.0) as i32 + 1000 / 256, (next() * 80.0 - 40.0) as i32 - 2000 / 256];
            let stands = sun.tile_stands(detail, p, held);
            let (o, off) = sun.tile_rays(eye, p);
            let reach = held.reach as f32 / DEPTH_ONE as f32 * sun.reach;
            let passes = over_any(o, off, sun.dir, reach, node, crate::UNIT_ZOOM);
            for k in 0..TILE * TILE {
                let q = [p[0] + k % TILE, p[1] + k / TILE];
                let (o, _, from) = ray(eye, &sun, q);
                let at = Detail { eye: from, ..detail };
                let word = words[k as usize];
                if stands {
                    assert!(sun.stands(at, word), "trial {trial}: slot {k} walks in a tile judged to stand");
                }
                if over(o, sun.dir, depth_of(word) * sun.reach, node, crate::UNIT_ZOOM) {
                    assert!(passes, "trial {trial}: slot {k} passes over a node its tile's rays miss");
                    met += 1;
                }
            }
            judged += 1;
            whole += u32::from(stands);
        }
        // Enough of either kind to have judged the bound.
        assert!(judged > 200 && whole > 20 && whole < judged - 20 && met > 50, "{judged} {whole} {met}");
        // A tile at the box's edge straddles it; one in the kept box is kept.
        let mut sun = Sun::of([x, y, z, [0.0; 3]], size, [3, -5]);
        assert!(!sun.tile_texel(size, [0, 0]).1);
        sun.kept = sun.corner;
        assert!(sun.tile_kept(size, [8, 16]));
        sun.corner[0] += 1;
        assert!(sun.tile_kept(size, [7, 16]) && !sun.tile_kept(size, [size - TILE, 16]));
    }
}
