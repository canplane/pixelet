// pxl-kernel/domain/query/decor.rs

//! Decor as the world answers it: what the ground grows, evaluated cell by
//! cell as the frame grows it (`grown_at`), off the ground's cells about the
//! cell, each read as asked (`World::surfaces`), and what objects place, each
//! struck by its model. A ray strikes it as the ground walk passes, only
//! where the frame draws it as held, at the leaf cells: a fold strikes none of
//! it. The walk follows the detail rule, not the chunk the view chose, which
//! may differ at a chunk's edge.
//!
//! Under it is the decors' runtime half, beside the catalog
//! (spec/spec/block/decor.json): their models as they arrive (`assets`),
//! and a decor thing struck by its model (`strike`).

pub mod asset;
pub mod strike;

use pxl_spec::space::layer::lattice::{demand_reach, Cells};

use pxl_spec::block::decor::{Decor, DECORS};
use pxl_spec::block::DecorId;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::zoom::{cube, index_of, units, Zoom, SHARD_ZOOM};
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use super::ray::Ray;
use super::scatter::{grown_at, grown_of, Asked};
use super::world::World;
use crate::domain::layer::instance::{Instance, Meta};
use crate::domain::layer::object::meta::placed;
use crate::domain::layer::object::store::tree_key;
use crate::domain::layer::residency::Line;
use crate::domain::query::decor::strike::{reach, strike};
use crate::domain::query::model::registry::Models;
use pxl_spec_core::area::{self, Shard};
use pxl_spec_core::cast;
use pxl_spec_core::overwrite::{self, Held};
use pxl_spec_core::window::Found;

/// How decor is struck: against its models, where the frame drawing at `tau` holds it.
#[derive(Clone, Copy)]
pub(super) struct Grown<'a> {
    pub(super) models: &'a Models,
    pub(super) tau: Tau,
}

/// One ray's decor strike, carried along its ground walk.
pub(super) struct Strike<'a> {
    models: &'a Models,
    /// How far any decor reaches from where it stands, and in leaf cells.
    pub(super) reach: f64,
    around: i32,
    /// How far along the ray decor is drawn as held.
    pub(super) held: f64,
    /// The cell met last, whose neighbors are struck already.
    last: Option<[i32; 2]>,
    pub(super) best: Option<(f64, Instance)>,
    /// What is placed about the nodes the walk went through (`Around`).
    pub(super) about: Around,
    /// The finest zoom whose nodes are as wide as the reach: a node finer
    /// is looked about at it (`about_zoom`).
    reach_zoom: Zoom,
    /// Where placed decor may stand along z for the ray: its height over
    /// the held span, widened by the reach; and the finest zoom whose nodes
    /// are wider than a leaf cell's box about it, the band's or the
    /// reach's, which the cell's box lies strictly within the 27 of
    /// (`strike_near`). Each the strike's own, the same at every cell.
    band: [f64; 2],
    box_zoom: Zoom,
}

impl Strike<'_> {
    /// The zoom a node at `zoom` is looked about at: its own, or the finest
    /// whose nodes are as wide as the reach.
    pub(super) fn about_zoom(&self, zoom: Zoom) -> Zoom {
        zoom.min(self.reach_zoom)
    }
}

/// The finest zoom, no finer than the walk goes, whose nodes' side `fits`;
/// 0 where none does. Sides grow as zooms coarsen, so a node at a zoom
/// finer than it is looked at it, and one at it or coarser at its own.
fn finest(fits: impl Fn(f64) -> bool) -> Zoom {
    let mut z = cast::DEPTH as Zoom - 1;
    while z > 0 && !fits(units(z)) {
        z -= 1;
    }
    z
}

/// What is placed in the 27 nodes about the node a striking walk is at, at
/// each zoom it went through: the objects' tree's cursors there
/// (pxl-spec-core `overwrite::Held`), each zoom's the children of the zoom
/// over's, as a node's neighbours are its parent's neighbours' children
/// (Samet 1989, neighbour finding). A walk enters a node through its
/// parent, so each zoom's is stepped from the one over in 27 `child` reads,
/// never walked from the roots.
pub(super) struct Around {
    at: [[i32; 3]; cast::DEPTH],
    held: [[Held; 27]; cast::DEPTH],
    /// The 3 × 3 columns about it, a zoom each, as the ground's relief reads
    /// them (`columns`).
    columns_at: [[i32; 2]; cast::DEPTH],
    columns: [[Option<Column>; 9]; cast::DEPTH],
}

/// A column about a striking walk's node, at its zoom, as its relief read
/// it: the ground's line its cells are read on, where they were found, and
/// the highest its relief reaches, none where it lacks.
#[derive(Clone, Copy)]
struct Column {
    line: Line,
    found: Found,
    hi: Option<i32>,
}

impl Around {
    pub(super) fn new() -> Around {
        Around {
            at: [[i32::MIN; 3]; cast::DEPTH],
            held: [[Held::NOTHING; 27]; cast::DEPTH],
            columns_at: [[i32::MIN; 2]; cast::DEPTH],
            columns: [[None; 9]; cast::DEPTH],
        }
    }

    /// The node `(zoom, c)` entered: its 27, off its parent's where they are
    /// the ones held, else each down from the roots.
    pub(super) fn visit(&mut self, lanes: &impl overwrite::Lanes, (zoom, c): Node<3>) {
        let z = zoom as usize;
        if self.at[z] == c {
            return;
        }
        let p = c.map(|v| v >> 1);
        let stepped = zoom > 0 && self.at[z - 1] == p;
        for k in 0..27 {
            let n = [c[0] + (k % 3) as i32 - 1, c[1] + (k / 3 % 3) as i32 - 1, c[2] + (k / 9) as i32 - 1];
            self.held[z][k] = if stepped {
                let q = [(n[0] >> 1) - p[0] + 1, (n[1] >> 1) - p[1] + 1, (n[2] >> 1) - p[2] + 1];
                overwrite::child(lanes, self.held[z - 1][(q[0] + 3 * q[1] + 9 * q[2]) as usize], zoom, n)
            } else {
                // Y and Z run over the two roots of zoom 0 alone; X round them.
                let inside = (-1..=0).contains(&(n[1] >> zoom)) && (-1..=0).contains(&(n[2] >> zoom));
                if inside {
                    overwrite::cursor(lanes, overwrite::PLACED, zoom, n)
                } else {
                    Held::NOTHING
                }
            };
        }
        self.at[z] = c;
    }

    /// The 3 × 3 columns about `(zoom, c)` read at its tile's cells: each
    /// column's line off its parent column's (`line_under`), its cells found
    /// from where its parent column's were (pxl-spec-core `window::step`),
    /// and its relief's highest, read once for every node under it. A walk
    /// enters a node through its parent, so each zoom's is stepped from the
    /// one over's, never walked from the top.
    pub(super) fn read_columns(&mut self, world: &World, (zoom, c): (Zoom, [i32; 2])) {
        let z = zoom as usize;
        if self.columns_at[z] == c {
            return;
        }
        let p = [c[0] >> 1, c[1] >> 1];
        let stepped = zoom > 0 && self.columns_at[z - 1] == p;
        let cell = GROUND.grid_zoom(zoom.min(LEAF_ZOOM));
        for k in 0..9 {
            let n = [c[0] + (k % 3) as i32 - 1, c[1] + (k / 3) as i32 - 1];
            let over = if stepped {
                self.columns[z - 1][((n[0] >> 1) - p[0] + 1 + 3 * ((n[1] >> 1) - p[1] + 1)) as usize]
            } else {
                None
            };
            let line = world.terrain.line_under(over.as_ref().map(|o| &o.line), (zoom, n), cell);
            let mut found = over.map_or(Found::NONE, |o| o.found);
            let hi = world.terrain.relief(&line, &mut found, (zoom, n), [cell, cell]).value().map(|[[_, hi], _]| hi);
            self.columns[z][k] = Some(Column { line, found, hi });
        }
        self.columns_at[z] = c;
    }

    /// The highest the relief reaches in each of the 3 × 3 columns about
    /// `(zoom, a)`, none where it lacks: the walk read them entering `a`
    /// (`read_columns`), the last it entered at `zoom`.
    fn reliefs(&self, (zoom, a): (Zoom, [i32; 2])) -> [Option<i32>; 9] {
        let z = zoom as usize;
        debug_assert!(self.columns_at[z] == a, "the walk entered the node it reads about");
        self.columns[z].map(|c| c.and_then(|c| c.hi))
    }

    /// The nodes the objects' tree holds about `a` at `zoom` or over them,
    /// each with what is held there, each once (a leaf over several is one)
    /// and in the tree's order (`tree_key`), and how many: what a walk from
    /// the roots pruned to a box within them enters first
    /// (`Store::entries_under`). None where the walk did not go through `a`.
    fn under(&self, (zoom, a): Node<3>) -> Option<([(Node<3>, Held); 27], usize)> {
        let z = zoom as usize;
        if self.at[z] != a {
            return None;
        }
        let (mut out, mut n) = ([((0, [0; 3]), Held::NOTHING); 27], 0);
        for (k, &held) in self.held[z].iter().enumerate() {
            if !held.holds() {
                continue;
            }
            let c = [a[0] + (k % 3) as i32 - 1, a[1] + (k / 3 % 3) as i32 - 1, a[2] + (k / 9) as i32 - 1];
            let h = held.zoom();
            let node = (h, overwrite::path(h, c.map(|v| v >> (zoom - h))));
            if !out[..n].iter().any(|&(o, _)| o == node) {
                out[n] = (node, held);
                n += 1;
            }
        }
        out[..n].sort_unstable_by_key(|&((h, c), _)| tree_key(h, c));
        Some((out, n))
    }

    /// Whether anything, air included, is placed in the 27 nodes about `a`
    /// at `zoom` or over them: the walk went through `a` (`visit`), the last
    /// it entered at `zoom`.
    fn holds(&self, (zoom, a): Node<3>) -> bool {
        let z = zoom as usize;
        debug_assert!(self.at[z] == a, "the walk entered the node it reads about");
        self.held[z].iter().any(|h| h.holds())
    }
}

impl<'a> World<'a> {
    /// Whether the ground grows `i`, a candidate of `decor` standing at the
    /// leaves' cells, as the frame drawing them draws it (pxl-spec-core
    /// `grows`): its box void, what is under it a look it grows on, and no
    /// placed decor claiming its room.
    pub fn grows(&self, decor: &Decor, i: &Instance) -> bool {
        grown_of(i).is_some_and(|g| pxl_spec_core::scatter::grows(&mut Asked::new(self), decor.row, GROUND.cell(), &g))
    }

    /// What the ground stands on the leaf `cell`, each decor's, handed to
    /// `each` until `each` says it has what it wants: each on its column's
    /// footing (`World::footing`). Nothing where the world does not hold
    /// the column.
    fn grown_in(&self, cell: [i32; 2], mut each: impl FnMut(&Decor, Instance) -> bool) -> bool {
        DECORS.iter().any(|decor| grown_at(&mut Asked::new(self), decor, cell).is_some_and(|i| each(decor, i)))
    }

    /// A placed decor whose box shares room with `space`.
    pub fn placed_claim(&self, space: &Aabb) -> Option<Instance> {
        let mut met = None;
        self.each_placed_claim(space, |_, i| {
            met = Some(i);
            true
        });
        met
    }

    /// Each placed decor whose box shares room with `space`, with that box,
    /// until `each` says it has what it wants.
    fn each_placed_claim(&self, space: &Aabb, mut each: impl FnMut(Aabb, Instance) -> bool) {
        let r = reach();
        let around = Aabb {
            min: space.min.map(|v| v - r),
            max: space.max,
        };
        let mut done = false;
        self.objects.entries_in(&around, &mut |e| {
            if done {
                return;
            }
            if let Some((c, i)) = placed(e).and_then(|i| claim(&i).filter(|c| c.shares(*space)).map(|c| (c, i))) {
                done = each(c, i);
            }
        });
    }

    /// A decor the ground grows on the leaves whose box shares room with `space`.
    pub fn grown_claim(&self, space: &Aabb) -> Option<Instance> {
        let mut met = None;
        self.each_grown_over(space, |decor, i| {
            met = Some(i).filter(|i| self.grows(decor, i));
            met.is_some()
        });
        met
    }

    /// Every decor the ground's scatter stands on the leaves whose box shares
    /// room with `space`, whether or not its footing is there now: what an
    /// erase took away included.
    pub fn grown_over(&self, space: &Aabb) -> Vec<Instance> {
        let mut out = Vec::new();
        self.each_grown_over(space, |_, i| {
            out.push(i);
            false
        });
        out
    }

    /// Until `each` says it has what it wants.
    fn each_grown_over(&self, space: &Aabb, mut each: impl FnMut(&Decor, Instance) -> bool) {
        let (r, unit) = (reach(), units(GROUND.cell()));
        let cells = |k: usize| libm::floor((space.min[k] - r) / unit) as i32..libm::ceil(space.max[k] / unit) as i32;
        for y in cells(1) {
            for x in cells(0) {
                if self.grown_in([x, y], |decor, i| claim(&i).is_some_and(|c| c.shares(*space)) && each(decor, i)) {
                    return;
                }
            }
        }
    }

    /// What the ground grows of `decor` standing in the leaf cell `at` lies
    /// in; none where it grows nothing there or the windows do not hold it.
    pub fn grown(&self, decor: &Decor, at: &Q3) -> Option<Instance> {
        let [x, y, z] = index_of(GROUND.cell(), at);
        let mut met = None;
        self.grown_in([x, y], |d, i| {
            met = Some(i).filter(|i| d.value() == decor.value() && i.grown_in().is_some_and(|c| c[2] == z));
            met.is_some()
        });
        met
    }

    /// The strike a ray carries, where decor is struck at all.
    pub(super) fn strike(&self, ray: &Ray) -> Option<Strike<'a>> {
        let Grown { models, tau } = self.decor?;
        // Held as it is out to where the leaves' tiles end.
        let held = match GROUND.z_at(tau, 0.0) {
            LEAF_ZOOM => (demand_reach(GROUND, LEAF_ZOOM) / tau.scale()).min(ray.reach),
            _ => return None,
        };
        let reach = reach();
        let around = reach.ceil() as i32;
        let (za, zb) = (ray.at(0.0)[2], ray.at(held)[2]);
        let band = [za.min(zb) - reach, za.max(zb) + reach];
        Some(Strike {
            models,
            reach,
            around,
            held,
            last: None,
            best: None,
            about: Around::new(),
            reach_zoom: finest(|side| side >= reach),
            band,
            box_zoom: finest(|side| side > f64::from(around + 1).max(band[1] - band[0])),
        })
    }

    /// Strikes the decor standing near the leaf cell `cell` the walk is in:
    /// those not near the cell met before it, since a walk only moves on.
    pub(super) fn strike_near(&self, s: &mut Strike<'a>, ray: &Ray, (zoom, at): Node<3>) {
        let cell = [at[0], at[1]];
        let (r, last) = (s.around, s.last.replace(cell));
        let near = |a: [i32; 2], b: [i32; 2]| (a[0] - b[0]).abs() <= r && (a[1] - b[1]).abs() <= r;
        // Placed decor stands where the ray's height over the held span, widened by the reach, can meet it.
        let [z0, z1] = s.band;
        let fresh = |xy: [i32; 2]| !last.is_some_and(|l| near(xy, l));
        for y in cell[1] - r..=cell[1] + r {
            for x in cell[0] - r..=cell[0] + r {
                if fresh([x, y]) {
                    self.grown_near(s, ray, [x, y]);
                }
            }
        }
        // What is placed about it, the objects' tree walked once over every
        // cell: met in the cell its box's corner is in, once.
        let space = Aabb {
            min: [(cell[0] - r) as f64, (cell[1] - r) as f64, z0],
            max: [(cell[0] + r + 1) as f64, (cell[1] + r + 1) as f64, z1],
        };
        // From the nodes about it at the finest zoom wider than the box, whose
        // 27 the walk stepped down to (`Around`): the cell lies in the middle
        // one, and the ray's height there within the band, so the box lies
        // strictly within the 27, touching none past them.
        let wide = zoom.min(s.box_zoom);
        let from = s.about.under((wide, at.map(|v| v >> (zoom - wide))));
        let mut visit = |e: &crate::domain::layer::object::store::Entry| {
            let c = e.cube().min;
            let own = [c[0].floor() as i32, c[1].floor() as i32];
            if let Some(i) = placed(e).filter(|_| space.holds(c) && fresh(own)) {
                meet(s, ray, i);
            }
        };
        match from {
            Some((from, n)) => self.objects.entries_under(&from[..n], &space, &mut visit),
            None => self.objects.entries_in(&space, &mut visit),
        }
    }

    /// Whether decor `s` strikes may reach into the node `(zoom, c)`, X on
    /// the asker's lap, where no block under it fills: decor stands on the
    /// ground or where something is placed, and reaches no farther than its
    /// reach from there, so a node is out of it where, about it and as wide
    /// as the reach, the windows' relief holds no top over its bottom less the
    /// reach and the objects' pyramid no record, past the area none, as
    /// `shard`, what the descent established at the node, says. What is not
    /// held may.
    ///
    /// Read off what the walk holds about the node's ancestor at the zoom it
    /// is looked about at (`Strike::about_zoom`, `Around`), its last entered
    /// there: the walk enters every node through its parent, an ancestor
    /// before its children and no later along the ray, so under the held
    /// span as they are; and it entered no other node at that zoom since,
    /// a node it comes back to being as fine or finer.
    pub(super) fn may_strike(&self, s: &Strike<'a>, (zoom, c): Node<3>, shard: Shard) -> bool {
        let z = s.about_zoom(zoom);
        let a = c.map(|v| v >> (zoom - z));
        let low = cube(zoom, c).min[2] - s.reach;
        let leaf = units(GROUND.cell());
        let node = (zoom, [c[0], c[1]]);
        let read = s.about.reliefs((z, [a[0], a[1]]));
        let ground = (-1..=1).any(|dy| {
            (-1..=1).any(|dx| {
                // Past the area no ground stands.
                if z >= SHARD_ZOOM && shard.past_leaf(node, area::tile_at(z, [a[0] + dx, a[1] + dy])) {
                    return false;
                }
                read[(dx + 1 + 3 * (dy + 1)) as usize].is_none_or(|hi| f64::from(hi) * leaf > low)
            })
        });
        ground || s.about.holds((z, a))
    }

    fn grown_near(&self, s: &mut Strike<'a>, ray: &Ray, cell: [i32; 2]) {
        self.grown_in(cell, |decor, i| {
            if self.grows(decor, &i) {
                meet(s, ray, i);
            }
            false
        });
    }
}

/// The box a decor thing claims.
pub(super) fn claim(i: &Instance) -> Option<Aabb> {
    let Meta::Decor(m) = i.meta;
    Some(Decor::of(DecorId::of(i.id)?).claim(i.at, &m))
}

fn meet(s: &mut Strike, ray: &Ray, i: Instance) {
    if let Some(t) = strike(s.models, &i, ray.origin, ray.direction, s.best.map_or(s.held, |b| b.0)) {
        s.best = Some((t, i));
    }
}
