// pxl-core/domain/query/decor.rs

//! Decor as the world answers it: what the ground generates, evaluated cell
//! by cell as the frame generates it (`generated_at`) from the ground's cells
//! around the cell, and what objects place. Either kind is struck as the box
//! it claims.
//!
//! A ray strikes decor as its ground walk passes, only where the frame draws
//! decor as held and only at the leaf cells: a fold strikes none of it. The
//! walk follows the detail rule. The box test is the submodule `intersect`,
//! and the catalog is spec/spec/asset/decor.json. The scene draws the decor's
//! models as they arrive (domain/scene/decor/asset.rs).

pub mod intersect;

use pxl_spec::space::layer::lattice::Cells;

use crate::domain::layer::object::asset::Asset;
use pxl_spec::asset::decor::{Decor, DECORS};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::zoom::{cube, idx_of, units, Zoom, SHARD_ZOOM};
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::lattice::{draw_dist, GROUND};
use pxl_spec::space::layer::quadrant::LEAF_ZOOM;

use super::ray::Ray;
use super::scatter::{generated_at, DecorWorld};
use super::world::World;
use crate::domain::layer::instance::Instance;
use crate::domain::layer::object::meta::decor_instance;
use crate::domain::layer::object::store::tree_key;
use crate::domain::layer::object::store::Entry;
use crate::domain::layer::object::store::ObjectQuery as _;
use crate::domain::layer::terrain::TerrainQuery as _;
use crate::domain::layer::tile_cache::Line;
use crate::domain::query::decor::intersect::{box_extent, intersect};
use pxl_engine::area::{self, Shard};
use pxl_engine::cast;
use pxl_engine::clipmap::Lookup;
use pxl_engine::object::{self, NodeSlot};

/// One ray's decor tests, carried along its ground walk.
pub(super) struct DecorRay {
    /// How far any decor reaches from where it stands: in units here, and in
    /// whole leaf cells in `around`.
    pub(super) box_extent: f64,
    around: i32,
    /// How far along the ray decor is drawn as held.
    pub(super) draw_dist: f64,
    /// The cell met last, whose neighbors are struck already.
    last: Option<[i32; 2]>,
    /// The nearest decor struck: how far along the ray, the decor, and the
    /// axis the ray entered its box across (`intersect`).
    pub(super) best: Option<(f64, Instance, usize)>,
    /// What is placed around the nodes the walk went through (`Around`).
    pub(super) about: Around,
    /// The finest zoom whose nodes are as wide as the extent. A finer node is
    /// looked around at this zoom (`about_zoom`).
    extent_zoom: Zoom,
    /// `band` is where placed decor may stand along z for this ray: the
    /// ray's heights over the span where decor is held, widened by the
    /// extent. `box_zoom` is the finest zoom whose nodes are wider than the
    /// box searched around a leaf cell, which is as tall as the band and as
    /// wide as the extent; that box then lies strictly within the 27 nodes
    /// around the cell (`intersect_near`). Both belong to the ray and are the
    /// same at every cell.
    band: [f64; 2],
    box_zoom: Zoom,
}

impl DecorRay {
    /// The zoom a node at `zoom` is looked around at: its own, or
    /// `extent_zoom` where that is coarser.
    pub(super) fn about_zoom(&self, zoom: Zoom) -> Zoom {
        zoom.min(self.extent_zoom)
    }
}

/// The finest zoom, no finer than the walk goes, whose nodes' side `fits`;
/// 0 where none does. Sides grow as zooms coarsen, so a node finer than this
/// zoom is looked at here, and a node at it or coarser at its own zoom.
fn finest(fits: impl Fn(f64) -> bool) -> Zoom {
    let mut z = cast::DEPTH as Zoom - 1;
    while z > 0 && !fits(units(z)) {
        z -= 1;
    }
    z
}

/// What is placed in the 27 nodes around the node a striking walk is at, for
/// each zoom it went through, held as the objects' tree's cursors there
/// (pxl-engine `object::NodeSlot`). A node's neighbors are children of its
/// parent's neighbors (Samet 1989, neighbor finding), and a walk enters a
/// node through its parent, so each zoom's 27 are stepped from the zoom
/// above's in 27 `child` reads.
pub(super) struct Around {
    at: [[i32; 3]; cast::DEPTH],
    slots: [[NodeSlot; 27]; cast::DEPTH],
    /// The 3 × 3 columns around the node at each zoom, as the ground's
    /// relief reads them (`read_columns`).
    columns_at: [[i32; 2]; cast::DEPTH],
    columns: [[Option<Column>; 9]; cast::DEPTH],
}

/// A column around a striking walk's node, at the node's zoom, as its relief
/// read it: the ground's line its cells are read on, where those cells were
/// found, and the highest top its relief reaches, None where the relief is
/// lacking.
#[derive(Clone, Copy)]
struct Column {
    line: Line,
    lookup: Lookup,
    hi: Option<i32>,
}

impl Around {
    pub(super) fn new() -> Around {
        Around {
            at: [[i32::MIN; 3]; cast::DEPTH],
            slots: [[NodeSlot::NOTHING; 27]; cast::DEPTH],
            columns_at: [[i32::MIN; 2]; cast::DEPTH],
            columns: [[None; 9]; cast::DEPTH],
        }
    }

    /// Records that the walk entered the node `(zoom, c)` and finds the
    /// cursors of its 27. They are stepped from its parent's where those are
    /// the ones held, or else each found down from the roots.
    pub(super) fn visit(&mut self, lanes: &impl object::Columns, (zoom, c): Node<3>) {
        let z = zoom as usize;
        if self.at[z] == c {
            return;
        }
        let p = c.map(|v| v >> 1);
        let stepped = zoom > 0 && self.at[z - 1] == p;
        for k in 0..27 {
            let n = [
                c[0] + (k % 3) as i32 - 1,
                c[1] + (k / 3 % 3) as i32 - 1,
                c[2] + (k / 9) as i32 - 1,
            ];
            self.slots[z][k] = if stepped {
                let q = [
                    (n[0] >> 1) - p[0] + 1,
                    (n[1] >> 1) - p[1] + 1,
                    (n[2] >> 1) - p[2] + 1,
                ];
                object::child(
                    lanes,
                    self.slots[z - 1][(q[0] + 3 * q[1] + 9 * q[2]) as usize],
                    zoom,
                    object::path(zoom, n),
                )
            } else {
                // Y and Z lie under the world's one root alone; X wraps
                // around it.
                let inside =
                    (-1..=0).contains(&(n[1] >> zoom)) && (-1..=0).contains(&(n[2] >> zoom));
                if inside {
                    object::cursor(lanes, object::ROOT, zoom, n)
                } else {
                    NodeSlot::NOTHING
                }
            };
        }
        self.at[z] = c;
    }

    /// Reads the 3 × 3 columns around `(zoom, c)` at its tile's cells. Each
    /// column's line steps from its parent column's (`line_under`), its
    /// cells are found from where its parent column's were (pxl-engine
    /// `clipmap::step`), and its relief's highest top is read once for every
    /// node under it. Each zoom's columns step from the zoom above's
    /// (`Around`).
    pub(super) fn read_columns(&mut self, world: &World, (zoom, c): (Zoom, [i32; 2])) {
        let z = zoom as usize;
        if self.columns_at[z] == c {
            return;
        }
        let p = [c[0] >> 1, c[1] >> 1];
        let stepped = zoom > 0 && self.columns_at[z - 1] == p;
        let cell_zoom = GROUND.grid_zoom(zoom.min(LEAF_ZOOM));
        for k in 0..9 {
            let n = [c[0] + (k % 3) as i32 - 1, c[1] + (k / 3) as i32 - 1];
            let over = if stepped {
                self.columns[z - 1]
                    [((n[0] >> 1) - p[0] + 1 + 3 * ((n[1] >> 1) - p[1] + 1)) as usize]
            } else {
                None
            };
            let line =
                world
                    .terrain
                    .line_under(over.as_ref().map(|o| &o.line), (zoom, n), cell_zoom);
            let mut found = over.map_or(Lookup::NONE, |o| o.lookup);
            let hi = world
                .terrain
                .relief(&line, &mut found, (zoom, n), [cell_zoom, cell_zoom])
                .val()
                .map(|[[_, hi], _]| hi);
            self.columns[z][k] = Some(Column {
                line,
                lookup: found,
                hi,
            });
        }
        self.columns_at[z] = c;
    }

    /// The highest top the relief reaches in each of the 3 × 3 columns
    /// around `(zoom, a)`, None where it is lacking. The walk read them on
    /// entering `a` (`read_columns`), which must be the last node it entered
    /// at `zoom`.
    fn reliefs(&self, (zoom, a): (Zoom, [i32; 2])) -> [Option<i32>; 9] {
        let z = zoom as usize;
        debug_assert!(
            self.columns_at[z] == a,
            "the walk entered the node it reads about"
        );
        self.columns[z].map(|c| c.and_then(|c| c.hi))
    }

    /// The nodes the objects' tree holds around `a`, at `zoom` or above it,
    /// each with what is held there, and how many there are. Each node
    /// appears once (a leaf above several of the 27 is one node), in the
    /// tree's order (`tree_key`). These are the nodes a walk from the roots,
    /// pruned to a box within them, enters first (`Store::entries_under`).
    /// None where the walk did not go through `a`.
    fn under(&self, (zoom, a): Node<3>) -> Option<([(Node<3>, NodeSlot); 27], usize)> {
        let z = zoom as usize;
        if self.at[z] != a {
            return None;
        }
        let (mut out, mut n) = ([((0, [0; 3]), NodeSlot::NOTHING); 27], 0);
        for (k, &slot) in self.slots[z].iter().enumerate() {
            if !slot.holds() {
                continue;
            }
            let c = [
                a[0] + (k % 3) as i32 - 1,
                a[1] + (k / 3 % 3) as i32 - 1,
                a[2] + (k / 9) as i32 - 1,
            ];
            let h = slot.zoom();
            let node = (h, object::ancestor_at(zoom, c, zoom - h));
            if !out[..n].iter().any(|&(o, _)| o == node) {
                out[n] = (node, slot);
                n += 1;
            }
        }
        out[..n].sort_unstable_by_key(|&((h, c), _)| tree_key(h, c));
        Some((out, n))
    }

    /// Whether anything, air included, is placed in the 27 nodes around `a`,
    /// at `zoom` or above it. The walk must have gone through `a` (`visit`),
    /// and `a` must be the last node it entered at `zoom`.
    fn holds(&self, (zoom, a): Node<3>) -> bool {
        let z = zoom as usize;
        debug_assert!(self.at[z] == a, "the walk entered the node it reads about");
        self.slots[z].iter().any(|h| h.holds())
    }
}

impl<'a> World<'a> {
    /// Hands what the ground generates on the leaf `cell`, decor by decor, to
    /// `each` until `each` returns true, and returns whether it did. Only the
    /// kinds whose bits are set in `kinds` are asked (`World::decor`). Each
    /// instance stands on its column's surface in the world unplaced
    /// (`World::column_surface`). With `boxed`, `each` is also told whether
    /// the decor generates there as the frame draws it (`generated_at`);
    /// without `boxed` that is true. Nothing is handed where the world does
    /// not hold the column.
    fn generated_in(
        &self,
        cell: [i32; 2],
        boxed: bool,
        kinds: u32,
        mut each: impl FnMut(&Decor, Instance, bool) -> bool,
    ) -> bool {
        DECORS.iter().enumerate().any(|(k, decor)| {
            kinds >> k & 1 == 1
                && generated_at(&mut DecorWorld::new(self), decor, cell, boxed)
                    .is_some_and(|(i, generates)| each(decor, i, generates))
        })
    }

    /// Every decor the ground's scatter generates on the leaves whose box
    /// shares room with `space`, whether or not its column surface is there
    /// now. This includes what an erase took away.
    pub fn generated_over(&self, space: &Aabb) -> Vec<Instance> {
        let (r, unit) = (box_extent(), units(GROUND.leaf_cell_zoom()));
        let cells = |k: usize| {
            libm::floor((space.min[k] - r) / unit) as i32..libm::ceil(space.max[k] / unit) as i32
        };
        let mut out = Vec::new();
        for y in cells(1) {
            for x in cells(0) {
                self.generated_in([x, y], false, u32::MAX, |_, i, _| {
                    if claim(&i).is_some_and(|c| c.shares(*space)) {
                        out.push(i);
                    }
                    false
                });
            }
        }
        out
    }

    /// The instance of `decor` the ground generates standing in the leaf
    /// cell that `at` lies in. None where it generates nothing there or the
    /// clip levels do not hold the cell.
    pub fn generated_instance(&self, decor: &Decor, at: &Q3) -> Option<Instance> {
        let [x, y, z] = idx_of(GROUND.leaf_cell_zoom(), at);
        let mut met = None;
        self.generated_in([x, y], false, u32::MAX, |d, i, _| {
            met = Some(i)
                .filter(|i| d.val() == decor.val() && i.generated_in().is_some_and(|c| c[2] == z));
            met.is_some()
        });
        met
    }

    /// The decor tests a ray carries, where decor is struck at all.
    pub(super) fn decor_ray(&self, ray: &Ray) -> Option<DecorRay> {
        self.decor?;
        // Decor is struck out to where the view draws the leaves' tiles.
        let dist = match GROUND.z_at(0.0) {
            LEAF_ZOOM => draw_dist(GROUND, LEAF_ZOOM).min(ray.t_max),
            _ => return None,
        };
        let extent = box_extent();
        let around = extent.ceil() as i32;
        let (za, zb) = (ray.at(0.0)[2], ray.at(dist)[2]);
        let band = [za.min(zb) - extent, za.max(zb) + extent];
        Some(DecorRay {
            box_extent: extent,
            around,
            draw_dist: dist,
            last: None,
            best: None,
            about: Around::new(),
            extent_zoom: finest(|side| side >= extent),
            band,
            box_zoom: finest(|side| side > f64::from(around + 1).max(band[1] - band[0])),
        })
    }

    /// Strikes the decor standing near the leaf cell the walk is in. Only
    /// decor not near the cell met before is tested, since a walk only moves
    /// on.
    pub(super) fn intersect_near(&self, s: &mut DecorRay, ray: &Ray, (zoom, at): Node<3>) {
        let cell = [at[0], at[1]];
        let (r, last) = (s.around, s.last.replace(cell));
        let near = |a: [i32; 2], b: [i32; 2]| (a[0] - b[0]).abs() <= r && (a[1] - b[1]).abs() <= r;
        // Placed decor can meet the ray only within the band: the ray's
        // heights over the held span, widened by the extent.
        let [z0, z1] = s.band;
        let fresh = |xy: [i32; 2]| !last.is_some_and(|l| near(xy, l));
        for y in cell[1] - r..=cell[1] + r {
            for x in cell[0] - r..=cell[0] + r {
                if fresh([x, y]) {
                    self.generated_near(s, ray, [x, y]);
                }
            }
        }
        // What is placed around the cell is found in one walk of the objects'
        // tree over all the cells. Each entry is met once, in the cell its
        // box's corner is in.
        let space = Aabb {
            min: [(cell[0] - r) as f64, (cell[1] - r) as f64, z0],
            max: [(cell[0] + r + 1) as f64, (cell[1] + r + 1) as f64, z1],
        };
        // The search starts from the nodes around the cell at the finest zoom
        // wider than the box, whose 27 the walk stepped down to (`Around`).
        // The cell lies in the middle node and the ray's height there lies
        // within the band, so the box lies strictly within the 27 and touches
        // nothing past them.
        let wide = zoom.min(s.box_zoom);
        let from = s.about.under((wide, at.map(|v| v >> (zoom - wide))));
        let mut visit = |e: &Entry| {
            let c = e.cube().min;
            let own = [c[0].floor() as i32, c[1].floor() as i32];
            if let Some(i) = decor_instance(e).filter(|_| space.holds(c) && fresh(own)) {
                meet(s, ray, i);
            }
        };
        match from {
            Some((from, n)) => self.object.entries_under(&from[..n], &space, &mut visit),
            None => self.object.entries_in(&space, &mut visit),
        }
    }

    /// Whether decor that `s` strikes may reach into the node `(zoom, c)`,
    /// where no cell under it fills; X is on the asker's lap. Decor stands on
    /// the ground or on something placed, and reaches no farther than its
    /// extent from there. So a node is out of reach when, around it and as
    /// wide as the extent, the clip levels' relief holds no top above the
    /// node's bottom less the extent, and the objects' pyramid holds no
    /// record. Past the service area there is no ground, as `shard`, what the
    /// descent established at the node, says. A node whose ground is not held
    /// may be reached.
    ///
    /// The answer is read off what the walk holds around the node's ancestor
    /// at the zoom it is looked around at (`DecorRay::about_zoom`, `Around`),
    /// which is the last node the walk entered there. The walk enters every
    /// node through its parent, so an ancestor comes before its children and
    /// no later along the ray, and lies under the held span as they do. The
    /// walk entered no other node at that zoom since, because a node it comes
    /// back to is as fine or finer.
    pub(super) fn may_intersect(&self, s: &DecorRay, (zoom, c): Node<3>, shard: Shard) -> bool {
        let z = s.about_zoom(zoom);
        let a = c.map(|v| v >> (zoom - z));
        let low = cube(zoom, c).min[2] - s.box_extent;
        let leaf = units(GROUND.leaf_cell_zoom());
        let node = (zoom, [c[0], c[1]]);
        let read = s.about.reliefs((z, [a[0], a[1]]));
        let ground = (-1..=1).any(|dy| {
            (-1..=1).any(|dx| {
                // Past the area no ground stands.
                if z >= SHARD_ZOOM
                    && shard.past_leaf(node, area::tile_at(z, [a[0] + dx, a[1] + dy]))
                {
                    return false;
                }
                read[(dx + 1 + 3 * (dy + 1)) as usize].is_none_or(|hi| f64::from(hi) * leaf > low)
            })
        });
        ground || s.about.holds((z, a))
    }

    fn generated_near(&self, s: &mut DecorRay, ray: &Ray, cell: [i32; 2]) {
        let kinds = self.decor.unwrap_or(0);
        self.generated_in(cell, true, kinds, |_, i, generates| {
            if generates {
                meet(s, ray, i);
            }
            false
        });
    }
}

/// The box a decor thing claims.
pub(super) fn claim(i: &Instance) -> Option<Aabb> {
    Some(Asset::of(u32::from(i.id)).decor()?.claim(i.at, &i.meta))
}

fn meet(s: &mut DecorRay, ray: &Ray, i: Instance) {
    if let Some((t, axis)) = intersect(&i, ray.origin, ray.dir, s.best.map_or(s.draw_dist, |b| b.0))
    {
        s.best = Some((t, i, axis));
    }
}
