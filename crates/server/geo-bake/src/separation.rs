// pxl-geo-bake/separation.rs

//! Bridges and tunnels: what a tile's bridges and tunnels are, read from the
//! source, and the lines the bake stores for them, solved as Arnis solves
//! them (`separation/arnis.rs`).

pub mod arnis;
pub mod elevation;
pub mod piece;
pub mod source;
pub mod way;

use postgres::Client;
use pxl_portable::ways;
use pxl_spec::bake::separation::Form;
use pxl_spec::space::coord::adapter::epsg3857::{mer_from_u, G3857};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use crate::terrain::repository::TerrainRepository;
use crate::terrain::CELL_U;
use source::Store;

pub use arnis::solve_tile;

/// What the solve leaves a tile: its lines, and the ranges of ways they draw,
/// which the ground leaves unpainted. One solve makes both, so the road
/// leaves the ground's paint exactly where its line begins.
#[derive(Default)]
pub struct Solved {
    pub lines: Vec<Line>,
    pub drawn: Vec<Drawn>,
}

/// A solved line: XY in DB coordinates, height in leaf cells, width in u.
pub struct Line {
    pub subtype: String,
    pub class: Option<String>,
    pub width_u: f64,
    pub form: Form,
    pub points: Vec<G3857>,
    /// The coarsest zoom each point is held at (`coarsest`).
    pub coarsest: Vec<u8>,
    /// A bore's clear space over its surface, in leaf cells; none for another form.
    pub clear: u8,
}

/// Each point's coarsest zoom (pxl-portable `ways::coarsest`) on the line
/// through `points`, its XY in DB coordinates and its height in leaf cells:
/// its effective area on the line, in leaf cells² — Visvalingam–Whyatt, as
/// TopoJSON's presimplify and mapshaper weigh a point: the triangle it makes
/// with the two about it, the least taken out first and those about it
/// weighed again, a point weighing no less than one taken before it. Its
/// height is weighed as its place is, so a deck's rise is kept where its
/// line runs straight. Its ends are held at every zoom where the line ends
/// there (`ends`), and past the leaves nowhere where only a cut ends it,
/// since there no other tile's line does.
pub fn coarsest(points: &[G3857], ends: [bool; 2]) -> Vec<u8> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let n = points.len();
    let cell = mer_from_u(CELL_U);
    let at = |i: usize| [points[i][0] / cell, points[i][1] / cell, points[i][2]];
    let triangle = |a: usize, b: usize, c: usize| {
        let (a, b, c) = (at(a), at(b), at(c));
        let (u, v) = ([0, 1, 2].map(|k| b[k] - a[k]), [0, 1, 2].map(|k| c[k] - a[k]));
        let x = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        0.5 * libm::sqrt(x[0] * x[0] + x[1] * x[1] + x[2] * x[2])
    };
    let (mut prev, mut next): (Vec<usize>, Vec<usize>) = ((0..n).map(|i| i.wrapping_sub(1)).collect(), (1..=n).collect());
    let mut area: Vec<f64> = (0..n).map(|i| if i == 0 || i + 1 == n { f64::INFINITY } else { triangle(i - 1, i, i + 1) }).collect();
    let mut heap: BinaryHeap<_> = (1..n.saturating_sub(1)).map(|i| Reverse((Area(area[i]), i))).collect();
    let (mut gone, mut floor) = (vec![false; n], 0.0f64);
    while let Some(Reverse((Area(a), i))) = heap.pop() {
        if gone[i] || a != area[i] {
            continue;
        }
        floor = floor.max(a);
        (area[i], gone[i]) = (floor, true);
        let (p, q) = (prev[i], next[i]);
        (next[p], prev[q]) = (q, p);
        for j in [p, q].into_iter().filter(|&j| j > 0 && j + 1 < n) {
            area[j] = triangle(prev[j], j, next[j]);
            heap.push(Reverse((Area(area[j]), j)));
        }
    }
    let end = |whole: bool| if whole { GROUND.top() } else { LEAF_ZOOM } as u8;
    (0..n)
        .map(|i| match i {
            0 => end(ends[0]),
            _ if i + 1 == n => end(ends[1]),
            _ => ways::coarsest(area[i] as f32) as u8,
        })
        .collect()
}

/// An area ordered as a number, ties by the point's place.
#[derive(Clone, Copy, PartialEq)]
struct Area(f64);

impl Eq for Area {}

impl PartialOrd for Area {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Area {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// A range of a way, as fractions of it, that a line draws.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawn {
    pub gers_id: String,
    pub range: [f64; 2],
}

/// What a solve reads through: the DB, and each tile's rows.
pub struct Reader<'a, S: Store> {
    pub client: &'a mut Client,
    pub repo: &'a TerrainRepository,
    pub store: &'a mut S,
}
