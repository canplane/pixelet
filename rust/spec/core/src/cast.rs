// pxl-spec-core/cast.rs

//! gpu-portable: the first cell a ray meets, at the zoom its distance asks
//! (doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md),
//! for the CPU and the GPU over a query each answers its own way (`Query`),
//! by one ray (`walk`) or by a packet of them walked as one (`packet`), each
//! node met by one rule (`meet`). A node coarser than the zoom its distance
//! asks is asked whether anything under it is not transparent: nothing, and
//! the ray goes on past it whole; else its children the ray meets, nearest
//! first. A node at that zoom is asked its asset, and the ray stops there or
//! goes on.
//!
//! Positions are integers: the ray's origin is a Q3, and a node's corner is
//! the Q3 steps of its root's finest grid (`Node`), the origin's own grid,
//! so only a plane's offset from the origin, summed as integers first, is
//! a float (`units`). So the nodes near the origin are as exact as f32 is
//! near zero wherever the origin stands, and X goes round the world as the
//! integers do. WGSL's f32 is not strictly IEEE (a division's last
//! place, a contraction into a fused multiply-add), so where a ray passes
//! within the last bit of a face, the CPU and a GPU may stop on either side
//! of it; that is the one rule computed twice, not a second rule.
//!
//! The descent holds one frame a zoom, so its stack is as deep as the zooms
//! a node can have, and from a child it steps to the next across the face
//! it leaves by, so no frame holds its siblings.

use crate::answer::{box_of, kept, side, transparent, In, Kind, AIR, BOX_BITS, BOX_STEPS, EIGHTHS};
use crate::grid::cell_at;
use crate::{axis, wrap_idx, Zoom, FINEST_ZOOM, FRAC_W, UNIT_ZOOM};

/// The direction at the tangents `tx` across and `ty` up from the optical
/// axis: a drawn pixel's middle, its tangents the screen's half-angles'
/// (pxl-kernel domain/view/lens.rs `Lens`) scaled by where it lies on the screen,
/// as a projection puts it.
pub fn through(forward: [f32; 3], right: [f32; 3], up: [f32; 3], tx: f32, ty: f32) -> [f32; 3] {
    let d = [
        forward[0] + tx * right[0] + ty * up[0],
        forward[1] + tx * right[1] + ty * up[1],
        forward[2] + tx * right[2] + ty * up[2],
    ];
    let l = libm::sqrtf(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
    [d[0] / l, d[1] / l, d[2] / l]
}

/// How far apart a beam's rays lie, `across + angle · d` units at `d` from
/// the eye (`Detail::eye`): the eye's an angle, its camera grid's cell
/// (`grid::CELL`); the sun's, whose rays are parallel, a width, `sun::BEAM`
/// of its map's texels. Only a beam reads it, its widening and `narrow`
/// (`Until::Beam`); an asker that casts no beam, control and physics, fills
/// in none (`NONE`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spacing {
    pub angle: f32,
    pub across: f32,
}

impl Spacing {
    pub const NONE: Spacing = Spacing {
        angle: 0.0,
        across: 0.0,
    };

    /// Its width at `d` from the eye.
    pub fn width(self, d: f32) -> f32 {
        self.across + self.angle * d
    }
}

/// The zoom asked at each distance: the camera grid's cells there
/// (`cell_at`), held to `range`, asked of a distance squared (`at`). A
/// stated zoom's cells wherever they are is a range of that zoom alone.
/// The distance is the eye's, `eye` units from the walk's origin: none for
/// the eye's own rays; for a ray from elsewhere, a ray toward the sun from
/// what an eye's ray met, where the eye stands from it, so a node is asked
/// at the zooms the eye's rays ask it (`reach`). `spacing` is how far a
/// beam ray lies from the next of its grid (`Spacing`, `Until::Beam`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detail {
    pub range: [Zoom; 2],
    pub eye: [f32; 3],
    pub spacing: Spacing,
}

impl Detail {
    /// At `d2`, a distance squared.
    pub fn at(self, d2: f32) -> Zoom {
        cell_at(d2).clamp(self.range[0], self.range[1])
    }
}

/// Whether a node `side` wide, squared, is too narrow for a beam ray to
/// share where the grid's rays lie `width` apart, squared: under twice the
/// width that holds one of them for certain, `2√2 · width`; the eye's
/// `s · d` at the node's farthest point.
pub fn narrow(side2: f32, width2: f32) -> bool {
    side2 < 8.0 * width2
}

/// The spacings a node is narrow under (`narrow`): twice the width that
/// holds one ray for certain, `2√2`; and its brick, exact.
pub const NARROW: f32 = 2.0 * core::f32::consts::SQRT_2;
const NARROW_SQUARED: f32 = 8.0;
const _: () = assert!(
    NARROW * NARROW - NARROW_SQUARED < 1e-6 && NARROW_SQUARED - NARROW * NARROW < 1e-6,
    "`NARROW_SQUARED` is `NARROW`'s brick"
);

/// √6, as an f32 rounds it: a cube `NARROW` spacings wide spans `√3 ·
/// NARROW`, `2√6` of them, corner to corner.
pub const SQRT_6: f32 = 2.449_489_8;

/// Which cell a ray stops at: the first it cannot see through
/// (`answer::transparent`), as an eye sees; or the first it can, as what is
/// climbed out of ends; or, for a beam ray of the camera grid, the first it
/// cannot see through or the first split too narrow to hold one of the
/// grid's rays for certain (`beam::narrow`), every split entered whose box
/// it meets widened by `2√2` of that width
/// (doc/minutes/20261022-a-cell-casts-a-beam.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Until {
    Opaque,
    Transparent,
    Beam,
}

/// A ray from an origin, a Q3 position: its direction, how far it reaches, in units, and
/// its direction's reciprocal on each axis, 0 where it does not move along
/// one: a plane's crossing is a product, not a division made again at every
/// node (Laine and Karras, _Efficient Sparse Voxel Octrees_, 2010, the
/// per-ray coefficients).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub dir: [f32; 3],
    pub reach: f32,
    inverse: [f32; 3],
}

impl Ray {
    /// A direction component under f32's least normal magnitude
    /// (`f32::MIN_POSITIVE`) is 0: the ray does not move along that axis.
    /// So every reciprocal is finite (1 / 2^-126 is), a plane's offset
    /// times it is a finite number or an infinity, never inf · 0, and no
    /// number the walk compares is NaN; which is what lets `greater` and
    /// `lesser` be one comparison each. The component and its reciprocal
    /// are zeroed together, as `planes` and `past` branch on the one and
    /// multiply by the other.
    pub fn new(direction: [f32; 3], reach: f32) -> Ray {
        let moves = |d: f32| {
            if libm::fabsf(d) < f32::MIN_POSITIVE {
                0.0
            } else {
                d
            }
        };
        let dir = [
            moves(direction[0]),
            moves(direction[1]),
            moves(direction[2]),
        ];
        let inv = |d: f32| if d == 0.0 { 0.0 } else { 1.0 / d };
        Ray {
            dir,
            reach,
            inverse: [inv(dir[0]), inv(dir[1]), inv(dir[2])],
        }
    }
}

/// The greater and the lesser of two floats, one comparison each. IEEE's
/// `f32::max` and `f32::min` hand back the other operand where one is NaN,
/// and rust-gpu lowers each into two NaN-check branches — seventeen a pass
/// of the walk's loop. No number the walk compares is NaN (`Ray::new`), so
/// the comparison is the whole rule.
fn greater(a: f32, b: f32) -> f32 {
    if a > b {
        a
    } else {
        b
    }
}

fn lesser(a: f32, b: f32) -> f32 {
    if a < b {
        a
    } else {
        b
    }
}

/// A node as the walk holds it, each axis an integer: its corner in its
/// root at the finest grid a position names (`FINEST_ZOOM`, 2^32 a root, so
/// a u32: the Q3 itself modulo the root's side), its zoom, and which root
/// it is in, counted from the origin's. Its corner's bits under its own
/// side are 0, so two corners part at the highest bit they differ in. The
/// one form of a node from an origin, the walk's and where it stops alike.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    pub corner: [u32; 3],
    pub zoom: Zoom,
    pub roots: [i32; 3],
}

impl Node {
    /// A root, the one holding the origin, whose corner is its own, 0.
    pub const ROOT: Node = Node {
        corner: [0; 3],
        zoom: 0,
        roots: [0; 3],
    };

    /// The deepest node over itself, or itself, that holds the point `p` on
    /// the finest grid in the root `roots` from the origin's, and whether
    /// there is one: none in another root.
    pub fn over(&self, (p, roots): ([u32; 3], [i32; 3])) -> (Node, bool) {
        let apart = (self.corner[0] ^ p[0]) | (self.corner[1] ^ p[1]) | (self.corner[2] ^ p[2]);
        let zoom = self.zoom.min(apart.leading_zeros());
        let keep = !under(zoom);
        let node = Node {
            corner: [
                self.corner[0] & keep,
                self.corner[1] & keep,
                self.corner[2] & keep,
            ],
            zoom,
            roots: self.roots,
        };
        (
            node,
            self.roots[0] == roots[0] && self.roots[1] == roots[1] && self.roots[2] == roots[2],
        )
    }

    /// The node a walk from the origin `from` holds as `self`, as a walk
    /// from `to` holds it: its corner is the Q3 itself modulo a root's side
    /// whatever the origin, and its roots move by the roots the two origins
    /// lie apart, the short way round as an i32 wraps on X, where the
    /// origins lie within a root of each other.
    pub fn moved(&self, from: [i32; 3], to: [i32; 3]) -> Node {
        let k = |a: usize| {
            let d = from[a].wrapping_sub(to[a]);
            let og = from[a] as u32;
            // `to`'s position on the finest grid is `from`'s less `d`: a
            // root the more where that borrows, a root the less where it
            // carries.
            if d >= 0 {
                self.roots[a] + (og < d as u32) as i32
            } else {
                self.roots[a] - (og.wrapping_add(d.unsigned_abs()) < og) as i32
            }
        };
        // [naga] each axis by itself, not a loop (cave.rs `node_seed`).
        Node {
            corner: self.corner,
            zoom: self.zoom,
            roots: [k(0), k(1), k(2)],
        }
    }

    /// The world's node it is from the origin `o`, X named as the world
    /// names it (pxl-spec space/coord/world.rs `wrap_index`).
    pub fn idx(&self, o: [i32; 3]) -> [i32; 3] {
        let r0 = roots_of(o);
        [
            wrap_idx(
                self.zoom,
                idx_on(r0[0] + self.roots[0], self.corner[0], self.zoom),
            ),
            idx_on(r0[1] + self.roots[1], self.corner[1], self.zoom),
            idx_on(r0[2] + self.roots[2], self.corner[2], self.zoom),
        ]
    }
}

/// The origin `o` on the finest grid within its root: the Q3 itself
/// modulo the root's side, 2^32.
fn grid_of(o: [i32; 3]) -> [u32; 3] {
    // [naga] each axis by itself, not a loop (cave.rs `node_seed`).
    [o[0] as u32, o[1] as u32, o[2] as u32]
}

/// The root holding the origin `o` on each axis: its Q3 over the root's
/// side, 2^32, floored, so 0 or −1.
fn roots_of(o: [i32; 3]) -> [i32; 3] {
    [o[0] >> 31, o[1] >> 31, o[2] >> 31]
}

/// Where `ray` from the origin `o` is at `t`: on the finest grid in its
/// root, and that root from the origin's, floored to a Q3 step.
pub fn point_at(o: [i32; 3], ray: &Ray, t: f32) -> ([u32; 3], [i32; 3]) {
    let og = grid_of(o);
    let on = |a: usize| {
        let d = libm::floorf(axis(ray.dir, a as u32) * t * (1 << FRAC_W) as f32) as i32;
        let p = og[a].wrapping_add(d as u32);
        (
            p,
            if d >= 0 && p < og[a] {
                1
            } else if d < 0 && p > og[a] {
                -1
            } else {
                0
            },
        )
    };
    // [naga] each axis by itself, not a loop (cave.rs `node_seed`).
    let (x, y, z) = (on(0), on(1), on(2));
    ([x.0, y.0, z.0], [x.1, y.1, z.1])
}

/// Units from the origin, at `og` in its root, to `p` in the root `roots`
/// from it: the whole units summed as integers before they are a float, so
/// a plane is as exact far from the origin's root as in it.
fn units(p: u32, og: u32, roots: i32) -> f32 {
    ((p >> FRAC_W) as i32 - (og >> FRAC_W) as i32 + (roots << UNIT_ZOOM)) as f32
        + ((p & 0xff) as i32 - (og & 0xff) as i32) as f32 / (1 << FRAC_W) as f32
}

/// A node's side at zoom `z` less one, on the finest grid: the bits under
/// its corner's, 2^32 − 1 at a root, whose side no u32 holds.
fn under(z: Zoom) -> u32 {
    if z >= FINEST_ZOOM {
        0
    } else {
        u32::MAX >> z
    }
}

/// The world's node on axis `a` of the node at zoom `z` whose corner is `c`
/// in the root `root`: `root · 2^z` and the corner's high bits, no shift
/// reaching the word's width.
fn idx_on(root: i32, c: u32, z: Zoom) -> i32 {
    let high = if z >= FINEST_ZOOM {
        0
    } else {
        (root << z) as u32
    };
    let low = if z == 0 { 0 } else { c >> (FINEST_ZOOM - z) };
    high.wrapping_add(low) as i32
}

/// The ray's t at the near and far planes on one axis, `lo` and `hi` units
/// from the origin (Revelles, Ureña & Lastra 2000); an axis it runs beside,
/// the walk's span whole inside the slab (from `t` to `reach`), none
/// outside: no extreme a float literal could name past a shader's range.
fn planes(d: f32, inv: f32, lo: f32, hi: f32, (t, reach): (f32, f32)) -> (f32, f32) {
    if d > 0.0 {
        (lo * inv, hi * inv)
    } else if d < 0.0 {
        (hi * inv, lo * inv)
    } else if lo <= 0.0 && 0.0 < hi {
        (t, reach)
    } else {
        (reach, t)
    }
}

/// A node's planes, in units from the origin at `og` on the finest grid, each
/// a corner's (`units`): the far plane is the next node's corner, in the next
/// root where the corner goes past its own.
fn bounds(n: &Node, og: [u32; 3]) -> ([f32; 3], [f32; 3]) {
    let u = under(n.zoom);
    let far = |c: u32, og: u32, roots: i32| {
        let last = c.wrapping_add(u);
        units(last.wrapping_add(1), og, roots + (last == u32::MAX) as i32)
    };
    let lo = [
        units(n.corner[0], og[0], n.roots[0]),
        units(n.corner[1], og[1], n.roots[1]),
        units(n.corner[2], og[2], n.roots[2]),
    ];
    let hi = [
        far(n.corner[0], og[0], n.roots[0]),
        far(n.corner[1], og[1], n.roots[1]),
        far(n.corner[2], og[2], n.roots[2]),
    ];
    (lo, hi)
}

/// The ray's t at each axis's near and far planes of a node (`planes`),
/// from `t`.
fn spans(
    ray: &Ray,
    (lo, hi): ([f32; 3], [f32; 3]),
    t: f32,
) -> ((f32, f32), (f32, f32), (f32, f32)) {
    (
        planes(ray.dir[0], ray.inverse[0], lo[0], hi[0], (t, ray.reach)),
        planes(ray.dir[1], ray.inverse[1], lo[1], hi[1], (t, ray.reach)),
        planes(ray.dir[2], ray.inverse[2], lo[2], hi[2], (t, ray.reach)),
    )
}

/// The node over `n`, or `n`, that holds where `ray` from `o` is at `t`, as
/// the walk's own planes say (`bounds`, `spans`): what a walk resumed at `t`
/// starts in, `n` found off a point the float put on the ray, which may lie
/// a step outside the node the ray is in. The root where none over it does.
pub fn holding(o: [i32; 3], ray: &Ray, mut n: Node, t: f32) -> Node {
    let og = grid_of(o);
    loop {
        let (x, y, z) = spans(ray, bounds(&n, og), t);
        let (t0, t1) = (
            greater(greater(greater(t, x.0), y.0), z.0),
            lesser(lesser(lesser(ray.reach, x.1), y.1), z.1),
        );
        if (t0 <= t && t < t1) || n.zoom == 0 {
            return n;
        }
        let keep = !under(n.zoom - 1);
        n = Node {
            corner: [n.corner[0] & keep, n.corner[1] & keep, n.corner[2] & keep],
            zoom: n.zoom - 1,
            ..n
        };
    }
}

/// Where a ray enters a node whose near planes it reaches at `near`: the
/// axis entered across, the last of those planes the ray reaches
/// (`crossing`), and that plane's t, no nearer than the origin — the node's
/// and the ray's alone, whatever walk came to it.
fn entered(ray: &Ray, near: [f32; 3]) -> (f32, u32) {
    let e = crossing(ray.dir, near, true);
    (greater(axis(near, e), 0.0), e)
}

/// A cell's planes as it is drawn, its node's planes `(lo, hi)`
/// (`bounds`) inset about their center on every axis alike by the share of
/// its cube its word `asset` keeps (`answer::kept`, `answer::side`): a box
/// that share's volume, its node itself where it keeps it whole.
pub fn kept_bounds((lo, hi): ([f32; 3], [f32; 3]), asset: u32) -> ([f32; 3], [f32; 3]) {
    let k = kept(asset);
    if k == EIGHTHS {
        return (lo, hi);
    }
    let by = 0.5 * (1.0 - side(k));
    let inset = [
        (hi[0] - lo[0]) * by,
        (hi[1] - lo[1]) * by,
        (hi[2] - lo[2]) * by,
    ];
    (
        [lo[0] + inset[0], lo[1] + inset[1], lo[2] + inset[2]],
        [hi[0] - inset[0], hi[1] - inset[1], hi[2] - inset[2]],
    )
}

/// The top of the cell a stop met, as it is drawn (`kept_bounds`): the
/// floor of its node `n` on its grid from the origin `o`, and the share of
/// the node's side its top lies over that floor — its cube's top where its
/// word `asset` keeps it whole, else its kept box's. What a walker and decor
/// stand on (pxl-kernel domain/query/ray.rs `hit`, `support::footing`).
pub fn top(o: [i32; 3], n: &Node, asset: u32) -> (i32, f32) {
    (n.idx(o)[2], kept_bounds(([0.0; 3], [1.0; 3]), asset).1[2])
}

/// The asset `asset` at the node `n` as drawn from the origin `o`, in units
/// from it (`bounds`, `kept_bounds`): the box `entry` asks a ray into, as
/// the samples' boxes are drawn (`raster`).
pub fn drawn(o: [i32; 3], n: &Node, asset: u32) -> ([f32; 3], [f32; 3]) {
    kept_bounds(bounds(n, grid_of(o)), asset)
}

/// Where `ray` from the origin `o` enters the cell `asset` is in (an asset's word,
/// `answer`) at the node `n`, and whether it does within its reach: its span
/// in the cell as drawn from the origin (`drawn`, `spans`), and
/// where that span begins (`entered`). Only touched, at a face or an edge,
/// is not entered, as the walk has it.
pub fn entry(o: [i32; 3], ray: &Ray, n: &Node, asset: u32) -> (f32, u32, bool) {
    let (x, y, z) = spans(ray, drawn(o, n, asset), 0.0);
    let (t0, t1) = (
        greater(greater(greater(0.0, x.0), y.0), z.0),
        lesser(lesser(lesser(ray.reach, x.1), y.1), z.1),
    );
    let (t, e) = entered(ray, [x.0, y.0, z.0]);
    (t, e, t1 > t0)
}

/// Whether the ray, at `t`, has crossed the mid plane `mid` units from the
/// origin on an axis: the child it is in lies past that plane.
fn past(d: f32, inv: f32, mid: f32, t: f32) -> bool {
    if d > 0.0 {
        mid * inv <= t
    } else if d < 0.0 {
        mid * inv > t
    } else {
        mid <= 0.0
    }
}

/// The axis the ray crosses at the last of its planes `t` (`last`), as it
/// enters a node, or at the first, as it leaves one: among the axes it moves
/// along, the lowest at a tie, the third where it moves along none.
fn crossing(d: [f32; 3], t: [f32; 3], last: bool) -> u32 {
    let moves = |a: u32| axis(d, a) != 0.0;
    let beats = |a: f32, b: f32| if last { a > b } else { a < b };
    let mut e = if moves(0) {
        0
    } else if moves(1) {
        1
    } else {
        2
    };
    // [naga] each axis by itself, not a loop (cave.rs `node_seed`).
    if e < 1 && moves(1) && beats(t[1], axis(t, e)) {
        e = 1;
    }
    if e < 2 && moves(2) && beats(t[2], axis(t, e)) {
        e = 2;
    }
    e
}

/// The world as a descent asks it, each node handed what its parent found
/// (the `At`). The CPU and a GPU answer it from the same reading of the
/// ground's windows, the caves, the ways' tree and what is placed; only the
/// CPU's answer strikes decor.
pub trait Query {
    type At: Copy;
    /// A root of zoom 0, `node` the world's.
    fn root(&mut self, node: [i32; 3]) -> Self::At;
    /// The child `node` at `zoom` of the node `at` was handed for, with what
    /// that one found handed down.
    fn child(&mut self, at: &Self::At, zoom: Zoom, node: [i32; 3]) -> Self::At;
    /// What is in the node `node` at `zoom`, `at` what its ancestors handed
    /// it, asked from `far` to `near`, the ray in it from `t0` (a packet's
    /// first ray in it, `packet`): the world's
    /// answer (pxl-spec-core `answer`), whichever side reads it.
    fn node(
        &mut self,
        at: &mut Self::At,
        zoom: Zoom,
        node: [i32; 3],
        asked: [Zoom; 2],
        t0: f32,
    ) -> In<[u32; 2]>;
}

/// Where a ray stopped: how far, the axis it entered across, the node
/// (`Node`, from the ray's origin) and what it is there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stop<A> {
    pub t: f32,
    pub axis: u32,
    pub node: Node,
    pub at: A,
    /// What it stopped at, as the query answered it: an asset's words, or,
    /// where a beam ray stopped at a narrow split, that split's box
    /// (`answer::box_word`), which no reader takes for an asset's.
    pub asset: [u32; 2],
}

/// How a walk ended: where it stopped, gone on to its reach, or at a node
/// not held, past which it tells nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum End {
    Stop,
    On,
    Lacks,
}

/// What the walk met: how it ended, where it stopped when it did, and how
/// many nodes it asked the world about (`Query::node`), the one count of a
/// ray's steps the CPU and a GPU both report.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Met<A> {
    pub ends: End,
    pub stop: Stop<A>,
    pub visits: u32,
}

/// The descent's stack, a zoom an entry, every zoom's: what the query
/// handed the node there (`Query::At`), the one thing the node's parent
/// cannot work out again without asking the world from the top. Where the ray is in the node, the child it is in and where the ray
/// leaves that child follow from the node's place by the slab test, so no
/// entry holds them.
pub const DEPTH: usize = FINEST_ZOOM as usize + 1;

/// A node's farthest and nearest distances from the eye, squared, its
/// planes `lo` and `hi` units from the eye on each axis: its corner
/// farthest from the eye and its point nearest it.
fn reach(lo: [f32; 3], hi: [f32; 3]) -> [f32; 2] {
    let near = [
        lesser(greater(lo[0], 0.0), hi[0]),
        lesser(greater(lo[1], 0.0), hi[1]),
        lesser(greater(lo[2], 0.0), hi[2]),
    ];
    let far = [
        greater(-lo[0], hi[0]),
        greater(-lo[1], hi[1]),
        greater(-lo[2], hi[2]),
    ];
    [squared(far), squared(near)]
}

/// The zooms a node is asked at by every ray from the origin in it, from
/// its farthest corner to its nearest point (`reach`, `Detail::at`). A
/// function of the node and the eye alone, so a node's answer is one for
/// every ray through it; a ray asks within it no coarser than the farthest
/// corner, so this only adds coarser zooms than its own, which splits more
/// and stops on the same cell (doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md,
/// "more cautiously").
fn asked(detail: Detail, [far, near]: [f32; 2]) -> [Zoom; 2] {
    [detail.at(far), detail.at(near)]
}

/// A vector's length squared.
fn squared(v: [f32; 3]) -> f32 {
    v[0] * v[0] + v[1] * v[1] + v[2] * v[2]
}

/// The first cell `ray` meets within its reach that `until` stops at, at
/// the zoom `detail` asks where it meets it, from the world's root holding
/// the origin (`walk`).
pub fn first<Q: Query>(
    q: &mut Q,
    o: [i32; 3],
    ray: &Ray,
    detail: Detail,
    until: Until,
) -> Met<Q::At> {
    rooted(q, o, ray, detail, until, &mut ())
}

/// `walk` from the world's root holding the origin, at 0, leaving its line
/// to `keep`.
pub fn rooted<Q: Query>(
    q: &mut Q,
    o: [i32; 3],
    ray: &Ray,
    detail: Detail,
    until: Until,
    keep: &mut impl Keep<Q::At>,
) -> Met<Q::At> {
    let at = q.root(Node::ROOT.idx(o));
    walk(
        q,
        o,
        ray,
        detail,
        until,
        (Node::ROOT, at, 0.0),
        &Rooted(at),
        keep,
    )
}

/// The nodes holding the origin `o`, from its root down: the line every ray
/// from `o` enters first, asked as each such ray's walk asks it, by that
/// walk, of a ray that goes nowhere, so it enters every node holding the
/// origin and no other. Asked once ahead of the rays, so a reader that
/// keeps the answers hands them to every ray (pxl-shader eye.rs
/// `origin`). The nodes asked.
pub fn origin_line<Q: Query>(q: &mut Q, o: [i32; 3], detail: Detail) -> u32 {
    let nowhere = Ray::new([0.0, 0.0, -1.0], f32::MIN_POSITIVE);
    rooted(q, o, &nowhere, detail, Until::Beam, &mut ()).visits
}

/// The rays a packet holds: a camera grid cell's samples, `beam::SAMPLE`
/// a side (param.rs), a bit each of a `u32`.
pub const PACKET: usize = (crate::param::beam::SAMPLE * crate::param::beam::SAMPLE) as usize;
const _: () = assert!(PACKET <= 32, "a packet's rays are the bits of a u32");

/// The first cell each live ray of `rays` (a bit of `live` each) from the
/// origin `o` meets that an eye stops at (`Until::Opaque`), the rays walked
/// as one, each handed to `found` with its index as it ends: down from the
/// root holding the origin, a node asked of the world once for the rays of
/// a sign that enter it (the first of them its `t0`), each ray meeting it by
/// the rule one ray's walk meets it by (`meet`) and keeping its own stop.
/// Rays whose directions part in sign on an axis take a node's children in
/// other orders, so each sign's rays walk apart, and those holding the
/// origin are asked once a sign. A ray that leaves a root goes on into the
/// next with the others leaving by the same face, as `walk` crosses a root.
/// A ray's met counts no visits: the packet's are its own. The nodes asked,
/// and the rays not ended, none where every one is.
pub fn packet<Q: Query>(
    q: &mut Q,
    o: [i32; 3],
    rays: &[Ray; PACKET],
    live: u32,
    detail: Detail,
    found: &mut impl FnMut(usize, Met<Q::At>),
) -> (u32, u32) {
    let og = grid_of(o);
    let sign = |r: &Ray| {
        (r.direction[0] < 0.0) as u32
            | ((r.direction[1] < 0.0) as u32) << 1
            | ((r.direction[2] < 0.0) as u32) << 2
    };
    let (mut visits, mut done) = (0, !live);
    let mut s = 0;
    while s < 8 {
        let mut group = 0;
        let mut r = 0;
        while r < PACKET {
            if live >> r & 1 != 0 && sign(&rays[r]) == s {
                group |= 1 << r;
            }
            r += 1;
        }
        if group != 0 {
            visits += packet_sign(q, o, og, rays, group, s, detail, found, &mut done);
        }
        s += 1;
    }
    let left = if PACKET == 32 {
        !done
    } else {
        !done & ((1 << PACKET) - 1)
    };
    (visits, left)
}

/// `packet`'s walk of the rays `group`, every one of whose directions has
/// the signs `sign` (a bit an axis, set where it falls): through the roots
/// they cross, a node's children taken nearest the rays first, a node asked
/// once where any of them enters it. `done` takes each ray as it ends.
#[allow(clippy::too_many_arguments)]
fn packet_sign<Q: Query>(
    q: &mut Q,
    o: [i32; 3],
    og: [u32; 3],
    rays: &[Ray; PACKET],
    group: u32,
    sign: u32,
    detail: Detail,
    found: &mut impl FnMut(usize, Met<Q::At>),
    done: &mut u32,
) -> u32 {
    let mut t = [0.0f32; PACKET];
    let mut visits = 0;
    // A node's turn: which of `rays_in` enter it, it asked once, met by
    // each; at a root (`fresh`), one only touching it there at its reach
    // ends there too, as `walk` ends it. What goes on into its children, what
    // was handed it, and whether any ray entered.
    let mut enter = |q: &mut Q,
                     n: Node,
                     parent: Q::At,
                     fresh: bool,
                     rays_in: u32,
                     t: &mut [f32; PACKET],
                     done: &mut u32|
     -> (u32, Q::At, bool) {
        let (lo, hi) = bounds(&n, og);
        let mut spans_of = [((0.0, 0.0), (0.0, 0.0), (0.0, 0.0)); PACKET];
        let mut span = [(0.0f32, 0.0f32); PACKET];
        let (mut entering, mut ending, mut first) = (0u32, 0u32, 0.0f32);
        let mut r = 0;
        while r < PACKET {
            if rays_in >> r & 1 != 0 && *done >> r & 1 == 0 {
                let ray = &rays[r];
                let (x, y, z) = spans(ray, (lo, hi), t[r]);
                let (t0, t1) = (
                    greater(greater(greater(t[r], x.0), y.0), z.0),
                    lesser(lesser(lesser(ray.reach, x.1), y.1), z.1),
                );
                if t1 > t0 {
                    if entering == 0 || t0 < first {
                        first = t0;
                    }
                    entering |= 1 << r;
                } else if fresh && t1 >= ray.reach {
                    ending |= 1 << r;
                }
                spans_of[r] = (x, y, z);
                span[r] = (t0, t1);
            }
            r += 1;
        }
        if entering | ending == 0 {
            return (0, parent, false);
        }
        let mut at = if fresh {
            parent
        } else {
            q.child(&parent, n.zoom, n.index(o))
        };
        // What was handed it before it was asked: a ray only touching it
        // ends with that, as its own walk never asks it.
        let (handed, mut into) = (at, 0);
        if entering != 0 {
            let e = detail.eye;
            let reach = reach(
                [lo[0] - e[0], lo[1] - e[1], lo[2] - e[2]],
                [hi[0] - e[0], hi[1] - e[1], hi[2] - e[2]],
            );
            let i = q.node(&mut at, n.zoom, n.index(o), asked(detail, reach), first);
            visits += 1;
            let mut r = 0;
            while r < PACKET {
                if entering >> r & 1 != 0 {
                    let ray = &rays[r];
                    let (t0, t1) = span[r];
                    let (ends, near, inward) = meet(
                        ray,
                        &n,
                        og,
                        (lo, hi),
                        spans_of[r],
                        (t[r], t0, t1),
                        reach,
                        &i,
                        detail,
                        Until::Opaque,
                    );
                    if ends != Ends::On {
                        let (t, axis) = entered(ray, near);
                        let stop = Stop {
                            t,
                            axis,
                            node: n,
                            at,
                            asset: i.asset,
                        };
                        found(
                            r as usize,
                            Met {
                                ends,
                                stop,
                                visits: 0,
                            },
                        );
                        *done |= 1 << r;
                    } else if inward {
                        into |= 1 << r;
                        t[r] = t0;
                    } else if t1 >= ray.reach {
                        ending |= 1 << r;
                    } else {
                        t[r] = greater(t[r], t1);
                    }
                }
                r += 1;
            }
        }
        // Gone on to its reach: no stop, which no asker reads.
        let mut r = 0;
        while r < PACKET {
            if ending >> r & 1 != 0 {
                let stop = Stop {
                    t: span[r].1,
                    axis: 0,
                    node: n,
                    at: if entering >> r & 1 != 0 { at } else { handed },
                    asset: AIR,
                };
                found(
                    r as usize,
                    Met {
                        ends: Ends::On,
                        stop,
                        visits: 0,
                    },
                );
                *done |= 1 << r;
            }
            r += 1;
        }
        (into, at, true)
    };
    // The roots the rays go through, each with the rays in it: the origin's
    // first, then each one rays still going leave it into, by the axis
    // each leaves by (as `walk` crosses a root).
    let mut todo = [([0i32; 3], 0u32); PACKET];
    todo[0] = ([0; 3], group);
    let (mut len, mut rounds) = (1, 0);
    while len > 0 && rounds < 4 * PACKET {
        rounds += 1;
        len -= 1;
        let (roots, rays_in) = todo[len];
        let root = Node {
            corner: [0; 3],
            zoom: 0,
            roots,
        };
        let parent = q.root(root.index(o));
        let (into, at, entered_any) = enter(q, root, parent, true, rays_in, &mut t, done);
        if entered_any && into != 0 {
            // The nodes gone into, a zoom an entry: the node, what was
            // handed it, the rays gone into it, and the next of its
            // children to take.
            let mut stack = [(root, at, 0u32, 0u32); DEPTH];
            stack[0] = (root, at, into, 0);
            let mut depth = 1;
            while depth > 0 {
                let (n, at, rays_in, k) = stack[depth - 1];
                if k == 8 || rays_in & !*done == 0 {
                    depth -= 1;
                    continue;
                }
                stack[depth - 1].3 = k + 1;
                // The children nearest the rays first: on an axis they
                // fall along, the upper half first.
                let c = k ^ sign;
                let half = (under(n.zoom) >> 1) + 1;
                let side = |a: u32, corner: u32| {
                    if c >> a & 1 != 0 {
                        corner | half
                    } else {
                        corner
                    }
                };
                let child = Node {
                    corner: [
                        side(0, n.corner[0]),
                        side(1, n.corner[1]),
                        side(2, n.corner[2]),
                    ],
                    zoom: n.zoom + 1,
                    ..n
                };
                let (into, child_at, entered_any) =
                    enter(q, child, at, false, rays_in, &mut t, done);
                if entered_any && into != 0 && depth < DEPTH {
                    stack[depth] = (child, child_at, into, 0);
                    depth += 1;
                }
            }
        }
        // The rays still going: into the next root, past the face each
        // leaves this one by.
        let (lo, hi) = bounds(&root, og);
        let mut r = 0;
        while r < PACKET {
            if rays_in >> r & 1 != 0 && *done >> r & 1 == 0 {
                let ray = &rays[r];
                let (x, y, z) = spans(ray, (lo, hi), t[r]);
                let e = crossing(ray.direction, [x.1, y.1, z.1], false);
                let by = if axis(ray.direction, e) > 0.0 { 1 } else { -1 };
                let next = [
                    roots[0] + (e == 0) as i32 * by,
                    roots[1] + (e == 1) as i32 * by,
                    roots[2] + (e == 2) as i32 * by,
                ];
                let mut j = 0;
                while j < len
                    && (todo[j].0[0] != next[0]
                        || todo[j].0[1] != next[1]
                        || todo[j].0[2] != next[2])
                {
                    j += 1;
                }
                if j == len && len < PACKET {
                    todo[len] = (next, 0);
                    len += 1;
                }
                if j < len {
                    todo[j].1 |= 1 << r;
                }
            }
            r += 1;
        }
    }
    visits
}

/// What was handed each node over where a walk starts, by its zoom: the
/// line it resumes from, as a beam ray left it (`Keep`), read only where
/// the walk climbs over its start.
pub trait Over<A: Copy> {
    fn over(&self, zoom: Zoom) -> A;
}

/// Over a walk from the roots, nothing: no node is over a root, so it is
/// never read.
#[derive(Clone, Copy)]
pub struct Rooted<A>(pub A);

impl<A: Copy> Over<A> for Rooted<A> {
    fn over(&self, _: Zoom) -> A {
        self.0
    }
}

/// What a walk leaves of its line where it ends (`walk`), for the walks
/// that resume from it: the node it ended at, what was handed it (`live`),
/// and what was handed every node over it, its stack by zoom. A walk none
/// resumes from leaves nothing (`()`), and holds no copy of its stack.
pub trait Keep<A: Copy> {
    fn keep(&mut self, node: &Node, live: A, stack: &[A; DEPTH]);
}

impl<A: Copy> Keep<A> for () {
    fn keep(&mut self, _: &Node, _: A, _: &[A; DEPTH]) {}
}

/// What a ray does at the node `n` it enters over `t0..t1`, having walked
/// to `t`, the node answered `i`: how its walk ends there, the planes it
/// entered where it stops, and whether it goes on into the node's
/// children. The one rule a node is met by, by one ray's walk or a
/// packet's (`packet`).
#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn meet(
    ray: &Ray,
    n: &Node,
    og: [u32; 3],
    (lo, hi): ([f32; 3], [f32; 3]),
    (x, y, z): ((f32, f32), (f32, f32), (f32, f32)),
    (t, t0, t1): (f32, f32, f32),
    reach: [f32; 2],
    i: &In<[u32; 2]>,
    detail: Detail,
    until: Until,
) -> (Ends, [f32; 3], bool) {
    let u = under(n.zoom);
    let mut into = false;
    let inside =
        lo[0] <= 0.0 && 0.0 < hi[0] && lo[1] <= 0.0 && 0.0 < hi[1] && lo[2] <= 0.0 && 0.0 < hi[2];
    let side = hi[0] - lo[0];
    // The planes the ray enters where it stops: the node's, or the
    // box a cell keeping less than its cube is.
    let mut near = [x.0, y.0, z.0];
    // ESVO's contour (Laine & Karras 2010) on every axis alike:
    // whether the ray's span in a split node meets the box in its
    // steps its content lies in (`answer::box_of`), grown by `w`
    // on every side. Nothing a ray meets is passed for being thin.
    let meets_box = |w: f32| {
        let (first, past) = box_of(i.asset);
        // Step `e` of the box on axis `a`, on the grid's own planes
        // (`units`), as a node's own are: the far one the node's.
        let step = |a: usize, e: u32, out: u32| {
            if e >= BOX_STEPS {
                return hi[a];
            }
            // A step's width a whole count of the finest grid's
            // over a node `BOX_STEPS` or more of them wide; finer,
            // rounded out (`out` the step less one to round up), as
            // `e · side` fits a u32 there.
            let at = if u >= BOX_STEPS - 1 {
                e * ((u >> BOX_BITS) + 1)
            } else {
                (e * (u + 1) + out) >> BOX_BITS
            };
            units(n.corner[a].wrapping_add(at), og[a], n.roots[a])
        };
        let (bx, by, bz) = spans(
            ray,
            (
                [
                    step(0, first[0], 0) - w,
                    step(1, first[1], 0) - w,
                    step(2, first[2], 0) - w,
                ],
                [
                    step(0, past[0], BOX_STEPS - 1) + w,
                    step(1, past[1], BOX_STEPS - 1) + w,
                    step(2, past[2], BOX_STEPS - 1) + w,
                ],
            ),
            t0,
        );
        greater(greater(greater(t0, bx.0), by.0), bz.0)
            < lesser(lesser(lesser(t1, bx.1), by.1), bz.1)
    };
    let ends = match i.kind {
        Kind::Lacks => Ends::Lacks,
        // A cell keeping less than its cube is the box its share
        // fills (`kept_bounds`): an eye's ray stops where it enters
        // the box and passes it as air where it misses; a beam ray
        // stops at its node, as the samples it hands on may meet it.
        Kind::Cell
            if until == Until::Opaque
                && !transparent(i.asset[0])
                && !inside
                && kept(i.asset[0]) < EIGHTHS =>
        {
            let (bx, by, bz) = spans(ray, kept_bounds((lo, hi), i.asset[0]), t);
            let (b0, b1) = (
                greater(greater(greater(t, bx.0), by.0), bz.0),
                lesser(lesser(lesser(ray.reach, bx.1), by.1), bz.1),
            );
            near = [bx.0, by.0, bz.0];
            if b1 > b0 {
                Ends::Stop
            } else {
                Ends::On
            }
        }
        Kind::Cell
            if match until {
                Until::Opaque => !transparent(i.asset[0]) && !inside,
                Until::Transparent => transparent(i.asset[0]),
                Until::Beam => !transparent(i.asset[0]),
            } =>
        {
            Ends::Stop
        }
        Kind::Cell => Ends::On,
        // A beam ray passes whole a split whose box it clears by
        // `2√2` of the grid's width there, as the grid's rays about
        // it clear it too; else it stops at one it may not share with
        // them, and enters every other.
        Kind::Split if until == Until::Beam => {
            // The grid's rays' spacing at the node's far corner: a
            // split narrower than `2√2` of that may lie between them
            // (`narrow`).
            let width = detail.spacing.width(libm::sqrtf(reach[0]));
            let narrow = narrow(side * side, width * width);
            let meets = meets_box(NARROW * width);
            into = meets && !narrow;
            if meets && narrow {
                Ends::Stop
            } else {
                Ends::On
            }
        }
        // A span wholly out of the box, or touching it alone, meets
        // nothing a ray cannot see through, and a walk to that passes
        // the node whole. A walk to what is transparent goes in: its
        // stop is where that span begins.
        Kind::Split => {
            into = until == Until::Transparent || meets_box(0.0);
            Ends::On
        }
    };
    (ends, near, into)
}

/// The first cell `ray` meets within its reach that `until` stops at, at
/// the zoom `detail` asks where it meets it, its line left where it ended
/// to `keep`: down the pyramid from `from`, a node holding where the ray
/// is at its `t`, what was handed it, and that `t` — the world's root holding
/// the origin at 0 (`first`), or a node of the line a beam ray handed on
/// (doc/minutes/20261022-a-cell-casts-a-beam.md) — what was handed each node
/// over it read off `over` where the walk climbs past it, and through the
/// roots the ray crosses. What the ray starts inside is not met when it
/// stops at what it cannot see through, so an eye under the ground is not stopped
/// where it stands.
///
/// The walk holds its node, each axis an integer in its root (`Node`), and
/// where the ray is, `t`; the planes, the span and the axis entered across
/// are worked out from those where asked. A node is entered once: asked
/// whether anything under it is not transparent at the zooms every ray from
/// the origin asks within it (`asked`); a cell — the asset at that zoom, or
/// air where all under it is — the walk stops at (for an eye, where it is not
/// transparent,
/// but not in the cell it starts in, and where the ray enters the box a
/// cell keeping less than its cube is, `kept_bounds`; for what is climbed
/// out of, where it does not) or goes on past; else its child the ray is in (PUSH, Laine &
/// Karras 2010 §3). Past a node the corner steps on the axis the ray leaves
/// by (Amanatides & Woo 1987) and the walk is at the child of the nearest
/// ancestor holding the next node, by the highest bit the step changed
/// (POP), or past a root, in the next root, from its top. One loop.
#[expect(
    clippy::too_many_arguments,
    reason = "the walk's query, origin, ray, detail, stop and start, and the two visitors it hands each node, each its own (Laine & Karras 2010)"
)]
pub fn walk<Q: Query>(
    q: &mut Q,
    o: [i32; 3],
    ray: &Ray,
    detail: Detail,
    until: Until,
    from: (Node, Q::At, f32),
    over: &impl Over<Q::At>,
    keep: &mut impl Keep<Q::At>,
) -> Met<Q::At> {
    let og = grid_of(o);
    // What the walk carries is what nothing it holds gives again: the node
    // and what was handed it, where the ray is, what it keeps of every node
    // over it (`stack`, its own from `low` down, `over`'s above), and its
    // visits. The node's planes and the world's node it is follow from its
    // corner, and are worked out where they are read.
    let (mut n, mut at, mut t) = from;
    // The cone's extent along each axis, `√(1 − dₐ²)`, a side's prune's
    // share (`meets_box`); a beam ray's less by the angle its grid's rays
    // part by, so it prunes no more than any of them on any side, up to the
    // second order in the cell's angle its window's farther rays may part.
    let parts = if until == Until::Beam {
        core::f32::consts::SQRT_2 * detail.apart * detail.cone.angle
    } else {
        0.0
    };
    let along = |d: f32| greater(libm::sqrtf(greater(1.0 - d * d, 0.0)) - parts, 0.0);
    let facing = [
        along(ray.direction[0]),
        along(ray.direction[1]),
        along(ray.direction[2]),
    ];
    let mut stack = [at; DEPTH];
    let (mut low, mut visits) = (n.zoom, 0);
    loop {
        let u = under(n.zoom);
        let (lo, hi) = bounds(&n, og);
        // The ray's span in it.
        let (x, y, z) = spans(ray, (lo, hi), t);
        let (t0, t1) = (
            greater(greater(greater(t, x.0), y.0), z.0),
            lesser(lesser(lesser(ray.reach, x.1), y.1), z.1),
        );
        // Only touched, at a face or an edge: not entered, nothing in it met.
        let mut into = false;
        if t1 > t0 {
            let e = detail.eye;
            let reach = reach(
                [lo[0] - e[0], lo[1] - e[1], lo[2] - e[2]],
                [hi[0] - e[0], hi[1] - e[1], hi[2] - e[2]],
            );
            let i = q.node(&mut at, n.zoom, n.idx(o), asked(detail, reach), t0);
            visits += 1;
            let inside = lo[0] <= 0.0
                && 0.0 < hi[0]
                && lo[1] <= 0.0
                && 0.0 < hi[1]
                && lo[2] <= 0.0
                && 0.0 < hi[2];
            let side = hi[0] - lo[0];
            // The planes the ray enters where it stops: the node's, or the
            // box a cell keeping less than its cube is.
            let mut near = [x.0, y.0, z.0];
            // ESVO's contour (Laine & Karras 2010) on every axis alike:
            // whether the ray's span in a split node meets the box in its
            // steps its content lies in (`answer::box_of`), grown by `w`
            // on every side. Nothing a ray meets is passed for being thin.
            let meets_box = |w: f32| {
                let (first, past) = box_of(i.asset);
                // Step `e` of the box on axis `a`, on the grid's own planes
                // (`units`), as a node's own are: the far one the node's.
                let step = |a: usize, e: u32, out: u32| {
                    if e >= BOX_STEPS {
                        return hi[a];
                    }
                    // A step's width a whole count of the finest grid's
                    // over a node `BOX_STEPS` or more of them wide; finer,
                    // rounded out (`out` the step less one to round up), as
                    // `e · side` fits a u32 there.
                    let at = if u >= BOX_STEPS - 1 {
                        e * ((u >> BOX_BITS) + 1)
                    } else {
                        (e * (u + 1) + out) >> BOX_BITS
                    };
                    units(n.corner[a].wrapping_add(at), og[a], n.roots[a])
                };
                let (bx, by, bz) = spans(
                    ray,
                    (
                        [
                            step(0, first[0], 0) - w,
                            step(1, first[1], 0) - w,
                            step(2, first[2], 0) - w,
                        ],
                        [
                            step(0, past[0], BOX_STEPS - 1) + w,
                            step(1, past[1], BOX_STEPS - 1) + w,
                            step(2, past[2], BOX_STEPS - 1) + w,
                        ],
                    ),
                    t0,
                );
                greater(greater(greater(t0, bx.0), by.0), bz.0)
                    < lesser(lesser(lesser(t1, bx.1), by.1), bz.1)
            };
            let ends = match i.kind {
                Kind::Lacks => End::Lacks,
                // A cell keeping less than its cube is the box its share
                // fills (`kept_bounds`): an eye's ray stops where it enters
                // the box and passes it as air where it misses; a beam ray
                // stops at its node, as the samples it hands on may meet it.
                Kind::Cell
                    if until == Until::Opaque
                        && !transparent(i.asset[0])
                        && !inside
                        && kept(i.asset[0]) < EIGHTHS =>
                {
                    let (bx, by, bz) = spans(ray, kept_bounds((lo, hi), i.asset[0]), t);
                    let (b0, b1) = (
                        greater(greater(greater(t, bx.0), by.0), bz.0),
                        lesser(lesser(lesser(ray.reach, bx.1), by.1), bz.1),
                    );
                    near = [bx.0, by.0, bz.0];
                    if b1 > b0 {
                        End::Stop
                    } else {
                        End::On
                    }
                }
                Kind::Cell
                    if match until {
                        Until::Opaque => !transparent(i.asset[0]) && !inside,
                        Until::Transparent => transparent(i.asset[0]),
                        Until::Beam => !transparent(i.asset[0]),
                    } =>
                {
                    End::Stop
                }
                Kind::Cell => End::On,
                // A beam ray passes whole a split whose box it clears by
                // `2√2` of the grid's width there, as the grid's rays about
                // it clear it too; else it stops at one it may not share with
                // them, and enters every other.
                Kind::Split if until == Until::Beam => {
                    // The grid's rays' spacing at the node's far corner: a
                    // split narrower than `2√2` of that may lie between them
                    // (`narrow`).
                    let width = detail.spacing.width(libm::sqrtf(reach[0]));
                    let narrow = narrow(side * side, width * width);
                    let meets = meets_box(NARROW * width);
                    into = meets && !narrow;
                    if meets && narrow {
                        End::Stop
                    } else {
                        End::On
                    }
                }
                // A span wholly out of the box, or touching it alone, meets
                // nothing a ray cannot see through, and a walk to that passes
                // the node whole. A walk to what is transparent goes in: its
                // stop is where that span begins.
                Kind::Split => {
                    into = until == Until::Transparent || meets_box(0.0);
                    End::On
                }
            };
            if ends != End::On {
                // Where the walk ends, its stop, made there and nowhere else:
                // where the ray enters the node, or its asset's box
                // (`entered`).
                let (t, axis) = entered(ray, near);
                let stop = Stop {
                    t,
                    axis,
                    node: n,
                    at,
                    asset: i.asset,
                };
                keep.keep(&n, at, &stack);
                return Met { ends, stop, visits };
            }
        }
        // Every way on makes the next node: what is handed it is decided here,
        // its parent's At or, at a root, its own (`fresh`), and the node is
        // made once, after (`q.child`, `q.root`).
        let (parent, fresh) = if into {
            // PUSH: the child holding where the ray is at `t0`, past each mid
            // plane it has crossed by then.
            let half = (u >> 1) + 1;
            let mid = [
                units(n.corner[0].wrapping_add(half), og[0], n.roots[0]),
                units(n.corner[1].wrapping_add(half), og[1], n.roots[1]),
                units(n.corner[2].wrapping_add(half), og[2], n.roots[2]),
            ];
            let crossed = [
                past(ray.dir[0], ray.inverse[0], mid[0], t0),
                past(ray.dir[1], ray.inverse[1], mid[1], t0),
                past(ray.dir[2], ray.inverse[2], mid[2], t0),
            ];
            stack[n.zoom as usize] = at;
            let side = |k: bool, c: u32| if k { c | half } else { c };
            n = Node {
                corner: [
                    side(crossed[0], n.corner[0]),
                    side(crossed[1], n.corner[1]),
                    side(crossed[2], n.corner[2]),
                ],
                zoom: n.zoom + 1,
                ..n
            };
            t = t0;
            (at, false)
        } else {
            if t1 >= ray.reach {
                // Gone on to its reach: no stop, which no asker reads.
                let stop = Stop {
                    t: t1,
                    axis: 0,
                    node: n,
                    at,
                    asset: AIR,
                };
                let met = Met {
                    ends: End::On,
                    stop,
                    visits,
                };
            }
            // ADVANCE: the corner stepped on the axis the ray leaves by, the
            // first far plane it reaches.
            let e = crossing(ray.dir, [x.1, y.1, z.1], false);
            let up = axis(ray.dir, e) > 0.0;
            let old = axis(n.corner, e);
            let new = if up {
                old.wrapping_add(u).wrapping_add(1)
            } else {
                old.wrapping_sub(u).wrapping_sub(1)
            };
            // Never back: the next node holds where the ray is on the integers,
            // but near an edge another axis's plane, a float product, may
            // fall a last bit before it.
            t = greater(t, t1);
            if (up && old.wrapping_add(u) == u32::MAX) || (!up && old == 0) {
                // Past a root: into the next, from its top, over which nothing
                // is asked or held.
                let by = if up { 1 } else { -1 };
                n.roots = [
                    n.roots[0] + (e == 0) as i32 * by,
                    n.roots[1] + (e == 1) as i32 * by,
                    n.roots[2] + (e == 2) as i32 * by,
                ];
                (n.corner, n.zoom, low) = ([0; 3], 0, 0);
                (at, true)
            } else {
                // POP: the child, at `l`, of the nearest ancestor holding the next
                // node, by the highest bit the step changed; each axis's corner
                // that ancestor's child's.
                n.corner = [
                    if e == 0 { new } else { n.corner[0] },
                    if e == 1 { new } else { n.corner[1] },
                    if e == 2 { new } else { n.corner[2] },
                ];
                let h = 31 - (old ^ new).leading_zeros();
                let keep = !((1u32 << h) - 1);
                n.corner = [n.corner[0] & keep, n.corner[1] & keep, n.corner[2] & keep];
                let l = FINEST_ZOOM - h;
                // What was handed the parent: its stack entry, the walk's
                // own from where it started down or `over`'s over it.
                n.zoom = l;
                let parent = if l > low {
                    stack[(l - 1) as usize]
                } else {
                    over.over(l - 1)
                };
                low = low.min(l);
                (parent, false)
            }
        };
        let node = n.idx(o);
        at = if fresh {
            q.root(node)
        } else {
            q.child(&parent, n.zoom, node)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A node held from one origin is the same world node held from
    /// another a little off, across a root's edge and X's seam alike.
    #[test]
    fn node_moves_origin() {
        let edges = [0i32, -1, 5, i32::MAX - 3, i32::MIN + 2, 1 << 20];
        for &a in &edges {
            for d in [-700i32, -1, 0, 1, 700] {
                // Y and Z do not go round: about their roots' edge at 0.
                let near = |v: i32| (v % 1000).wrapping_sub(500);
                let from = [a, near(a.wrapping_add(13)), near(a.wrapping_sub(29))];
                let to = from.map(|v| v.wrapping_add(d));
                for zoom in [1, 9, 20, 30] {
                    let ray = Ray::new([0.6, -0.48, 0.64], 1000.0);
                    let p = point_at(from, &ray, 2.5);
                    let (n, _) = Node {
                        corner: [0; 3],
                        zoom,
                        roots: [0; 3],
                    }
                    .over(p);
                    let n = Node {
                        zoom: n.zoom.min(zoom),
                        ..n
                    };
                    let m = n.moved(from, to);
                    assert_eq!(m.idx(to), n.idx(from), "{a} {d} {zoom}");
                }
            }
        }
    }
    use crate::answer;
    use crate::answer::BoxWords;
    use crate::answer::{kept_word, BOX_WHOLE, EIGHTHS, FULL};
    use crate::cube::ground::stone;
    use crate::grid;
    use crate::overwrite::AIR;

    /// One cell of stone at `TARGET`, a unit cell, keeping `kept` eighths of
    /// its cube; air about it.
    struct One {
        kept: u32,
    }

    const TARGET: [i32; 3] = [10, 0, 0];

    impl Query for One {
        type At = ();
        fn root(&mut self, _: [i32; 3]) {}
        fn child(&mut self, _: &(), _: Zoom, _: [i32; 3]) {}
        fn node(
            &mut self,
            _: &mut (),
            zoom: Zoom,
            node: [i32; 3],
            _: [Zoom; 2],
            _: f32,
        ) -> In<[u32; 2]> {
            let s = UNIT_ZOOM - zoom;
            let over = (0..3).all(|a| TARGET[a] >> s == node[a]);
            match (zoom < UNIT_ZOOM, over) {
                (true, true) => In {
                    kind: Kind::Split,
                    asset: BOX_WHOLE,
                },
                (false, true) => In {
                    kind: Kind::Cell,
                    asset: [kept_word(stone as u32, self.kept), 0],
                },
                _ => In {
                    kind: Kind::Cell,
                    asset: [AIR, 0],
                },
            }
        }
    }

    /// A ray along +x from five units, at `y` and `z` units, against a cell
    /// keeping `kept` eighths.
    fn along(kept: u32, y: f32, z: f32) -> Met<()> {
        let steps = |u: f32| (u * (1 << FRAC_W) as f32) as i32;
        let o = [steps(5.0), steps(y), steps(z)];
        let ray = Ray::new([1.0, 0.0, 0.0], 20.0);
        let detail = Detail {
            range: [UNIT_ZOOM, UNIT_ZOOM],
            eye: [0.0; 3],
            spacing: Spacing::NONE,
        };
        first(&mut One { kept }, o, &ray, detail, Until::Opaque)
    }

    #[test]
    fn share_box_volume() {
        for k in 1..=EIGHTHS {
            let (lo, hi) = kept_bounds(([0.0; 3], [1.0; 3]), kept_word(0, k));
            let volume = (hi[0] - lo[0]) * (hi[1] - lo[1]) * (hi[2] - lo[2]);
            assert!(
                (volume - k as f32 / EIGHTHS as f32).abs() < 1e-6,
                "{k}: {volume}"
            );
            // About the cube's center.
            assert!((lo[0] + hi[0] - 1.0).abs() < 1e-6);
        }
        // What fills a share keeps its eighths, floored.
        assert_eq!(answer::eighths(FULL), EIGHTHS);
        assert_eq!(answer::eighths(FULL / 8 - 1), 0);
        assert_eq!(answer::eighths(3 * FULL / 8), 3);
    }

    #[test]
    fn ray_stops_at_entry() {
        // An eighth kept is a box half the side, a quarter in from each face.
        let met = along(1, 0.5, 0.5);
        assert_eq!(met.ends, End::Stop);
        assert!((met.stop.t - 5.25).abs() < 1e-5, "{}", met.stop.t);
        assert_eq!(met.stop.axis, 0);
        // Whole, the cube's face.
        let met = along(EIGHTHS, 0.5, 0.5);
        assert!((met.stop.t - 5.0).abs() < 1e-5, "{}", met.stop.t);
    }

    /// Every split's content in the box `0` names, air at the leaves.
    struct Boxed(BoxWords);

    impl Query for Boxed {
        type At = ();
        fn root(&mut self, _: [i32; 3]) {}
        fn child(&mut self, _: &(), _: Zoom, _: [i32; 3]) {}
        fn node(
            &mut self,
            _: &mut (),
            zoom: Zoom,
            _: [i32; 3],
            _: [Zoom; 2],
            _: f32,
        ) -> In<[u32; 2]> {
            if zoom < UNIT_ZOOM {
                In {
                    kind: Kind::Split,
                    asset: self.0,
                }
            } else {
                In {
                    kind: Kind::Cell,
                    asset: [AIR, 0],
                }
            }
        }
    }

    /// A ray passes a split whole where it misses the split's box on any
    /// axis, and enters it where it meets it.
    #[test]
    fn ray_passes_missed_split() {
        use crate::answer::box_word;
        let steps = |u: f32| (u * (1 << FRAC_W) as f32) as i32;
        let o = [steps(5.0), steps(0.4), steps(0.6)];
        let ray = Ray::new([1.0, 0.0, 0.0], 20.0);
        let detail = Detail {
            range: [UNIT_ZOOM, UNIT_ZOOM],
            eye: [0.0; 3],
            spacing: Spacing {
                angle: grid::CELL,
                across: 0.0,
            },
        };
        let walk = |b: BoxWords| first(&mut Boxed(b), o, &ray, detail, Until::Opaque);
        // The origin lies in the root's lower half on every axis.
        for missed in [
            box_word([512, 0, 0], [1024; 3]),
            box_word([0, 512, 0], [1024; 3]),
            box_word([0, 0, 512], [1024; 3]),
        ] {
            let met = walk(missed);
            assert_eq!((met.ends, met.visits), (End::On, 1), "{:?}", box_of(missed));
        }
        assert!(walk(box_word([0; 3], [512; 3])).visits > 1);
        // An edge that is not a half: the origin lies in the root's first
        // step along z, so a box from its second misses and one to it
        // meets.
        assert_eq!(walk(box_word([0, 0, 1], [1024; 3])).visits, 1);
        assert!(walk(box_word([0; 3], [1024, 1024, 1])).visits > 1);
        // A beam ray meets a box it clears by less than `2√2` of its width,
        // and passes one it clears by more: the root's second step along z
        // starts 2^14 units over the origin.
        let beam = |width: f32| {
            let detail = Detail {
                spacing: Spacing {
                    angle: 0.0,
                    across: width,
                },
                ..detail
            };
            first(
                &mut Boxed(box_word([0, 0, 1], [1024; 3])),
                o,
                &ray,
                detail,
                Until::Beam,
            )
        };
        assert_eq!(beam(1.0).visits, 1);
        assert!(beam(1.0e4).visits > 1);
        // Nothing is passed for being thin: a slab one step thick about the
        // origin, 2^14 units, is entered by a ray whose beam's spacing is far
        // wider, which no walk but a beam's reads.
        let wide = Detail {
            spacing: Spacing {
                angle: 0.0,
                across: 1.0e5,
            },
            ..detail
        };
        let met = first(
            &mut Boxed(box_word([0; 3], [1024, 1024, 1])),
            o,
            &down,
            wide,
            Until::Opaque,
        );
        assert!(met.visits > 1);
        // With no cone, nothing is pruned.
        let exact = Detail {
            cone: Cone::EXACT,
            ..detail
        };
        let met = first(
            &mut Boxed(box_word([0; 3], [1024, 1024, 1])),
            o,
            &ray,
            exact,
            Until::Opaque,
        );
        assert!(met.visits > 1);
    }

    /// Flat ground under z = 5, stone a leaf, air over it, every split's
    /// box its span along z.
    struct Flat;

    impl Query for Flat {
        type At = ();
        fn root(&mut self, _: [i32; 3]) {}
        fn child(&mut self, _: &(), _: Zoom, _: [i32; 3]) {}
        fn node(
            &mut self,
            _: &mut (),
            zoom: Zoom,
            node: [i32; 3],
            _: [Zoom; 2],
            _: f32,
        ) -> In<[u32; 2]> {
            let side = 1i64 << (UNIT_ZOOM - zoom.min(UNIT_ZOOM));
            let bottom = node[2] as i64 * side;
            if zoom >= UNIT_ZOOM || bottom >= 5 || bottom + side <= 5 {
                let solid = zoom >= UNIT_ZOOM && node[2] < 5 || zoom < UNIT_ZOOM && bottom < 5;
                In {
                    kind: Kind::Cell,
                    asset: [
                        if solid {
                            kept_word(stone as u32, EIGHTHS)
                        } else {
                            AIR
                        },
                        0,
                    ],
                }
            } else {
                In {
                    kind: Kind::Split,
                    asset: crate::ground::box_along_z([i16::MIN as i32, 5], (zoom, node[2])),
                }
            }
        }
    }

    /// A ray down onto flat ground beside a seam of the nodes over it stops
    /// on the ground, its prune taking no strip off the nodes' own faces.
    #[test]
    fn a_ray_down_beside_a_seam_stops_on_the_ground() {
        let steps = |u: f32| (u * (1 << FRAC_W) as f32) as i32;
        let o = [steps(0.01), steps(5.3), steps(10.0)];
        let ray = Ray::new([0.0, 0.0, -1.0], 40.0);
        let detail = Detail {
            tau: 1.0,
            range: [UNIT_ZOOM, UNIT_ZOOM],
            eye: [0.0; 3],
            cone: Cone {
                angle: 0.0,
                across: 1.0,
            },
            apart: 1.0,
        };
        let met = first(&mut Flat, o, &ray, detail, Until::Opaque);
        assert_eq!(met.ends, Ends::Stop);
        assert!((met.stop.t - 5.0).abs() < 1e-3, "{}", met.stop.t);
    }

    #[test]
    fn ray_beside_box_passes() {
        let met = along(1, 0.1, 0.5);
        assert_eq!(met.ends, End::On);
        let met = along(4, 0.5, 0.05);
        assert_eq!(met.ends, End::On);
        // The whole cube stops it there.
        assert_eq!(along(EIGHTHS, 0.1, 0.5).ends, End::Stop);
    }

    /// Hills: ground of a few waves, solid under `h(x, y)`, a cell at the
    /// zoom asked keeping the share of its column under the ground, a split
    /// over it whose box runs to the highest ground under it, rounded to its
    /// coarsest zoom asked, as the world's ground is answered.
    struct Hills {
        nodes: u32,
        /// Flat terraces a whole unit apart, stepping along x + y, in place
        /// of the waves: ground whose faces lie on the grid's own planes.
        terraces: bool,
    }

    impl Hills {
        fn h(&self, x: f64, y: f64) -> f64 {
            if self.terraces {
                return 20.0 + libm::floor((x + y) / 16.0);
            }
            40.0 + 30.0 * libm::sin(x / 97.0)
                + 22.0 * libm::sin(y / 61.0 + 1.3)
                + 9.0 * libm::sin((x + y) / 23.0)
        }

        /// The ground's range over `[x0, x1] × [y0, y1]`: its waves' sum at
        /// their widest, a bound, never a value it misses.
        fn range(&self, x0: f64, x1: f64, y0: f64, y1: f64) -> (f64, f64) {
            if self.terraces {
                return (self.h(x0, y0), self.h(x1, y1) + 1.0);
            }
            let wave = |a: f64, k: f64, phi: f64, u0: f64, u1: f64| {
                let (v0, v1) = (u0 / k + phi, u1 / k + phi);
                if v1 - v0 >= core::f64::consts::TAU {
                    return (-a, a);
                }
                let (s0, s1) = (libm::sin(v0), libm::sin(v1));
                let (mut lo, mut hi) = (s0.min(s1), s0.max(s1));
                let pi = core::f64::consts::PI;
                if pi / 2.0 + libm::ceil((v0 - pi / 2.0) / (2.0 * pi)) * 2.0 * pi <= v1 {
                    hi = 1.0;
                }
                if -pi / 2.0 + libm::ceil((v0 + pi / 2.0) / (2.0 * pi)) * 2.0 * pi <= v1 {
                    lo = -1.0;
                }
                (a * lo, a * hi)
            };
            let (a, b, c) = (
                wave(30.0, 97.0, 0.0, x0, x1),
                wave(22.0, 61.0, 1.3, y0, y1),
                wave(9.0, 23.0, 0.0, x0 + y0, x1 + y1),
            );
            (40.0 + a.0 + b.0 + c.0, 40.0 + a.1 + b.1 + c.1)
        }
    }

    impl Query for Hills {
        type At = ();
        fn root(&mut self, _: [i32; 3]) {}
        fn child(&mut self, _: &(), _: Zoom, _: [i32; 3]) {}
        fn node(
            &mut self,
            _: &mut (),
            zoom: Zoom,
            c: [i32; 3],
            asked: [Zoom; 2],
            _: f32,
        ) -> In<[u32; 2]> {
            self.nodes += 1;
            let side = libm::pow(2.0, UNIT_ZOOM as f64 - zoom as f64);
            let (x0, y0, z0) = (c[0] as f64 * side, c[1] as f64 * side, c[2] as f64 * side);
            let air = In {
                kind: Kind::Cell,
                asset: crate::answer::AIR,
            };
            if zoom >= asked[1] {
                let h = self.h(x0 + side / 2.0, y0 + side / 2.0);
                let k = (((h - z0) / side).clamp(0.0, 1.0) * 8.0) as u32;
                return match k {
                    0 => air,
                    8.. => In {
                        kind: Kind::Cell,
                        asset: [stone as u32, 0],
                    },
                    k => In {
                        kind: Kind::Cell,
                        asset: [kept_word(stone as u32, k), 0],
                    },
                };
            }
            let (lo, hi) = self.range(x0, x0 + side, y0, y0 + side);
            let zc = asked[0].max(zoom + 1);
            let sn = libm::pow(2.0, UNIT_ZOOM as f64 - zc as f64);
            let hi = (libm::floor(hi / sn) + 1.0) * sn;
            if hi <= z0 {
                return air;
            }
            if lo >= z0 + side {
                return In {
                    kind: Kind::Split,
                    asset: BOX_WHOLE,
                };
            }
            let hz = libm::ceil((hi - z0) / side * BOX_STEPS as f64).clamp(1.0, BOX_STEPS as f64);
            In {
                kind: Kind::Split,
                asset: crate::answer::box_word([0; 3], [BOX_STEPS, BOX_STEPS, hz as u32]),
            }
        }
    }

    /// Hills with what each node is handed (a hash of its line, so a node
    /// handed another's would show), and nothing held past `x` 2000: there
    /// the world lacks.
    struct Handed(Hills);

    impl Query for Handed {
        type At = u64;
        fn root(&mut self, node: [i32; 3]) -> u64 {
            node.iter().fold(0x9e37_79b9_7f4a_7c15, |h, &v| {
                (h ^ v as u64).wrapping_mul(0x100_0000_01b3)
            })
        }
        fn child(&mut self, at: &u64, zoom: Zoom, node: [i32; 3]) -> u64 {
            node.iter().fold(at ^ zoom as u64, |h, &v| {
                (h ^ v as u64).wrapping_mul(0x100_0000_01b3)
            })
        }
        fn node(
            &mut self,
            at: &mut u64,
            zoom: Zoom,
            c: [i32; 3],
            asked: [Zoom; 2],
            t0: f32,
        ) -> In<[u32; 2]> {
            *at = at.rotate_left(7) ^ asked[0] as u64;
            let side = libm::pow(2.0, UNIT_ZOOM as f64 - zoom as f64);
            if c[0] as f64 * side > 2000.0 {
                self.0.nodes += 1;
                return In {
                    kind: Kind::Lacks,
                    asset: crate::answer::AIR,
                };
            }
            self.0.node(&mut (), zoom, c, asked, t0)
        }
    }

    /// A packet's rays stop each where its own walk stops, over hills seen
    /// from low and high, steep and grazing, up into the sky, and where the
    /// world lacks, each handed what its own walk is; and in fewer asks.
    #[test]
    fn a_packet_s_rays_stop_where_each_alone_stops() {
        let steps = |u: f64| (u * (1 << FRAC_W) as f64) as i32;
        let (mut alone, mut together, mut cells) = (0u32, 0u32, 0u32);
        let mut ended = [0u32; 3];
        for &(terraces, ex, ey, ez) in &[
            (false, 10.3, 7.9, 120.0),
            (false, -300.6, 40.2, 600.0),
            (false, 55.5, -12.25, 2900.0),
            // On the grid's planes: an eye on whole units over terraces.
            (true, 16.0, 32.0, 64.0),
            (true, -8.0, 0.0, 300.0),
        ] {
            let o = [steps(ex), steps(ey), steps(ez)];
            let detail = Detail {
                range: [13, UNIT_ZOOM],
                eye: [0.0; 3],
                spacing: Spacing {
                    angle: crate::grid::CELL,
                    across: 0.0,
                },
            };
            // A packet of rays on the grid's own diagonals and axes, of every
            // sign: each sign's rays walk apart, and ties at planes and
            // edges fall as each ray's own walk takes them.
            let exact = [
                [1.0, 0.0, -1.0],
                [-1.0, 0.0, -1.0],
                [0.0, 1.0, -1.0],
                [0.0, -1.0, -1.0],
                [1.0, 1.0, -1.0],
                [-1.0, 1.0, -1.0],
                [1.0, -1.0, -1.0],
                [-1.0, -1.0, -1.0],
                [0.0, 0.0, -1.0],
                [1.0, 0.0, 0.0],
                [0.0, -1.0, 0.0],
                [2.0, 1.0, -1.0],
                [1.0, 2.0, -2.0],
                [-2.0, 1.0, -1.0],
                [1.0, 1.0, 0.0],
                [3.0, -1.0, -1.0],
            ];
            let mut rays = [Ray::new([1.0, 0.0, 0.0], 1.0); PACKET];
            for k in 0..PACKET {
                let d = exact[k];
                let l = libm::sqrtf(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
                // The last reaches nowhere: it ends on the root it touches.
                let reach = if k == PACKET - 1 { 0.0 } else { 3.0e5 };
                rays[k] = Ray::new([d[0] / l, d[1] / l, d[2] / l], reach);
            }
            let mut met = [first(
                &mut Handed(Hills { nodes: 0, terraces }),
                o,
                &rays[0],
                detail,
                Until::Opaque,
            ); PACKET];
            let (_, left) = packet(
                &mut Handed(Hills { nodes: 0, terraces }),
                o,
                &rays,
                (1 << PACKET) - 1,
                detail,
                &mut |k, m| met[k] = m,
            );
            assert_eq!(left, 0, "rays the packet left from {:?}", (ex, ey, ez));
            for k in 0..PACKET {
                let m = first(
                    &mut Handed(Hills { nodes: 0, terraces }),
                    o,
                    &rays[k],
                    detail,
                    Until::Opaque,
                );
                ended[m.ends as usize] += 1;
                let got = met[k];
                assert_eq!(
                    (
                        got.ends,
                        got.stop.t.to_bits(),
                        got.stop.axis,
                        got.stop.node,
                        got.stop.at,
                        got.stop.asset
                    ),
                    (
                        m.ends,
                        m.stop.t.to_bits(),
                        m.stop.axis,
                        m.stop.node,
                        m.stop.at,
                        m.stop.asset
                    ),
                    "exact ray {k} from {:?}",
                    (ex, ey, ez)
                );
            }
            // Cells across a view from steep to grazing, each its 4 × 4
            // samples a quarter of a cell apart.
            for row in 0..24 {
                for col in 0..6 {
                    let pitch = 0.1 - 0.06 * row as f32;
                    let yaw = 0.3 * col as f32 - 0.8;
                    let mut rays = [Ray::new([1.0, 0.0, 0.0], 1.0); PACKET];
                    let a = crate::grid::CELL / 4.0;
                    for k in 0..PACKET {
                        let (p, w) = (pitch + a * (k / 4) as f32, yaw + a * (k % 4) as f32);
                        let d = [
                            libm::cosf(p) * libm::cosf(w),
                            libm::cosf(p) * libm::sinf(w),
                            libm::sinf(p),
                        ];
                        rays[k] = Ray::new(d, 3.0e5);
                    }
                    let mut met = [first(
                        &mut Handed(Hills { nodes: 0, terraces }),
                        o,
                        &rays[0],
                        detail,
                        Until::Opaque,
                    ); PACKET];
                    let mut world = Handed(Hills { nodes: 0, terraces });
                    let (asked, left) = packet(
                        &mut world,
                        o,
                        &rays,
                        (1 << PACKET) - 1,
                        detail,
                        &mut |k, m| met[k] = m,
                    );
                    assert_eq!(left, 0, "rays the packet left: row {row} col {col}");
                    together += asked;
                    cells += 1;
                    for k in 0..PACKET {
                        let mut one = Handed(Hills { nodes: 0, terraces });
                        let m = first(&mut one, o, &rays[k], detail, Until::Opaque);
                        ended[m.ends as usize] += 1;
                        alone += one.0.nodes;
                        let got = met[k];
                        let key = |m: Met<u64>| {
                            (
                                m.ends,
                                m.stop.t.to_bits(),
                                m.stop.axis,
                                m.stop.node,
                                m.stop.at,
                                m.stop.asset,
                            )
                        };
                        assert_eq!(
                            key(got),
                            key(m),
                            "eye {:?} row {row} col {col} ray {k}",
                            (ex, ey, ez)
                        );
                    }
                }
            }
        }
        // Every way a walk ends is among them: a stop, the sky, a lack.
        assert!(ended.iter().all(|&n| n > 0), "{ended:?}");
        assert!(
            together * 3 < alone,
            "{together} asks together, {alone} alone, {cells} cells"
        );
    }

    /// What a walk asks, in order: each node, and the zooms it is asked at.
    struct Asked<Q> {
        q: Q,
        log: [(Zoom, [i32; 3], [Zoom; 2]); 512],
        len: usize,
    }

    impl<Q: Query<At = ()>> Query for Asked<Q> {
        type At = ();
        fn root(&mut self, node: [i32; 3]) {
            self.q.root(node)
        }
        fn child(&mut self, at: &(), zoom: Zoom, node: [i32; 3]) {
            self.q.child(at, zoom, node)
        }
        fn node(
            &mut self,
            at: &mut (),
            zoom: Zoom,
            node: [i32; 3],
            asked: [Zoom; 2],
            t0: f32,
        ) -> In<[u32; 2]> {
            if self.len < self.log.len() {
                self.log[self.len] = (zoom, node, asked);
                self.len += 1;
            }
            self.q.node(at, zoom, node, asked, t0)
        }
    }

    fn asked_by<Q: Query<At = ()>>(q: Q) -> Asked<Q> {
        Asked {
            q,
            log: [(0, [0; 3], [0; 2]); 512],
            len: 0,
        }
    }

    /// The nodes holding the origin, asked once, are the nodes every ray from
    /// it enters first, each asked at the same zooms: what a ray reads of
    /// them kept is what it would have asked.
    #[test]
    fn every_ray_enters_the_origin_s_line_first_as_it_is_asked() {
        use crate::answer::box_word;
        let steps = |u: f32| (u * (1 << FRAC_W) as f32) as i32;
        let detail = Detail {
            range: [12, UNIT_ZOOM],
            eye: [0.0; 3],
            spacing: Spacing {
                angle: crate::grid::CELL,
                across: 0.0,
            },
        };
        for o in [
            [steps(5.3), steps(0.4), steps(1.6)],
            [steps(-2.7), steps(30.1), steps(-0.2)],
            [steps(1.0e5), steps(-7.5e3), steps(300.0)],
        ] {
            let world = || Boxed(box_word([0; 3], [1024; 3]));
            let mut line = asked_by(world());
            let visits = origin_line(&mut line, o, detail);
            assert_eq!(visits as usize, line.len);
            assert!(line.len > 1);
            for d in [
                [1.0, 0.2, -0.3],
                [-0.5, 0.9, 0.1],
                [0.1, -0.2, -1.0],
                [-0.7, -0.7, 0.05],
            ] {
                let l = libm::sqrtf(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
                let ray = Ray::new([d[0] / l, d[1] / l, d[2] / l], 50.0);
                let mut walk = asked_by(world());
                first(&mut walk, o, &ray, detail, Until::Opaque);
                // Each node the walk asks that holds the origin is the
                // line's at its zoom, asked alike.
                let (og, mut k) = (grid_of(o), 0);
                while k < walk.len {
                    let (zoom, node, asked) = walk.log[k];
                    let keep = !under(zoom);
                    let holding = Node {
                        corner: [og[0] & keep, og[1] & keep, og[2] & keep],
                        zoom,
                        roots: [0; 3],
                    };
                    if holding.index(o) == node && (zoom as usize) < line.len {
                        assert_eq!(line.log[zoom as usize], (zoom, node, asked));
                    }
                    k += 1;
                }
                // And it enters them first, root down.
                let mut z = 0;
                while z < line.len.min(walk.len) && walk.log[z] == line.log[z] {
                    z += 1;
                }
                assert!(z > 1, "{o:?} {d:?}: {z}");
            }
        }
    }
}
