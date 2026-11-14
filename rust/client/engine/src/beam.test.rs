// pxl-engine/beam.test.rs

use super::*;
use crate::answer::{
    box_join, box_word, fill_eighths, fill_word, In, Kind, BOX_NONE, BOX_STEPS, EMPTY,
};
use crate::cast::{Detail, End, Node, Query, Ray, Spacing, Trace, Until, DEPTH};
use crate::content::Sample;
use pxl_spec::share::EIGHTH_CNT;
use pxl_spec::space::coord::q3::FRAC_W;
use pxl_spec::space::coord::world::wrap_idx;
use pxl_spec::space::coord::zoom::HEIGHT_BITS;
use pxl_spec::space::coord::zoom::{starts, Zoom, MIN_ZOOM, UNIT_ZOOM};
use pxl_spec::space::hierarchy::tree::ancestor_at;

#[test]
fn largest_beam_fits_binding() {
    // A target of 4112 × 2338 drawn pixels, a ratio of 2, at the view's field of view (the
    // tangent of its half is 1/2, pxl-spec space/camera/grid.rs `HALF_TAN`), must fit within
    // the 33,554,432 words a storage binding holds on every device (pxl-render floor.rs).
    let up = 0.5;
    let words = Grid::of([up * 4112.0 / 2338.0, up]).size();
    assert!(words <= 33_554_432, "{words} words");
}

/// The views the grid is tried at: the screen's tangents across and up.
const VIEWS: [[f32; 2]; 3] = [
    [0.5 * 16.0 / 9.0, 0.5],
    [0.613, 0.5],
    [0.5, 0.5 * 19.5 / 9.0],
];

#[test]
fn parked_items_fit_the_asks() {
    for tan in VIEWS {
        let g = Grid::of(tan);
        let asks = g.sample_lattice[0] * g.sample_lattice[1] * ASK_WORDS;
        assert!(
            g.corner_lattice[0] * g.corner_lattice[1] <= asks
                && g.sample_lattice[0] * g.sample_lattice[1] <= asks
        );
    }
}

/// What a stand-in node is handed: itself, as its zoom and its world index, so a walk resumed
/// from a line can be checked to hand each node its own.
type At = (Zoom, [i32; 3]);

/// A box in units, lo and hi on each axis, and the eighths of its cube a
/// cell wholly in it keeps.
struct Thing {
    lo: [f32; 3],
    hi: [f32; 3],
    eighths: u32,
}

/// What stands on the stand-in's ground around the eye (`EYE`): a tower; a slab over the
/// ground with air under it; a pole half a unit thick, thinner than the beam rays' spacing
/// where it stands, its cells keeping half their cubes; a block far off whose cells keep six
/// eighths; and a cube a quarter of a unit a side ten units before the eye.
const THINGS: [Thing; 5] = [
    Thing {
        lo: [994.0, 30.0, -8.0],
        hi: [998.0, 34.0, 20.0],
        eighths: EIGHTH_CNT,
    },
    Thing {
        lo: [1002.0, 45.0, 8.0],
        hi: [1014.0, 52.0, 9.0],
        eighths: EIGHTH_CNT,
    },
    Thing {
        lo: [1000.5, 0.0, -8.0],
        hi: [1001.0, 0.5, 10.0],
        eighths: 4,
    },
    Thing {
        lo: [1028.0, 148.0, -8.0],
        hi: [1060.0, 160.0, 40.0],
        eighths: 6,
    },
    Thing {
        lo: [1000.0, -31.0, 9.5],
        hi: [1000.25, -30.75, 9.75],
        eighths: EIGHTH_CNT,
    },
];

/// The highest the ground's top reaches (`top`).
const RELIEF: f32 = 6.0;

/// The ground's top at `[x, y]` units: bumps up to `RELIEF` around sea level.
fn top(x: f32, y: f32) -> f32 {
    3.0 * libm::sinf(x * 0.37) + 3.0 * libm::cosf(y * 0.29)
}

/// A stand-in world: the ground's bumps (`top`) and the things on it (`THINGS`). A cell is
/// what it lies wholly in: a thing, or the ground where the cell's middle lies under the top;
/// otherwise air. A node coarser than asked is a split, boxed around the ground under `RELIEF`
/// and the things it meets, rounded out by a step, or else air. So every cell under a split
/// lies in its box, and a node's answer depends only on the node and the zooms asked.
struct Stand;

impl Query for Stand {
    type At = At;
    fn root(&mut self, node: [i32; 3]) -> At {
        (0, node)
    }
    fn child(&mut self, at: &At, zoom: Zoom, node: [i32; 3]) -> At {
        let parent = ancestor_at(zoom, node, 1);
        let parent = [wrap_idx(zoom - 1, parent[0]), parent[1], parent[2]];
        assert_eq!(*at, (zoom - 1, parent), "handed {node:?} at {zoom}");
        (zoom, node)
    }
    fn node(
        &mut self,
        at: &mut At,
        zoom: Zoom,
        c: [i32; 3],
        [_, near]: [Zoom; 2],
        _: f32,
    ) -> In<[u32; 2]> {
        assert_eq!(*at, (zoom, c), "asked");
        let side = libm::ldexpf(1.0, UNIT_ZOOM as i32 - zoom as i32);
        // `c + s` counts Y and Z from the world's edge (`zoom::starts`).
        let s = starts(zoom);
        let lo = [
            c[0] as f32 * side,
            (c[1] + s[1]) as f32 * side - (1u32 << (UNIT_ZOOM - 1)) as f32,
            (c[2] + s[2]) as f32 * side - (1u32 << (HEIGHT_BITS - 1)) as f32,
        ];
        let hi = lo.map(|v| v + side);
        if zoom >= near {
            let within = |t: &Thing| (0..3).all(|a| t.lo[a] <= lo[a] && hi[a] <= t.hi[a]);
            let asset = if let Some(t) = THINGS.iter().find(|t| within(t)) {
                fill_word(Sample::GROUND, t.eighths)
            } else if hi[2] <= top(lo[0] + side / 2.0, lo[1] + side / 2.0) {
                Sample::GROUND
            } else {
                EMPTY[0]
            };
            return In {
                kind: Kind::Cell,
                asset: [asset, 0],
            };
        }
        // The steps of the node's box around `[a, b]` on each axis, one step past it each way.
        let step = side / BOX_STEPS as f32;
        let boxed = |a: [f32; 3], b: [f32; 3]| {
            let first = |k: usize| (libm::floorf((a[k] - lo[k]) / step) - 1.0).max(0.0) as u32;
            let past =
                |k: usize| (libm::ceilf((b[k] - lo[k]) / step) + 1.0).min(BOX_STEPS as f32) as u32;
            box_word([first(0), first(1), first(2)], [past(0), past(1), past(2)])
        };
        let mut b = BOX_NONE;
        if lo[2] < RELIEF {
            b = box_join(b, boxed(lo, [hi[0], hi[1], hi[2].min(RELIEF)]));
        }
        for t in &THINGS {
            if (0..3).all(|a| t.lo[a] < hi[a] && lo[a] < t.hi[a]) {
                let from = [0, 1, 2].map(|k| t.lo[k].max(lo[k]));
                let to = [0, 1, 2].map(|k| t.hi[k].min(hi[k]));
                b = box_join(b, boxed(from, to));
            }
        }
        if b == BOX_NONE {
            In {
                kind: Kind::Cell,
                asset: EMPTY,
            }
        } else {
            In {
                kind: Kind::Split,
                asset: b,
            }
        }
    }
}

/// A beam ray's line as the shader leaves it (pxl-render walk.wgsl,
/// resume.wgsl): the node it stopped at and what was handed each
/// node from the root to it.
#[derive(Clone, Copy)]
struct Line {
    node: Node,
    at: [At; DEPTH],
}

impl cast::Keep<At> for Line {
    fn keep(&mut self, trace: &Trace<At>, stack: &[At; DEPTH]) {
        let z = trace.hit.node.zoom as usize;
        self.node = trace.hit.node;
        self.at[..z].copy_from_slice(&stack[..z]);
        self.at[z] = trace.hit.at;
    }
}

impl Line {
    /// This line as a walk from `origin` resumes from it.
    fn whole(&self, origin: [i32; 3]) -> cast::Line<At> {
        cast::Line::of_walk(
            origin,
            &self.node,
            self.at[self.node.zoom as usize],
            &self.at,
        )
    }
}

/// The eye the views are cast from, in units: above the bumps, south of the things, with a
/// root's edge under it and before it (sea level, and Y's 0).
const EYE: [f32; 3] = [1000.37, -40.6, 12.3];

/// How far below the horizon the eye looks, in radians, and how far its
/// rays reach, in units.
const DOWN: f32 = 0.3;
const FAR: f32 = 300.0;

/// The eye looking north, `DOWN` below the horizon, at the screen's
/// tangents `tan`: its origin, its axes and the zooms it asks, as the
/// core writes its words (pxl-core domain/view/lens.rs `gpu_cast`).
struct View {
    origin: [i32; 3],
    axes: [[f32; 3]; 3],
    tan: [f32; 2],
    range: [Zoom; 2],
}

impl View {
    fn of(tan: [f32; 2]) -> View {
        let (s, c) = (libm::sinf(DOWN), libm::cosf(DOWN));
        View {
            origin: EYE.map(|v| libm::rintf(v * (1 << FRAC_W) as f32) as i32),
            axes: [[0.0, c, -s], [1.0, 0.0, 0.0], [0.0, s, c]],
            tan,
            range: [MIN_ZOOM + 9, UNIT_ZOOM + 3],
        }
    }

    /// The eye's zooms, its beam rays a cell apart (pxl-render
    /// walk_node.wgsl `eye_aim`).
    fn detail(&self) -> Detail {
        Detail {
            range: self.range,
            spacing: Spacing {
                angle: CELL,
                across: 0.0,
            },
        }
    }

    /// The eye's ray at the tangents `at` (pxl-render walk_node.wgsl `eye_aim`).
    fn ray(&self, [tx, ty]: [f32; 2]) -> Ray {
        let [f, r, u] = self.axes;
        Ray::new(cast::through(f, r, u, tx, ty), FAR)
    }
}

/// `f` at each of `0..n`, across the machine's threads, in order.
fn each<T: Send>(n: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let threads = std::thread::available_parallelism().map_or(1, |k| k.get());
    let chunk = n.div_ceil(threads).max(1);
    std::thread::scope(|s| {
        let f = &f;
        let parts: Vec<_> = (0..n)
            .step_by(chunk)
            .map(|a| s.spawn(move || (a..(a + chunk).min(n)).map(f).collect::<Vec<_>>()))
            .collect();
        parts
            .into_iter()
            .flat_map(|p| p.join().expect("a thread of the cast"))
            .collect()
    })
}

/// A sample's hit as the sample leaves it for the raster (pxl-render sample_put.wgsl
/// `sample_put`): how it ended, the node, its asset and the axis it entered across. For a
/// sample that went on to its reach, only that is kept, because its zoom `SAMPLE_NONE` is all
/// the raster reads of it (pxl-render sample_hit.wgsl `hits`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Hit {
    end: End,
    node: Node,
    asset: [u32; 2],
    axis: u32,
}

impl Hit {
    fn of(trace: &Trace<At>) -> Hit {
        if trace.end == End::Miss {
            return Hit {
                end: End::Miss,
                node: Node::ROOT,
                asset: EMPTY,
                axis: 0,
            };
        }
        Hit {
            end: trace.end,
            node: trace.hit.node,
            asset: trace.hit.asset,
            axis: trace.hit.axis,
        }
    }
}

/// A sample's hit, and how it started: its cell, that cell's start, and whether it resumed
/// from a node of the line rather than from a root.
#[derive(Clone, Copy)]
struct Cast {
    hit: Hit,
    cell: [u32; 2],
    t: f32,
    resumed: bool,
}

/// The view cast as the shader casts it (pxl-render corner.wgsl, cell.wgsl, sample.wgsl
/// over sample.eye.wgsl): every corner's beam ray to where it stops, and its start distance;
/// each cell's start, the least of its window's, with that corner's line and whether the cell
/// resumes, which it does where every beam ray of the window asks as its rays ask; and every
/// sample walked to its hit from its cell's start along that line (pxl-render resume.wgsl),
/// or from the world's root where it does not resume. The samples come row by row,
/// `samples[0]` to a row.
fn cast_view(view: &View) -> Vec<Cast> {
    let g = Grid::of(view.tan);
    let detail = view.detail();
    let o = view.origin;
    let [across, down] = g.corner_lattice;
    let corners = each((across * down) as usize, |k| {
        let c = [k as u32 % across, k as u32 / across];
        let mut line = Line {
            node: Node::ROOT,
            at: [(0, [0; 3]); DEPTH],
        };
        let ray = view.ray(g.corner(c));
        let trace = cast::walk_from_root(&mut Stand, o, &ray, detail, Until::Beam, &mut line);
        (line, g.start_dist(trace.hit.t))
    });
    let idx = |[i, j]: [u32; 2]| j * across + i;
    let cells = each((g.span[0] * g.span[1]) as usize, |k| {
        let c = [k as u32 % g.span[0], k as u32 / g.span[0]];
        let w = g.cell_window(c);
        let [x, y] = g.first(c).map(|v| v - w);
        let side = 2 + 2 * w;
        let mut from = idx([x, y]);
        let mut least = corners[from as usize].1;
        let mut alike = true;
        for j in 0..side {
            for i in 0..side {
                let k = idx([x + i, y + j]);
                let t = corners[k as usize].1;
                if t < least {
                    (least, from) = (t, k);
                }
                // The eye's corners and cells ask its one detail (pxl-render
                // cell.wgsl).
                alike = alike && detail.alike(detail);
            }
        }
        (least, from, alike)
    });
    let [n, m] = g.sample_lattice;
    each((n * m) as usize, |k| {
        let s = [k as u32 % n, k as u32 / n];
        let ray = view.ray(g.sample(s));
        let cell = g.sample_cell(s);
        let (t, from, resumes) = cells[(cell[1] * g.span[0] + cell[0]) as usize];
        let line = &corners[from as usize].0;
        let root = (Node::ROOT, Stand.root(Node::ROOT.idx(o)));
        let (start, resumed) = if !resumes {
            ((root.0, root.1, 0.0), false)
        } else {
            let (h, found) = line.node.over(cast::point_at(o, &ray, t));
            let h = cast::holding(o, &ray, h, t);
            if found {
                ((h, line.at[h.zoom as usize], t), true)
            } else {
                ((root.0, root.1, t), false)
            }
        };
        let trace = cast::walk(
            &mut Stand,
            o,
            &ray,
            detail,
            Until::Opaque,
            start,
            &line.whole(o),
            &mut (),
        );
        Cast {
            hit: Hit::of(&trace),
            cell,
            t,
            resumed,
        }
    })
}

/// A sample's hit on a world is the hit of its ray walked from the eye with nothing resumed:
/// each cell's start, the least start distance of its window's beam rays, lies short of
/// everything any ray of the cell meets.
#[test]
fn sample_hit_is_the_walks_from_the_eye() {
    for tan in VIEWS {
        let view = View::of(tan);
        let g = Grid::of(tan);
        let detail = view.detail();
        let [n, m] = g.sample_lattice;
        let fresh = each((n * m) as usize, |k| {
            let ray = view.ray(g.sample([k as u32 % n, k as u32 / n]));
            Hit::of(&cast::first(
                &mut Stand,
                view.origin,
                &ray,
                detail,
                Until::Opaque,
            ))
        });
        let cast = cast_view(&view);
        let (mut met, mut misses) = ([0; EIGHTH_CNT as usize + 1], 0);
        for (k, f) in fresh.iter().enumerate() {
            let s = [k as u32 % n, k as u32 / n];
            let a = &cast[k];
            assert!(
                a.hit == *f,
                "{tan:?} sample {s:?}: fresh {f:?}; cell {:?} from {} resumed {}: {:?}",
                a.cell,
                a.t,
                a.resumed,
                a.hit,
            );
            if f.end == End::Hit {
                met[fill_eighths(f.asset[0]) as usize] += 1;
            }
            misses += u32::from(f.end == End::Miss);
        }
        // The ground and the whole things, the pole and the far block are
        // met, and something gone on past.
        assert!(
            [EIGHTH_CNT, 4, 6].iter().all(|&k| met[k as usize] > 0) && misses > 0,
            "{tan:?}: {met:?} by the eighths met, {misses} gone on"
        );
    }
}
