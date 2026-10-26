// pxl-spec-core/cast.rs

//! gpu-portable: the first block a ray meets, at the zoom its distance asks
//! (doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md),
//! one descent for the CPU and the GPU over a query each answers its own
//! way (`Query`). A node coarser than the zoom its distance asks is asked
//! whether anything under it fills: nothing, and the ray goes on past it
//! whole; else its children the ray meets, nearest first. A node at that
//! zoom is asked its block, and the ray stops there or goes on.
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

use crate::answer::{fills, kept, side, slab_of, In, Kind, AIR, EIGHTHS};
use crate::grid::cell_at;
use crate::{axis, wrap_index, Zoom, FINEST_ZOOM, FRAC_W, UNIT_ZOOM};

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

/// The zoom asked at each distance: the camera grid's cells there
/// (`cell_at`), the distance scaled by `tau`, at least 1, held to `range`,
/// asked of a distance squared (`at`). A
/// stated zoom's cells wherever they are is a range of that zoom alone.
/// The distance is the eye's, `eye` units from the walk's origin: none for
/// the eye's own rays; for a ray from elsewhere, a ray toward the sun from
/// what an eye's ray met, where the eye stands from it, so a node is asked
/// at the zooms the eye's rays ask it (`reach`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detail {
    pub tau: f32,
    pub range: [Zoom; 2],
    pub eye: [f32; 3],
}

impl Detail {
    /// At `d2`, a distance squared.
    pub fn at(self, d2: f32) -> Zoom {
        cell_at(d2 * (self.tau * self.tau)).clamp(self.range[0], self.range[1])
    }
}

/// Which block a ray stops at: the first that fills, as an eye sees; or the
/// first that does not, as what is climbed out of ends; or, for a beam ray of
/// the camera grid, the first that fills or the first split too narrow to
/// hold one of the grid's rays for certain (`beam::narrow`), every split
/// entered whatever its slab (doc/minutes/20261022-a-cell-casts-a-beam.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Until {
    Fills,
    Air,
    Beam,
}

/// A ray from an origin, a Q3 position: its direction, how far it reaches, in units, and
/// its direction's reciprocal on each axis, 0 where it does not move along
/// one: a plane's crossing is a product, not a division made again at every
/// node (Laine and Karras, _Efficient Sparse Voxel Octrees_, 2010, the
/// per-ray coefficients).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub direction: [f32; 3],
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
        let direction = [
            moves(direction[0]),
            moves(direction[1]),
            moves(direction[2]),
        ];
        let inv = |d: f32| if d == 0.0 { 0.0 } else { 1.0 / d };
        Ray {
            direction,
            reach,
            inverse: [inv(direction[0]), inv(direction[1]), inv(direction[2])],
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

    /// The world's node it is from the origin `o`, X named as the world
    /// names it (pxl-spec space/coord/world.rs `wrap_index`).
    pub fn index(&self, o: [i32; 3]) -> [i32; 3] {
        let r0 = roots_of(o);
        [
            wrap_index(
                self.zoom,
                index_on(r0[0] + self.roots[0], self.corner[0], self.zoom),
            ),
            index_on(r0[1] + self.roots[1], self.corner[1], self.zoom),
            index_on(r0[2] + self.roots[2], self.corner[2], self.zoom),
        ]
    }
}

/// The origin `o` on the finest grid within its root: the Q3 itself
/// modulo the root's side, 2^32.
fn grid_of(o: [i32; 3]) -> [u32; 3] {
    // [naga] each axis by itself, not a loop (caves.rs `node_seed`).
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
        let d =
            libm::floorf(crate::axis(ray.direction, a as u32) * t * (1 << FRAC_W) as f32) as i32;
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
    // [naga] each axis by itself, not a loop (caves.rs `node_seed`).
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
fn index_on(root: i32, c: u32, z: Zoom) -> i32 {
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
        planes(
            ray.direction[0],
            ray.inverse[0],
            lo[0],
            hi[0],
            (t, ray.reach),
        ),
        planes(
            ray.direction[1],
            ray.inverse[1],
            lo[1],
            hi[1],
            (t, ray.reach),
        ),
        planes(
            ray.direction[2],
            ray.inverse[2],
            lo[2],
            hi[2],
            (t, ray.reach),
        ),
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
    let e = crossing(ray.direction, near, true);
    (greater(axis(near, e), 0.0), e)
}

/// A block's planes as it is drawn, its node's planes `(lo, hi)`
/// (`bounds`) inset about their center on every axis alike by the share of
/// its cube its word `block` keeps (`answer::kept`, `answer::side`): a box
/// that share's volume, its node itself where it keeps it whole.
pub fn kept_bounds((lo, hi): ([f32; 3], [f32; 3]), block: u32) -> ([f32; 3], [f32; 3]) {
    let k = kept(block);
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

/// The top of the block a stop met, as it is drawn (`kept_bounds`): the
/// floor of its node `n` on its grid from the origin `o`, and the share of
/// the node's side its top lies over that floor — its cube's top where its
/// word `block` keeps it whole, else its kept box's. What a walker and decor
/// stand on (pxl-kernel domain/query/ray.rs `hit`, `scatter::footing`).
pub fn top(o: [i32; 3], n: &Node, block: u32) -> (i32, f32) {
    (n.index(o)[2], kept_bounds(([0.0; 3], [1.0; 3]), block).1[2])
}

/// Where `ray` from the origin `o` enters the block `block` (a block's word,
/// `answer`) at the node `n`, and whether it does within its reach: its span
/// in the block as drawn from the origin (`kept_bounds`, `spans`), and
/// where that span begins (`entered`). Only touched, at a face or an edge,
/// is not entered, as the walk has it.
pub fn entry(o: [i32; 3], ray: &Ray, n: &Node, block: u32) -> (f32, u32, bool) {
    let (x, y, z) = spans(ray, kept_bounds(bounds(n, grid_of(o)), block), 0.0);
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
    // [naga] each axis by itself, not a loop (caves.rs `node_seed`).
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
/// ground's windows, the caves, the ways' runs and what is placed; only the
/// CPU's answer strikes decor.
pub trait Query {
    type At: Copy;
    /// A root of zoom 0, `node` the world's.
    fn root(&mut self, node: [i32; 3]) -> Self::At;
    /// The child `node` at `zoom` of the node `at` was handed for, with what
    /// that one found handed down.
    fn child(&mut self, at: &Self::At, zoom: Zoom, node: [i32; 3]) -> Self::At;
    /// What is in the node `node` at `zoom`, `at` what its ancestors handed
    /// it, asked from `far` to `near`, the ray in it from `t0`: the world's
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
    pub block: [u32; 2],
}

/// How a walk ended: where it stopped, gone on to its reach, or at a node
/// not held, past which it tells nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Ends {
    Stop,
    On,
    Lacks,
}

/// What the walk met: how it ended, where it stopped when it did, and how
/// many nodes it asked the world about (`Query::node`), the one count of a
/// ray's steps the CPU and a GPU both report.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Met<A> {
    pub ends: Ends,
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
/// and stops on the same block (doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md,
/// "more cautiously").
fn asked(detail: Detail, [far, near]: [f32; 2]) -> [Zoom; 2] {
    [detail.at(far), detail.at(near)]
}

/// A vector's length squared.
fn squared(v: [f32; 3]) -> f32 {
    v[0] * v[0] + v[1] * v[1] + v[2] * v[2]
}

/// The first block `ray` meets within its reach that `until` stops at, at
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
    let at = q.root(Node::ROOT.index(o));
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

/// The first block `ray` meets within its reach that `until` stops at, at
/// the zoom `detail` asks where it meets it, its line left where it ended
/// to `keep`: down the pyramid from `from`, a node holding where the ray
/// is at its `t`, what was handed it, and that `t` — the world's root holding
/// the origin at 0 (`first`), or a node of the line a beam ray handed on
/// (doc/minutes/20261022-a-cell-casts-a-beam.md) — what was handed each node
/// over it read off `over` where the walk climbs past it, and through the
/// roots the ray crosses. What the ray starts inside is not met when it
/// stops where a block fills, so an eye under the ground is not stopped
/// where it stands.
///
/// The walk holds its node, each axis an integer in its root (`Node`), and
/// where the ray is, `t`; the planes, the span and the axis entered across
/// are worked out from those where asked. A node is entered once: asked
/// whether anything under it fills at the zooms every ray from the origin
/// asks within it (`asked`); a block — the block at that zoom, or air where
/// nothing under it fills — the walk stops at (for an eye, where it fills,
/// but not in the block it starts in, and where the ray enters the box a
/// block keeping less than its cube is, `kept_bounds`; for what is climbed
/// out of, where it does not) or goes on past; else its child the ray is in (PUSH, Laine &
/// Karras 2010 §3). Past a node the corner steps on the axis the ray leaves
/// by (Amanatides & Woo 1987) and the walk is at the child of the nearest
/// ancestor holding the next node, by the highest bit the step changed
/// (POP), or past a root, in the next root, from its top. One loop.
#[allow(clippy::too_many_arguments)]
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
    let (og, r0) = (grid_of(o), roots_of(o));
    // What the walk carries is what nothing it holds gives again: the node
    // and what was handed it, where the ray is, what it keeps of every node
    // over it (`stack`, its own from `low` down, `over`'s above), and its
    // visits. The node's planes and the world's node it is follow from its
    // corner, and are worked out where they are read.
    let (mut n, mut at, mut t) = from;
    let mut stack = [at; DEPTH];
    let (mut low, mut visits) = (n.zoom, 0);
    // The cell's angle a beam ray's narrow splits are measured by.
    let s = crate::beam::spacing(detail.tau);
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
            let i = q.node(&mut at, n.zoom, n.index(o), asked(detail, reach), t0);
            visits += 1;
            let inside = lo[0] <= 0.0
                && 0.0 < hi[0]
                && lo[1] <= 0.0
                && 0.0 < hi[1]
                && lo[2] <= 0.0
                && 0.0 < hi[2];
            let side = hi[0] - lo[0];
            // The planes the ray enters where it stops: the node's, or the
            // box a block keeping less than its cube is.
            let mut near = [x.0, y.0, z.0];
            let ends = match i.kind {
                Kind::Lacks => Ends::Lacks,
                // A block keeping less than its cube is the box its share
                // fills (`kept_bounds`): an eye's ray stops where it enters
                // the box and passes it as air where it misses; a beam ray
                // stops at its node, as the samples it hands on may meet it.
                Kind::Block
                    if until == Until::Fills
                        && fills(i.block[0])
                        && !inside
                        && kept(i.block[0]) < EIGHTHS =>
                {
                    let (bx, by, bz) = spans(ray, kept_bounds((lo, hi), i.block[0]), t);
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
                Kind::Block
                    if match until {
                        Until::Fills => fills(i.block[0]) && !inside,
                        Until::Air => !fills(i.block[0]),
                        Until::Beam => fills(i.block[0]),
                    } =>
                {
                    Ends::Stop
                }
                Kind::Block => Ends::On,
                // A beam ray stops at a split it may not share with the
                // grid's rays about it, and enters every other whatever its
                // slab, as Laine and Karras's coarse pass turns contours off.
                Kind::Split if until == Until::Beam => {
                    let narrow = crate::beam::narrow(side * side, reach[0], s);
                    into = !narrow;
                    if narrow {
                        Ends::Stop
                    } else {
                        Ends::On
                    }
                }
                // ESVO's contour (Laine & Karras 2010): the ray's span in the
                // node clipped to the slab along z its content lies in, its
                // planes a face's own (`level`); a span wholly out of it, or
                // touching it alone, meets nothing that fills, and a walk to
                // what fills passes the node whole. A walk to air goes in: its
                // stop is where that span begins.
                Kind::Split => {
                    let [lo, hi] = slab_of(i.block[0]);
                    let level = |v: i32| {
                        let root = v >> UNIT_ZOOM;
                        units(
                            ((v - (root << UNIT_ZOOM)) as u32) << FRAC_W,
                            og[2],
                            root - r0[2],
                        )
                    };
                    let (bottom, top) = planes(
                        ray.direction[2],
                        ray.inverse[2],
                        level(lo),
                        level(hi),
                        (t0, t1),
                    );
                    into = until == Until::Air || greater(t0, bottom) < lesser(t1, top);
                    Ends::On
                }
            };
            if ends != Ends::On {
                // Where the walk ends, its stop, made there and nowhere else:
                // where the ray enters the node, or its block's box
                // (`entered`).
                let (t, axis) = entered(ray, near);
                let stop = Stop {
                    t,
                    axis,
                    node: n,
                    at,
                    block: i.block,
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
                past(ray.direction[0], ray.inverse[0], mid[0], t0),
                past(ray.direction[1], ray.inverse[1], mid[1], t0),
                past(ray.direction[2], ray.inverse[2], mid[2], t0),
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
                    block: AIR,
                };
                keep.keep(&n, at, &stack);
                return Met {
                    ends: Ends::On,
                    stop,
                    visits,
                };
            }
            // ADVANCE: the corner stepped on the axis the ray leaves by, the
            // first far plane it reaches.
            let e = crossing(ray.direction, [x.1, y.1, z.1], false);
            let up = axis(ray.direction, e) > 0.0;
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
        let node = n.index(o);
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
    use crate::answer::{kept_word, slab_word, ANY, EIGHTHS};
    use crate::cube::ground::stone;
    use crate::overwrite::{AIR, FULL};

    /// One block of stone at `TARGET`, a unit cell, keeping `kept` eighths of
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
                    block: [slab_word(ANY), 0],
                },
                (false, true) => In {
                    kind: Kind::Block,
                    block: [kept_word(stone as u32, self.kept), 0],
                },
                _ => In {
                    kind: Kind::Block,
                    block: [AIR, 0],
                },
            }
        }
    }

    /// A ray along +x from five units, at `y` and `z` units, against a block
    /// keeping `kept` eighths.
    fn along(kept: u32, y: f32, z: f32) -> Met<()> {
        let steps = |u: f32| (u * (1 << FRAC_W) as f32) as i32;
        let o = [steps(5.0), steps(y), steps(z)];
        let ray = Ray::new([1.0, 0.0, 0.0], 20.0);
        let detail = Detail {
            tau: 1.0,
            range: [UNIT_ZOOM, UNIT_ZOOM],
            eye: [0.0; 3],
        };
        first(&mut One { kept }, o, &ray, detail, Until::Fills)
    }

    #[test]
    fn a_share_s_box_has_the_share_s_volume() {
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
        assert_eq!(crate::answer::eighths(FULL), EIGHTHS);
        assert_eq!(crate::answer::eighths(FULL / 8 - 1), 0);
        assert_eq!(crate::answer::eighths(3 * FULL / 8), 3);
    }

    #[test]
    fn a_ray_stops_where_it_enters_the_box() {
        // An eighth kept is a box half the side, a quarter in from each face.
        let met = along(1, 0.5, 0.5);
        assert_eq!(met.ends, Ends::Stop);
        assert!((met.stop.t - 5.25).abs() < 1e-5, "{}", met.stop.t);
        assert_eq!(met.stop.axis, 0);
        // Whole, the cube's face.
        let met = along(EIGHTHS, 0.5, 0.5);
        assert!((met.stop.t - 5.0).abs() < 1e-5, "{}", met.stop.t);
    }

    #[test]
    fn a_ray_through_the_cell_beside_the_box_passes() {
        let met = along(1, 0.1, 0.5);
        assert_eq!(met.ends, Ends::On);
        let met = along(4, 0.5, 0.05);
        assert_eq!(met.ends, Ends::On);
        // The whole cube stops it there.
        assert_eq!(along(EIGHTHS, 0.1, 0.5).ends, Ends::Stop);
    }
}
