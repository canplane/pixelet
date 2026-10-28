// pxl-kernel/domain/view/observation.rs

//! The kernel's side of an observation: the eye, what it looks at, and what
//! is known of what a box of the world holds.

use crate::sys::io::demand::Order;
use pxl_spec::space::layer::lattice::{demand_reach, draw_reach};
use xpute_guest::global::Global;

use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::camera::grid::split_distance;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::demand::{band, squares_within, tiles_within};
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};
use pxl_spec_core::window::{bricks_zoom, BRICK_SHIFT};

use crate::domain::layer::tile_cache::floor;
use crate::domain::layer::Base as _;
use crate::domain::query::{Walked, World};
use crate::domain::view::frame::pose::view;
use crate::domain::view::reach::eye_height;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::coord::zoom::Zoom;

pub fn eye() -> Option<V3> {
    Some(view().pose?.eye)
}

/// How far from the eye the layers are demanded and the windows hold: as far
/// as the source's top is, past where it is drawn by the windows' margin.
/// None before the source declares its top.
pub fn eye_demand_reach() -> Option<f64> {
    crate::sys::io::source::top().map(|top| demand_reach(GROUND, top))
}

/// How far the eye draws, the rays and the clouds alike: as far as the
/// source's top is drawn, short of what the windows hold past it
/// (`eye_demand_reach`). None before the source declares its top.
pub fn eye_draw_reach() -> Option<f64> {
    crate::sys::io::source::top().map(|top| draw_reach(GROUND, top))
}

/// The turn's observation, made by its first reader after the view is
/// composed where it moved (`turn`, `forget`): the eye and its ground's
/// floor asked once where they moved, however many read it.
static OBSERVED: Global<Option<Option<Observation>>> = Global::new();

/// The turn's view moved: the next reader makes the observation again.
pub fn forget() {
    *OBSERVED.get(|| None) = None;
}

/// A turn begins, the pose `moved` or not: the observation stands where it
/// did not, the reach is what it was, and no change touched the ground under
/// the eye, whose height over it it holds (`changed::within`, asked each
/// frame turn, so each change is seen once); else it is made again.
pub fn turn(moved: bool) {
    let stands = !moved
        && match *OBSERVED.get(|| None) {
            Some(Some(o)) => {
                Some(o.reach) == eye_demand_reach()
                    && !crate::domain::query::changed::within([[o.eye[0], o.eye[1]]; 2], &[])
            }
            Some(None) => eye().is_none() || eye_demand_reach().is_none(),
            None => false,
        };
    if !stands {
        forget();
    }
}

/// Out to `eye_demand_reach`. None before the camera stands anywhere or the source
/// declares its top.
pub fn observation() -> Option<Observation> {
    *OBSERVED.get(|| None).get_or_insert_with(|| {
        let eye = eye()?;
        let reach = eye_demand_reach()?;
        let height = eye_height();
        Some(Observation {
            eye,
            frustum: Some(view().frustum),
            reach,
            ground: eye[2] - height,
        })
    })
}

/// The eye's windows' bricks (pxl-spec-core window.rs), the ground about
/// the eye cut into one partition: from the coarsest bricks within the
/// floor's ring (tile_cache/floor.rs `ring`), the far edge cut by the ground,
/// a brick splits into its four while its nearest point is nearer than its
/// cells' split distance (pxl-spec space/camera/grid.rs `split_distance`),
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
    // Depth first, so each brick's lines step from its neighbour's.
    let (world, mut walked) = (World::now(), Walked::default());
    while let Some((zoom, xy)) = stack.pop() {
        let (cell, d) = (zoom + BRICK_SHIFT, near(zoom, xy));
        if d > o.reach {
            continue;
        }
        if cell < GROUND.cell() && d < split_distance(cell) {
            stack.extend((0..4).map(|k| (zoom + 1, [2 * xy[0] + (k & 1), 2 * xy[1] + (k >> 1)])));
            continue;
        }
        let Some(b) = world
            .bounds(&mut walked, (zoom, xy), cell)
            .value()
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
pub fn reach(o: &Observation, lattice: Lattice, top: Zoom, zoom: Zoom) -> Option<f64> {
    if zoom == top {
        return floor::ring(o);
    }
    band(o, lattice, top, zoom).map(|(_, to)| to)
}

/// What the view asks a layer for: at each zoom under the source's top, which
/// the floor holds, to the leaves, every tile within `reach` of the eye. It
/// is a function of the eye alone, never of what the rays met, so what is
/// asked, and so what is read, cannot depend on what was held before: the
/// rays only see it arrive.
/// It asks by x and y alone, the eye's height no part of it: a ray picks a
/// cell's zoom by its distance in 3D (pxl-spec-core `grid::cell_at`), never
/// shorter than its distance across the ground, so more is asked than is
/// read and never less.
pub struct ByEye {
    /// The layer's, whose tiles' cells say which zoom a distance asks for.
    pub lattice: Lattice,
    /// The coarsest zoom the source supplies whole; no requirement is coarser.
    pub top: Zoom,
}

impl ByEye {
    pub fn requirements(&self, o: &Observation, each: &mut dyn FnMut(Tile)) {
        for zoom in self.top + 1..=LEAF_ZOOM {
            if let Some(r) = reach(o, self.lattice, self.top, zoom) {
                tiles_within(zoom, [o.eye[0], o.eye[1]], r).for_each(&mut *each);
            }
        }
    }
}

/// A layer's tile's order, from where anything stands in it as the grid's
/// cells its own fold into read it, stepped from where `walked` last went:
/// all of it where nothing is known or nothing stands.
pub fn order_of_tile(o: &Observation, lattice: Lattice, p: Tile, walked: &mut Walked) -> Order {
    let stands = World::now().bounds(walked, p.node(), lattice.grid_zoom(p.zoom));
    Order::of(o, &stands.value().flatten().unwrap_or(p.region()))
}
