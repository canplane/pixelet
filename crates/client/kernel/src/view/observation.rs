// pxl-kernel/view/observation.rs

//! The kernel's side of an observation: the eye, what it looks at, and what
//! is known of what a box of the world holds.

use crate::io::demand::Order;
use pxl_spec::space::layer::lattice::demand_reach;

use pxl_spec::space::camera::eye::{Frustum, Observation};
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::hierarchy::tree::{region, Node};
use pxl_spec::space::layer::demand::{band, tiles_within};
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

use crate::layer::residency::floor;
use crate::layer::{Answer, Query};
use crate::query::World;
use crate::view::frame::pose::globe;
use crate::view::frame::pose::view;
use crate::view::reach::{eye_height, reach_at};
use pxl_spec::space::coord::zoom::Zoom;

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
        reach: demand_reach(GROUND, top),
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

/// How far from the eye a layer's tiles at `zoom` are asked for: at the
/// source's `top` the floor's (residency/floor.rs `ring`), past it the
/// zoom's band (pxl-spec layer/demand.rs `band`); none past the far edge.
pub fn reach(o: &Observation, tau: Tau, lattice: Lattice, top: Zoom, zoom: Zoom) -> Option<f64> {
    if zoom == top {
        return floor::ring(o);
    }
    band(o, tau, lattice, top, zoom).map(|(_, to)| to)
}

/// What the view asks a layer for: at each zoom under the source's top, which
/// the floor holds, to the leaves, every tile within `reach` of the eye. It
/// is a function of the eye alone, never of what the rays met, so what is
/// asked, and so what is read, cannot depend on what was held before: the
/// rays only see it arrive.
pub struct ByEye {
    pub tau: Tau,
    /// The layer's, whose tiles' cells say which zoom a distance asks for.
    pub lattice: Lattice,
    /// The coarsest zoom the source supplies whole; no requirement is coarser.
    pub top: Zoom,
}

impl ByEye {
    pub fn requirements(&self, o: &Observation, each: &mut dyn FnMut(Tile)) {
        for zoom in self.top + 1..=LEAF_ZOOM {
            if let Some(r) = reach(o, self.tau, self.lattice, self.top, zoom) {
                tiles_within(zoom, [o.eye[0], o.eye[1]], r).for_each(&mut *each);
            }
        }
    }
}

/// A layer's tile's order, read in the grid's cells its own fold into: all
/// of it where nothing is known.
pub fn order_of_tile(o: &Observation, lattice: Lattice, p: Tile) -> Order {
    Order::of(o, &within(p.node(), lattice.grid_zoom(p.zoom)).unwrap_or(p.region()))
}
