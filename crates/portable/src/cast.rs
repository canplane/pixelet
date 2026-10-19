// pxl-portable/cast.rs

//! gpu-portable: the first block a ray meets, at the zoom its distance asks
//! (doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md),
//! one descent for the CPU and the GPU over a query each answers its own
//! way (`Query`). A node coarser than the zoom its distance asks is asked
//! whether anything under it fills: nothing, and the ray goes on past it
//! whole; else its children the ray meets, nearest first. A node at that
//! zoom is asked its block, and the ray stops there or goes on.
//!
//! Positions are the ray's origin's: a node's corner is an exact integer
//! offset from the unit cell holding the origin, at the node's zoom or the
//! unit's, whichever is finer, and only that offset less the origin's
//! fraction is a float. So the nodes near the origin are as exact as f32
//! is near zero wherever the origin stands, and X goes round the world as
//! the integers do. WGSL's f32 is not strictly IEEE (a division's last
//! place, a contraction into a fused multiply-add), so where a ray passes
//! within the last bit of a face, the CPU and a GPU may stop on either side
//! of it; that is the one rule computed twice, not a second rule.
//!
//! The descent holds one frame a zoom, so its stack is as deep as the zooms
//! a node can have, and from a child it steps to the next across the face
//! it leaves by, so no frame holds its siblings.

use crate::{wrap_index, Zoom, FINEST_ZOOM, FRAC_W, UNIT_ZOOM};

/// Drawn pixels per CSS pixel at most: the finest ratio a surface is drawn at,
/// and where more pixels stop being seen. The CSS reference pixel subtends
/// 0.0213° (CSS Values and Units: 96 dpi at 28 inches), 46.9 a degree, and
/// the fovea resolves 94 a degree at most (Ashraf, Chapiro & Mantiuk,
/// "Resolution limit of the eye: how many pixels can we see?", Nature
/// Communications, 2025, doi:10.1038/s41467-025-64679-2): 94 / 46.9 is 2.
pub const RATIO_MAX: f32 = 2.0;

/// The camera grid, the one knob of resolution: a cell is an angle of
/// 1/`SPLIT` radians about the eye (pxl-spec space/camera/grid.rs), stated
/// at `RATIO_MAX`: 96 a radian a CSS pixel asks, in drawn pixels. A lower
/// ratio's cells are wider by `RATIO_MAX` over it, its τ, so whatever is
/// sized by `SPLIT` holds at every ratio.
pub const SPLIT: f32 = 96.0 * RATIO_MAX;

/// The camera grid's cell zoom at `r2`, a distance from the eye squared,
/// in units: the widest cell whose split distance (`SPLIT` of its sides,
/// halved) is within the distance, held to what a zoom can name. The power
/// of two is the float's own exponent, halved, so neither a logarithm nor a
/// square root rounds it: a distance over the half split, `m·2^e` with `m`
/// in [1, 2), squares to `m²·2^2e` with `m²` in [1, 4), whose exponent is
/// `2e` or `2e + 1`, so halving it, rounding down, is `e`; and rounding the
/// square never reaches 4, since the widest `m` short of 2, `2 − 2⁻²³`,
/// squares to `4 − 2⁻²¹` and a bit, which rounds down to it. Every asker
/// passes a square (pxl-spec space/camera/grid.rs, `scatter::drawn_at`), so
/// a distance on a power of two falls the same side everywhere.
pub fn cell_at(r2: f32) -> Zoom {
    let x2 = r2 / (SPLIT * SPLIT / 4.0);
    let e = (((x2.to_bits() >> 23) & 0xff) as i32 - 127) >> 1;
    (UNIT_ZOOM as i32 - e).clamp(0, FINEST_ZOOM as i32) as Zoom
}

/// The direction at the tangents `tx` across and `ty` up from the optical
/// axis: a drawn pixel's middle, its tangents the screen's half-angles'
/// (pxl-kernel view/lens.rs `Lens`) scaled by where it lies on the screen,
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
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detail {
    pub tau: f32,
    pub range: [Zoom; 2],
}

impl Detail {
    /// At `d2`, a distance squared.
    pub fn at(self, d2: f32) -> Zoom {
        cell_at(d2 * (self.tau * self.tau)).clamp(self.range[0], self.range[1])
    }
}

/// Which block a ray stops at: the first that fills, as an eye sees; or the
/// first that does not, as what is climbed out of ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Until {
    Fills,
    Air,
}

/// Where rays start, from a Q3 position: the unit cell holding it, X going
/// round as an i32 does, and where in it it is, `0..1` each. What follows
/// from it alone is the same for every ray from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Origin {
    pub unit: [i32; 3],
    pub frac: [f32; 3],
}

impl Origin {
    pub fn of(q: [i32; 3]) -> Origin {
        let step = |v: i32| (v & ((1 << FRAC_W) - 1)) as f32 / (1 << FRAC_W) as f32;
        Origin {
            unit: [q[0] >> FRAC_W, q[1] >> FRAC_W, q[2] >> FRAC_W],
            frac: [step(q[0]), step(q[1]), step(q[2])],
        }
    }
}

/// A ray from an origin: its direction, how far it reaches, in units, and
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
    pub fn new(direction: [f32; 3], reach: f32) -> Ray {
        let inv = |d: f32| if d == 0.0 { 0.0 } else { 1.0 / d };
        Ray {
            direction,
            reach,
            inverse: [inv(direction[0]), inv(direction[1]), inv(direction[2])],
        }
    }
}

/// A node's place from an origin: its zoom and its corner's offset from the
/// origin's unit cell, counted in its own cells where they are finer than a
/// unit, else in units.
#[derive(Clone, Copy, Debug, Eq)]
pub struct Place {
    pub zoom: Zoom,
    pub corner: [i32; 3],
}

/// Field by field: an array's own `==` is an intrinsic rust-gpu has no
/// word for.
impl PartialEq for Place {
    fn eq(&self, other: &Place) -> bool {
        self.zoom == other.zoom && self.corner[0] == other.corner[0] && self.corner[1] == other.corner[1] && self.corner[2] == other.corner[2]
    }
}

impl Place {
    /// The zoom its corner is counted at.
    const fn grid(zoom: Zoom) -> Zoom {
        if zoom > UNIT_ZOOM {
            zoom
        } else {
            UNIT_ZOOM
        }
    }

    /// A count at its grid, in units.
    fn scale(self) -> f32 {
        1.0 / (1u32 << (Place::grid(self.zoom) - UNIT_ZOOM)) as f32
    }

    /// Its side, in units.
    pub fn side(self) -> f32 {
        if self.zoom > UNIT_ZOOM {
            1.0 / (1u32 << (self.zoom - UNIT_ZOOM)) as f32
        } else {
            (1u32 << (UNIT_ZOOM - self.zoom)) as f32
        }
    }

    /// Its min corner, from the origin, in units.
    pub fn min(self, o: &Origin) -> [f32; 3] {
        let s = self.scale();
        [self.corner[0] as f32 * s - o.frac[0], self.corner[1] as f32 * s - o.frac[1], self.corner[2] as f32 * s - o.frac[2]]
    }

    /// Whether it holds the origin.
    pub fn holds(self, o: &Origin) -> bool {
        let (min, side) = (self.min(o), self.side());
        min[0] <= 0.0 && 0.0 < min[0] + side && min[1] <= 0.0 && 0.0 < min[1] + side && min[2] <= 0.0 && 0.0 < min[2] + side
    }

    /// A child of a node at `zoom`: its corner is the parent's times `m`,
    /// plus `h` where it is the upper half on an axis.
    const fn step(zoom: Zoom) -> (i32, i32) {
        if zoom < UNIT_ZOOM {
            (1, 1 << (UNIT_ZOOM - zoom - 1))
        } else {
            (2, 1)
        }
    }

    /// Its child `k`, `dx + 2·dy + 4·dz`.
    pub fn child(self, k: u32) -> Place {
        let (m, h) = Place::step(self.zoom);
        let at = |a: usize| self.corner[a] * m + ((k >> a) & 1) as i32 * h;
        Place {
            zoom: self.zoom + 1,
            corner: [at(0), at(1), at(2)],
        }
    }

    /// Which child of its parent it is, `dx + 2·dy + 4·dz`: its index's low
    /// bits, the origin's unit cell put back on its corner.
    pub fn slot(self, o: &Origin) -> u32 {
        let g = Place::grid(self.zoom);
        let bit = |a: usize| ((self.corner[a].wrapping_add(o.unit[a].wrapping_shl(g - UNIT_ZOOM)) >> (g - self.zoom)) & 1) as u32;
        bit(0) | bit(1) << 1 | bit(2) << 2
    }

    /// Its parent: `child`'s inverse.
    pub fn parent(self, o: &Origin) -> Place {
        let k = self.slot(o);
        let (m, h) = Place::step(self.zoom - 1);
        // Exact: a child's corner is its parent's times `m`, 1 or 2, plus `h`.
        let at = |a: usize| (self.corner[a] - ((k >> a) & 1) as i32 * h) >> (m >> 1);
        Place {
            zoom: self.zoom - 1,
            corner: [at(0), at(1), at(2)],
        }
    }

    /// The world's node it is, X named as the world names it (pxl-spec
    /// space/coord/world.rs `wrap_index`): its offset put back on the
    /// origin's unit cell.
    pub fn node(self, o: &Origin) -> [i32; 3] {
        let g = Place::grid(self.zoom);
        let at = |a: usize| (self.corner[a].wrapping_add(o.unit[a].wrapping_shl(g - UNIT_ZOOM))) >> (g - self.zoom);
        [wrap_index(self.zoom, at(0)), at(1), at(2)]
    }
}

/// A node as the walk holds it, each axis an integer: its corner in its
/// root at the finest grid a position names (`FINEST_ZOOM`, 2^32 a root, so
/// a u32: the Q3 itself modulo the root's side), its zoom, and which root
/// it is in, counted from the origin's. Its corner's bits under its own
/// side are 0, so two corners part at the highest bit they differ in.
#[derive(Clone, Copy)]
struct Node {
    corner: [u32; 3],
    zoom: Zoom,
    roots: [i32; 3],
}

/// The origin on the finest grid within its root, and its root.
fn grid_of(o: &Origin) -> ([u32; 3], [i32; 3]) {
    let at = |unit: i32, frac: f32| ((unit as u32) & ((1 << UNIT_ZOOM) - 1)) << FRAC_W | (frac * (1 << FRAC_W) as f32) as u32;
    // [naga] each axis by itself, not a loop (caves.rs `node_seed`).
    (
        [at(o.unit[0], o.frac[0]), at(o.unit[1], o.frac[1]), at(o.unit[2], o.frac[2])],
        [o.unit[0] >> UNIT_ZOOM, o.unit[1] >> UNIT_ZOOM, o.unit[2] >> UNIT_ZOOM],
    )
}

/// Units from the origin, at `og` in its root, to `p` in the root `roots`
/// from it: the whole units summed as integers before they are a float, so
/// a plane is as exact far from the origin's root as in it.
fn units(p: u32, og: u32, roots: i32) -> f32 {
    ((p >> FRAC_W) as i32 - (og >> FRAC_W) as i32 + (roots << UNIT_ZOOM)) as f32 + ((p & 0xff) as i32 - (og & 0xff) as i32) as f32 / (1 << FRAC_W) as f32
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
/// in the root `root`: `Place::node`'s value, `root · 2^z` and the corner's
/// high bits, no shift reaching the word's width.
fn index_on(root: i32, c: u32, z: Zoom) -> i32 {
    let high = if z >= FINEST_ZOOM { 0 } else { (root << z) as u32 };
    let low = if z == 0 { 0 } else { c >> (FINEST_ZOOM - z) };
    high.wrapping_add(low) as i32
}

/// The world's node the walk's node is, X named as the world names it.
fn index_of(n: &Node, r0: [i32; 3]) -> [i32; 3] {
    [
        wrap_index(n.zoom, index_on(r0[0] + n.roots[0], n.corner[0], n.zoom)),
        index_on(r0[1] + n.roots[1], n.corner[1], n.zoom),
        index_on(r0[2] + n.roots[2], n.corner[2], n.zoom),
    ]
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

/// The node as a `Place`, a corner counted from the origin's unit cell at
/// its grid (`Place::grid`), for a stop's asker.
fn place_of(n: &Node, og: [u32; 3]) -> Place {
    let g = Place::grid(n.zoom);
    let on = |c: u32, og: u32, roots: i32| {
        let unit = og >> FRAC_W << FRAC_W;
        if g == UNIT_ZOOM {
            (c >> FRAC_W) as i32 - (unit >> FRAC_W) as i32 + (roots << UNIT_ZOOM)
        } else {
            (c.wrapping_sub(unit) as i32) >> (FINEST_ZOOM - g)
        }
    };
    Place {
        zoom: n.zoom,
        corner: [on(n.corner[0], og[0], n.roots[0]), on(n.corner[1], og[1], n.roots[1]), on(n.corner[2], og[2], n.roots[2])],
    }
}

/// What a node is to a descent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Kind {
    /// Coarser than asked: something under it may fill.
    Split,
    /// A block: at the zoom asked, what it is; coarser, `Query::AIR` where
    /// nothing under it fills at the zooms asked, as a block of air its size.
    Block,
    /// Not held: past it the ray tells nothing.
    Lacks,
}

/// What is in a node: its kind, and the block where it is one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct In<B> {
    pub kind: Kind,
    pub block: B,
}

/// The world as a descent asks it, each node handed what its parent found
/// (the `At`). The CPU and a GPU answer it from the same reading of the
/// ground's windows, the caves, the ways' runs and what is placed; only the
/// CPU's answer strikes decor.
pub trait Query {
    type At: Copy;
    type Block: Copy;
    /// What a descent's stack keeps of what was handed a node it went
    /// into (`keep`), in the fewest words: only what it cannot get again
    /// from what it holds when it comes back for the node's next child
    /// (`unkeep`).
    type Kept: Copy;
    /// Where a descent keeps what it keeps of each level (`Levels`).
    type Stack: Levels<Self::Kept>;
    /// What no block fills: air, or void.
    const AIR: Self::Block;
    /// A root of zoom 0, `node` the world's.
    fn root(&mut self, node: [i32; 3]) -> Self::At;
    /// The child `node` at `zoom` of the node `at` was handed for, with what
    /// that one found handed down.
    fn child(&mut self, at: &Self::At, zoom: Zoom, node: [i32; 3]) -> Self::At;
    /// What is in the node `node` at `zoom`, `at` what its ancestors handed
    /// it, asked from `far` to `near`, the ray in it from `t0`. Where the
    /// stack holds fewer levels than a descent goes down (`Levels`), the
    /// walk may ask a node again as it first did (`first_from`'s restart), so
    /// there it does nothing but answer and leave `at`.
    fn node(&mut self, at: &mut Self::At, zoom: Zoom, node: [i32; 3], asked: [Zoom; 2], t0: f32) -> In<Self::Block>;
    fn fills(&self, block: &Self::Block) -> bool;
    /// A split's slab along z, from the lowest leaf cell anything in it may
    /// fill to past the highest (pxl-portable `world::slab_of`): where a ray
    /// through the node is wholly out of it, it meets nothing there.
    fn slab(&self, block: &Self::Block) -> [i32; 2];
    /// What the stack keeps of `at`, handed a node the descent goes into.
    fn keep(at: &Self::At) -> Self::Kept;
    /// What was handed the node at `zoom` the stack kept `kept` of, `live`
    /// what was handed the node the descent comes back from, under it.
    fn unkeep(kept: Self::Kept, live: &Self::At, zoom: Zoom) -> Self::At;
}

/// A descent's stack: what it keeps of each node it went into
/// (`Query::Kept`), `LEVELS` of them, a zoom's where `zoom % LEVELS` puts
/// it. An entry is read where the walk comes back to its node for its next
/// child, once every level it went down under it since is written; with
/// fewer levels than that, a deeper level has taken the entry's place, and
/// the walk goes down to the node again from the nearest node over it it
/// holds without a stack (`first_from`'s restart). The CPU's walk keeps every
/// level, so it never does.
pub trait Levels<K: Copy>: Copy {
    const LEVELS: Zoom;
    fn filled(k: K) -> Self;
    fn at(&self, zoom: Zoom) -> K;
    fn put(&mut self, zoom: Zoom, k: K);
}

impl<K: Copy, const N: usize> Levels<K> for [K; N] {
    const LEVELS: Zoom = N as Zoom;
    fn filled(k: K) -> [K; N] {
        [k; N]
    }
    fn at(&self, zoom: Zoom) -> K {
        self[zoom as usize % N]
    }
    fn put(&mut self, zoom: Zoom, k: K) {
        self[zoom as usize % N] = k;
    }
}

/// Where a ray stopped: how far, the axis it entered across, the node and
/// what it is there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stop<A, B> {
    pub t: f32,
    pub axis: u32,
    pub place: Place,
    pub at: A,
    pub block: B,
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
pub struct Met<A, B> {
    pub ends: Ends,
    pub stop: Stop<A, B>,
    pub visits: u32,
}

/// The descent's stack: a zoom's entry is what the query handed its node
/// (`Query::At`), the one thing the node's parent cannot work out again
/// without asking the world from the top. Where the ray is in the node, the
/// child it is in and where the ray leaves that child follow from the node's
/// place by the slab test, so no entry holds them.
pub const DEPTH: usize = FINEST_ZOOM as usize + 1;

/// A descent's start as a GPU hands it from the kernel that finds it to the
/// kernels that cast from it (gpu-query-shader `start_cs`), in words: the
/// node's zoom then its corner (`START_PLACE`), what was handed it
/// (`START_AT`), and the stack (`start_stack`), each `Query::At` `AT_WORDS`
/// (gpu-query-shader `at_words`).
pub const AT_WORDS: u32 = 5;
pub const START_PLACE: u32 = 0;
pub const START_AT: u32 = START_PLACE + 4;
/// The stack's entry `k`.
pub const fn start_stack(k: u32) -> u32 {
    START_AT + AT_WORDS * (1 + k)
}
pub const START_WORDS: u32 = start_stack(DEPTH as u32);

/// A descent's start: the node entered first, what was handed it, and the
/// stack down to it.
#[derive(Clone, Copy)]
pub struct Start<A: Copy> {
    pub place: Place,
    pub at: A,
    pub stack: [A; DEPTH],
}

/// What was handed each node over a descent's start, the nodes holding its
/// origin, asked by its zoom where a ray climbs past the start to one: the
/// start's stack (`start`) as a GPU holds it, read where asked, or nothing
/// over the roots (`Rooted`).
pub trait Over<A: Copy> {
    fn over(&self, zoom: Zoom) -> A;
}

/// Over a walk from the roots, nothing: no node over a root, so it is never
/// asked (`top` 0), and answers the root's own.
#[derive(Clone, Copy)]
pub struct Rooted<A>(pub A);

impl<A: Copy> Over<A> for Rooted<A> {
    fn over(&self, _: Zoom) -> A {
        self.0
    }
}

/// The world's root holding `o`: its corner the origin's unit cell's offset
/// into it, negated.
fn root_of(o: &Origin) -> Place {
    let side = Place { zoom: 0, corner: [0; 3] }.side() as i32;
    Place {
        zoom: 0,
        corner: [-(o.unit[0] & (side - 1)), -(o.unit[1] & (side - 1)), -(o.unit[2] & (side - 1))],
    }
}

/// The zooms a node is asked at by the rays from `o` in it reaching at most
/// `far` from it, `far2` that squared: from there to the node's point
/// nearest `o` (`Detail::at`), no root taken.
fn asked(o: &Origin, detail: Detail, place: Place, far2: f32) -> [Zoom; 2] {
    let (min, side) = (place.min(o), place.side());
    let near = [min[0].max(0.0).min(min[0] + side), min[1].max(0.0).min(min[1] + side), min[2].max(0.0).min(min[2] + side)];
    [detail.at(far2), detail.at(squared(near))]
}

/// The zooms a node is asked at by every ray from `o` in it: `asked` out
/// to its corner farthest from the origin.
fn widest(o: &Origin, detail: Detail, place: Place) -> [Zoom; 2] {
    let (min, side) = (place.min(o), place.side());
    let far2 = squared([(-min[0]).max(min[0] + side), (-min[1]).max(min[1] + side), (-min[2]).max(min[2] + side)]);
    asked(o, detail, place, far2)
}

/// A vector's length squared.
fn squared(v: [f32; 3]) -> f32 {
    v[0] * v[0] + v[1] * v[1] + v[2] * v[2]
}

/// Where every ray from `o` starts alike: down from the
/// world's root, each node holding the origin asked at the zooms some ray
/// asks in it — from the origin, inside it, to its corner farthest from the
/// origin (`Detail::at`) — and entered while it splits, to the first that
/// does not. A ray asks a node it enters at those zooms or narrower ones,
/// and where a node splits at these and is air at a ray's own, every node
/// under it is air at that ray's, narrower still: so a ray started here
/// (`first_from`) meets what it meets from the root, and the nodes over
/// here, which every ray from the origin would ask, are asked once. Where
/// the origin lies on a node's mid plane the child holding it is the ray's
/// direction's to say, so the start stops there.
pub fn start<Q: Query>(q: &mut Q, o: &Origin, detail: Detail) -> Start<Q::At> {
    let mut place = root_of(o);
    let mut at = q.root(place.node(o));
    let mut stack = [at; DEPTH];
    while (place.zoom as usize) + 1 < DEPTH {
        let mut asking = at;
        if q.node(&mut asking, place.zoom, place.node(o), widest(o, detail, place), 0.0).kind != Kind::Split {
            break;
        }
        let half = place.child(7).min(o);
        if half[0] == 0.0 || half[1] == 0.0 || half[2] == 0.0 {
            break;
        }
        let k = (half[0] < 0.0) as u32 | ((half[1] < 0.0) as u32) << 1 | ((half[2] < 0.0) as u32) << 2;
        stack[place.zoom as usize] = asking;
        let kid = place.child(k);
        at = q.child(&asking, kid.zoom, kid.node(o));
        place = kid;
    }
    Start { place, at, stack }
}

/// The first block `ray` meets within its reach that `until` stops at, at
/// the zoom `detail` asks where it meets it: down the pyramid from the
/// world's roots, the cubes of zoom 0 it crosses. What the ray starts inside
/// is not met when it stops where a block fills, so an eye under the ground
/// is not stopped where it stands.
pub fn first<Q: Query>(q: &mut Q, o: &Origin, ray: &Ray, detail: Detail, until: Until) -> Met<Q::At, Q::Block> {
    let root = root_of(o);
    let at = q.root(root.node(o));
    first_from(q, o, ray, detail, until, (root, at), &Rooted(at))
}

/// `first`, the ray's descent started at `from`, a node holding its origin
/// and what was handed it, `over` what was handed the nodes over it
/// (`start`). The first block the walk stops at, through the roots the
/// ray crosses: its node, each axis an integer in its root (`Node`), and
/// where the ray is, `t`; the planes, the span and the axis entered across
/// worked out from those where asked. A node is entered once: asked
/// whether anything under it fills at the zooms the ray asks within it, at
/// its point nearest the origin and where the ray leaves it; a block — the
/// block at that zoom, or air where nothing under it fills — the walk
/// stops at (for an eye, where it fills, but not in the block it starts
/// in; for what is climbed out of, where it does not) or goes on past;
/// else its child the ray is in (PUSH, Laine & Karras 2010 §3). Past a node
/// the corner steps on the axis the ray leaves by (Amanatides & Woo 1987)
/// and the walk is at the child of the nearest ancestor holding the next
/// node, by the highest bit the step changed (POP), or past a root, in the
/// next root, from its top. One loop.
#[allow(clippy::too_many_arguments)]
pub fn first_from<Q: Query>(q: &mut Q, o: &Origin, ray: &Ray, detail: Detail, until: Until, from: (Place, Q::At), over: &impl Over<Q::At>) -> Met<Q::At, Q::Block> {
    let (og, r0) = grid_of(o);
    // What the walk carries is what nothing it holds gives again: the node
    // and what was handed it, where the ray is, the levels it went into
    // (`stack`, from `floor` its own), the zoom over which the origin's line
    // is `over`'s (`top`, 0 past the start's root), and its visits.
    let (mut top, mut stack, mut floor) = (from.0.zoom, Q::Stack::filled(Q::keep(&from.1)), 0);
    let g = Place::grid(from.0.zoom);
    let start = |c: i32, og: u32| (og >> FRAC_W << FRAC_W).wrapping_add((c as u32) << (FINEST_ZOOM - g));
    let mut n = Node {
        corner: [start(from.0.corner[0], og[0]), start(from.0.corner[1], og[1]), start(from.0.corner[2], og[2])],
        zoom: from.0.zoom,
        roots: [0; 3],
    };
    let (mut at, mut t, mut visits) = (from.1, 0.0f32, 0);
    // The world's node the walk's node is, worked out where the node is
    // made, its At handed it, and read where it is asked.
    let mut node = index_of(&n, r0);
    // The node's planes, in units from the origin, each a corner's
    // (`units`): a PUSH hands its child the parent's and the mid's it
    // crossed by (`carried`), and the walk works them out of the corner
    // only where it stepped or went up, which hands it no plane.
    let (mut lo, mut hi, mut carried) = ([0.0f32; 3], [0.0f32; 3], false);
    loop {
        let u = under(n.zoom);
        if !carried {
            // The far plane is the next node's corner, in the next root
            // where the corner goes past its own.
            let far = |c: u32, og: u32, roots: i32| {
                let last = c.wrapping_add(u);
                units(last.wrapping_add(1), og, roots + (last == u32::MAX) as i32)
            };
            lo = [units(n.corner[0], og[0], n.roots[0]), units(n.corner[1], og[1], n.roots[1]), units(n.corner[2], og[2], n.roots[2])];
            hi = [far(n.corner[0], og[0], n.roots[0]), far(n.corner[1], og[1], n.roots[1]), far(n.corner[2], og[2], n.roots[2])];
        }
        carried = false;
        // The ray's span in it.
        let (x, y, z) = (
            planes(ray.direction[0], ray.inverse[0], lo[0], hi[0], (t, ray.reach)),
            planes(ray.direction[1], ray.inverse[1], lo[1], hi[1], (t, ray.reach)),
            planes(ray.direction[2], ray.inverse[2], lo[2], hi[2], (t, ray.reach)),
        );
        let (t0, t1) = (t.max(x.0).max(y.0).max(z.0), ray.reach.min(x.1).min(y.1).min(z.1));
        // Only touched, at a face or an edge: not entered, nothing in it met.
        let mut into = false;
        if t1 > t0 {
            let near = [lo[0].max(0.0).min(hi[0]), lo[1].max(0.0).min(hi[1]), lo[2].max(0.0).min(hi[2])];
            let asking = [detail.at(t1 * t1 * squared(ray.direction)), detail.at(squared(near))];
            let i = q.node(&mut at, n.zoom, node, asking, t0);
            visits += 1;
            let inside = lo[0] <= 0.0 && 0.0 < hi[0] && lo[1] <= 0.0 && 0.0 < hi[1] && lo[2] <= 0.0 && 0.0 < hi[2];
            let ends = match i.kind {
                Kind::Lacks => Ends::Lacks,
                Kind::Block if if until == Until::Fills { q.fills(&i.block) && !inside } else { !q.fills(&i.block) } => Ends::Stop,
                Kind::Block => Ends::On,
                // ESVO's contour (Laine & Karras 2010): the ray's span in the
                // node clipped to the slab along z its content lies in, its
                // planes a face's own (`level`); a span wholly out of it, or
                // touching it alone, meets nothing that fills, and a walk to
                // what fills passes the node whole. A walk to air goes in: its
                // stop is where that span begins.
                Kind::Split => {
                    let [lo, hi] = q.slab(&i.block);
                    let level = |v: i32| {
                        let root = v >> UNIT_ZOOM;
                        units(((v - (root << UNIT_ZOOM)) as u32) << FRAC_W, og[2], root - r0[2])
                    };
                    let (bottom, top) = planes(ray.direction[2], ray.inverse[2], level(lo), level(hi), (t0, t1));
                    into = until == Until::Air || t0.max(bottom) < t1.min(top);
                    Ends::On
                }
            };
            if ends != Ends::On {
                // Where the walk ends, its stop, made there and nowhere else:
                // the axis entered across the last of the near planes the
                // ray reaches.
                let moves = |d: f32| d != 0.0;
                let (mut axis, mut best) = if moves(ray.direction[0]) {
                    (0, x.0)
                } else if moves(ray.direction[1]) {
                    (1, y.0)
                } else {
                    (2, z.0)
                };
                if axis < 1 && moves(ray.direction[1]) && y.0 > best {
                    (axis, best) = (1, y.0);
                }
                if axis < 2 && moves(ray.direction[2]) && z.0 > best {
                    axis = 2;
                }
                let stop = Stop {
                    t: t0,
                    axis,
                    place: place_of(&n, og),
                    at,
                    block: i.block,
                };
                return Met { ends, stop, visits };
            }
        }
        // Every way on makes the next node: what is handed it is decided here,
        // its parent's At or, at a root, its own (`fresh`), and the node is
        // made once, after (`q.child`, `q.root`).
        let (parent, fresh) = if into {
            // PUSH: the child holding where the ray is at `t0`, past each mid
            // plane it has crossed by then; its planes the parent's on the
            // side it lies, and the mid.
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
            stack.put(n.zoom, Q::keep(&at));
            if n.zoom + 1 > Q::Stack::LEVELS {
                floor = floor.max(n.zoom + 1 - Q::Stack::LEVELS);
            }
            let side = |k: bool, c: u32| if k { c | half } else { c };
            n = Node {
                corner: [side(crossed[0], n.corner[0]), side(crossed[1], n.corner[1]), side(crossed[2], n.corner[2])],
                zoom: n.zoom + 1,
                ..n
            };
            lo = [
                if crossed[0] { mid[0] } else { lo[0] },
                if crossed[1] { mid[1] } else { lo[1] },
                if crossed[2] { mid[2] } else { lo[2] },
            ];
            hi = [
                if crossed[0] { hi[0] } else { mid[0] },
                if crossed[1] { hi[1] } else { mid[1] },
                if crossed[2] { hi[2] } else { mid[2] },
            ];
            carried = true;
            t = t0;
            (at, false)
        } else {
            if t1 >= ray.reach {
                // Gone on to its reach: no stop, which no asker reads.
                return Met {
                    ends: Ends::On,
                    stop: Stop {
                        t: t1,
                        axis: 0,
                        place: from.0,
                        at,
                        block: Q::AIR,
                    },
                    visits,
                };
            }
            // ADVANCE: the corner stepped on the axis the ray leaves by, the
            // first far plane it reaches.
            let moves = |d: f32| d != 0.0;
            let (mut e, mut best) = if moves(ray.direction[0]) {
                (0, x.1)
            } else if moves(ray.direction[1]) {
                (1, y.1)
            } else {
                (2, z.1)
            };
            if e < 1 && moves(ray.direction[1]) && y.1 < best {
                (e, best) = (1, y.1);
            }
            if e < 2 && moves(ray.direction[2]) && z.1 < best {
                e = 2;
            }
            let pick = |a: [u32; 3]| {
                if e == 0 {
                    a[0]
                } else if e == 1 {
                    a[1]
                } else {
                    a[2]
                }
            };
            let up = if e == 0 {
                ray.direction[0] > 0.0
            } else if e == 1 {
                ray.direction[1] > 0.0
            } else {
                ray.direction[2] > 0.0
            };
            let old = pick(n.corner);
            let new = if up { old.wrapping_add(u).wrapping_add(1) } else { old.wrapping_sub(u).wrapping_sub(1) };
            // Never back: a walk started past its node's far plane (a beam's
            // t) keeps where it is.
            t = t.max(t1);
            if (up && old.wrapping_add(u) == u32::MAX) || (!up && old == 0) {
                // Past a root: into the next, from its top, over which nothing
                // is asked or held.
                let by = if up { 1 } else { -1 };
                n.roots = [n.roots[0] + (e == 0) as i32 * by, n.roots[1] + (e == 1) as i32 * by, n.roots[2] + (e == 2) as i32 * by];
                (n.corner, n.zoom, top, floor) = ([0; 3], 0, 0, 0);
                (at, true)
            } else {
                // POP: the child, at `l`, of the nearest ancestor holding the next
                // node, by the highest bit the step changed; each axis's corner
                // that ancestor's child's.
                n.corner = [if e == 0 { new } else { n.corner[0] }, if e == 1 { new } else { n.corner[1] }, if e == 2 { new } else { n.corner[2] }];
                let h = 31 - (old ^ new).leading_zeros();
                let keep = !((1u32 << h) - 1);
                n.corner = [n.corner[0] & keep, n.corner[1] & keep, n.corner[2] & keep];
                let l = FINEST_ZOOM - h;
                // What was handed the parent: on the origin's line over the start,
                // `over`'s, which a node shares with the origin down to the bits they
                // part at; the stack's while its entry is its own; else the walk goes
                // down again, as kd-restart does (Foley & Sugerman 2005), from the
                // first node under the deepest it shares with the origin's line,
                // handed `over`'s, or from the root, its At its own, each node on the
                // way asked as it was (its span its own box, its At of what was
                // handed it alone, `Query::node`) and into the child where the ray
                // now is, every level under where it went in written before it is read.
                let home = n.roots[0] == 0 && n.roots[1] == 0 && n.roots[2] == 0;
                let apart = (og[0] ^ n.corner[0]) | (og[1] ^ n.corner[1]) | (og[2] ^ n.corner[2]);
                let shared = apart.leading_zeros();
                if home && l - 1 < top && shared >= l - 1 {
                    n.zoom = l;
                    (over.over(l - 1), false)
                } else if l > floor {
                    n.zoom = l;
                    (Q::unkeep(stack.at(l - 1), &at, l - 1), false)
                } else if home && top > 0 {
                    let b = shared.min(top - 1) + 1;
                    let keep = !under(b);
                    (n.corner, n.zoom, floor) = ([n.corner[0] & keep, n.corner[1] & keep, n.corner[2] & keep], b, b);
                    (over.over(b - 1), false)
                } else {
                    (n.corner, n.zoom, floor) = ([0; 3], 0, 0);
                    (at, true)
                }
            }
        };
        node = index_of(&n, r0);
        at = if fresh { q.root(node) } else { q.child(&parent, n.zoom, node) };
    }
}
