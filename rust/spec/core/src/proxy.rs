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
//!
//! Under the ground a slab runs to the world's floor, so each box is cut
//! off at a floor too (`floor`): a block under every face about the node
//! is the ground's, uncarved or carved out of it. A ray from an eye in the
//! open (`aired`) reaches one only through the ground's surface, where it
//! enters a box first. Every cut node is a split, never a block (`cut`), so
//! that surface lies in a box wherever it is carved. An eye not in the
//! open has no proxy: every ray starts at the eye.

use crate::answer::{fills, slab_of, In, Kind};
use crate::area::Area;
use crate::cast::{
    asked as zooms, bounds, greater, grid_of, kept_bounds, lesser, level, reach, roots_of, under,
    Detail, Node,
};
use crate::ground::{blocks, face_height, leaf_blocks, Held, UNBOUNDED};
use crate::window::{Found, Squares};
use crate::{wrap_index, Zoom, FRAC_W, LEAF_CELL, TOP_ZOOM};

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
/// from the eye: the camera grid's (`Detail::at`), `coarser` zooms coarser
/// and at least one, held from a tile's coarsest to the leaves' cells'
/// parents. So a cut node is always coarser than the cast asks it, and
/// coarser than the leaves, where a node with nothing placed finer is a
/// block (`world::node`): it is a split, or air, or not held.
pub fn cut(detail: Detail, coarser: Zoom, near: f32) -> Zoom {
    let k = if coarser < 1 { 1 } else { coarser };
    let z = detail.at(near);
    let z = if z >= TOP_ZOOM + k { z - k } else { TOP_ZOOM };
    if z >= LEAF_CELL {
        LEAF_CELL - 1
    } else {
        z
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

/// The lowest a box over the node `n` from the origin `o` need reach, in
/// leaf cells: a block under the lowest face of the node and the eight
/// about it at its zoom (`Held::relief`), read from the coarsest zoom the
/// cast may draw a block in them at to the finest it asks anywhere, rounded
/// at that coarsest as the ground rounds a face (`ground::top`). Under it a
/// block lies wholly under every face about it, at every zoom it may be
/// asked at. The world's bottom where any of them is not held.
///
/// No block meeting the nine is coarser than the camera grid's zoom at
/// their farthest corner (pxl-spec-core cast.rs `walk`), and a coarser
/// zoom's floor is no higher than a finer's, so the coarsest's stands for
/// all of them. Read to the finest, a node's floor is no higher than its
/// children's, so a node over the cut is cut off at its own as well.
pub fn floor<H: Held>(held: &mut H, o: [i32; 3], detail: Detail, n: &Node) -> i32 {
    let (lo, hi) = bounds(n, grid_of(o));
    let (side, e) = (hi[0] - lo[0], detail.eye);
    let r = reach(
        [lo[0] - side - e[0], lo[1] - side - e[1], lo[2] - e[2]],
        [hi[0] + side - e[0], hi[1] + side - e[1], hi[2] - e[2]],
    );
    let (coarse, fine) = (detail.at(r[0]), detail.range[1]);
    let c = n.index(o);
    let mut low = UNBOUNDED[1];
    let mut k = 0u32;
    while k < 9 {
        let m = [
            wrap_index(n.zoom, c[0] + (k % 3) as i32 - 1),
            c[1] + (k / 3) as i32 - 1,
            c[2],
        ];
        let mut place = Found::NONE;
        let ([g, _], ok) = held.relief(&mut place, [coarse, fine], (n.zoom, m));
        // Not held, or bounding nothing: no floor is known.
        if !ok || g[0] > g[1] {
            return UNBOUNDED[0];
        }
        low = if g[0] < low { g[0] } else { low };
        k += 1;
    }
    face_height(coarse, blocks(coarse, low as f32) - 2 * leaf_blocks(coarse))
}

/// Whether the eye at `o` is in the open: its leaf cell held, and no ground
/// there, or the eye at its face or over it. Else the proxy draws nothing,
/// and every ray starts at the eye (`floor`).
pub fn aired(w: &(impl Squares + Area), o: [i32; 3]) -> bool {
    let b = [wrap_index(LEAF_CELL, o[0] >> FRAC_W), o[1] >> FRAC_W];
    let (cell, f) = crate::window::ground(w, Found::NONE, LEAF_CELL, b);
    f.here() && (!cell.grounded() || o[2] >> FRAC_W >= cell.top)
}

/// The node `n` from the origin `o` as the proxy draws it, `i` the world's
/// answer for it at the zooms it is asked at, `node` its planes and `cut`
/// its cut's zoom (`asked`), and `floor` its floor (`floor`): a
/// block that fills, the box it is drawn as (`cast::kept_bounds`); one that
/// does not, or air coarser than asked, nothing; a split, the node clipped
/// along z to its slab and cut off at `floor`, nothing where that is empty,
/// a box at the cut's zoom or finer and `UNDER` coarser; not held, the node
/// whole, since the cast stops at a lack.
pub fn of(o: [i32; 3], n: &Node, node: Proxy, cut: Zoom, i: In<[u32; 2]>, floor: i32) -> Proxy {
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
            let boxed = n.zoom >= cut;
            let bottom = if floor > bottom { floor } else { bottom };
            let lo = greater(node.lo[2], level(bottom, og, r0));
            let hi = lesser(node.hi[2], level(top, og, r0));
            if lo >= hi {
                return none;
            }
            Proxy {
                says: if boxed { BOX } else { UNDER },
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
