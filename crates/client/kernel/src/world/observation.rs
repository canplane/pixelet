// pxl-kernel/world/observation.rs

//! The kernel's side of an observation: the eye, what it looks at, and what
//! is known of what a box of the world holds.

use std::cmp::Ordering;

use pxl_spec::bake::format::Terrain;
use pxl_spec::space::camera::eye::{Frustum, Observation};
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::camera::grid::CHUNK_STEPS;
use pxl_spec::space::coord::v3::Aabb;
use pxl_spec::space::coord::world::REACH;
use pxl_spec::space::hierarchy::tree::ancestor_of;
use pxl_spec::space::layer::demand::{band, Bounds, Contents, Requirement};
use pxl_spec::space::layer::lattice::{Lattice, GROUND};
use pxl_spec::space::layer::tile::{tile_at, Tile, LEAF_ZOOM};

use crate::kernel::kernel;
use crate::layer::earth::Earth;
use crate::layer::environment::fog::{eye_height, reach_at};
use crate::layer::environment::state::globe;
use crate::world::access::Access;
use crate::world::frame::view::view;
use crate::world::observer::Footprint;
use crate::world::pyramid::domain;
use crate::world::pyramid::Layer as _;
use crate::world::residency::floor;
use pxl_spec::bake::poi_tile::PoiTile;
use pxl_spec::space::coord::zoom::{of_side, units, Zoom};

pub fn eye() -> Option<[f64; 3]> {
    crate::control::camera::controller::camera().eye?;
    Some(view().pose.eye)
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

/// What the kernel knows of the world: terrain index bounds (from the tile
/// whose cells fold into the grid's at `cell`, or the nearest resident one
/// over it) and the objects' 3D bounds. Unknown where no terrain tile over
/// part of the box is held; a folded-away object counts whole.
pub struct Known;

impl Bounds for Known {
    fn contents(&self, cell: Zoom, region: Aabb) -> Contents {
        let zoom = GROUND.tile_zoom_of_grid(cell).clamp(i32::from(GROUND.top()), i32::from(LEAF_ZOOM)) as Zoom;
        let (h0, h1) = (region.min[2], region.max[2]);
        let cut = |(lo, hi): (f64, f64)| (lo <= h1 && hi >= h0).then(|| (lo.max(h0), hi.min(h1)));
        let mut known = true;
        let mut bound: Option<Aabb> = None;
        under(zoom, cell - CHUNK_STEPS, region, &mut |node, square| {
            let Some(n) = read(zoom, node) else {
                known = false;
                return;
            };
            let mut hold = |b: Aabb| bound = Some(bound.map_or(b, |a| a.join(b)));
            if let Some((lo, hi)) = cut(n.ground) {
                hold(Aabb {
                    min: [square.min[0], square.min[1], lo],
                    max: [square.max[0], square.max[1], hi],
                });
            }
            if let Some(objects) = kernel().objects.bound_in(&square) {
                hold(objects);
            }
        });
        match (known, bound) {
            (false, _) => Contents::Unknown,
            (true, None) => Contents::Empty,
            (true, Some(bound)) => Contents::Holds { bound },
        }
    }

    fn finest(&self, region: Aabb) -> Zoom {
        kernel().objects.finest_in(&region).map_or(GROUND.cell(), |z| z.max(GROUND.cell()))
    }
}

#[derive(Clone, Copy)]
struct Node {
    ground: (f64, f64),
}

/// None where no terrain tile over the node is resident.
fn read(zoom: Zoom, node: (Zoom, [i64; 2])) -> Option<Node> {
    let tile = tile_at(zoom, ancestor_of(node.1, u32::from(node.0 - zoom)));
    let relief = Access::now().relief(tile, node).value()?;
    Some(Node { ground: relief.extent })
}

/// Each node of a tile at `zoom` under `region`: the region itself where it is
/// exactly one, else its squares `column` wide.
fn under(zoom: Zoom, column: Zoom, region: Aabb, each: &mut dyn FnMut((Zoom, [i64; 2]), Aabb)) {
    let side = region.max[0] - region.min[0];
    let square = |s: Zoom, [x, y]: [i64; 2]| {
        let (x0, y0) = (x as f64 * units(s), y as f64 * units(s));
        let covered = Aabb {
            min: [x0, y0, region.min[2]],
            max: [x0 + units(s), y0 + units(s), region.max[2]],
        };
        covered.meet(region).unwrap_or(covered)
    };
    let whole = side > 0.0 && region.max[1] - region.min[1] == side && units(of_side(side)) == side;
    if let Some(s) = whole
        .then(|| of_side(side))
        .filter(|&s| s >= zoom && region.min[0] % units(s) == 0.0 && region.min[1] % units(s) == 0.0)
    {
        let at = [(region.min[0] / units(s)) as i64, (region.min[1] / units(s)) as i64];
        return each((s, at), square(s, at));
    }
    let column = column.max(zoom);
    let rows = |k: usize| (region.min[k] / units(column)).floor() as i64..=((region.max[k] / units(column)).ceil() as i64 - 1);
    for y in rows(1) {
        for x in rows(0) {
            each((column, [x, y]), square(column, [x, y]));
        }
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
    pub lacks: &'a [Requirement],
}

impl ByFootprints<'_> {
    pub fn requirements(&self, o: &Observation, bounds: &impl Bounds, each: &mut dyn FnMut(Requirement)) {
        let column = |x0: f64, y0: f64, x1: f64, y1: f64| Aabb {
            min: [x0, y0, REACH.min[2]],
            max: [x1, y1, REACH.max[2]],
        };
        let required = |zoom: Zoom, region: Aabb| {
            let (from, to) = band(o, self.tau, self.lattice, self.top, zoom)?;
            let held = bounds.contents(self.lattice.grid_zoom(zoom), region).within(region)?;
            let m = o.measure(&held);
            (m.seen && m.near < to && m.near <= o.far() && m.far >= from).then_some(Requirement { zoom, region })
        };
        for lack in self.lacks {
            let r = lack.region;
            let region = column(r.min[0], r.min[1], r.max[0], r.max[1]);
            if let Some(r) = (self.top..=lack.zoom).rev().find_map(|z| required(z, region)) {
                each(r);
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
                for y in rows(1) {
                    for x in rows(0) {
                        let (x0, y0) = (x as f64 * span, y as f64 * span);
                        let square = column(x0, y0, x0 + span, y0 + span);
                        if f.touches(&square) {
                            if let Some(r) = required(zoom, square) {
                                each(r);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// What `Known` and the heights read of the ground: the terrain tiles known.
pub fn ground_known(out: &mut Vec<(Tile, bool)>) {
    Terrain::pyramid().residency().known(out);
}

/// Where the objects changed, told to what works anything out from them;
/// what is drawn hears it through its sources (`chunk/source.rs`).
pub fn objects_written(regions: &[Aabb]) {
    domain::objects_written(regions);
    Terrain::pyramid().objects_written();
    PoiTile::pyramid().objects_written();
    Earth::pyramid().objects_written();
    crate::layer::poi::placement::objects_written();
}

/// Seen before unseen, then nearer first. Every layer's work is taken in
/// this order, and eviction in reverse.
#[derive(Clone, Copy, Debug)]
pub struct Order {
    pub seen: bool,
    pub near: f64,
}

impl Order {
    pub const LAST: Order = Order { seen: false, near: f64::INFINITY };

    pub const FIRST: Order = Order { seen: true, near: 0.0 };

    pub fn of(o: &Observation, b: &Aabb) -> Order {
        let m = o.measure(b);
        Order { seen: m.seen, near: m.near }
    }

    /// All of `region` where nothing is known.
    pub fn of_region<B: Bounds + ?Sized>(o: &Observation, bounds: &B, cell: Zoom, region: Aabb) -> Order {
        Order::of(o, &bounds.contents(cell, region).within(region).unwrap_or(region))
    }

    /// A layer's tile, read in the grid's cells its own fold into.
    pub fn of_tile(o: &Observation, lattice: Lattice, p: Tile) -> Order {
        Order::of_region(o, &Known, lattice.grid_zoom(p.zoom), p.region())
    }
}

impl Ord for Order {
    fn cmp(&self, other: &Order) -> Ordering {
        other.seen.cmp(&self.seen).then(self.near.total_cmp(&other.near))
    }
}

impl PartialOrd for Order {
    fn partial_cmp(&self, other: &Order) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Order {
    fn eq(&self, other: &Order) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for Order {}
