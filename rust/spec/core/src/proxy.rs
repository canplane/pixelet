// pxl-spec-core/proxy.rs

//! gpu-portable: the boxes a rasterized proxy draws, each where a ray may
//! start and nothing else (doc/minutes/20261024-a-proxy-only-starts-the-ray.md).
//!
//! The boxes are the nodes of one cut through the pyramid, the camera
//! grid's (`cut`): a node is drawn where its zoom is at least the one the
//! grid asks at its nearest point (`Detail::at`), `coarser` zooms coarser,
//! as the cast stops at a block where its zoom is that one's. A node is
//! asked what is in it at the zooms the cast asks it at, a function of the
//! node and the eye alone (`asked`), through the world's one query
//! (`world::node`), which reads only what is already held: the windows'
//! levels, the objects' tree, the ways' pool, the caves' hash. Its box is
//! the node clipped along z to its split's slab (`of`).
//!
//! That slab is the one the cast's contour test passes a ray by
//! (`cast::walk`, `Kind::Split`), and a node a cut's or coarser holds every
//! block the cast stops at under it, each asked at a zoom within what the
//! node is asked at. So every block a ray stops at lies in a box, and a ray
//! enters a box no later than it stops: the box's front starts it, and a
//! ray entering no box meets nothing. A node not held is its box whole.

use crate::answer::{fills, slab_of, In, Kind};
use crate::cast::{
    asked as zooms, bounds, greater, grid_of, kept_bounds, lesser, level, reach, roots_of, under,
    Detail, Node,
};
use crate::Zoom;

/// What a node is to the proxy: nothing to draw, a box, or a node over the
/// cut whose children are asked in turn.
pub const NONE: u32 = 0;
pub const BOX: u32 = 1;
pub const UNDER: u32 = 2;

/// A node as the proxy draws it: what it is (`NONE`, `BOX`, `UNDER`), and
/// its box's planes in units from the origin, each axis's lowest and
/// highest; an `UNDER`'s, what its children's boxes lie in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Proxy {
    pub says: u32,
    pub lo: [f32; 3],
    pub hi: [f32; 3],
}

/// The cut's zoom at a node whose nearest point is `near` units squared
/// from the eye: the camera grid's (`Detail::at`), `coarser` zooms
/// coarser, held to the coarsest the range asks.
pub fn cut(detail: Detail, coarser: Zoom, near: f32) -> Zoom {
    let z = detail.at(near);
    if z >= detail.range[0] + coarser {
        z - coarser
    } else {
        detail.range[0]
    }
}

/// The zooms the node `n` from the origin `o` is asked at, the cast's for
/// it (`cast::asked`), its planes in units from the origin, and its cut's
/// zoom (`cut`).
pub fn asked(o: [i32; 3], detail: Detail, coarser: Zoom, n: &Node) -> ([Zoom; 2], Proxy, Zoom) {
    let (lo, hi) = bounds(n, grid_of(o));
    let e = detail.eye;
    let r = reach(
        [lo[0] - e[0], lo[1] - e[1], lo[2] - e[2]],
        [hi[0] - e[0], hi[1] - e[1], hi[2] - e[2]],
    );
    (
        zooms(detail, r),
        Proxy { says: BOX, lo, hi },
        cut(detail, coarser, r[1]),
    )
}

/// The node `n` from the origin `o` as the proxy draws it, `i` the world's
/// answer for it at the zooms it is asked at, and `node` its planes and
/// `cut` its cut's zoom (`asked`): a block that fills, the box it is drawn
/// as (`cast::kept_bounds`); one that does not, or air coarser than asked,
/// nothing; a split, the node clipped along z to its slab, nothing where
/// that is empty, a box at the cut's zoom or finer and `UNDER` coarser; not
/// held, the node whole, since the cast stops at a lack.
pub fn of(o: [i32; 3], n: &Node, node: Proxy, cut: Zoom, i: In<[u32; 2]>) -> Proxy {
    let none = Proxy { says: NONE, ..node };
    match i.kind {
        Kind::Lacks => node,
        Kind::Block if fills(i.block[0]) => {
            let (lo, hi) = kept_bounds((node.lo, node.hi), i.block[0]);
            Proxy { says: BOX, lo, hi }
        }
        Kind::Block => none,
        Kind::Split => {
            let [bottom, top] = slab_of(i.block[0]);
            let (og, r0) = (grid_of(o)[2], roots_of(o)[2]);
            let lo = greater(node.lo[2], level(bottom, og, r0));
            let hi = lesser(node.hi[2], level(top, og, r0));
            if lo >= hi {
                return none;
            }
            Proxy {
                says: if n.zoom >= cut { BOX } else { UNDER },
                lo: [node.lo[0], node.lo[1], lo],
                hi: [node.hi[0], node.hi[1], hi],
            }
        }
    }
}

/// Child `k` of the node `n`: its corner past the mid plane on each axis
/// whose bit `k` sets, X's low, then Y's, then Z's.
pub fn child(n: &Node, k: u32) -> Node {
    let half = (under(n.zoom) >> 1) + 1;
    let side = |b: u32, c: u32| if k >> b & 1 != 0 { c | half } else { c };
    Node {
        corner: [
            side(0, n.corner[0]),
            side(1, n.corner[1]),
            side(2, n.corner[2]),
        ],
        zoom: n.zoom + 1,
        roots: n.roots,
    }
}
