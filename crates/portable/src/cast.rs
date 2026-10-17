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

/// The camera grid's cell zoom at `r` units from the eye: the widest cell
/// whose split distance (`SPLIT` of its sides, halved) is within `r`, held
/// to what a zoom can name. The power of two is the float's own exponent,
/// so no logarithm rounds it.
pub fn cell_at(r: f32) -> Zoom {
    let x = r / (SPLIT / 2.0);
    let e = ((x.to_bits() >> 23) & 0xff) as i32 - 127;
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
/// (`cell_at`), the distance scaled by `tau`, at least 1, held to `range`. A
/// stated zoom's cells wherever they are is a range of that zoom alone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detail {
    pub tau: f32,
    pub range: [Zoom; 2],
}

impl Detail {
    pub fn at(self, d: f32) -> Zoom {
        cell_at(d * self.tau).clamp(self.range[0], self.range[1])
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

/// A ray from an origin: its direction, and how far it reaches, in units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub direction: [f32; 3],
    pub reach: f32,
}

/// A node's place from an origin: its zoom and its corner's offset from the
/// origin's unit cell, counted in its own cells where they are finer than a
/// unit, else in units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub zoom: Zoom,
    pub corner: [i32; 3],
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

/// Where the ray is within a box from `min`, `side` wide, between `t0` and
/// `t1`: its entry and exit, empty where the entry is past the exit, and the
/// axis it enters across, which is the box's own even where the entry is
/// held to `t0`.
pub fn slab(ray: &Ray, min: [f32; 3], side: f32, t0: f32, t1: f32) -> (f32, f32, u32) {
    let (mut enter, mut leave, mut axis) = (t0, t1, 0);
    // The nearest plane yet, once an axis the ray moves along gave one.
    let (mut near_most, mut moved) = (0.0, false);
    // Each axis in turn, its components named, not indexed.
    let mut each = |a: u32, d: f32, min: f32| {
        // An axis the ray runs beside: within the box, any t; outside it,
        // none past the span's ends.
        let (near, far) = if d == 0.0 {
            if 0.0 < min || 0.0 > min + side {
                (t1, t0)
            } else {
                (t0, t1)
            }
        } else {
            let (p, q) = (min / d, (min + side) / d);
            let (near, far) = if d > 0.0 { (p, q) } else { (q, p) };
            if !moved || near > near_most {
                (near_most, moved, axis) = (near, true, a);
            }
            (near, far)
        };
        enter = enter.max(near);
        leave = leave.min(far);
    };
    each(0, ray.direction[0], min[0]);
    each(1, ray.direction[1], min[1]);
    each(2, ray.direction[2], min[2]);
    (enter, leave, axis)
}

/// The axis across which the ray leaves the box from `min`, `side` wide:
/// the first of its far faces it reaches.
fn leaves(ray: &Ray, min: [f32; 3], side: f32) -> u32 {
    let (mut best, mut moved, mut axis) = (0.0, false, 0);
    let mut each = |a: u32, d: f32, min: f32| {
        let t = if d > 0.0 { (min + side) / d } else { min / d };
        if d != 0.0 && (!moved || t < best) {
            (best, moved, axis) = (t, true, a);
        }
    };
    each(0, ray.direction[0], min[0]);
    each(1, ray.direction[1], min[1]);
    each(2, ray.direction[2], min[2]);
    axis
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
    /// What no block fills: air, or void.
    const AIR: Self::Block;
    /// A root of zoom 0, `node` the world's.
    fn root(&mut self, node: [i32; 3]) -> Self::At;
    /// The child `node` at `zoom` of the node `at` was handed for, with what
    /// that one found handed down.
    fn child(&mut self, at: &Self::At, zoom: Zoom, node: [i32; 3]) -> Self::At;
    /// What is in the node `node` at `zoom`, `at` what its ancestors handed
    /// it, asked from `far` to `near`, the ray in it from `t0`.
    fn node(&mut self, at: &mut Self::At, zoom: Zoom, node: [i32; 3], asked: [Zoom; 2], t0: f32) -> In<Self::Block>;
    fn fills(&self, block: &Self::Block) -> bool;
    /// What the stack keeps of `at`, handed a node the descent goes into.
    fn keep(at: &Self::At) -> Self::Kept;
    /// What was handed the node at `zoom` the stack kept `kept` of, `live`
    /// what was handed the node the descent comes back from, under it.
    fn unkeep(kept: Self::Kept, live: &Self::At, zoom: Zoom) -> Self::At;
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
pub const AT_WORDS: u32 = 4;
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
/// start's stack as found (`start`), or as a GPU holds it, read where
/// asked.
pub trait Over<A: Copy> {
    fn over(&self, zoom: Zoom) -> A;
}

impl<A: Copy> Over<A> for [A; DEPTH] {
    fn over(&self, zoom: Zoom) -> A {
        self[zoom as usize]
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
/// `far` from it: from there to the node's point nearest `o`
/// (`Detail::at`).
fn asked(o: &Origin, detail: Detail, place: Place, far: f32) -> [Zoom; 2] {
    let (min, side) = (place.min(o), place.side());
    let near = [min[0].max(0.0).min(min[0] + side), min[1].max(0.0).min(min[1] + side), min[2].max(0.0).min(min[2] + side)];
    [detail.at(far), detail.at(length(near))]
}

fn length(v: [f32; 3]) -> f32 {
    libm::sqrtf(v[0] * v[0] + v[1] * v[1] + v[2] * v[2])
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
        let (min, side) = (place.min(o), place.side());
        let far = length([(-min[0]).max(min[0] + side), (-min[1]).max(min[1] + side), (-min[2]).max(min[2] + side)]);
        let mut asking = at;
        if q.node(&mut asking, place.zoom, place.node(o), asked(o, detail, place, far), 0.0).kind != Kind::Split {
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
    first_from(q, o, ray, detail, until, (root, at), &[at; DEPTH])
}

/// `first`, the ray's descent started at `from`, a node holding its origin
/// and what was handed it, `over` what was handed the nodes over it
/// (`start`).
pub fn first_from<Q: Query>(q: &mut Q, o: &Origin, ray: &Ray, detail: Detail, until: Until, from: (Place, Q::At), over: &impl Over<Q::At>) -> Met<Q::At, Q::Block> {
    let side = Place { zoom: 0, corner: [0; 3] }.side() as i32;
    let mut root = root_of(o);
    let mut met = Met {
        ends: Ends::On,
        stop: Stop {
            t: 0.0,
            axis: 0,
            place: from.0,
            at: from.1,
            block: Q::AIR,
        },
        visits: 0,
    };
    let (mut t, mut started) = (0.0, false);
    while t < ray.reach {
        let min = root.min(o);
        let span = slab(ray, min, root.side(), t, ray.reach);
        if span.0 > span.1 {
            break;
        }
        let visits = met.visits;
        // The first root from `from`, each after it from its own top, over
        // which nothing is asked.
        let (here, span) = if started {
            ((root, q.root(root.node(o))), span)
        } else {
            (from, slab(ray, from.0.min(o), from.0.side(), t, ray.reach))
        };
        met = descend(q, o, ray, detail, until, here, over, span);
        started = true;
        met.visits += visits;
        if met.ends != Ends::On {
            break;
        }
        // On to the root across the face the ray leaves by.
        let k = leaves(ray, min, root.side());
        let step = if crate::axis(ray.direction, k) > 0.0 { side } else { -side };
        root.corner = [
            root.corner[0] + (k == 0) as i32 * step,
            root.corner[1] + (k == 1) as i32 * step,
            root.corner[2] + (k == 2) as i32 * step,
        ];
        t = span.1;
    }
    met
}

/// What the walk does at a node, `block` what it is: for an eye, stops
/// where it fills, but not in the block it starts in; for what is climbed
/// out of, stops where it does not.
fn stops<Q: Query>(q: &Q, o: &Origin, until: Until, place: Place, block: &Q::Block) -> bool {
    let fills = q.fills(block);
    match until {
        Until::Fills => fills && !place.holds(o),
        Until::Air => !fills,
    }
}

/// The first block in a root's node the walk stops at, from `from`, the
/// ray within it from `t0` to `t1`, entering across `axis`. A node is entered once: asked
/// whether anything under it fills at the zooms the ray asks within it, at
/// its point nearest the origin and where the ray leaves it; a block — the
/// block at that zoom, or air where nothing under it fills — the walk stops
/// at or goes on past; else its child the ray enters first, and from a child
/// the next across the face the ray leaves that one by, back up a zoom where
/// that face is the node's own.
#[allow(clippy::too_many_arguments)]
fn descend<Q: Query>(q: &mut Q, o: &Origin, ray: &Ray, detail: Detail, until: Until, from: (Place, Q::At), over: &impl Over<Q::At>, span: (f32, f32, u32)) -> Met<Q::At, Q::Block> {
    let dlen = length(ray.direction);
    // What was handed each node the descent went into, from its start's
    // zoom; over it, `over`'s.
    let (top, mut stack) = (from.0.zoom, [Q::keep(&from.1); DEPTH]);
    // The node being entered, and the ray within it.
    let (mut at, mut place, (mut t0, mut t1, mut axis)) = (from.1, from.0, span);
    let mut visits = 0;
    loop {
        // Only touched, at a face or an edge: not entered, nothing in it met.
        let mut into = false;
        if t1 > t0 {
            let i = q.node(&mut at, place.zoom, place.node(o), asked(o, detail, place, t1 * dlen), t0);
            visits += 1;
            // Where the walk ends, its stop, made there and nowhere else.
            let ends = match i.kind {
                Kind::Lacks => Ends::Lacks,
                Kind::Block if stops(q, o, until, place, &i.block) => Ends::Stop,
                Kind::Block => Ends::On,
                Kind::Split => {
                    into = true;
                    Ends::On
                }
            };
            if ends != Ends::On {
                let stop = Stop {
                    t: t0,
                    axis,
                    place,
                    at,
                    block: i.block,
                };
                return Met { ends, stop, visits };
            }
        }
        // The child gone into next, what was handed its parent, and the span
        // the ray is in it within: into this node, the child holding where
        // the ray enters, past a mid plane on each axis the ray has crossed
        // by then; on past it, the next child of the nearest ancestor that
        // has one across the face the ray leaves its current child by, the
        // ray in that ancestor up to where it leaves the ancestor's box.
        let (kid, handed, from, to) = if into {
            let half = place.child(7).min(o);
            let upper = |d: f32, half: f32| {
                (if d > 0.0 {
                    half / d <= t0
                } else if d < 0.0 {
                    half / d > t0
                } else {
                    half <= 0.0
                }) as u32
            };
            let k = upper(ray.direction[0], half[0]) | upper(ray.direction[1], half[1]) << 1 | upper(ray.direction[2], half[2]) << 2;
            stack[place.zoom as usize] = Q::keep(&at);
            (place.child(k), at, t0, t1)
        } else {
            loop {
                if place.zoom == 0 {
                    return Met {
                        ends: Ends::On,
                        stop: Stop {
                            t: t1,
                            axis,
                            place,
                            at,
                            block: Q::AIR,
                        },
                        visits,
                    };
                }
                let e = leaves(ray, place.min(o), place.side());
                let k = place.slot(o);
                let parent = place.parent(o);
                let bit = 1 << e;
                let d = crate::axis(ray.direction, e);
                let end = slab(ray, parent.min(o), parent.side(), 0.0, ray.reach).1;
                if (d > 0.0) == (k & bit != 0) || d == 0.0 {
                    // The face is the parent's own: on past the parent.
                    (place, t1) = (parent, end);
                    continue;
                }
                // Over the start, `over` holds what was handed the nodes
                // holding the origin; one the ray went into past them, the
                // stack.
                let handed = if parent.zoom < top && parent.holds(o) {
                    over.over(parent.zoom)
                } else {
                    Q::unkeep(stack[parent.zoom as usize], &at, parent.zoom)
                };
                break (parent.child(k ^ bit), handed, t1, end);
            }
        };
        at = q.child(&handed, kid.zoom, kid.node(o));
        (place, (t0, t1, axis)) = (kid, slab(ray, kid.min(o), kid.side(), from, to));
    }
}
