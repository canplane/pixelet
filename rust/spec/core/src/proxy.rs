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

use crate::answer::ANY;
use crate::answer::{fills, slab_of, In, Kind};
use crate::area::Area;
use crate::cast::{
    asked as zooms, bounds, greater, grid_of, kept_bounds, lesser, level, reach, roots_of, under,
    Detail, Node,
};
use crate::ground::{blocks, face_height, leaf_blocks, Held, UNBOUNDED};
use crate::window::{squares_zoom, Found, Squares, ACROSS, FIRST_CELL};
use crate::{axis, wrap_index, Zoom, FINEST_ZOOM, FRAC_W, LEAF_CELL, SHARD_ZOOM, TOP_ZOOM};

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
/// leaf cells: a block under the lowest face about the node, the four
/// nodes a zoom coarser that cover it and the eight about it
/// (`Held::relief`), read from the coarsest zoom the cast may draw a block
/// in those nine at to the finest it asks anywhere, rounded at that
/// coarsest as the ground rounds a face (`ground::top`). Under it a block
/// lies wholly under every face about it, at every zoom it may be asked at.
/// The world's bottom where any of the four is not held, or for a root.
///
/// No block meeting the nine is coarser than the camera grid's zoom at
/// their farthest corner (pxl-spec-core cast.rs `walk`), and a coarser
/// zoom's floor is no higher than a finer's, so the coarsest's stands for
/// all of them; the four hold more than the nine, which only lowers it.
/// Read to the finest, a node's floor is no higher than its children's, so
/// a node over the cut is cut off at its own as well.
pub fn floor<H: Held>(held: &mut H, o: [i32; 3], detail: Detail, n: &Node) -> i32 {
    if n.zoom == 0 {
        return UNBOUNDED[0];
    }
    let (lo, hi) = bounds(n, grid_of(o));
    let (side, e) = (hi[0] - lo[0], detail.eye);
    let r = reach(
        [lo[0] - side - e[0], lo[1] - side - e[1], lo[2] - e[2]],
        [hi[0] + side - e[0], hi[1] + side - e[1], hi[2] - e[2]],
    );
    let (coarse, fine) = (detail.at(r[0]), detail.range[1]);
    let (c, z) = (n.index(o), n.zoom - 1);
    // The nine's two parents a side: c − 1 and c + 1 halved differ by one.
    let first = [(c[0] - 1) >> 1, (c[1] - 1) >> 1];
    let mut low = UNBOUNDED[1];
    let mut k = 0u32;
    while k < 4 {
        let m = [
            wrap_index(z, first[0] + (k % 2) as i32),
            first[1] + (k / 2) as i32,
            c[2] >> 1,
        ];
        let mut place = Found::NONE;
        let ([g, _], ok) = held.relief(&mut place, [coarse, fine], (z, m));
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

/// The node at `zoom` whose world index is `c`, as a walk from the origin
/// `o` holds it (`cast::Node`): X the short way round from the origin's,
/// and its roots from the origin's root. `Node::index`'s inverse.
pub fn node_at(o: [i32; 3], zoom: Zoom, c: [i32; 3]) -> Node {
    let r0 = roots_of(o);
    if zoom == 0 {
        return Node {
            corner: [0; 3],
            zoom,
            roots: [c[0] - r0[0], c[1] - r0[1], c[2] - r0[2]],
        };
    }
    let s = FINEST_ZOOM - zoom;
    let on = |a: usize, v: i32| {
        let own = axis(o, a as u32) >> s;
        let i = if a == 0 {
            own.wrapping_add(wrap_index(zoom, v.wrapping_sub(own)))
        } else {
            v
        };
        ((i as u32).wrapping_shl(s), (i >> zoom) - axis(r0, a as u32))
    };
    // [naga] each axis by itself, not a loop (caves.rs `node_seed`).
    let (x, y, z) = (on(0, c[0]), on(1, c[1]), on(2, c[2]));
    Node {
        corner: [x.0, y.0, z.0],
        zoom,
        roots: [x.1, y.1, z.1],
    }
}

/// The node over `n` at `zoom`, no finer than its own.
pub fn ancestor(n: &Node, zoom: Zoom) -> Node {
    let keep = !under(zoom);
    Node {
        corner: [n.corner[0] & keep, n.corner[1] & keep, n.corner[2] & keep],
        zoom,
        roots: n.roots,
    }
}

/// The ground the cut is built within: the eye's coarsest window, its first
/// square, counted at its squares' zoom, and whether it is laid out
/// (`Squares::origin`). Past it no window holds the ground, so the cast
/// lacks there, and a ray starts no later than where it leaves it
/// (`coverage`).
pub fn covered(w: &impl Squares) -> ([i32; 2], bool) {
    w.origin(FIRST_CELL)
}

/// The window's planes from the origin `o`, its first square `first`
/// (`covered`): its squares across, the world's whole height along z.
pub fn coverage(o: [i32; 3], first: [i32; 2]) -> Proxy {
    let (z, og, a) = (squares_zoom(FIRST_CELL), grid_of(o), ACROSS as i32 - 1);
    let (lo, _) = bounds(&node_at(o, z, [first[0], first[1], 0]), og);
    let (_, hi) = bounds(&node_at(o, z, [first[0] + a, first[1] + a, 0]), og);
    Proxy {
        says: BOX,
        lo: [lo[0], lo[1], level(ANY[0], og[2], roots_of(o)[2])],
        hi: [hi[0], hi[1], level(ANY[1], og[2], roots_of(o)[2])],
    }
}

/// The cut as a GPU builds it, a round a dispatch from the seeds over the
/// window it is built within (`covered`; pxl-shader proxy.rs `proxy_cs`,
/// `proxy_args_cs`), and the frame's header a reader of its boxes reads:
/// its counts, a word each, the boxes written, the nodes asked, the round
/// the next dispatch reads, each queue's nodes (two words, the round read
/// and the round written alternating), what was dropped past the scratch's
/// room, `coarser`, whether the eye was found not in the open (`aired`),
/// the boxes dropped of those dropped, the observed pyramid's frame the
/// boxes' entries were read at (`observed::FRAME`, the reader's); then the
/// host's: the window's planes (`coverage`), lowest then highest, and a
/// drawn pixel's angle (`inflated`), as f32 bits; then the boxes about the
/// eye (`eyed`), how many and, up to `EYES`, which. A frame that dropped
/// anything, or whose eye is not in the open, has no proxy: its rays start
/// at the eye.
pub const COUNT_BOXES: u32 = 0;
pub const COUNT_NODES: u32 = 1;
pub const COUNT_LEVEL: u32 = 2;
pub const COUNT_QUEUE: u32 = 3;
pub const COUNT_DROPPED: u32 = 5;
pub const COUNT_COARSER: u32 = 6;
pub const COUNT_SHUT: u32 = 7;
pub const COUNT_DROPPED_BOXES: u32 = 8;
pub const COUNT_FRAME: u32 = 9;
pub const COUNT_COVER: u32 = 10;
pub const COUNT_PIXEL: u32 = 16;
pub const COUNT_EYES: u32 = 17;
pub const COUNT_EYE: u32 = 18;
pub const EYES: u32 = 8;
pub const COUNTS: u32 = 32;
const _: () = assert!(COUNT_EYE + EYES <= COUNTS);

/// The zoom the cut starts at, a node the world's height tall: none
/// coarser is ever a box (`cut`).
pub const SEED_ZOOM: Zoom = SHARD_ZOOM;
const SEED_SHIFT: u32 = SEED_ZOOM - squares_zoom(FIRST_CELL);
const SEEDS_ACROSS: u32 = ACROSS << SEED_SHIFT;
/// The seeds: every node at `SEED_ZOOM` over the window, the two along z
/// either side of sea level the world's height spans.
pub const SEEDS: u32 = SEEDS_ACROSS * SEEDS_ACROSS * 2;
const _: () = assert!(
    SEED_ZOOM >= squares_zoom(FIRST_CELL) && crate::HEIGHT_BITS + SEED_ZOOM == crate::UNIT_ZOOM,
    "a seed is a window square's or less across and the world's height tall"
);

/// Seed `k` of the window whose first square is `first`, from the origin `o`.
pub fn seed(o: [i32; 3], first: [i32; 2], k: u32) -> Node {
    let a = SEEDS_ACROSS;
    node_at(
        o,
        SEED_ZOOM,
        [
            (first[0] << SEED_SHIFT) + (k % a) as i32,
            (first[1] << SEED_SHIFT) + (k / a % a) as i32,
            (k / (a * a)) as i32 - 1,
        ],
    )
}

/// The rounds the cut is built in, a dispatch each: as many as the zooms
/// from the seeds' to the finest it draws at (`cut`), as a round descends a
/// node by a zoom at least (`steps`).
pub const LEVELS: u32 = LEAF_CELL - SEED_ZOOM;

/// A `proxy_cs` workgroup's threads.
pub const WORKGROUP: u32 = 64;

/// A queued node's words (`node_words`), the nodes a queue holds, the
/// boxes the scratch holds and a box's words: its node's (`node_words`);
/// its planes in units from the eye, lowest then highest, as f32 bits
/// (`of`); and its node's entry in the observed pyramid, `observed::NONE`
/// where the pyramid holds none, read once a box (`COUNT_FRAME`).
pub const NODE_WORDS: u32 = 4;
pub const QUEUE_NODES: u32 = 1 << 18;
pub const BOXES: u32 = 1 << 19;
pub const BOX_WORDS: u32 = 12;
pub const BOX_LO: u32 = NODE_WORDS;
pub const BOX_HI: u32 = BOX_LO + 3;
pub const BOX_ENTRY: u32 = BOX_HI + 3;
/// A box's last word: `EYED` where it is about the eye (`eyed`), which the
/// raster leaves out.
pub const BOX_FLAGS: u32 = BOX_ENTRY + 1;
pub const EYED: u32 = 1;
/// The scratch: the two queues, then the boxes.
pub const DATA_WORDS: u32 = 2 * QUEUE_NODES * NODE_WORDS + BOXES * BOX_WORDS;

/// The indirect args the cut leaves: the next round's dispatch, then the
/// boxes' draw, a box's corners an instance (`BOX_VERTICES`).
pub const ARGS_DRAW: u32 = 3;
pub const ARGS_WORDS: u32 = 8;
pub const BOX_VERTICES: u32 = 36;

/// Where node `k` of queue `q` begins in the scratch.
pub const fn queue_at(q: u32, k: u32) -> u32 {
    (q * QUEUE_NODES + k) * NODE_WORDS
}

/// Where box `k` begins in the scratch.
pub const fn box_at(k: u32) -> u32 {
    2 * QUEUE_NODES * NODE_WORDS + k * BOX_WORDS
}

/// A node as a queue holds it: its corner, then its zoom and its roots,
/// each one more than the root from the origin's, two bits an axis.
pub fn node_words(n: &Node) -> [u32; NODE_WORDS as usize] {
    let r = |a: usize| ((n.roots[a] + 1) as u32 & 3) << (8 + 2 * a as u32);
    [
        n.corner[0],
        n.corner[1],
        n.corner[2],
        n.zoom | r(0) | r(1) | r(2),
    ]
}

/// `node_words`'s node.
pub fn node_of(w: [u32; NODE_WORDS as usize]) -> Node {
    let r = |a: u32| (w[3] >> (8 + 2 * a) & 3) as i32 - 1;
    Node {
        corner: [w[0], w[1], w[2]],
        zoom: w[3] & 0xff,
        roots: [r(0), r(1), r(2)],
    }
}

/// Whether the eye's `right`, `up` and `forward` turn the other way from
/// the one `corner` winds a front face for: right × up along forward.
pub fn flipped(right: [f32; 3], up: [f32; 3], forward: [f32; 3]) -> bool {
    let x = [
        right[1] * up[2] - right[2] * up[1],
        right[2] * up[0] - right[0] * up[2],
        right[0] * up[1] - right[1] * up[0],
    ];
    x[0] * forward[0] + x[1] * forward[1] + x[2] * forward[2] > 0.0
}

/// The most zooms a node over the cut is descended by in one of the GPU's
/// rounds (`steps`): its descendants that many zooms finer queued at once.
/// One: three, a third of the rounds, asked three times the nodes, the
/// zooms between no longer pruning by their own slabs, and the pass took
/// half again as long (c6b3d4991, on the fixed path: 122k nodes and 15.6 ms
/// against 41.8k and 10.4).
pub const SKIP: Zoom = 1;

/// The zooms the node `n` from the origin `o` may be descended by at once,
/// one to `SKIP`: to the cut's zoom at its farthest point (`cut`), which no
/// node under it is coarser than, so no node between is a box, and the
/// boxes are the cut's whichever way it is descended.
pub fn steps(o: [i32; 3], detail: Detail, coarser: Zoom, n: &Node) -> Zoom {
    let (lo, hi) = bounds(n, grid_of(o));
    let e = detail.eye;
    let r = reach(
        [lo[0] - e[0], lo[1] - e[1], lo[2] - e[2]],
        [hi[0] - e[0], hi[1] - e[1], hi[2] - e[2]],
    );
    let far = cut(detail, coarser, r[0]);
    if far >= n.zoom + SKIP {
        SKIP
    } else if far > n.zoom + 1 {
        far - n.zoom
    } else {
        1
    }
}

/// Descendant `i` of the node `n`, `k` zooms finer, of its `8^k`: bit `3j`
/// X's, `3j + 1` Y's and `3j + 2` Z's of the `j`-th zoom down from `n`'s
/// finest, as `child` reads `i`'s low three.
pub fn descendant(n: &Node, k: Zoom, i: u32) -> Node {
    let mut d = *n;
    let mut j = 0;
    while j < k {
        d = child(&d, i >> (3 * (k - 1 - j)) & 7);
        j += 1;
    }
    d
}

/// A box's corner `k` of the raster's 36 (proxy_raster.wgsl, where build.rs
/// writes them): two triangles a face, the faces in their order across X,
/// then Y, then Z, low side first, each wound counter-clockwise seen from
/// outside, so a face turned to the eye is wound counter-clockwise on the
/// screen (`a_face_turned_to_the_eye_is_wound_counter_clockwise`) and the
/// raster culls the faces turned away; each triangle wound the other way
/// where `flipped`, an eye whose right, up and forward turn the other way
/// (`flipped`).
pub fn corner(k: u32, flipped: bool) -> [u32; 3] {
    let face = k / 6;
    // A triangle's second and third corners swapped turn it the other way.
    let j = match (k % 6, flipped) {
        (1, true) => 2,
        (2, true) => 1,
        (4, true) => 5,
        (5, true) => 4,
        (j, _) => j,
    };
    let t = match j {
        0 | 3 => 0,
        1 => 1,
        2 | 4 => 2,
        _ => 3,
    };
    let (a, high) = (face / 2, face % 2);
    // The quad's corners in (u, v), u along the next axis and v the one
    // after it: counter-clockwise about the face's outward normal on the
    // high side, clockwise on the low, whose normal is the other way.
    let (u, v) = match t {
        0 => (0, 0),
        1 => (1, 0),
        2 => (1, 1),
        _ => (0, 1),
    };
    let (u, v) = if high == 1 { (u, v) } else { (v, u) };
    let mut c = [0u32; 3];
    c[a as usize] = high;
    c[((a + 1) % 3) as usize] = u;
    c[((a + 2) % 3) as usize] = v;
    c
}

/// The raster's reversed depth, a box's face at view depth `w` drawn at
/// `NEAR · (1 + BIAS) / w`: nearer is greater, and every face drawn a share
/// `BIAS` nearer than it lies, far past what an f32 depth or a pixel's
/// interpolation rounds by (2⁻²⁴), so the depth a pixel keeps, read back
/// (`bound`), is never past the front of the box it picked nor of any box
/// its ray enters before it. `NEAR` is a Q3 step: nothing nearer is drawn,
/// so a box that near is the eye's (`eyed`).
pub const NEAR: f32 = 1.0 / (1 << FRAC_W) as f32;
pub const BIAS: f32 = 1.0 / 4096.0;

/// Drawn pixels a box is grown by on every side, at its nearest point's
/// distance, `pixel` the angle a drawn pixel spans: so a ray through a
/// pixel's middle that enters the box, or passes within its rounding of an
/// edge, lands in it as drawn, and two boxes that share a face leave no
/// crack. Growing a box only brings its front nearer.
pub const GROWN: f32 = 2.0;

/// `p` grown by `GROWN` drawn pixels of `pixel` radians at its nearest
/// point from the eye.
pub fn inflated(p: &Proxy, pixel: f32) -> Proxy {
    let near = |a: usize| {
        let v = if p.lo[a] > 0.0 {
            p.lo[a]
        } else if p.hi[a] < 0.0 {
            p.hi[a]
        } else {
            0.0
        };
        v * v
    };
    let d = libm::sqrtf(near(0) + near(1) + near(2));
    let m = GROWN * pixel * d + NEAR;
    Proxy {
        says: p.says,
        lo: [p.lo[0] - m, p.lo[1] - m, p.lo[2] - m],
        hi: [p.hi[0] + m, p.hi[1] + m, p.hi[2] + m],
    }
}

/// Whether the grown box `g` (`inflated`) is about the eye: its nearest
/// point nearer than `NEAR` past what the screen's corner `tan` (its
/// half-angles' tangents) lets a drawn face be at the near plane, so the
/// raster, which draws nothing nearer than `NEAR`, may cut it. Every ray
/// enters such a box at once; the raster leaves it out, and a reader walks
/// it from the eye (`COUNT_EYE`).
pub fn eyed(g: &Proxy, tan: [f32; 2]) -> bool {
    let near = |a: usize| {
        let v = if g.lo[a] > 0.0 {
            g.lo[a]
        } else if g.hi[a] < 0.0 {
            g.hi[a]
        } else {
            0.0
        };
        v * v
    };
    let k = 2.0 * (1.0 + BIAS) * NEAR;
    near(0) + near(1) + near(2) < k * k * (1.0 + tan[0] * tan[0] + tan[1] * tan[1])
}

/// The bound a pixel's kept depth `depth` (the raster's, reversed) gives:
/// the t along its ray, `cos` its direction's dot with the eye's forward,
/// before which the ray enters no box but those about the eye. None
/// (0) where the raster drew nothing there.
pub fn bound(depth: f32, cos: f32) -> f32 {
    if depth > 0.0 {
        NEAR / (depth * cos)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every face of a box the eye stands outside of and before, projected
    /// as the raster projects it (proxy_raster.wgsl `vs`), is wound
    /// counter-clockwise on the screen where its outward normal is turned
    /// to the eye, clockwise where it is turned away: a front face is kept.
    #[test]
    fn a_face_turned_to_the_eye_is_wound_counter_clockwise() {
        // An eye looking north, then one turned and pitched down.
        let views: [[[f32; 3]; 3]; 2] = [
            [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]],
            [
                [0.8, -0.6, 0.0],
                [0.36, 0.48, 0.8],
                [-0.48, -0.64, 0.6].map(|v: f32| -v),
            ],
        ];
        // Each mirrored too, as an eye whose axes turn the other way.
        let views = views
            .into_iter()
            .flat_map(|[r, u, f]| [[r, u, f], [r.map(|v| -v), u, f]]);
        for [r, u, f] in views {
            let flip = flipped(r, u, f);
            let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            for at in [[-0.5f32, 4.0, -0.7], [2.0, 6.0, 1.0], [-3.0, 3.0, 2.5]] {
                let p = |c: [u32; 3]| {
                    let q = [
                        at[0] + c[0] as f32,
                        at[1] + c[1] as f32,
                        at[2] + c[2] as f32,
                    ];
                    let w = dot(q, f);
                    [dot(q, r) / w, dot(q, u) / w, w]
                };
                for face in 0..6u32 {
                    let [a, b, c] = [0, 1, 2].map(|j| p(corner(face * 6 + j, flip)));
                    if a[2] <= 0.0 || b[2] <= 0.0 || c[2] <= 0.0 {
                        continue;
                    }
                    let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
                    // The face's outward normal and a point on it.
                    let axis = (face / 2) as usize;
                    let mut n = [0.0f32; 3];
                    n[axis] = if face % 2 == 1 { 1.0 } else { -1.0 };
                    let mut on = at;
                    on[axis] += (face % 2) as f32;
                    let turned = dot(n, on) < 0.0;
                    assert_eq!(area > 0.0, turned, "{at:?} face {face}, flipped {flip}");
                }
            }
        }
    }

    /// `node_at` is `Node::index`'s inverse, X across the antimeridian and
    /// the roots apart included.
    #[test]
    fn a_node_is_found_again_from_its_index() {
        let origins = [
            [0, 0, 0],
            [i32::MAX - 5, -7, 1 << 20],
            [i32::MIN + 3, 1 << 30, -9],
        ];
        for o in origins {
            for zoom in [1, 7, 8, 13, 23, 24, 30] {
                let s = FINEST_ZOOM - zoom;
                for d in [[0, 0, 0], [1, -1, 2], [-3, 4, -2]] {
                    let own = [o[0] >> s, o[1] >> s, o[2] >> s];
                    let c = [
                        wrap_index(zoom, own[0].wrapping_add(d[0])),
                        own[1] + d[1],
                        own[2] + d[2],
                    ];
                    let n = node_at(o, zoom, c);
                    assert_eq!(n.index(o), c, "{o:?} z{zoom} {d:?}");
                    assert_eq!(n.corner.map(|v| v & under(zoom)), [0; 3]);
                }
            }
        }
    }
}
