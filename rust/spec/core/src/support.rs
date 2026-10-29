// pxl-spec-core/support.rs

//! gpu-portable: where something stands, asked of the world's one query
//! (pxl-kernel domain/query/support.rs, a walker's support): a column's
//! footing, the first cell a ray down it from the world's top cannot see
//! through at a cell zoom, its top as drawn (`footing`); and the query's
//! cursor (ARCHITECTURE.md "The query, the only seam"): a descent's line
//! (`Line`), a footing's or one straight down to a node (`line`), and a
//! asset about it asked from the deepest node of that line holding it and
//! stepped down by what each parent hands its child, never again from the
//! top (`asset`). The CPU's and a GPU's alike, each over its own reading of
//! the world (`cast::Query`).

use crate::cast::{self, Detail, Ends, Keep, Met, Node, Query, Ray, Until, DEPTH};
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

/// A column's footing: its top as drawn at its cell zoom (`cast::top`),
/// how the ray down ended — met (`End::Stop`), nothing it cannot see
/// through (`On`) or a node not held (`Lacks`) — and the line it
/// descended.
#[derive(Clone, Copy)]
pub struct Footing<A> {
    pub top: (i32, f32),
    pub ends: End,
    pub line: Line<A>,
    origin: [i32; 3],
}

impl<A: Copy> Line<A> {
    /// The line a walk from `origin` left where it ended at `node`
    /// (`cast::Keep`): what was handed it, `live`, and what was handed each
    /// node over it, by zoom.
    pub fn kept(origin: [i32; 3], node: &Node, live: A, stack: &[A; DEPTH]) -> Line<A> {
        Line {
            at: node.index(origin),
            zoom: node.zoom,
            live,
            stack: *stack,
        }
    }
}

impl<A: Copy> Keep<A> for Footing<A> {
    fn keep(&mut self, met: &Met<A>, stack: &[A; DEPTH]) {
        self.line = Line::kept(self.origin, &met.stop.node, met.stop.at, stack);
    }
}

/// The footing down column `g` at `cell`: the first cell that fills on the
/// world's one ray straight down the column's middle, at the cells of `cell`
/// alone, from the top of the world (`HEIGHT_BITS`), whichever layer holds
/// it; its line kept for the assets asked about it (`asset`).
pub fn footing<Q: Query>(q: &mut Q, cell: Zoom, g: [i32; 2]) -> Footing<Q::At> {
    let (o, ray, detail) = down(cell, g);
    let at = Node::ROOT.index(o);
    let root = q.root(at);
    let mut f = Footing {
        top: (0, 0.0),
        ends: End::On,
        line: Line {
            at,
            zoom: 0,
            live: root,
            stack: [root; DEPTH],
        },
        origin: o,
    };
    let met = cast::rooted(q, o, &ray, detail, Until::Opaque, &mut f);
    f.ends = met.ends;
    f.top = cast::top(o, &met.stop.node, met.stop.asset[0]);
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
            eye: [0.0; 3],
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

/// The line straight down to node `c` of `zoom` from its root, each node
/// handed down by its parent: where a reader of the assets about a node
/// asks them from (`asset`).
pub fn line<Q: Query>(q: &mut Q, zoom: Zoom, c: [i32; 3]) -> Line<Q::At> {
    let mut at = q.root(over(c, zoom, 0));
    let mut stack = [at; DEPTH];
    let mut z = 0;
    while z < zoom {
        stack[z as usize] = at;
        z += 1;
        at = q.child(&at, z, over(c, zoom, z));
    }
    Line {
        at: c,
        zoom,
        live: at,
        stack,
    }
}

/// What is in the cell `c` of `zoom` about the line `l`, as the world's
/// one query answers it at that zoom (`Query::node`'s first word): asked
/// from the deepest node of the line holding it, each node under that
/// handed down by its parent (`Query::child`), and from its own root where
/// no node of the line holds it.
pub fn asset<Q: Query>(q: &mut Q, l: &Line<Q::At>, zoom: Zoom, c: [i32; 3]) -> u32 {
    let (line, lz) = (l.at, l.zoom);
    let mut z = if zoom < lz { zoom } else { lz };
    while z > 0 && !same(over(line, lz, z), over(c, zoom, z)) {
        z -= 1;
    }
    let mut at = if !same(over(line, lz, z), over(c, zoom, z)) {
        q.root(over(c, zoom, 0))
    } else if z == lz {
        l.live
    } else {
        l.stack[z as usize]
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
    use crate::cube::ground::stone;
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

    const STONE: [u32; 2] = [stone as u32, 0];

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

    /// The footing stands on the step's top, and every asset about it,
    /// asked off its line, is what the world holds there; one in another
    /// root is asked from that root.
    #[test]
    fn footing_asset_is_worlds() {
        let f = footing(&mut Step, UNIT_ZOOM, [6, 3]);
        assert_eq!(f.ends, End::Stop);
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

    type At = (Zoom, [i32; 3]);

    /// A walk's line where it ends (`cast::Keep`).
    struct Kept(Option<Line<At>>, [i32; 3]);

    impl Keep<At> for Kept {
        fn keep(&mut self, met: &Met<At>, stack: &[At; DEPTH]) {
            self.0 = Some(Line::kept(self.1, &met.stop.node, met.stop.at, stack));
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
    fn a_walk_keeps_the_line_down_to_where_it_stops() {
        let s = FINEST_ZOOM - UNIT_ZOOM;
        let o = [(6 << s) + 17, (3 << s) + 90, 9 << s];
        let detail = Detail {
            range: [UNIT_ZOOM, UNIT_ZOOM],
            eye: [0.0; 3],
            spacing: cast::Spacing::NONE,
        };
        let beam = Ray::new([-0.45, 0.05, -1.0], 64.0);
        let mut first = Kept(None, o);
        let met = cast::rooted(&mut Step, o, &beam, detail, Until::Opaque, &mut first);
        assert_eq!(met.ends, Ends::Stop);
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
                let t = met.stop.t * share;
                let (h, held) = met.stop.node.over(cast::point_at(o, &ray, t));
                let h = cast::holding(o, &ray, h, t);
                let from = if held {
                    (h, cast::Over::over(&Along(&first), h.zoom), t)
                } else {
                    (Node::ROOT, Step.root(Node::ROOT.index(o)), t)
                };
                let mut kept = Kept(None, o);
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
                assert_eq!(resumed.ends, Ends::Stop, "{d:?} {share}");
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
