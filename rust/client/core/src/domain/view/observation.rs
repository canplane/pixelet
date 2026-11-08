// pxl-core/domain/view/observation.rs

//! The core's side of an observation (pxl-spec `camera::eye::Observation`).
//! The observation is made from the view as the turn composed it
//! (domain/view/frame/pose.rs) and from what the world is known to hold.
//! From it come the bricks of the eye's clip levels, the tiles each layer is
//! asked for, and the order a tile is read in.

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

/// How far from the eye, in world units, the layers are asked for and the
/// clip levels hold. This reaches as far as the source's top zoom does, past
/// the draw distance (`eye_draw_dist`) by the clip levels' margin. None until
/// the source declares its top.
pub fn eye_demand_dist() -> Option<f64> {
    source::top().map(|top| demand_dist(GROUND, top))
}

/// How far the eye draws, in world units. The rays and the clouds both stop
/// here, short of `eye_demand_dist` by the margin the clip levels hold past
/// it. None until the source declares its top.
pub fn eye_draw_dist() -> Option<f64> {
    source::top().map(|top| draw_dist(GROUND, top))
}

/// The observation the last reader made. It is kept so that the floor under
/// the eye is looked up once however many parts read the observation.
/// `Some(None)` means the last reader found no eye or no demand distance.
static OBSERVATION: Global<Option<Option<Observation>>> = Global::new();

/// Called as a frame turn begins. The kept observation holds the eye's
/// height over the ground under it, so it is dropped when a change touched
/// that ground. `changes::within` is asked every frame turn, so each change
/// is seen once.
pub fn turn() {
    let made = OBSERVATION.get(|| None);
    if made.is_some_and(|o| o.is_some_and(|o| changes::within([[o.eye[0], o.eye[1]]; 2], &[]))) {
        *made = None;
    }
}

/// The observation out to `eye_demand_dist`. It is made again when the eye,
/// the frustum or the demand distance differs from what it was made of. None
/// until the camera stands somewhere and the source declares its top.
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

/// Cuts the ground around the eye into the bricks of the eye's clip levels
/// (pxl-engine clipmap.rs), one partition with no overlap, and hands each
/// brick to `each` with the lowest and highest height the world holds in it.
///
/// The cut starts from the coarsest bricks within the floor's ring
/// (tile_cache/floor.rs `ring`) and ends at the demand distance. A brick
/// splits into its four while its nearest point is nearer than the split
/// distance of its cells (pxl-spec space/camera/grid.rs `split_dist`),
/// measured with the eye's height over the ground. That is the rule a ray's
/// descent uses, so each brick handed on is at the zoom its nearest point
/// asks for.
///
/// The bricks depend on the eye and the world alone, never on what a ray
/// met. A column the world knows nothing of yet gets no brick, since there is
/// nothing in it for a brick to stand in.
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
    // The stack's allocation is kept from one call to the next.
    static STACK: Global<Vec<(Zoom, [i32; 2])>> = Global::new();
    let mut stack = core::mem::take(STACK.get(Vec::new));
    stack.clear();
    stack.extend(squares_within(zoom, [o.eye[0], o.eye[1]], flat).map(|xy| (zoom, xy)));
    // The cut goes depth first, so each brick's column walk steps on from
    // the brick walked just before it, which is its neighbor.
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

/// How far from the eye a layer's tiles at `zoom` are asked for. At the
/// source's `top` zoom this is the floor's ring (tile_cache/floor.rs `ring`).
/// At a finer zoom it is the outer edge of the zoom's band (pxl-spec
/// layer/demand.rs `band`), and None when that band lies past the far edge.
pub fn demand_radius(o: &Observation, lattice: Lattice, top: Zoom, zoom: Zoom) -> Option<f64> {
    if zoom == top {
        return floor::ring(o);
    }
    band(o, lattice, top, zoom).map(|(_, to)| to)
}

/// What the view asks a layer for: at each zoom finer than the source's top,
/// down to the leaves, every tile within `demand_radius` of the eye. The
/// floor holds the top zoom itself.
///
/// The demand depends on the eye alone, never on what the rays met. So what
/// is asked for, and therefore what is read, cannot depend on what was held
/// before; the rays only see the tiles arrive.
///
/// The demand measures distance across the ground and leaves out the eye's
/// height. A ray picks a cell's zoom by its distance in 3D (pxl-engine
/// `grid::cell_zoom_at`), which is never shorter than the distance across
/// the ground, so the demand asks for at least what the rays read.
pub struct ByEye {
    /// The layer's lattice. Its tiles' cells decide which zoom a distance
    /// asks for.
    pub lattice: Lattice,
    /// The coarsest zoom the source supplies whole. No tile is asked for at a
    /// coarser zoom.
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

/// The order a layer's tile `p` is read in, judged by the box of what stands
/// in it. That box is the world's bounds over the tile at the grid zoom the
/// tile's own cells fold into, walked on from where `walked` last stopped.
/// Where nothing is known or nothing stands, the tile's whole region is used.
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
/// near it is (`Order`).
pub fn order_of(o: &Observation, b: &Aabb) -> Order {
    let m = o.measure(b);
    Order {
        seen: m.seen,
        near: m.near,
    }
}
