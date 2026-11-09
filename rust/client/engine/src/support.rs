// pxl-engine/support.rs

//! wgsl-twin: where something stands, asked of the world's one query (pxl-core
//! domain/query/support.rs, a walker's support). A column's surface is the first cell that a
//! ray down the column from the world's top cannot see through at a given cell zoom, with its
//! top as drawn (`column_surface`). The module also holds the query's cursor
//! (ARCHITECTURE.md, "The query, the only seam"): a descent's line (`Line`), either a column
//! surface's or one straight down to a node (`line`), and an asset near that node, asked from
//! the deepest node of the line that holds it and stepped down by what each parent hands its
//! child (`asset`). The CPU and a GPU run it alike, each over its own reading of the world
//! (`cast::Query`).

use crate::cast::{self, Detail, End, Keep, Line, Node, Query, Ray, Trace, Until, DEPTH};
use crate::object::ancestor_at;
use pxl_spec::space::coord::q3::FRAC_W;
use pxl_spec::space::coord::world::HEIGHT_BITS;
use pxl_spec::space::coord::zoom::{Zoom, FINEST_ZOOM};

/// A column's surface: its top as drawn at its cell zoom (`cast::top`), how the ray down it
/// ended (it met a cell, `End::Hit`; it met nothing it cannot see through, `Miss`; or it met
/// a node not held, `Lacks`), and the line it descended.
#[derive(Clone, Copy)]
pub struct ColumnSurface<A> {
    pub top: (i32, f32),
    pub end: End,
    pub line: Line<A>,
    origin: [i32; 3],
}

impl<A: Copy> Keep<A> for ColumnSurface<A> {
    fn keep(&mut self, trace: &Trace<A>, stack: &[A; DEPTH]) {
        self.line = Line::of_walk(self.origin, &trace.hit.node, trace.hit.at, stack);
    }
}

/// The column surface down column `g` at `cell_zoom`: the first cell that fills on the
/// world's one ray straight down the column's middle from the top of the world
/// (`HEIGHT_BITS`), at the cells of `cell_zoom` alone, whichever layer holds it. Its line is
/// kept so that the assets around it can be asked (`asset`).
pub fn column_surface<Q: Query>(q: &mut Q, cell_zoom: Zoom, g: [i32; 2]) -> ColumnSurface<Q::At> {
    let (o, ray, detail) = down(cell_zoom, g);
    let at = Node::ROOT.idx(o);
    let root = q.root(at);
    let start = Line::of_root(at, root);
    let mut f = ColumnSurface {
        top: (0, 0.0),
        end: End::Miss,
        line: start,
        origin: o,
    };
    // Walk from the root just asked; `walk_from_root` would ask it again.
    let trace = cast::walk(
        q,
        o,
        &ray,
        detail,
        Until::Opaque,
        (Node::ROOT, root, 0.0),
        &start,
        &mut f,
    );
    f.end = trace.end;
    f.top = cast::top(o, &trace.hit.node, trace.hit.asset[0]);
    f
}

/// The ray down column `g` at `cell_zoom`: from the top of the world over the
/// column's middle, straight down, at the cells of `cell_zoom` alone.
fn down(cell_zoom: Zoom, g: [i32; 2]) -> ([i32; 3], Ray, Detail) {
    let s = FINEST_ZOOM - cell_zoom;
    let mid = |v: i32| v.wrapping_shl(s).wrapping_add((1 << s) >> 1);
    let top = ((1i32 << (HEIGHT_BITS - 1)) << FRAC_W) - 1;
    let o = [mid(g[0]), mid(g[1]), top];
    let ray = Ray::new([0.0, 0.0, -1.0], (1u32 << HEIGHT_BITS) as f32);
    (
        o,
        ray,
        Detail {
            range: [cell_zoom, cell_zoom],
            spacing: cast::Spacing::NONE,
        },
    )
}

/// The node at `z` over node `c` of `zoom`, where `z` is no finer than `zoom`.
fn over(c: [i32; 3], zoom: Zoom, z: Zoom) -> [i32; 3] {
    ancestor_at(zoom, c, zoom - z)
}

fn same(a: [i32; 3], b: [i32; 3]) -> bool {
    a[0] == b[0] && a[1] == b[1] && a[2] == b[2]
}

/// The deepest zoom at which the line `l` and the node `c` of `zoom` share a node, no finer
/// than either, and whether they share one at all; they share none where they lie under two
/// roots.
fn shared_zoom<A: Copy>(l: &Line<A>, zoom: Zoom, c: [i32; 3]) -> (Zoom, bool) {
    let (line, lz) = (l.at, l.zoom);
    let mut z = if zoom < lz { zoom } else { lz };
    while z > 0 && !same(over(line, lz, z), over(c, zoom, z)) {
        z -= 1;
    }
    (z, same(over(line, lz, z), over(c, zoom, z)))
}

/// Descends to node `c` of `zoom` from its node at `z`, which was handed `at`, each node
/// handed down by its parent (`Query::child`). It returns what was handed `c` and leaves what
/// was handed each node from `z` down to it in `stack`, by zoom.
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

/// The line straight down to node `c` of `zoom` from its root, each node handed down by its
/// parent: the line from which a reader of the assets around a node asks them (`asset`).
pub fn line<Q: Query>(q: &mut Q, zoom: Zoom, c: [i32; 3]) -> Line<Q::At> {
    let root = q.root(over(c, zoom, 0));
    let mut stack = [root; DEPTH];
    let live = descend(q, root, 0, zoom, c, &mut stack);
    Line::down_to(c, zoom, live, stack)
}

/// What is in the cell `c` of `zoom` near the line `l`, as the world's one query answers it
/// at that zoom (the first word of `Query::node`). It is asked from the deepest node of the
/// line that holds the cell, each node under that handed down by its parent (`Query::child`),
/// or from the cell's own root where no node of the line holds it.
pub fn asset<Q: Query>(q: &mut Q, l: &Line<Q::At>, zoom: Zoom, c: [i32; 3]) -> u32 {
    let (mut z, shared) = shared_zoom(l, zoom, c);
    let mut at = if shared {
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
    use crate::answer::{fill_word, In, Kind, AIR, BOX_WHOLE};
    use crate::content::Sample;
    use pxl_spec::share::EIGHTHS;
    use pxl_spec::space::coord::zoom::{SHARD_ZOOM, UNIT_ZOOM};

    /// Ground of stone up to `height` unit cells: two over the cells west of x = 4, none over
    /// the rest, with air above. Each node's `At` is the node it was handed for, checked where it
    /// is asked, so an asset asked from a node not over it is caught.
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
            let up = ancestor_at(zoom, node, 1);
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

    /// The column surface stands on the step's top, and every asset around it, asked from its
    /// line, is what the world holds there; an asset under another root is asked from that root.
    #[test]
    fn footing_asset_is_worlds() {
        let f = column_surface(&mut Step, UNIT_ZOOM, [6, 3]);
        assert_eq!(f.end, End::Hit);
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

    /// The line a column surface keeps answers as the line straight down to its top cell does,
    /// which is the line a reader that did not walk the column asks the assets from.
    #[test]
    fn column_surface_line_is_its_top_cells() {
        for g in [[6, 3], [2, 3], [3, 0], [9, 5]] {
            let f = column_surface(&mut Step, UNIT_ZOOM, g);
            assert_eq!(f.end, End::Hit, "{g:?}");
            let rebuilt = line(&mut Step, UNIT_ZOOM, [g[0], g[1], f.top.0]);
            for c in [
                [g[0] + 1, g[1], f.top.0],
                [g[0] - 1, g[1], f.top.0 + 1],
                [g[0], g[1] + 2, 0],
            ] {
                assert_eq!(
                    asset(&mut Step, &f.line, UNIT_ZOOM, c),
                    asset(&mut Step, &rebuilt, UNIT_ZOOM, c),
                    "{g:?} {c:?}"
                );
            }
        }
    }

    /// Ground up to unit cell `top`, that cell keeping `eighths` of its cube; air over it.
    struct Ground {
        top: i32,
        eighths: u32,
    }

    impl Query for Ground {
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
            // A node of a shard's size or coarser spans the world's height, so its Z index says nothing.
            if zoom < UNIT_ZOOM {
                return if zoom > SHARD_ZOOM && node[2] << (UNIT_ZOOM - zoom) > self.top {
                    In {
                        kind: Kind::Cell,
                        asset: AIR,
                    }
                } else {
                    In {
                        kind: Kind::Split,
                        asset: BOX_WHOLE,
                    }
                };
            }
            let top = [fill_word(STONE[0], self.eighths % EIGHTHS), 0];
            In {
                kind: Kind::Cell,
                asset: match node[2].cmp(&self.top) {
                    core::cmp::Ordering::Less => STONE,
                    core::cmp::Ordering::Equal => top,
                    core::cmp::Ordering::Greater => AIR,
                },
            }
        }
    }

    /// The walk down a column, in floats, stands on exactly the cell and share the ground holds
    /// (`cast::top`). This is decor's footing on the CPU, as a GPU reads it in integers
    /// (pxl-render decor_pose.wgsl `surface`). It holds from the world's ceiling to its floor,
    /// where every plane lies a whole number of Q3 steps under 2^24 from the ray's start, and for
    /// every fill share.
    #[test]
    fn column_surface_is_exact_to_the_worlds_edges() {
        let half = 1i32 << (HEIGHT_BITS - 1);
        for top in [-half, 1 - half, -1, 0, 1, 1000, half - 2] {
            for eighths in 1..=EIGHTHS {
                let mut q = Ground { top, eighths };
                let f = column_surface(&mut q, UNIT_ZOOM, [5, 7]);
                let asset = fill_word(STONE[0], eighths % EIGHTHS);
                let rise = cast::fill_bounds(([0.0; 3], [1.0; 3]), asset).1[2];
                assert_eq!(f.end, End::Hit, "{top} {eighths}");
                assert_eq!(f.top, (top, rise), "{top} {eighths}");
            }
        }
    }

    type At = (Zoom, [i32; 3]);

    /// A walk's line where it ends (`cast::Keep`).
    struct LineCollector(Option<Line<At>>, [i32; 3]);

    impl Keep<At> for LineCollector {
        fn keep(&mut self, trace: &Trace<At>, stack: &[At; DEPTH]) {
            self.0 = Some(Line::of_walk(self.1, &trace.hit.node, trace.hit.at, stack));
        }
    }

    /// The assets around a walk's stop, asked from the line it keeps there, are the world's,
    /// whether the walk started from the root or resumed under another ray's line and climbed
    /// above where it resumed.
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
        let trace = cast::walk_from_root(&mut Step, o, &beam, detail, Until::Opaque, &mut first);
        assert_eq!(trace.end, End::Hit);
        let first = first.0.unwrap();
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
                let t = trace.hit.t * share;
                let (h, found) = trace.hit.node.over(cast::point_at(o, &ray, t));
                let h = cast::holding(o, &ray, h, t);
                let from = if found {
                    (h, first.ancestor(h.zoom), t)
                } else {
                    (Node::ROOT, Step.root(Node::ROOT.idx(o)), t)
                };
                let mut collected = LineCollector(None, o);
                let resumed = cast::walk(
                    &mut Step,
                    o,
                    &ray,
                    detail,
                    Until::Opaque,
                    from,
                    &first,
                    &mut collected,
                );
                assert_eq!(resumed.end, End::Hit, "{d:?} {share}");
                let collected = collected.0.unwrap();
                let walked = line(&mut Step, collected.zoom, collected.at);
                for d in 0..6u32 {
                    let mut c = collected.at;
                    c[(d / 2) as usize] += if d % 2 == 0 { -1 } else { 1 };
                    assert_eq!(
                        asset(&mut Step, &collected, UNIT_ZOOM, c),
                        asset(&mut Step, &walked, UNIT_ZOOM, c),
                        "{c:?}"
                    );
                }
            }
        }
    }

    /// A line straight down to a node holds the assets around it as the world does, its own
    /// among them.
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
