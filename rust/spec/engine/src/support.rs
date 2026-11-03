// pxl-engine/support.rs

//! gpu-portable: where something stands, asked of the world's one query
//! (pxl-kernel domain/query/support.rs, a walker's support): a column's
//! column surface, the first cell a ray down it from the world's top cannot see
//! through at a cell zoom, its top as drawn (`column_surface`); and the query's
//! cursor (ARCHITECTURE.md "The query, the only seam"): a descent's line
//! (`Line`), a column_surface's or one straight down to a node (`line`), and a
//! asset about it asked from the deepest node of that line holding it and
//! stepped down by what each parent hands its child (`asset`). The CPU's and
//! a GPU's alike, each over its own reading of
//! the world (`cast::Query`).

use crate::cast::{self, Detail, End, Keep, Node, Query, Ray, Trace, Until, DEPTH};
use crate::{Zoom, FINEST_ZOOM, FRAC_W, HEIGHT_BITS};

/// A descent's line: the node it ended at, by its index at its zoom, what
/// was handed it, and what was handed each node over it, by zoom.
#[derive(Clone, Copy)]
pub struct Line<A> {
    at: [i32; 3],
    zoom: Zoom,
    live: A,
    stack: [A; DEPTH],
}

/// A column's surface: its top as drawn at its cell zoom (`cast::top`),
/// how the ray down ended — met (`End::Hit`), nothing it cannot see
/// through (`Miss`) or a node not held (`Lacks`) — and the line it
/// descended.
#[derive(Clone, Copy)]
pub struct ColumnSurface<A> {
    pub top: (i32, f32),
    pub ends: End,
    pub line: Line<A>,
    origin: [i32; 3],
}

impl<A: Copy> Line<A> {
    /// The line a walk from `origin` left where it ended at `node`
    /// (`cast::Keep`): what was handed it, `live`, and what was handed each
    /// node over it, by zoom.
    pub fn of_walk(origin: [i32; 3], node: &Node, live: A, stack: &[A; DEPTH]) -> Line<A> {
        Line {
            at: node.idx(origin),
            zoom: node.zoom,
            live,
            stack: *stack,
        }
    }
}

impl<A: Copy> ColumnSurface<A> {
    /// A column surface whose assets are asked off `line` alone (`asset`),
    /// as a pass that did not walk the column holds it: the line straight
    /// down to its top cell (`line`), which is the one its walk kept
    /// (`column_surface_line_is_its_top_cells`).
    pub fn of_line(line: Line<A>) -> ColumnSurface<A> {
        ColumnSurface {
            top: (0, 0.0),
            ends: End::Hit,
            line,
            origin: [0; 3],
        }
    }
}

impl<A: Copy> Keep<A> for ColumnSurface<A> {
    fn keep(&mut self, met: &Trace<A>, stack: &[A; DEPTH]) {
        self.line = Line::of_walk(self.origin, &met.hit.node, met.hit.at, stack);
    }
}

/// The column surface down column `g` at `cell`: the first cell that fills on the
/// world's one ray straight down the column's middle, at the cells of `cell`
/// alone, from the top of the world (`HEIGHT_BITS`), whichever layer holds
/// it; its line kept for the assets asked about it (`asset`).
pub fn column_surface<Q: Query>(q: &mut Q, cell: Zoom, g: [i32; 2]) -> ColumnSurface<Q::At> {
    let (o, ray, detail) = down(cell, g);
    let at = Node::ROOT.idx(o);
    let root = q.root(at);
    let mut f = ColumnSurface {
        top: (0, 0.0),
        ends: End::Miss,
        line: Line {
            at,
            zoom: 0,
            live: root,
            stack: [root; DEPTH],
        },
        origin: o,
    };
    // From the root just asked: `walk_from_root` would ask it again.
    let met = cast::walk(
        q,
        o,
        &ray,
        detail,
        Until::Opaque,
        (Node::ROOT, root, 0.0),
        &cast::Root(root),
        &mut f,
    );
    f.ends = met.ends;
    f.top = cast::top(o, &met.hit.node, met.hit.asset[0]);
    f
}

/// The ray down column `g` at `cell`: from the top of the world over the
/// column's middle, straight down, at the cells of `cell` alone.
fn down(cell: Zoom, g: [i32; 2]) -> ([i32; 3], Ray, Detail) {
    let s = FINEST_ZOOM - cell;
    let mid = |v: i32| v.wrapping_shl(s).wrapping_add((1 << s) >> 1);
    let top = ((1i32 << (HEIGHT_BITS - 1)) << FRAC_W) - 1;
    let o = [mid(g[0]), mid(g[1]), top];
    let ray = Ray::new([0.0, 0.0, -1.0], (1u32 << HEIGHT_BITS) as f32);
    (
        o,
        ray,
        Detail {
            range: [cell, cell],
            spacing: cast::Spacing::NONE,
        },
    )
}

/// The node over node `c` of `zoom` at `z`, `z` no finer.
fn over(c: [i32; 3], zoom: Zoom, z: Zoom) -> [i32; 3] {
    let s = zoom - z;
    [c[0] >> s, c[1] >> s, c[2] >> s]
}

/// Whether two nodes are one. [rust-gpu] By its words: an array's `==` is
/// an intrinsic it does not lower.
fn same(a: [i32; 3], b: [i32; 3]) -> bool {
    a[0] == b[0] && a[1] == b[1] && a[2] == b[2]
}

/// A line as a reader of the assets about its node reads it (`asset`), in
/// place: its node, by its index at its zoom, and what was handed it and
/// each node over it, by zoom. A descent's own (`Line`), or one a reader
/// holds elsewhere, read where it lies (`JoinedLine`; pxl-shader a beam ray's).
pub trait Ancestors<A: Copy> {
    fn at(&self) -> [i32; 3];
    fn zoom(&self) -> Zoom;
    /// What was handed its node over its own at `z`, its own at its zoom.
    fn ancestor(&self, z: Zoom) -> A;
}

impl<A: Copy> Ancestors<A> for Line<A> {
    fn at(&self) -> [i32; 3] {
        self.at
    }

    fn zoom(&self) -> Zoom {
        self.zoom
    }

    fn ancestor(&self, z: Zoom) -> A {
        if z == self.zoom {
            self.live
        } else {
            self.stack[z as usize]
        }
    }
}

/// A line down to a node off another (`joined`): its own descent from `low`,
/// the zoom of the deepest node of the other over it, down.
pub struct SubLine<A> {
    low: Zoom,
    line: Line<A>,
}

/// A line down to a node read with the line it was descended off
/// (`SubLine::joined`): over `low`, the other's, read where it lies; from `low`
/// down, its own descent's. [rust-gpu] Two references and no array of its
/// own: naga refuses a pointer in a value a shader keeps whole, as one with
/// an array is kept.
pub struct JoinedLine<'a, L, A> {
    base: &'a L,
    sub: &'a SubLine<A>,
}

impl<A: Copy> SubLine<A> {
    /// It read with `base`, the line it was descended off.
    pub fn joined<'a, L: Ancestors<A>>(&'a self, base: &'a L) -> JoinedLine<'a, L, A> {
        JoinedLine { base, sub: self }
    }
}

impl<A: Copy, L: Ancestors<A>> Ancestors<A> for JoinedLine<'_, L, A> {
    fn at(&self) -> [i32; 3] {
        self.sub.line.at
    }

    fn zoom(&self) -> Zoom {
        self.sub.line.zoom
    }

    fn ancestor(&self, z: Zoom) -> A {
        if z < self.sub.low {
            self.base.ancestor(z)
        } else {
            self.sub.line.ancestor(z)
        }
    }
}

/// The deepest zoom at which the line `l` and node `c` of `zoom` share a
/// node, no finer than either, and whether they share one at all: none
/// where they lie in two roots.
fn shared_zoom<A: Copy>(l: &impl Ancestors<A>, zoom: Zoom, c: [i32; 3]) -> (Zoom, bool) {
    let (line, lz) = (l.at(), l.zoom());
    let mut z = if zoom < lz { zoom } else { lz };
    while z > 0 && !same(over(line, lz, z), over(c, zoom, z)) {
        z -= 1;
    }
    (z, same(over(line, lz, z), over(c, zoom, z)))
}

/// Down to node `c` of `zoom` from its node at `z`, handed `at`, each node
/// handed down by its parent (`Query::child`): what was handed `c`, and
/// each node from `z` down to it, by zoom, in `stack`.
fn descend<Q: Query>(
    q: &mut Q,
    mut at: Q::At,
    mut z: Zoom,
    zoom: Zoom,
    c: [i32; 3],
    stack: &mut [Q::At; DEPTH],
) -> Q::At {
    while z < zoom {
        stack[z as usize] = at;
        z += 1;
        at = q.child(&at, z, over(c, zoom, z));
    }
    at
}

/// The line straight down to node `c` of `zoom` from its root, each node
/// handed down by its parent: where a reader of the assets about a node
/// asks them from (`asset`).
pub fn line<Q: Query>(q: &mut Q, zoom: Zoom, c: [i32; 3]) -> Line<Q::At> {
    let root = q.root(over(c, zoom, 0));
    let mut stack = [root; DEPTH];
    let live = descend(q, root, 0, zoom, c, &mut stack);
    Line {
        at: c,
        zoom,
        live,
        stack,
    }
}

/// The line down to node `c` of `zoom` off the line `base`, its own part
/// (`SubLine`, read with `base` by `SubLine::joined`): from the deepest node of
/// `base` over it, each node under it handed down by its parent; from its
/// own root where no node of `base` is over it. What `line` asks from the
/// top, asked from where `base` already holds it.
pub fn sub_line<Q: Query>(
    q: &mut Q,
    base: &impl Ancestors<Q::At>,
    zoom: Zoom,
    c: [i32; 3],
) -> SubLine<Q::At> {
    let (z, held) = shared_zoom(base, zoom, c);
    let (from, low) = if held {
        (base.ancestor(z), z)
    } else {
        (q.root(over(c, zoom, 0)), 0)
    };
    let mut stack = [from; DEPTH];
    let live = descend(q, from, low, zoom, c, &mut stack);
    SubLine {
        low,
        line: Line {
            at: c,
            zoom,
            live,
            stack,
        },
    }
}

/// What is in the cell `c` of `zoom` about the line `l`, as the world's
/// one query answers it at that zoom (`Query::node`'s first word): asked
/// from the deepest node of the line holding it, each node under that
/// handed down by its parent (`Query::child`), and from its own root where
/// no node of the line holds it.
pub fn asset<Q: Query>(q: &mut Q, l: &impl Ancestors<Q::At>, zoom: Zoom, c: [i32; 3]) -> u32 {
    let (mut z, held) = shared_zoom(l, zoom, c);
    let mut at = if held {
        l.ancestor(z)
    } else {
        z = 0;
        q.root(over(c, zoom, 0))
    };
    while z < zoom {
        z += 1;
        at = q.child(&at, z, over(c, zoom, z));
    }
    q.node(&mut at, zoom, c, [zoom, zoom], 0.0).asset[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::answer::{In, Kind, AIR, BOX_WHOLE};
    use crate::content::Sample;
    use crate::UNIT_ZOOM;

    /// Ground of stone up to `height` unit cells, two over the cells west of
    /// x = 4, none over the rest; air over it. Each node's `At` is the node
    /// it was handed for, checked where it is asked, so an asset asked off
    /// a node not over it is caught.
    struct Step;

    fn height(x: i32) -> i32 {
        if x < 4 {
            2
        } else {
            0
        }
    }

    const STONE: [u32; 2] = [Sample::GROUND, 0];

    impl Query for Step {
        type At = (Zoom, [i32; 3]);
        fn root(&mut self, node: [i32; 3]) -> Self::At {
            (0, node)
        }
        fn child(&mut self, at: &Self::At, zoom: Zoom, node: [i32; 3]) -> Self::At {
            let up = [node[0] >> 1, node[1] >> 1, node[2] >> 1];
            assert_eq!((at.0 + 1, at.1), (zoom, up), "a child of another node");
            (zoom, node)
        }
        fn node(
            &mut self,
            at: &mut Self::At,
            zoom: Zoom,
            node: [i32; 3],
            _: [Zoom; 2],
            _: f32,
        ) -> In<[u32; 2]> {
            assert_eq!(*at, (zoom, node), "asked off another node");
            let s = UNIT_ZOOM - zoom;
            if zoom < UNIT_ZOOM {
                return if node[2] << s < 2 {
                    In {
                        kind: Kind::Split,
                        asset: BOX_WHOLE,
                    }
                } else {
                    In {
                        kind: Kind::Cell,
                        asset: AIR,
                    }
                };
            }
            In {
                kind: Kind::Cell,
                asset: if node[2] < height(node[0]) {
                    STONE
                } else {
                    AIR
                },
            }
        }
    }

    /// The column surface stands on the step's top, and every asset about it,
    /// asked off its line, is what the world holds there; one in another
    /// root is asked from that root.
    #[test]
    fn footing_asset_is_worlds() {
        let f = column_surface(&mut Step, UNIT_ZOOM, [6, 3]);
        assert_eq!(f.ends, End::Hit);
        assert_eq!(f.top.0, -1, "the top of the cell under z = 0");
        for x in 2..10 {
            for y in 0..7 {
                for z in -2..4 {
                    let want = if z < height(x) { STONE[0] } else { AIR[0] };
                    let c = [x, y, z];
                    assert_eq!(asset(&mut Step, &f.line, UNIT_ZOOM, c), want, "{c:?}");
                }
            }
        }
        let far = [(3 << UNIT_ZOOM) + 6, 3, -1];
        assert_eq!(asset(&mut Step, &f.line, UNIT_ZOOM, far), STONE[0]);
    }

    /// The line a column surface keeps is the line straight down to its top
    /// cell, its column's at its zoom: what a pass that did not walk the
    /// column asks its assets off (`ColumnSurface::of_line`), the same answers.
    #[test]
    fn column_surface_line_is_its_top_cells() {
        for g in [[6, 3], [2, 3], [3, 0], [9, 5]] {
            let f = column_surface(&mut Step, UNIT_ZOOM, g);
            assert_eq!(f.ends, End::Hit, "{g:?}");
            let rebuilt = line(&mut Step, UNIT_ZOOM, [g[0], g[1], f.top.0]);
            same(&f.line, &rebuilt);
            for c in [
                [g[0] + 1, g[1], f.top.0],
                [g[0] - 1, g[1], f.top.0 + 1],
                [g[0], g[1] + 2, 0],
            ] {
                assert_eq!(
                    asset(&mut Step, &f.line, UNIT_ZOOM, c),
                    asset(
                        &mut Step,
                        &ColumnSurface::of_line(rebuilt).line,
                        UNIT_ZOOM,
                        c
                    ),
                    "{g:?} {c:?}"
                );
            }
        }
    }

    type At = (Zoom, [i32; 3]);

    /// A walk's line where it ends (`cast::Keep`).
    struct LineCollector(Option<Line<At>>, [i32; 3]);

    impl Keep<At> for LineCollector {
        fn keep(&mut self, met: &Trace<At>, stack: &[At; DEPTH]) {
            self.0 = Some(Line::of_walk(self.1, &met.hit.node, met.hit.at, stack));
        }
    }

    /// What was handed each node of a kept line, as a beam ray's resumed
    /// walk reads it (`cast::Over`).
    struct Along<'a>(&'a Line<At>);

    impl cast::Over<At> for Along<'_> {
        fn over(&self, zoom: Zoom) -> At {
            if zoom == self.0.zoom {
                self.0.live
            } else {
                self.0.stack[zoom as usize]
            }
        }
    }

    /// The same node, handed the same down every zoom to it.
    fn same(a: &Line<At>, b: &Line<At>) {
        assert_eq!((a.at, a.zoom, a.live), (b.at, b.zoom, b.live));
        assert_eq!(a.stack[..a.zoom as usize], b.stack[..b.zoom as usize]);
    }

    /// The line a walk keeps where it stops is the one straight down to its
    /// stop, whether it walked from the root or resumed under another
    /// ray's line and climbed over where it resumed; the assets about the
    /// stop asked off it are the world's.
    #[test]
    fn walk_keeps_line() {
        let s = FINEST_ZOOM - UNIT_ZOOM;
        let o = [(6 << s) + 17, (3 << s) + 90, 9 << s];
        let detail = Detail {
            range: [UNIT_ZOOM, UNIT_ZOOM],
            spacing: cast::Spacing::NONE,
        };
        let beam = Ray::new([-0.45, 0.05, -1.0], 64.0);
        let mut first = LineCollector(None, o);
        let met = cast::walk_from_root(&mut Step, o, &beam, detail, Until::Opaque, &mut first);
        assert_eq!(met.ends, End::Hit);
        let first = first.0.unwrap();
        same(&first, &line(&mut Step, first.zoom, first.at));
        for d in [
            [-0.45, 0.05],
            [-0.5, 0.0],
            [-0.4, 0.12],
            [-0.7, -0.1],
            [-0.2, 0.3],
            [-0.45, 0.9],
            [-0.1, 2.5],
            [0.3, 0.05],
        ] {
            for share in [0.2, 0.5, 0.9] {
                let ray = Ray::new([d[0], d[1], -1.0], 64.0);
                let t = met.hit.t * share;
                let (h, held) = met.hit.node.over(cast::point_at(o, &ray, t));
                let h = cast::holding(o, &ray, h, t);
                let from = if held {
                    (h, cast::Over::over(&Along(&first), h.zoom), t)
                } else {
                    (Node::ROOT, Step.root(Node::ROOT.idx(o)), t)
                };
                let mut kept = LineCollector(None, o);
                let resumed = cast::walk(
                    &mut Step,
                    o,
                    &ray,
                    detail,
                    Until::Opaque,
                    from,
                    &Along(&first),
                    &mut kept,
                );
                assert_eq!(resumed.ends, End::Hit, "{d:?} {share}");
                let kept = kept.0.unwrap();
                let walked = line(&mut Step, kept.zoom, kept.at);
                same(&kept, &walked);
                for d in 0..6u32 {
                    let mut c = kept.at;
                    c[(d / 2) as usize] += if d % 2 == 0 { -1 } else { 1 };
                    assert_eq!(
                        asset(&mut Step, &kept, UNIT_ZOOM, c),
                        asset(&mut Step, &walked, UNIT_ZOOM, c),
                        "{c:?}"
                    );
                }
            }
        }
    }

    /// A line straight down to a node holds the assets about it as the
    /// world does, its own among them.
    #[test]
    fn line_asset_is_worlds() {
        let at = [3, 2, 1];
        let l = line(&mut Step, UNIT_ZOOM, at);
        for x in 1..6 {
            for y in 0..5 {
                for z in -1..4 {
                    let want = if z < height(x) { STONE[0] } else { AIR[0] };
                    let c = [x, y, z];
                    assert_eq!(asset(&mut Step, &l, UNIT_ZOOM, c), want, "{c:?}");
                }
            }
        }
    }
}
