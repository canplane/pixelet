// pxl-kernel/view/observation.rs

//! The kernel's side of an observation: the eye, what it looks at, and what
//! is known of what a box of the world holds.

use crate::io::demand::Order;

use pxl_spec::space::camera::eye::{Frustum, Observation};
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::hierarchy::tree::{region, Node};
use pxl_spec::space::layer::demand::band;
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use pxl_spec::space::layer::tile::{tile_at, tile_rows, Tile, LEAF_ZOOM};

use crate::layer::residency::floor;
use crate::layer::{Answer, Query};
use crate::query::World;
use crate::view::frame::pose::globe;
use crate::view::frame::pose::view;
use crate::view::reach::{eye_height, reach_at};
use crate::view::sight::Footprint;
use pxl_spec::space::coord::zoom::{units, Zoom};

pub fn eye() -> Option<V3> {
    Some(view().pose?.eye)
}

pub fn frustum_of_view() -> Frustum {
    view().frustum
}

/// Out to where the air ends the ground or the source's top ends, whichever
/// is nearer. None before the source declares its top.
pub fn observation() -> Option<Observation> {
    let eye = eye()?;
    let top = floor::top()?;
    let height = eye_height();
    Some(Observation {
        eye,
        frustum: Some(frustum_of_view()),
        reach: GROUND.demand_reach(top),
        fog: reach_at(eye[2]),
        planet: globe().radius_u(),
        ground: eye[2] - height,
    })
}

/// Where anything stands in a node as the world answers it at `cell`: none
/// where nothing does, the whole node where nothing is known.
pub fn within<const D: usize>(node: Node<D>, cell: Zoom) -> Option<Aabb> {
    match World::now().bounds(node, cell) {
        Answer::Nothing(_) => Some(region(node)),
        held => held.value().flatten().map(|b| b.aabb),
    }
}

/// The estimator from what the screen's patches saw: the ground their footprints touch
/// at each zoom whose band holds it, plus what their rays lacked. Ground no
/// ray reaches is left to the floor. A lack is recast when it arrives, so
/// requirements grow toward what exact records would give and never rest
/// short of it.
pub struct ByFootprints<'a> {
    pub tau: Tau,
    /// The layer's, whose tiles' cells say which zoom a distance asks for.
    pub lattice: Lattice,
    /// The coarsest zoom the source supplies whole; no requirement is coarser.
    pub top: Zoom,
    pub footprints: &'a [Footprint],
    pub lacks: &'a [Tile],
}

impl ByFootprints<'_> {
    /// The tiles required: each at a zoom whose band holds what its column
    /// is known to hold, as `at` reads at that zoom's cells.
    pub fn requirements(&self, o: &Observation, each: &mut dyn FnMut(Tile)) {
        let required = |zoom: Zoom, at: Node<2>| {
            let Some((from, to)) = band(o, self.tau, self.lattice, self.top, zoom) else { return false };
            let Some(held) = within(at, self.lattice.grid_zoom(zoom)) else { return false };
            let m = o.measure(&held);
            m.seen && m.near < to && m.near <= o.far() && m.far >= from
        };
        for &lack in self.lacks {
            let line = std::iter::successors(Some(lack), |q| (q.zoom > self.top).then(|| q.parent()));
            if let Some(p) = line.into_iter().find(|q| required(q.zoom, lack.node())) {
                each(p);
            }
        }
        for f in self.footprints {
            let b = f.aabb();
            for zoom in (self.top..=LEAF_ZOOM).rev() {
                let Some((_, to)) = band(o, self.tau, self.lattice, self.top, zoom) else { break };
                let reach = to.min(o.far());
                let lo = [b.min[0].max(o.eye[0] - reach), b.min[1].max(o.eye[1] - reach)];
                let hi = [b.max[0].min(o.eye[0] + reach), b.max[1].min(o.eye[1] + reach)];
                if lo[0] > hi[0] || lo[1] > hi[1] {
                    continue;
                }
                let span = units(zoom);
                let rows = |k: usize| (lo[k] / span).floor() as i64..=(hi[k] / span).floor() as i64;
                for y in rows(1).filter(|y| tile_rows(zoom).contains(y)) {
                    for x in rows(0) {
                        let p = tile_at(zoom, [x, y]);
                        if f.touches(&p.region()) && required(zoom, p.node()) {
                            each(p);
                        }
                    }
                }
            }
        }
    }
}

/// A layer's tile's order, read in the grid's cells its own fold into: all
/// of it where nothing is known.
pub fn order_of_tile(o: &Observation, lattice: Lattice, p: Tile) -> Order {
    Order::of(o, &within(p.node(), lattice.grid_zoom(p.zoom)).unwrap_or(p.region()))
}
