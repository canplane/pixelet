// pxl-portable/cast.rs

//! gpu-portable: the first block a ray meets, at the zoom its distance asks
//! (decision/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md),
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

/// The camera grid, the one knob of resolution: a cell is an angle of
/// 1/`SPLIT` radians about the eye (pxl-spec space/camera/grid.rs).
pub const SPLIT: f32 = 96.0;

/// The camera grid's cell zoom at `r` units from the eye: the widest cell
/// whose split distance (`SPLIT` of its sides, halved) is within `r`, held
/// to what a zoom can name. The power of two is the float's own exponent,
/// so no logarithm rounds it.
pub fn cell_at(r: f32) -> Zoom {
    let x = r / (SPLIT / 2.0);
    let e = ((x.to_bits() >> 23) & 0xff) as i32 - 127;
    (UNIT_ZOOM as i32 - e).clamp(0, FINEST_ZOOM as i32) as Zoom
}

/// The direction through a cell of the camera grid whose middle is at the
/// tangents `tx` across and `ty` up from the optical axis: the camera grid's
/// cells at equal steps of angle (pxl-kernel view/sight.rs `Lens`), the
/// tangents stated once by the CPU, since a tangent of an angle is libm's in
/// f64 on the CPU and a WGSL has none.
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

/// A ray from a Q3 position, its direction and how far it reaches, in units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    /// The unit cell holding the origin, X going round as an i32 does.
    pub unit: [i32; 3],
    /// Where in it the origin is, `0..1` each.
    pub frac: [f32; 3],
    pub direction: [f32; 3],
    pub reach: f32,
}

impl Ray {
    pub fn of(origin: [i32; 3], direction: [f32; 3], reach: f32) -> Ray {
        let unit = [origin[0] >> FRAC_W, origin[1] >> FRAC_W, origin[2] >> FRAC_W];
        let step = |v: i32| (v & ((1 << FRAC_W) - 1)) as f32 / (1 << FRAC_W) as f32;
        Ray {
            unit,
            frac: [step(origin[0]), step(origin[1]), step(origin[2])],
            direction,
            reach,
        }
    }
}

/// A node's place along a ray: its zoom and its corner's offset from the
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

    /// Its min corner, from the ray's origin, in units.
    pub fn min(self, ray: &Ray) -> [f32; 3] {
        let s = self.scale();
        [
            self.corner[0] as f32 * s - ray.frac[0],
            self.corner[1] as f32 * s - ray.frac[1],
            self.corner[2] as f32 * s - ray.frac[2],
        ]
    }

    /// Its child `k`, `dx + 2·dy + 4·dz`.
    pub fn child(self, k: u32) -> Place {
        let z = self.zoom;
        let (m, h) = if z < UNIT_ZOOM { (1, 1 << (UNIT_ZOOM - z - 1)) } else { (2, 1) };
        let at = |a: usize| self.corner[a] * m + ((k >> a) & 1) as i32 * h;
        Place {
            zoom: z + 1,
            corner: [at(0), at(1), at(2)],
        }
    }

    /// The world's node it is, X named as the world names it (pxl-spec
    /// space/coord/world.rs `wrap_index`): its offset put back on the
    /// origin's unit cell.
    pub fn node(self, ray: &Ray) -> [i32; 3] {
        let g = Place::grid(self.zoom);
        let at = |a: usize| (self.corner[a].wrapping_add(ray.unit[a].wrapping_shl(g - UNIT_ZOOM))) >> (g - self.zoom);
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
    let mut a = 0;
    while a < 3 {
        let d = ray.direction[a];
        // An axis the ray runs beside: within the box, any t; outside it,
        // none past the span's ends.
        let (near, far) = if d == 0.0 {
            if 0.0 < min[a] || 0.0 > min[a] + side {
                (t1, t0)
            } else {
                (t0, t1)
            }
        } else {
            let (p, q) = (min[a] / d, (min[a] + side) / d);
            let (near, far) = if d > 0.0 { (p, q) } else { (q, p) };
            if !moved || near > near_most {
                (near_most, moved, axis) = (near, true, a as u32);
            }
            (near, far)
        };
        enter = enter.max(near);
        leave = leave.min(far);
        a += 1;
    }
    (enter, leave, axis)
}

/// The axis across which the ray leaves the box from `min`, `side` wide:
/// the first of its far faces it reaches.
fn leaves(ray: &Ray, min: [f32; 3], side: f32) -> u32 {
    let (mut best, mut moved, mut axis) = (0.0, false, 0);
    let mut a = 0;
    while a < 3 {
        let d = ray.direction[a];
        let t = if d > 0.0 { (min[a] + side) / d } else { min[a] / d };
        if d != 0.0 && (!moved || t < best) {
            (best, moved, axis) = (t, true, a as u32);
        }
        a += 1;
    }
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
/// (the `At`). The CPU answers it from everything it holds; a GPU from the
/// ground's tiles and the caves alone. What only the CPU answers yet —
/// separation (the ways), what is placed, decor — is this experiment's
/// stated gap: a GPU's answer holds none of it.
pub trait Query {
    type At: Copy;
    type Block: Copy;
    /// What no block fills: air, or void.
    const AIR: Self::Block;
    /// A root of zoom 0, `node` the world's.
    fn root(&mut self, node: [i32; 3]) -> Self::At;
    /// `at`'s child `k`, with what it found handed down.
    fn child(&mut self, at: &Self::At, k: u32) -> Self::At;
    /// What is in `at`'s node, asked from `far` to `near`, the ray in it
    /// from `t0`.
    fn node(&mut self, at: &mut Self::At, asked: [Zoom; 2], t0: f32) -> In<Self::Block>;
    fn fills(&self, block: &Self::Block) -> bool;
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

/// What the walk met: how it ended, and where it stopped when it did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Met<A, B> {
    pub ends: Ends,
    pub stop: Stop<A, B>,
}

/// One zoom of the descent: the node, the ray within it, and the child it
/// is in and the ray's exit from that child.
#[derive(Clone, Copy)]
struct Frame<A> {
    at: A,
    place: Place,
    t1: f32,
    child: u32,
    child_leave: f32,
    child_exit: u32,
}

/// The frames of a descent, one a zoom.
const DEPTH: usize = FINEST_ZOOM as usize + 1;

/// The first block `ray` meets within its reach that `until` stops at, at
/// the zoom `detail` asks where it meets it: down the pyramid from the
/// world's roots, the cubes of zoom 0 it crosses. What the ray starts inside
/// is not met when it stops where a block fills, so an eye under the ground
/// is not stopped where it stands.
pub fn first<Q: Query>(q: &mut Q, ray: &Ray, detail: Detail, until: Until) -> Met<Q::At, Q::Block> {
    let side = Place { zoom: 0, corner: [0; 3] }.side() as i32;
    // The root holding the origin: its corner the origin's unit cell's
    // offset into it, negated.
    let mut root = Place {
        zoom: 0,
        corner: [-ray.unit[0].rem_euclid(side), -ray.unit[1].rem_euclid(side), -ray.unit[2].rem_euclid(side)],
    };
    let mut met = Met {
        ends: Ends::On,
        stop: Stop {
            t: 0.0,
            axis: 0,
            place: root,
            at: q.root(root.node(ray)),
            block: Q::AIR,
        },
    };
    let mut t = 0.0;
    while t < ray.reach {
        let min = root.min(ray);
        let span = slab(ray, min, root.side(), t, ray.reach);
        if span.0 > span.1 {
            break;
        }
        let at = q.root(root.node(ray));
        met = descend(q, ray, detail, until, at, root, span);
        if met.ends != Ends::On {
            break;
        }
        // On to the root across the face the ray leaves by.
        let k = leaves(ray, min, root.side()) as usize;
        root.corner[k] += if ray.direction[k] > 0.0 { side } else { -side };
        t = span.1;
    }
    met
}

/// What the walk does at a node, `block` what it is: for an eye, stops
/// where it fills, but not in the block it starts in; for what is climbed
/// out of, stops where it does not.
fn stops<Q: Query>(q: &Q, ray: &Ray, until: Until, place: Place, block: &Q::Block) -> bool {
    let fills = q.fills(block);
    match until {
        Until::Fills => {
            let (min, side) = (place.min(ray), place.side());
            let inside = min[0] <= 0.0 && 0.0 < min[0] + side && min[1] <= 0.0 && 0.0 < min[1] + side && min[2] <= 0.0 && 0.0 < min[2] + side;
            fills && !inside
        }
        Until::Air => !fills,
    }
}

/// The first block in a root's node the walk stops at, the ray within it
/// from `t0` to `t1`, entering across `axis`. A node is entered once: asked
/// whether anything under it fills at the zooms the ray asks within it, at
/// its point nearest the origin and where the ray leaves it; a block — the
/// block at that zoom, or air where nothing under it fills — the walk stops
/// at or goes on past; else its child the ray enters first, and from a child
/// the next across the face the ray leaves that one by, back up a zoom where
/// that face is the node's own.
fn descend<Q: Query>(q: &mut Q, ray: &Ray, detail: Detail, until: Until, root_at: Q::At, root: Place, span: (f32, f32, u32)) -> Met<Q::At, Q::Block> {
    let dlen = libm::sqrtf(ray.direction[0] * ray.direction[0] + ray.direction[1] * ray.direction[1] + ray.direction[2] * ray.direction[2]);
    let empty = Frame {
        at: root_at,
        place: root,
        t1: 0.0,
        child: 0,
        child_leave: 0.0,
        child_exit: 0,
    };
    let mut stack = [empty; DEPTH];
    // The node being entered, and the ray within it.
    let (mut at, mut place, (mut t0, mut t1, mut axis)) = (root_at, root, span);
    loop {
        // Only touched, at a face or an edge: not entered, nothing in it met.
        let mut into = false;
        if t1 > t0 {
            let (min, side) = (place.min(ray), place.side());
            let near = [min[0].max(0.0).min(min[0] + side), min[1].max(0.0).min(min[1] + side), min[2].max(0.0).min(min[2] + side)];
            let asked = [detail.at(t1 * dlen), detail.at(libm::sqrtf(near[0] * near[0] + near[1] * near[1] + near[2] * near[2]))];
            let i = q.node(&mut at, asked, t0);
            let stop = Stop {
                t: t0,
                axis,
                place,
                at,
                block: i.block,
            };
            match i.kind {
                Kind::Lacks => return Met { ends: Ends::Lacks, stop },
                Kind::Block => {
                    if stops(q, ray, until, place, &i.block) {
                        return Met { ends: Ends::Stop, stop };
                    }
                }
                Kind::Split => into = true,
            }
        }
        if into {
            // The child holding where the ray enters: past a mid plane on
            // each axis the ray has crossed by then.
            let half = place.child(7).min(ray);
            let mut k = 0;
            let mut a = 0;
            while a < 3 {
                let d = ray.direction[a];
                let upper = if d > 0.0 {
                    half[a] / d <= t0
                } else if d < 0.0 {
                    half[a] / d > t0
                } else {
                    half[a] <= 0.0
                };
                k |= (upper as u32) << a;
                a += 1;
            }
            let kid = place.child(k);
            let (kmin, kside) = (kid.min(ray), kid.side());
            let span = slab(ray, kmin, kside, t0, t1);
            stack[place.zoom as usize] = Frame {
                at,
                place,
                t1,
                child: k,
                child_leave: span.1,
                child_exit: leaves(ray, kmin, kside),
            };
            at = q.child(&at, k);
            (place, (t0, t1, axis)) = (kid, span);
            continue;
        }
        // On past it: to the next child of the nearest ancestor that has
        // one across the face the ray leaves its current child by.
        loop {
            if place.zoom == root.zoom {
                return Met {
                    ends: Ends::On,
                    stop: Stop {
                        t: t1,
                        axis,
                        place,
                        at,
                        block: Q::AIR,
                    },
                };
            }
            let z = (place.zoom - 1) as usize;
            let f = stack[z];
            let e = f.child_exit;
            let bit = 1 << e;
            let forward = ray.direction[e as usize] > 0.0;
            let upper = f.child & bit != 0;
            if forward == upper || ray.direction[e as usize] == 0.0 {
                // The face is the node's own: on past the node.
                place = f.place;
                continue;
            }
            let child = f.child ^ bit;
            let kid = f.place.child(child);
            let (kmin, kside) = (kid.min(ray), kid.side());
            let span = slab(ray, kmin, kside, f.child_leave, f.t1);
            stack[z] = Frame {
                child,
                child_leave: span.1,
                child_exit: leaves(ray, kmin, kside),
                ..f
            };
            at = q.child(&f.at, child);
            (place, (t0, t1, axis)) = (kid, span);
            break;
        }
    }
}
