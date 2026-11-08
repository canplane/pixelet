// pxl-core/domain/view/observation.rs

//! The core's side of an observation: what the eye looks at as the turn
//! composed the view (domain/view/frame/pose.rs), and what is known of what
//! a box of the world holds.

use crate::sys::io::demand::Order;
use pxl_spec::space::layer::lattice::{demand_dist, draw_dist};
use xpute_core::global::Global;

use pxl_engine::clipmap::{bricks_zoom, BRICK_SHIFT};
use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::camera::grid::split_dist;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::demand::{band, squares_within, tiles_within};
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use pxl_spec::space::layer::quadrant::{Quadrant, LEAF_ZOOM};

use crate::domain::layer::tile_cache::floor;
use crate::domain::layer::Base as _;
use crate::domain::query::changes;
use crate::domain::query::{ColumnWalk, World};
use crate::domain::view::above_ground::height_at;
use crate::domain::view::frame::pose::{eye, view};
use crate::sys::io::source;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::coord::zoom::Zoom;

/// How far from the eye the layers are demanded and the clip levels hold: as
/// far as the source's top is, past where it is drawn by the clip levels'
/// margin. None before the source declares its top.
pub fn eye_demand_dist() -> Option<f64> {
    source::top().map(|top| demand_dist(GROUND, top))
}

/// How far the eye draws, the rays and the clouds alike: as far as the
/// source's top is drawn, short of what the clip levels hold past it
/// (`eye_demand_dist`). None before the source declares its top.
pub fn eye_draw_dist() -> Option<f64> {
    source::top().map(|top| draw_dist(GROUND, top))
}

/// The observation as its last reader made it: the eye, its frustum and its
/// reach as the view and the source stood, and the ground's floor under the
/// eye, asked once however many read it.
static OBSERVATION: Global<Option<Option<Observation>>> = Global::new();

/// A frame turn begins: the observation stands where no change touched the
/// ground under the eye, whose height over it it holds (`changes::within`,
/// asked each frame turn, so each change is seen once); else it is made
/// again.
pub fn turn() {
    let made = OBSERVATION.get(|| None);
    if made.is_some_and(|o| o.is_some_and(|o| changes::within([[o.eye[0], o.eye[1]]; 2], &[]))) {
        *made = None;
    }
}

/// Out to `eye_demand_dist`, made again where the eye, its frustum or its
/// reach is not what it was made of. None before the camera stands anywhere
/// or the source declares its top.
pub fn observation() -> Option<Observation> {
    let (eye, frustum, demand_dist) = (eye(), view().frustum, eye_demand_dist());
    let made = OBSERVATION.get(|| None);
    let stands = match *made {
        Some(Some(o)) => {
            Some(o.eye) == eye && o.frustum == Some(frustum) && Some(o.demand_dist) == demand_dist
        }
        Some(None) => eye.is_none() || demand_dist.is_none(),
        None => false,
    };
    if !stands {
        *made = Some(eye.zip(demand_dist).map(|(eye, demand_dist)| Observation {
            eye,
            frustum: Some(frustum),
            demand_dist,
            ground: eye[2] - height_at(eye),
        }));
    }
    made.flatten()
}

/// The eye's clip levels' bricks (pxl-engine clipmap.rs), the ground about
/// the eye cut into one partition: from the coarsest bricks within the
/// floor's ring (tile_cache/floor.rs `ring`), the far edge cut by the ground,
/// a brick splits into its four while its nearest point is nearer than its
/// cells' split distance (pxl-spec space/camera/grid.rs `split_dist`),
/// the rule a ray's descent asks by, taken about the eye's height; each brick left is at the zoom
/// its nearest point asks, and handed with the heights the world stands
/// between in it. A function of the eye and the world alone, never of what
/// a ray met; a column it knows nothing of yet is handed none, as nothing
/// there is to stand in.
pub fn bricks(o: &Observation, each: &mut dyn FnMut(Node<2>, [f64; 2])) {
    let (Some(top), Some(flat)) = (TerrainTile::top(), floor::ring(o)) else {
        return;
    };
    let over = (o.eye[2] - o.ground).max(0.0);
    let coarsest = GROUND.grid_zoom(top).max(BRICK_SHIFT);
    let near = |zoom: Zoom, [x, y]: [i32; 2]| {
        let side = units(zoom);
        let gap = |lo: f64, e: f64| (lo - e).max(e - lo - side).max(0.0);
        libm::hypot(
            libm::hypot(
                gap(x as f64 * side, o.eye[0]),
                gap(y as f64 * side, o.eye[1]),
            ),
            over,
        )
    };
    let zoom = bricks_zoom(coarsest);
    // Its room kept from one walk to the next.
    static STACK: Global<Vec<(Zoom, [i32; 2])>> = Global::new();
    let mut stack = core::mem::take(STACK.get(Vec::new));
    stack.clear();
    stack.extend(squares_within(zoom, [o.eye[0], o.eye[1]], flat).map(|xy| (zoom, xy)));
    // Depth first, so each brick's lines step from its neighbor's.
    let (world, mut walked) = (World::now(), ColumnWalk::default());
    while let Some((zoom, xy)) = stack.pop() {
        let (cell_zoom, d) = (zoom + BRICK_SHIFT, near(zoom, xy));
        if d > o.demand_dist {
            continue;
        }
        if cell_zoom < GROUND.leaf_cell_zoom() && d < split_dist(cell_zoom) {
            stack.extend((0..4).map(|k| (zoom + 1, [2 * xy[0] + (k & 1), 2 * xy[1] + (k >> 1)])));
            continue;
        }
        let Some(b) = world
            .bounds(&mut walked, (zoom, xy), cell_zoom)
            .val()
            .flatten()
        else {
            continue;
        };
        each((zoom, xy), [b.min[2], b.max[2]]);
    }
    *STACK.get(Vec::new) = stack;
}

/// How far from the eye a layer's tiles at `zoom` are asked for: at the
/// source's `top` the floor's (tile_cache/floor.rs `ring`), past it the
/// zoom's band (pxl-spec layer/demand.rs `band`); none past the far edge.
pub fn demand_radius(o: &Observation, lattice: Lattice, top: Zoom, zoom: Zoom) -> Option<f64> {
    if zoom == top {
        return floor::ring(o);
    }
    band(o, lattice, top, zoom).map(|(_, to)| to)
}

/// What the view asks a layer for: at each zoom under the source's top, which
/// the floor holds, to the leaves, every tile within `demand_radius` of the eye. It
/// is a function of the eye alone, never of what the rays met, so what is
/// asked, and so what is read, cannot depend on what was held before: the
/// rays only see it arrive.
/// It asks by x and y alone, the eye's height no part of it: a ray picks a
/// cell's zoom by its distance in 3D (pxl-engine `grid::cell_zoom_at`),
/// never shorter than its distance across the ground, so more is asked than is
/// read and never less.
pub struct ByEye {
    /// The layer's, whose tiles' cells say which zoom a distance asks for.
    pub lattice: Lattice,
    /// The coarsest zoom the source supplies whole; no requirement is coarser.
    pub top: Zoom,
}

impl ByEye {
    pub fn requirements(&self, o: &Observation, each: &mut dyn FnMut(Quadrant)) {
        for zoom in self.top + 1..=LEAF_ZOOM {
            if let Some(r) = demand_radius(o, self.lattice, self.top, zoom) {
                tiles_within(zoom, [o.eye[0], o.eye[1]], r).for_each(&mut *each);
            }
        }
    }
}

/// A layer's tile's order, from where anything stands in it as the grid's
/// cells its own fold into read it, stepped from where `walked` last went:
/// all of it where nothing is known or nothing stands.
pub fn order_of_tile(
    o: &Observation,
    lattice: Lattice,
    p: Quadrant,
    walked: &mut ColumnWalk,
) -> Order {
    let stands = World::now().bounds(walked, p.node(), lattice.grid_zoom(p.zoom));
    order_of(o, &stands.val().flatten().unwrap_or(p.region()))
}

/// Where the box `b` stands in the view of `o`: whether it is seen, and how
/// near (`Order`).
pub fn order_of(o: &Observation, b: &Aabb) -> Order {
    let m = o.measure(b);
    Order {
        seen: m.seen,
        near: m.near,
    }
}
