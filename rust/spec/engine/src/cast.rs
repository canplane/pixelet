// pxl-engine/cast.rs

//! gpu-portable: the first cell a ray meets, at the zoom its distance asks: one descent for
//! the CPU and the GPU, over a query each answers in its own way (`Query`). A node coarser
//! than the zoom its distance asks is asked whether anything under it is not transparent. If
//! nothing is, the ray goes on past it whole; otherwise the ray descends into the children it
//! meets, nearest first. A node at that zoom is asked its asset, and the ray stops there or
//! goes on.
//!
//! Positions are integers: the ray's origin is a Q3, and a node's corner is counted in Q3
//! steps on its root's finest grid (`Node`), the origin's own grid. Only a plane's offset from
//! the origin, summed as integers first, becomes a float (`units`). So the nodes near the
//! origin are as exact as f32 is near zero wherever the origin stands, and X wraps around the
//! world as the integers do. WGSL's f32 is not strictly IEEE (a division's last place, or a
//! contraction into a fused multiply-add), so where a ray passes within the last bit of a
//! face, the CPU and a GPU may stop on either side of it.
//!
//! The descent holds one frame a zoom, so its stack is as deep as the zooms a node can have.
//! From a child it steps to the next child across the face it leaves by, so no frame holds
//! its siblings.

use crate::answer::{
    box_of, fill_eighths, side, transparent, In, Kind, AIR, BOX_BITS, BOX_STEPS, EIGHTHS,
};
use crate::grid::cell_zoom_at;
use crate::object::{y_start, z_start};
use crate::{axis, wrap_idx, Zoom, FINEST_ZOOM, FRAC_W, HEIGHT_BITS, UNIT_ZOOM};

/// The direction at the tangents `tx` across and `ty` up from the optical axis, as a
/// projection puts a drawn pixel's middle: its tangents are those of the screen's half-angles
/// (pxl-core domain/view/lens.rs `Lens`), scaled by where the pixel lies on the screen.
pub fn through(forward: [f32; 3], right: [f32; 3], up: [f32; 3], tx: f32, ty: f32) -> [f32; 3] {
    let d = [
        forward[0] + tx * right[0] + ty * up[0],
        forward[1] + tx * right[1] + ty * up[1],
        forward[2] + tx * right[2] + ty * up[2],
    ];
    let l = libm::sqrtf(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
    [d[0] / l, d[1] / l, d[2] / l]
}

/// How far apart a beam's rays lie: `across + angle · d` units at `d` from the walk's origin.
/// The eye's spacing is an angle, its camera grid's lattice (`beam::Grid::s`); the light's,
/// whose rays are parallel, is a width, `light::BEAM` of its map's texels. Only a beam reads
/// it, for its widening and for `narrow` (`Until::Beam`); an asker that casts no beam, such as
/// control or physics, gives none (`NONE`).
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

    /// Its width at `d` from the walk's origin.
    pub fn width(self, d: f32) -> f32 {
        self.across + self.angle * d
    }
}

/// The zoom asked at each distance from the walk's origin: the camera grid's cells there
/// (`cell_zoom_at`), held to `range`, asked of a squared distance (`at`). Asking one zoom's
/// cells wherever they are is a range of that zoom alone, as the light's walks ask their band
/// (pxl-engine `light::Light::band`). `spacing` is how far a beam ray lies from the next of
/// its grid (`Spacing`, `Until::Beam`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detail {
    pub range: [Zoom; 2],
    pub spacing: Spacing,
}

impl Detail {
    /// The zoom at `d2`, a squared distance. It is held by `max` and then `min`, not by `clamp`,
    /// which aborts where `range` is inverted (lib.rs: no check here fails); there it gives the
    /// finest.
    pub fn at(self, d2: f32) -> Zoom {
        self.hold(cell_zoom_at(d2))
    }

    /// Zoom `z` held to `range`, as `at` holds it.
    pub fn hold(self, z: Zoom) -> Zoom {
        z.max(self.range[0]).min(self.range[1])
    }

    /// Whether a walk asking this detail may take what a walk asking `other` met: the cell it
    /// stopped at, the sky past it, the line it left. It may only where both asked the same zooms
    /// at the same spacing.
    pub fn alike(self, other: Detail) -> bool {
        self.range[0] == other.range[0]
            && self.range[1] == other.range[1]
            && self.spacing.angle == other.spacing.angle
            && self.spacing.across == other.spacing.across
    }
}

/// Whether a node whose squared side is `side2` is too narrow for a beam ray to share, where
/// the grid's rays lie `width2` apart, squared: it is narrower than twice the width that
/// holds one of them for certain, `NARROW · width`. For the eye, the width is `s · d` at the
/// node's farthest point.
pub fn narrow(side2: f32, width2: f32) -> bool {
    side2 < NARROW_SQUARED * width2
}

/// The spacings under which a node is narrow (`narrow`): twice the width that holds one ray
/// for certain, `2√2`, and its square, exact (`NARROW_SQUARED`).
pub const NARROW: f32 = 2.0 * core::f32::consts::SQRT_2;
pub const NARROW_SQUARED: f32 = 8.0;
const _: () = assert!(
    NARROW * NARROW - NARROW_SQUARED < 1e-6 && NARROW_SQUARED - NARROW * NARROW < 1e-6,
    "`NARROW_SQUARED` is `NARROW`'s brick"
);

/// √6 as an f32 rounds it. A cube `NARROW` spacings wide spans `√3 · NARROW`, which is `2√6`
/// spacings, corner to corner.
pub const SQRT_6: f32 = 2.449_489_8;

/// Which cell a ray stops at: the first it cannot see through (`answer::transparent`), as an
/// eye sees (`Opaque`); the first it can see through, where what is climbed out of ends
/// (`Transparent`); or, for a beam ray of the camera grid, the first it cannot see through or
/// the first split too narrow to hold one of the grid's rays for certain (`narrow`), with the
/// box of every split it enters widened by `2√2` of that width (`Beam`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Until {
    Opaque,
    Transparent,
    Beam,
}

/// A ray from an origin, a Q3 position: its direction, how far it reaches in units, and its
/// direction's reciprocal on each axis, 0 where it does not move along one. With the
/// reciprocal, a plane's crossing is a product, not a division made again at every node
/// (Laine and Karras, _Efficient Sparse Voxel Octrees_, 2010, the per-ray coefficients).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub dir: [f32; 3],
    pub t_max: f32,
    inverse: [f32; 3],
}

impl Ray {
    /// A direction component under f32's least normal magnitude (`f32::MIN_POSITIVE`) is taken
    /// as 0: the ray does not move along that axis. So every reciprocal is finite (1 / 2^-126
    /// is), a plane's offset times it is a finite number or an infinity, never inf · 0, and no
    /// number the walk compares is NaN, which is what lets `greater` and `lesser` be one
    /// comparison each. The component and its reciprocal are zeroed together, because `planes`
    /// and `past` branch on the one and multiply by the other.
    pub fn new(dir: [f32; 3], t_max: f32) -> Ray {
        let moves = |d: f32| {
            if libm::fabsf(d) < f32::MIN_POSITIVE {
                0.0
            } else {
                d
            }
        };
        let dir = [moves(dir[0]), moves(dir[1]), moves(dir[2])];
        let inv = |d: f32| if d == 0.0 { 0.0 } else { 1.0 / d };
        Ray {
            dir,
            t_max,
            inverse: [inv(dir[0]), inv(dir[1]), inv(dir[2])],
        }
    }

    /// Its direction's reciprocal on each axis, 0 where it does not move along one.
    pub fn inverse(&self) -> [f32; 3] {
        self.inverse
    }
}

/// The greater and the lesser of two floats, one comparison each. IEEE's
/// `f32::max` and `f32::min` hand back the other operand where one is NaN,
/// and no number the walk compares is NaN (`Ray::new`), so the comparison is
/// the whole rule.
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

/// A node as the walk holds it, each axis an integer: its corner in its root on the finest
/// grid a position names (`FINEST_ZOOM`, 2^32 to a root, so a u32 holds it: the Q3 itself
/// modulo the root's side), its zoom, and which root it is in, counted from the origin's. Its
/// corner's bits under its own side are 0, so two corners part at the highest bit in which
/// they differ. It is the one form of a node seen from an origin, for the walk and for where
/// it stops alike.
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

    /// The deepest node above `self`, or `self` itself, that holds the point `p` on the finest
    /// grid in the root `roots` counted from the origin's, and whether there is one: there is
    /// none in another root.
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

    /// The node that a walk from the origin `from` holds as `self`, as a walk from `to` holds it.
    /// Its corner is the Q3 itself modulo a root's side, whatever the origin, and its roots move
    /// by the roots the two origins lie apart, the short way around as an i32 wraps on X, where
    /// the origins lie within a root of each other.
    pub fn moved(&self, from: [i32; 3], to: [i32; 3]) -> Node {
        let k = |a: usize| {
            let d = from[a].wrapping_sub(to[a]);
            let og = grid_of(from)[a];
            // `to`'s position on the finest grid is `from`'s less `d`: one root more where that borrows,
            // one root less where it carries.
            if d >= 0 {
                self.roots[a] + (og < d as u32) as i32
            } else {
                self.roots[a] - (og.wrapping_add(d.unsigned_abs()) < og) as i32
            }
        };
        // [naga] Each axis by itself, not a loop (seed.rs `node_seed` says why).
        Node {
            corner: self.corner,
            zoom: self.zoom,
            roots: [k(0), k(1), k(2)],
        }
    }

    /// The world's index of this node, seen from the origin `o`, with X as the world names it
    /// (pxl-spec space/coord/world.rs `wrap_idx`).
    pub fn idx(&self, o: [i32; 3]) -> [i32; 3] {
        let r0 = roots_of(o);
        [
            wrap_idx(
                self.zoom,
                idx_on(r0[0] + self.roots[0], self.corner[0], self.zoom),
            ),
            idx_on(r0[1] + self.roots[1], self.corner[1], self.zoom)
                .wrapping_sub(y_start(self.zoom)),
            idx_on(r0[2] + self.roots[2], self.corner[2], self.zoom)
                .wrapping_sub(z_start(self.zoom)),
        ]
    }

    /// The node the world indexes `idx` at `zoom`, as a walk from the origin `o` holds it (the
    /// inverse of `idx`). Its corner comes from the index's low bits. On each axis its root is the
    /// origin's or one on either side: the one whose copy of the node has its middle nearer the
    /// origin than half a root, which is the walk's root wherever the node lies within `REACH` of
    /// the origin at zoom 1 or finer (pxl-render record.wgsl `node_of_idx`).
    pub fn of_idx(o: [i32; 3], zoom: Zoom, idx: [i32; 3]) -> Node {
        let og = grid_of(o);
        let start = [0, y_start(zoom), z_start(zoom)];
        let low = if zoom >= FINEST_ZOOM {
            u32::MAX
        } else {
            (1u32 << zoom) - 1
        };
        let u = under(zoom);
        let axis = |a: usize| {
            let n = (idx[a] as u32).wrapping_add(start[a] as u32) & low;
            let corner = if zoom == 0 {
                0
            } else {
                n << (FINEST_ZOOM - zoom.min(FINEST_ZOOM))
            };
            // The node's middle on the finest grid of its root, and the origin's on the grid of its own
            // root.
            let mid = corner + (u >> 1);
            let root = if mid >= og[a] {
                -i32::from(mid - og[a] >= 1 << 31)
            } else {
                i32::from(og[a] - mid > 1 << 31)
            };
            (corner, root)
        };
        let (x, y, z) = (axis(0), axis(1), axis(2));
        Node {
            corner: [x.0, y.0, z.0],
            zoom,
            roots: [x.1, y.1, z.1],
        }
    }
}

/// The farthest the eye's rays reach from their origin, in units: a quarter of a root, within
/// which every draw distance stays (pxl-spec layer/lattice.rs). Within it, a node of zoom 1 or
/// finer lies nearer than half the world's X to its origin on one side only
/// (`Node::of_idx`), and every node coarser than `FINEST_ZOOM` is one the node cache keeps
/// (`node_cache::keeps`).
pub const REACH: u32 = 1 << (UNIT_ZOOM - 2);
const _: () = assert!(
    2 * REACH <= 1 << (UNIT_ZOOM - 1),
    "a node of zoom 1 within reach has one copy within half the world's X of its origin"
);

/// Y's Q3 steps from the world's south edge to 0, and Z's from its floor.
pub const Y_START: u32 = 1 << 31;
pub const Z_START: u32 = 1 << (HEIGHT_BITS - 1 + FRAC_W);

/// The origin `o` on the finest grid within its root, modulo the root's side of 2^32: X is the
/// Q3 itself, and Y and Z are counted from the world's edge (`object::path`), so the world is
/// one root and a shard is one cube.
pub fn grid_of(o: [i32; 3]) -> [u32; 3] {
    // [naga] Each axis by itself, not a loop (seed.rs `node_seed` says why).
    [
        o[0] as u32,
        (o[1] as u32).wrapping_add(Y_START),
        (o[2] as u32).wrapping_add(Z_START),
    ]
}

/// The root holding the origin `o` on each axis: its place on the grid divided by the root's
/// side, 2^32, floored, so 0 or −1: X on either side of the seam, Y always 0, and Z 0 from the
/// world's floor up.
pub fn roots_of(o: [i32; 3]) -> [i32; 3] {
    [o[0] >> 31, 0, o[2].wrapping_add(Z_START as i32) >> 31]
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
    // [naga] Each axis by itself, not a loop (seed.rs `node_seed` says why).
    let (x, y, z) = (on(0), on(1), on(2));
    ([x.0, y.0, z.0], [x.1, y.1, z.1])
}

/// Units from the origin, at `og` in its root, to `p` in the root `roots` counted from it.
/// The whole units are summed as integers before they become a float, so a plane is as exact
/// far from the origin's root as in it.
fn units(p: u32, og: u32, roots: i32) -> f32 {
    ((p >> FRAC_W) as i32 - (og >> FRAC_W) as i32 + (roots << UNIT_ZOOM)) as f32
        + ((p & 0xff) as i32 - (og & 0xff) as i32) as f32 / (1 << FRAC_W) as f32
}

/// A node's side at zoom `z`, less one, on the finest grid: the bits under its corner's,
/// 2^32 − 1 at a root, whose side no u32 holds.
fn under(z: Zoom) -> u32 {
    if z >= FINEST_ZOOM {
        0
    } else {
        u32::MAX >> z
    }
}

/// The world's index on axis `a` of the node at zoom `z` whose corner is `c` in the root
/// `root`: `root · 2^z` and the corner's high bits, with no shift reaching the word's width.
fn idx_on(root: i32, c: u32, z: Zoom) -> i32 {
    let high = if z >= FINEST_ZOOM {
        0
    } else {
        (root << z) as u32
    };
    let low = if z == 0 { 0 } else { c >> (FINEST_ZOOM - z) };
    high.wrapping_add(low) as i32
}

/// The ray's t at the near and far planes on one axis, `lo` and `hi` units from the origin
/// (Revelles, Ureña & Lastra 2000). On an axis the ray runs beside, it is the walk's whole
/// span inside the slab (from `t` to `t_max`), and none outside it, so no extreme value is
/// needed that a float literal could name past a shader's range.
fn planes(d: f32, inv: f32, lo: f32, hi: f32, (t, t_max): (f32, f32)) -> (f32, f32) {
    if d > 0.0 {
        (lo * inv, hi * inv)
    } else if d < 0.0 {
        (hi * inv, lo * inv)
    } else if lo <= 0.0 && 0.0 < hi {
        (t, t_max)
    } else {
        (t_max, t)
    }
}

/// A node's planes, in units from the origin at `og` on the finest grid, each a corner's
/// (`units`). The far plane is the next node's corner, in the next root where the corner goes
/// past its own.
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
        planes(ray.dir[0], ray.inverse[0], lo[0], hi[0], (t, ray.t_max)),
        planes(ray.dir[1], ray.inverse[1], lo[1], hi[1], (t, ray.t_max)),
        planes(ray.dir[2], ray.inverse[2], lo[2], hi[2], (t, ray.t_max)),
    )
}

/// The node above `n`, or `n` itself, that holds where `ray` from `o` is at `t`, as the walk's
/// own planes say (`bounds`, `spans`): the node a walk resumed at `t` starts in. `n` was found
/// from a point the float put on the ray, which may lie a step outside the node the ray is in.
/// It is the root where no node above `n` holds it.
pub fn holding(o: [i32; 3], ray: &Ray, mut n: Node, t: f32) -> Node {
    let og = grid_of(o);
    loop {
        let (x, y, z) = spans(ray, bounds(&n, og), t);
        let (t0, t1) = (
            greater(greater(greater(t, x.0), y.0), z.0),
            lesser(lesser(lesser(ray.t_max, x.1), y.1), z.1),
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

/// Where a ray enters a node whose near planes it reaches at `near`: the axis entered across,
/// which is the last of those planes the ray reaches (`crossing`), and that plane's t, no
/// nearer than the origin. It depends on the node and the ray alone, whatever walk came to it.
fn entry_plane(ray: &Ray, near: [f32; 3]) -> (f32, u32) {
    let e = crossing(ray.dir, near, true);
    (greater(axis(near, e), 0.0), e)
}

/// A cell's planes as it is drawn: its node's planes `(lo, hi)` (`bounds`) inset about their
/// center alike on every axis by the share of its cube that its word `asset` keeps
/// (`answer::fill_eighths`, `answer::side`). It is a box of that share's volume, the node
/// itself where it keeps it whole.
pub fn fill_bounds((lo, hi): ([f32; 3], [f32; 3]), asset: u32) -> ([f32; 3], [f32; 3]) {
    let k = fill_eighths(asset);
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

/// The top of the cell a hit met, as it is drawn (`fill_bounds`): the floor of its node `n`
/// on its grid from the origin `o`, and the share of the node's side by which its top lies
/// over that floor. The top is its cube's where its word `asset` keeps the cube whole, and its
/// fill box's otherwise. A walker and decor stand on it (pxl-core domain/query/ray.rs `hit`,
/// `support::column_surface`).
pub fn top(o: [i32; 3], n: &Node, asset: u32) -> (i32, f32) {
    (n.idx(o)[2], fill_bounds(([0.0; 3], [1.0; 3]), asset).1[2])
}

/// The asset `asset` at the node `n` as drawn from the origin `o`, in units from it
/// (`bounds`, `fill_bounds`): the box that `entry` asks a ray into, as the samples' boxes are
/// drawn (`raster`).
pub fn asset_box(o: [i32; 3], n: &Node, asset: u32) -> ([f32; 3], [f32; 3]) {
    fill_bounds(bounds(n, grid_of(o)), asset)
}

/// Where `ray` from the origin `o` enters the cell of `asset` (an asset's word, `answer`) at
/// the node `n`, and whether it enters within its interval: its span in the cell as drawn from
/// the origin (`asset_box`, `spans`), and where that span begins (`entry_plane`). A cell the
/// ray only touches, at a face or an edge, is not entered, as for the walk.
pub fn entry(o: [i32; 3], ray: &Ray, n: &Node, asset: u32) -> (f32, u32, bool) {
    let e = entry_with(o, ray, ray.inverse, n, asset);
    (e.t, e.axis, e.enters)
}

/// Where a ray enters a cell, as `entry` takes it, and the planes that decided it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    /// The t of the plane it enters across, no nearer than the origin.
    pub t: f32,
    pub axis: u32,
    /// Whether it enters within its interval, `span[1] > span[0]`.
    pub enters: bool,
    /// The t at each axis's near plane and far plane (`planes`).
    pub near: [f32; 3],
    pub far: [f32; 3],
    /// Where its interval in the cell begins and ends.
    pub span: [f32; 2],
}

/// `entry` along `ray`, with `inverse` as its direction's reciprocal. A device whose division
/// may round as it will hands in its own reciprocal this way.
pub fn entry_with(o: [i32; 3], ray: &Ray, inverse: [f32; 3], n: &Node, asset: u32) -> Entry {
    let ray = Ray { inverse, ..*ray };
    let (x, y, z) = spans(&ray, asset_box(o, n, asset), 0.0);
    let (t0, t1) = (
        greater(greater(greater(0.0, x.0), y.0), z.0),
        lesser(lesser(lesser(ray.t_max, x.1), y.1), z.1),
    );
    let (t, e) = entry_plane(&ray, [x.0, y.0, z.0]);
    Entry {
        t,
        axis: e,
        enters: t1 > t0,
        near: [x.0, y.0, z.0],
        far: [x.1, y.1, z.1],
        span: [t0, t1],
    }
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
    // [naga] Each axis by itself, not a loop (seed.rs `node_seed` says why).
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
/// ground's clip levels, the caves, the ways' spans and what is placed; only the
/// CPU's answer strikes decor.
pub trait Query {
    type At: Copy;
    /// A root of zoom 0, where `node` is the world's index of it.
    fn root(&mut self, node: [i32; 3]) -> Self::At;
    /// The child `node` at `zoom` of the node `at` was handed for, with what
    /// that one found handed down.
    fn child(&mut self, at: &Self::At, zoom: Zoom, node: [i32; 3]) -> Self::At;
    /// What is in the node `node` at `zoom`, given what its ancestors handed it (`at`), asked
    /// from `far` to `near`, with the ray in it from `t0`: the world's answer (pxl-engine
    /// `answer`), whichever side reads it.
    fn node(
        &mut self,
        at: &mut Self::At,
        zoom: Zoom,
        node: [i32; 3],
        zooms: [Zoom; 2],
        t0: f32,
    ) -> In<[u32; 2]>;
}

/// Where a ray stopped: how far, the axis it entered across, the node
/// (`Node`, from the ray's origin) and what it is there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit<A> {
    pub t: f32,
    pub axis: u32,
    pub node: Node,
    pub at: A,
    /// What it stopped at, as the query answered it: an asset's words, or,
    /// where a beam ray stopped at a narrow split, that split's box
    /// (`answer::box_word`), which no reader takes for an asset's.
    pub asset: [u32; 2],
}

/// How a walk ended: where it stopped, gone on to its interval's end, or at a node not held,
/// past which it can tell nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum End {
    Hit,
    Miss,
    Lacks,
}

/// What the walk met: how it ended, and where it stopped when it did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trace<A> {
    pub end: End,
    pub hit: Hit<A>,
}

/// The most steps a walk takes, one node looked at each step. The engine's walk (`walk`) and
/// the one written by hand (pxl-render walk.wgsl), over either node module, end there as if
/// gone to their reach. A float gone wrong then still ends, which matters on a phone, where a
/// walk that does not end loses the device. It is far more than a ray takes.
pub const STEPS_CAP: u32 = 4096;

/// The descent's stack, one entry for every zoom: what the query handed the node there
/// (`Query::At`), the one thing the node's parent cannot work out again without asking the
/// world from the top. Where the ray is in the node, which child it is in and where it leaves
/// that child follow from the node's place by the slab test, so no entry holds them.
pub const DEPTH: usize = FINEST_ZOOM as usize + 1;

/// A node's farthest and nearest squared distances from the walk's origin, given its planes
/// `lo` and `hi` units from the origin on each axis: from its corner farthest from the origin
/// and from its point nearest it.
fn node_dist_sq(lo: [f32; 3], hi: [f32; 3]) -> [f32; 2] {
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

/// The zooms at which every ray from the origin asks a node, from its farthest corner to its
/// nearest point (`node_dist_sq`, `Detail::at`). They depend on the node and the origin
/// alone, so a node has one answer for every ray through it. A ray asks no coarser within the
/// node than at its farthest corner, so this only adds zooms coarser than the ray's own, which
/// splits more and stops on the same cell.
fn ask_zooms(detail: Detail, [far, near]: [f32; 2]) -> [Zoom; 2] {
    [detail.at(far), detail.at(near)]
}

/// The zooms every ray from the origin `o` asks the node `n` at (`ask_zooms`), as the walk
/// asks them: far, then near.
pub fn node_zooms(o: [i32; 3], n: &Node, detail: Detail) -> [Zoom; 2] {
    let (lo, hi) = bounds(n, grid_of(o));
    ask_zooms(detail, node_dist_sq(lo, hi))
}

fn squared(v: [f32; 3]) -> f32 {
    v[0] * v[0] + v[1] * v[1] + v[2] * v[2]
}

/// The first cell `ray` meets within its interval that `until` stops at, at
/// the zoom `detail` asks where it meets it, from the world's root holding
/// the origin (`walk`).
pub fn first<Q: Query>(
    q: &mut Q,
    o: [i32; 3],
    ray: &Ray,
    detail: Detail,
    until: Until,
) -> Trace<Q::At> {
    walk_from_root(q, o, ray, detail, until, &mut ())
}

/// `walk` from the world's root holding the origin, at t = 0, leaving its line to `keep`.
pub fn walk_from_root<Q: Query>(
    q: &mut Q,
    o: [i32; 3],
    ray: &Ray,
    detail: Detail,
    until: Until,
    keep: &mut impl Keep<Q::At>,
) -> Trace<Q::At> {
    let at = q.root(Node::ROOT.idx(o));
    walk(
        q,
        o,
        ray,
        detail,
        until,
        (Node::ROOT, at, 0.0),
        &Root(at),
        keep,
    )
}

/// What was handed each node above where a walk starts, by zoom: the line the walk resumes
/// from, as a beam ray left it (`Keep`). It is read where the walk climbs above its start, and
/// where the walk ends, for whatever keeps its line.
pub trait Over<A: Copy> {
    fn over(&self, zoom: Zoom) -> A;
}

/// What lies above a walk from the roots: nothing. No node is above a root, so it is never
/// read.
#[derive(Clone, Copy)]
pub struct Root<A>(pub A);

impl<A: Copy> Over<A> for Root<A> {
    fn over(&self, _: Zoom) -> A {
        self.0
    }
}

/// What a walk leaves where it ends (`walk`), for whatever reads on from it: what it met,
/// whose stop is the node it ended at and what was handed that node (`Hit::node`, `Hit::at`),
/// and what was handed every node above that, its stack by zoom, taken from `over` above where
/// the walk started (`LINE`). A walk that nothing reads on from leaves nothing (`()`), and
/// reads nothing of `over` for it.
pub trait Keep<A: Copy> {
    /// Whether it reads the line, so the walk's stack is whole where it
    /// ends.
    const LINE: bool = true;

    fn keep(&mut self, trace: &Trace<A>, stack: &[A; DEPTH]);
}

impl<A: Copy> Keep<A> for () {
    const LINE: bool = false;

    fn keep(&mut self, _: &Trace<A>, _: &[A; DEPTH]) {}
}

/// The first cell `ray` meets within its interval that `until` stops at, at the zoom `detail`
/// asks where it meets it, with its line left to `keep` where it ends. The walk goes down the
/// pyramid from `from`: a node holding where the ray is at its `t`, what was handed it, and
/// that `t`. That is the world's root holding the origin, at 0 (`first`), or a node of the
/// line a beam ray handed on. What was handed each node above it is read from `over` where the
/// walk climbs past it, and the walk goes on through the roots the ray crosses. What the ray
/// starts inside is not met when it stops at what it cannot see through, so an eye under the
/// ground is not stopped where it stands.
///
/// The walk holds its node, each axis an integer in its root (`Node`), and where the ray is,
/// `t`; the planes, the span and the axis entered across are worked out from those where
/// asked. A node is entered once and asked whether anything under it is not transparent at
/// the zooms every ray from the origin asks within it (`zooms`). A cell, the asset at that
/// zoom or air where everything under it is air, is where the walk stops or goes on past. For
/// an eye it stops where the cell is not transparent, except in the cell it starts in, and
/// where the ray enters the box of a cell keeping less than its cube (`fill_bounds`); for what
/// is climbed out of, it stops where the cell is transparent. Otherwise the walk enters the
/// child the ray is in (PUSH, Laine & Karras 2010 §3). Past a node, the corner steps on the
/// axis the ray leaves by (Amanatides & Woo 1987), and the walk moves to the child, holding
/// the next node, of the nearest ancestor that holds it, found by the highest bit the step
/// changed (POP); past a root, it moves into the next root, from its top. It is one loop of
/// at most `STEPS_CAP` steps (`walk_at_most`).
#[expect(
    clippy::too_many_arguments,
    reason = "the walk's query, origin, ray, detail, stop and start, the line over its start and what keeps its own, each its own (Laine & Karras 2010)"
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
) -> Trace<Q::At> {
    walk_at_most(q, o, ray, detail, until, from, over, keep, STEPS_CAP)
}

/// `walk` of at most `step_cap` steps, one node looked at each step, as the walks written by
/// hand count theirs. A walk that runs out of steps counts as gone to its reach, `ray.t_max`,
/// and leaves its line to `keep` where it is, at the node it would look at next.
#[expect(clippy::too_many_arguments, reason = "`walk`'s, and its bound")]
pub fn walk_at_most<Q: Query>(
    q: &mut Q,
    o: [i32; 3],
    ray: &Ray,
    detail: Detail,
    until: Until,
    from: (Node, Q::At, f32),
    over: &impl Over<Q::At>,
    keep: &mut impl Keep<Q::At>,
    step_cap: u32,
) -> Trace<Q::At> {
    let og = grid_of(o);
    // The walk carries only what nothing else it holds gives again: the node and what was handed
    // it, where the ray is, and what it keeps of every node above it (`stack`, its own from `low`
    // down, and `over`'s above).
    let (mut n, mut at, mut t) = from;
    let mut stack = [at; DEPTH];
    let mut low = n.zoom;
    let mut step = 0;
    while step < step_cap {
        step += 1;
        let u = under(n.zoom);
        let (lo, hi) = bounds(&n, og);
        // The ray's span in it.
        let (x, y, z) = spans(ray, (lo, hi), t);
        let (t0, t1) = (
            greater(greater(greater(t, x.0), y.0), z.0),
            lesser(lesser(lesser(ray.t_max, x.1), y.1), z.1),
        );
        // Only touched, at a face or an edge: not entered, so nothing in it is met.
        let mut into = false;
        if t1 > t0 {
            let dist_sq = node_dist_sq(lo, hi);
            let i = q.node(&mut at, n.zoom, n.idx(o), ask_zooms(detail, dist_sq), t0);
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
            // ESVO's contour (Laine & Karras 2010), alike on every axis: whether the ray's span in a
            // split node meets the box, in the node's steps, in which its content lies
            // (`answer::box_of`), grown by `w` on every side. Nothing a ray meets is passed over for
            // being thin.
            let meets_box = |w: f32| {
                let (first, past) = box_of(i.asset);
                // Step `e` of the box on axis `a`, on the grid's own planes (`units`), as a node's own
                // planes are; the far one is the node's.
                let step = |a: usize, e: u32, out: u32| {
                    if e >= BOX_STEPS {
                        return hi[a];
                    }
                    // A step's width is a whole count of the finest grid's cells over a node `BOX_STEPS` or
                    // more of them wide. On a finer node it is rounded out (`out` is the step less one, to round
                    // up), since `e · side` fits a u32 there.
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
            let end = match i.kind {
                Kind::Lacks => End::Lacks,
                // A cell keeping less than its cube is the box its share fills (`fill_bounds`): an eye's ray
                // stops where it enters the box and passes it as air where it misses; a beam ray stops at
                // its node, since the samples it hands on may meet the box.
                Kind::Cell
                    if until == Until::Opaque
                        && !transparent(i.asset[0])
                        && !inside
                        && fill_eighths(i.asset[0]) < EIGHTHS =>
                {
                    let (bx, by, bz) = spans(ray, fill_bounds((lo, hi), i.asset[0]), t);
                    let (b0, b1) = (
                        greater(greater(greater(t, bx.0), by.0), bz.0),
                        lesser(lesser(lesser(ray.t_max, bx.1), by.1), bz.1),
                    );
                    near = [bx.0, by.0, bz.0];
                    if b1 > b0 {
                        End::Hit
                    } else {
                        End::Miss
                    }
                }
                Kind::Cell
                    if match until {
                        Until::Opaque => !transparent(i.asset[0]) && !inside,
                        Until::Transparent => transparent(i.asset[0]),
                        Until::Beam => !transparent(i.asset[0]),
                    } =>
                {
                    End::Hit
                }
                Kind::Cell => End::Miss,
                // A beam ray passes whole a split whose box it clears by `2√2` of the grid's width there,
                // since the grid's rays around it clear it too. Otherwise it stops at a split it may not
                // share with them, and enters every other. For any other walk, a span wholly outside the box,
                // or only touching it, meets nothing a ray cannot see through, so a walk that stops at that
                // passes the node whole, and a walk that stops at what is transparent goes in, its stop
                // being where that span begins. Either way the box is asked once.
                Kind::Split => {
                    let beam = until == Until::Beam;
                    // The grid's rays' spacing at the node's far corner: a split narrower than `2√2` of it may
                    // lie between them (`narrow`).
                    let width = if beam {
                        detail.spacing.width(libm::sqrtf(dist_sq[0]))
                    } else {
                        0.0
                    };
                    let narrow = beam && narrow(side * side, width * width);
                    let meets = until != Until::Transparent
                        && meets_box(if beam { NARROW * width } else { 0.0 });
                    into = if beam {
                        meets && !narrow
                    } else {
                        until == Until::Transparent || meets
                    };
                    if meets && narrow {
                        End::Hit
                    } else {
                        End::Miss
                    }
                }
            };
            if end != End::Miss {
                // Where the walk ends, its hit is where the ray enters the node or its asset's box
                // (`entry_plane`).
                let (t, axis) = entry_plane(ray, near);
                let hit = Hit {
                    t,
                    axis,
                    node: n,
                    at,
                    asset: i.asset,
                };
                let trace = Trace { end, hit };
                leave(keep, &trace, &mut stack, low, over);
                return trace;
            }
        }
        // Every way on makes the next node. What it is handed is decided here, its parent's `At`
        // or, at a root, its own (`fresh`), and the node is made once, afterwards (`q.child`,
        // `q.root`).
        let (parent, fresh) = if into {
            // PUSH: the child holding where the ray is at `t0`, past each mid plane it has crossed by
            // then.
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
            if t1 >= ray.t_max {
                // Gone on to its interval's end: no hit, and no asker reads one.
                let hit = Hit {
                    t: t1,
                    axis: 0,
                    node: n,
                    at,
                    asset: AIR,
                };
                let trace = Trace {
                    end: End::Miss,
                    hit,
                };
                leave(keep, &trace, &mut stack, low, over);
                return trace;
            }
            // ADVANCE: the corner steps on the axis the ray leaves by, at the first far plane it
            // reaches.
            let e = crossing(ray.dir, [x.1, y.1, z.1], false);
            let up = axis(ray.dir, e) > 0.0;
            let old = axis(n.corner, e);
            let new = if up {
                old.wrapping_add(u).wrapping_add(1)
            } else {
                old.wrapping_sub(u).wrapping_sub(1)
            };
            // Never step back: the next node holds where the ray is on the integers, but near an edge
            // another axis's plane, a float product, may fall a last bit before it.
            t = greater(t, t1);
            if (up && old.wrapping_add(u) == u32::MAX) || (!up && old == 0) {
                // Past a root: into the next root, from its top, above which nothing is asked or held.
                let by = if up { 1 } else { -1 };
                n.roots = [
                    n.roots[0] + (e == 0) as i32 * by,
                    n.roots[1] + (e == 1) as i32 * by,
                    n.roots[2] + (e == 2) as i32 * by,
                ];
                (n.corner, n.zoom, low) = ([0; 3], 0, 0);
                (at, true)
            } else {
                // POP: the child, at `l`, of the nearest ancestor holding the next node, found by the
                // highest bit the step changed; each axis's corner becomes that ancestor's child's.
                n.corner = [
                    if e == 0 { new } else { n.corner[0] },
                    if e == 1 { new } else { n.corner[1] },
                    if e == 2 { new } else { n.corner[2] },
                ];
                let h = 31 - (old ^ new).leading_zeros();
                let keep = !((1u32 << h) - 1);
                n.corner = [n.corner[0] & keep, n.corner[1] & keep, n.corner[2] & keep];
                let l = FINEST_ZOOM - h;
                // What was handed the parent: its stack entry, the walk's own from where it started down,
                // or `over`'s above that.
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
    // Out of steps: gone to its reach, with no hit, and no asker reads one.
    let hit = Hit {
        t: ray.t_max,
        axis: 0,
        node: n,
        at,
        asset: AIR,
    };
    let trace = Trace {
        end: End::Miss,
        hit,
    };
    leave(keep, &trace, &mut stack, low, over);
    trace
}

/// The walk's line, left to `keep` where it ends at `trace`: its stack, under `over`'s above
/// where it started at `low`, made whole first where `keep` reads it (`Keep::LINE`).
fn leave<A: Copy, K: Keep<A>>(
    keep: &mut K,
    trace: &Trace<A>,
    stack: &mut [A; DEPTH],
    low: Zoom,
    over: &impl Over<A>,
) {
    if K::LINE {
        let mut z = 0;
        while z < low {
            stack[z as usize] = over.over(z);
            z += 1;
        }
    }
    keep.keep(trace, stack);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use xpute_kit::math::rng::mix64;

    /// A point on the finest grid in its root, and that root (`point_at`).
    type Point = ([u32; 3], [i32; 3]);

    /// The `i`th of a spread of origins in the world, near X's seam and its edges, its floor and
    /// its top among them, with a point on a ray from it within `REACH`; none where the direction
    /// drawn is too short to give a ray.
    pub(crate) fn reached(i: u64) -> Option<([i32; 3], Point)> {
        // The world's height in Q3 steps, from its floor.
        let height = 1i64 << (HEIGHT_BITS + FRAC_W);
        let h = mix64(i);
        let x = [0, i32::MAX, i32::MIN, (h >> 7) as i32][(h & 3) as usize]
            .wrapping_add((h >> 9) as i32 >> 12);
        let y = [i32::MAX, i32::MIN, (h >> 21) as i32][(h >> 3) as usize % 3]
            .wrapping_sub((h >> 40) as i32 >> 14);
        let z = match (h >> 5) % 3 {
            0 => -(Z_START as i32),
            1 => (height - i64::from(Z_START) - 1) as i32,
            _ => ((h >> 24) as i64 % height - i64::from(Z_START)) as i32,
        };
        let o = [x, y, z];
        let g = mix64(i ^ 0xA5A5);
        let d = [g, g >> 21, g >> 42].map(|v| (v & 0x1f_ffff) as f32 / 1_048_576.0 - 1.0);
        let len = libm::sqrtf(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
        if len < 1e-3 {
            return None;
        }
        let ray = Ray::new(d.map(|v| v / len), REACH as f32);
        let t = (g >> 50) as f32 / (1u64 << 14) as f32 * (REACH - 1) as f32;
        Some((o, point_at(o, &ray, t)))
    }

    /// A node held from one origin is the same world node when held from another a little off,
    /// across a root's edge and X's seam alike.
    #[test]
    fn node_moves_origin() {
        let edges = [0i32, -1, 5, i32::MAX - 3, i32::MIN + 2, 1 << 20];
        for &a in &edges {
            for d in [-700i32, -1, 0, 1, 700] {
                // Y and Z do not wrap: take origins around their roots' edge at 0.
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

    /// The world's index of a node within `REACH` of an origin in the world gives back the node
    /// as the walk from that origin holds it, at every zoom from 1 to the finest, near X's seam
    /// and the world's edges alike.
    #[test]
    fn idx_names_its_node_within_reach() {
        let mut checked = 0;
        for (o, p) in (0..20_000u64).filter_map(reached) {
            for zoom in 1..=FINEST_ZOOM {
                let n = Node {
                    corner: p.0.map(|c| c & !under(zoom)),
                    zoom,
                    roots: p.1,
                };
                assert_eq!(Node::of_idx(o, zoom, n.idx(o)), n, "{o:?} {p:?} {zoom}");
                checked += 1;
            }
        }
        assert!(checked > 500_000);
    }
    use crate::answer;
    use crate::answer::{fill_word, BOX_WHOLE, EIGHTHS, FULL};
    use crate::content::Sample;
    use crate::object::AIR;

    /// One cell of stone at `TARGET`, a unit cell, keeping `fill` eighths of its cube, with air
    /// around it.
    struct One {
        fill: u32,
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
                    asset: [fill_word(Sample::GROUND, self.fill), 0],
                },
                _ => In {
                    kind: Kind::Cell,
                    asset: [AIR, 0],
                },
            }
        }
    }

    /// A ray along +x from five units, at `y` and `z` units, against a cell keeping `fill`
    /// eighths.
    fn along(fill: u32, y: f32, z: f32) -> Trace<()> {
        let steps = |u: f32| (u * (1 << FRAC_W) as f32) as i32;
        let o = [steps(5.0), steps(y), steps(z)];
        let ray = Ray::new([1.0, 0.0, 0.0], 20.0);
        let detail = Detail {
            range: [UNIT_ZOOM, UNIT_ZOOM],
            spacing: Spacing::NONE,
        };
        first(&mut One { fill }, o, &ray, detail, Until::Opaque)
    }

    #[test]
    fn share_box_volume() {
        for k in 1..=EIGHTHS {
            let (lo, hi) = fill_bounds(([0.0; 3], [1.0; 3]), fill_word(0, k));
            let volume = (hi[0] - lo[0]) * (hi[1] - lo[1]) * (hi[2] - lo[2]);
            assert!(
                (volume - k as f32 / EIGHTHS as f32).abs() < 1e-6,
                "{k}: {volume}"
            );
            // Centered on the cube's center.
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
        let trace = along(1, 0.5, 0.5);
        assert_eq!(trace.end, End::Hit);
        assert!((trace.hit.t - 5.25).abs() < 1e-5, "{}", trace.hit.t);
        assert_eq!(trace.hit.axis, 0);
        // Whole, the cube's face.
        let trace = along(EIGHTHS, 0.5, 0.5);
        assert!((trace.hit.t - 5.0).abs() < 1e-5, "{}", trace.hit.t);
    }

    /// A range that inverts, its least over its most, as the eye's may while it crosses, holds
    /// every distance to its finest zoom and aborts nothing.
    #[test]
    fn inverted_range_holds() {
        let detail = Detail {
            range: [UNIT_ZOOM, UNIT_ZOOM - 4],
            spacing: Spacing::NONE,
        };
        for d in [0.0f32, 1.0, 96.0, 1.0e4, 1.0e9] {
            assert_eq!(detail.at(d * d), UNIT_ZOOM - 4, "{d}");
        }
    }

    #[test]
    fn ray_beside_box_passes() {
        let trace = along(1, 0.1, 0.5);
        assert_eq!(trace.end, End::Miss);
        let trace = along(4, 0.5, 0.05);
        assert_eq!(trace.end, End::Miss);
        // The whole cube stops it there.
        assert_eq!(along(EIGHTHS, 0.1, 0.5).end, End::Hit);
    }
}
