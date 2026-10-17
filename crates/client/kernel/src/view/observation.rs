// pxl-kernel/view/observation.rs

//! The kernel's side of an observation: the eye, what it looks at, and what
//! is known of what a box of the world holds.

use crate::io::demand::Order;
use pxl_spec::space::layer::lattice::demand_reach;
use xpute_guest::global::Global;

use pxl_spec::space::camera::eye::{Frustum, Observation};
use pxl_spec::space::camera::grid::{Tau, CHUNK_STEPS};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::demand::{band, squares_within, tiles_within};
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

use crate::layer::residency::{floor, Lines};
use crate::query::World;
use crate::view::frame::pose::view;
use crate::view::reach::eye_height;
use pxl_spec::space::coord::zoom::Zoom;

pub fn eye() -> Option<V3> {
    Some(view().pose?.eye)
}

pub fn frustum_of_view() -> Frustum {
    view().frustum
}

/// How far the eye observes: as far as the source's top is demanded. None
/// before the source declares its top.
pub fn eye_reach() -> Option<f64> {
    floor::top().map(|top| demand_reach(GROUND, top))
}

/// The turn's observation, made by its first reader after the view is
/// composed (`forget`): the eye and its ground's floor asked once a turn,
/// however many read it.
static OBSERVED: Global<Option<Option<Observation>>> = Global::new();

/// The turn's view moved, or a new turn began: the next reader makes the
/// observation again.
pub fn forget() {
    *OBSERVED.get(|| None) = None;
}

/// Out to `eye_reach`. None before the camera stands anywhere or the source
/// declares its top.
pub fn observation() -> Option<Observation> {
    *OBSERVED.get(|| None).get_or_insert_with(|| {
        let eye = eye()?;
        let reach = eye_reach()?;
        let height = eye_height();
        Some(Observation {
            eye,
            frustum: Some(frustum_of_view()),
            reach,
            ground: eye[2] - height,
        })
    })
}

/// The cubes `CHUNK_STEPS` zooms over the cells a reader draws, the ground
/// about the eye cut into one partition: from the coarsest squares within the
/// far edge, a square splits into its four while its nearest point is nearer
/// than its cell zoom's split distance (`Tau::split_distance`), the rule a
/// ray's descent asks by, taken about the eye's height; each square left is
/// at the zoom its nearest point asks, and in each, the cubes the world stands
/// in there. A function of the eye and the world alone, never of what a ray
/// met; a column it knows nothing of yet names none, as nothing there is to
/// stand in.
pub fn cubes(o: &Observation, tau: Tau, each: &mut dyn FnMut(Node<3>)) {
    let Some(top) = floor::top() else { return };
    let over = (o.eye[2] - o.ground).max(0.0);
    let coarsest = GROUND.grid_zoom(top.max(GROUND.top())).max(CHUNK_STEPS);
    let near = |zoom: Zoom, [x, y]: [i64; 2]| {
        let side = units(zoom);
        let gap = |lo: f64, e: f64| (lo - e).max(e - lo - side).max(0.0);
        libm::hypot(libm::hypot(gap(x as f64 * side, o.eye[0]), gap(y as f64 * side, o.eye[1])), over)
    };
    let zoom = coarsest - CHUNK_STEPS;
    let flat = (o.reach * o.reach - over * over).max(0.0).sqrt();
    // Its room kept from one walk to the next.
    static STACK: Global<Vec<(Zoom, [i64; 2])>> = Global::new();
    let mut stack = core::mem::take(STACK.get(Vec::new));
    stack.clear();
    stack.extend(squares_within(zoom, [o.eye[0], o.eye[1]], flat).map(|xy| (zoom, xy)));
    // Depth first, so each square's ground line steps from its neighbour's.
    let (world, mut lines) = (World::now(), Lines::default());
    while let Some((zoom, xy)) = stack.pop() {
        let (cell, d) = (zoom + CHUNK_STEPS, near(zoom, xy));
        if d > o.reach {
            continue;
        }
        if cell < GROUND.cell() && d < tau.split_distance(cell) {
            stack.extend((0..4).map(|k| (zoom + 1, [2 * xy[0] + (k & 1), 2 * xy[1] + (k >> 1)])));
            continue;
        }
        let Some(b) = world.bounds(&mut lines, (zoom, xy), cell).value().flatten() else { continue };
        let side = units(zoom);
        for z in (b.aabb.min[2] / side).floor() as i64..=(b.aabb.max[2] / side).floor() as i64 {
            each((zoom, [xy[0], xy[1], z]));
        }
    }
    *STACK.get(Vec::new) = stack;
}

/// The cell zoom the eye's rays draw at `at`: the camera grid's rule at its
/// distance (`Tau::cell_at`), no coarser than a tile draws and no finer than
/// the ground's cells.
pub fn cell_at(o: &Observation, tau: Tau, at: V3) -> Option<Zoom> {
    let top = floor::top()?;
    let v = pxl_spec::space::coord::v3::sub(&at, &o.eye);
    Some(tau.cell_at(pxl_spec::space::coord::v3::dot(&v, &v)).clamp(GROUND.grid_zoom(top.max(GROUND.top())), GROUND.cell()))
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

/// A layer's tile's order, from where anything stands in it as the grid's
/// cells its own fold into read it, its ground's line stepped from where
/// `lines` last walked: all of it where nothing is known or nothing stands.
pub fn order_of_tile(o: &Observation, lattice: Lattice, p: Tile, lines: &mut Lines) -> Order {
    let stands = World::now().bounds(lines, p.node(), lattice.grid_zoom(p.zoom));
    Order::of(o, &stands.value().flatten().map_or(p.region(), |b| b.aabb))
}
