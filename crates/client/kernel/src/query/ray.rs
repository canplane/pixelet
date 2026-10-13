// pxl-kernel/query/ray.rs

//! The first block a ray meets, at the zoom its distance asks
//! (decision/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md): it
//! descends the pyramid by the rule of detail that sizes every drawn cell
//! (`Tau::cell_at`) and no other. A node coarser than the zoom its distance
//! asks, from its point nearest the eye, is split, its children taken in the
//! order the ray meets them; a node at that zoom is asked the world's one
//! query (`World::block`), and the ray goes on past it where it is air or
//! void, else stops there.
//!
//! From a node the ray enters the child it meets next, and past a node it
//! climbs, by returning, only as far as the ancestor holding the next one.
//! A node is at most eight slab tests of its children, or one ask.

use super::decor::Strike;
use super::node::{coarsest, At, In};
use super::world::World;
use crate::layer::instance::{Instance, Source};
use crate::layer::object::store::EntryAt;
use crate::layer::Answer;
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::block::Block;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::v3::{len3, sub3, V3};
use pxl_spec::space::coord::world::wrap_cell;
use pxl_spec::space::coord::zoom::{cube, units, Zoom};
use pxl_spec::space::hierarchy::tree::child_of;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::Tile;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Detail {
    /// The cells of tiles at this zoom, wherever they are.
    Zoom(Zoom),
    /// The camera grid's cells at each distance from the origin, the rule
    /// that sizes every drawn cell.
    Distance(Tau),
}

impl Detail {
    /// The zoom asked `d` from the eye: a stated tile zoom's cells, or the
    /// camera grid's there (`Tau::cell_at`), whose block, drawn whole, spans
    /// at least one cell of the grid from any direction; no coarser than a
    /// tile draws and no finer than what is placed is held.
    pub(super) fn at(self, d: f64) -> Zoom {
        let zoom = match self {
            Detail::Zoom(z) => GROUND.cell_zoom(z),
            Detail::Distance(tau) => tau.cell_at(d),
        };
        zoom.clamp(coarsest(), OBJECT_FINEST)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub origin: V3,
    pub direction: V3,
    pub reach: f64,
}

impl Ray {
    pub fn at(&self, t: f64) -> V3 {
        std::array::from_fn(|k| self.origin[k] + self.direction[k] * t)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Struck {
    Ground,
    Object(EntryAt),
    /// Objects folded into a cell above the leaves; no single object is met.
    Objects,
    /// A decor thing, struck by its model: grown by the ground or placed.
    Decor(Instance),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub t: f64,
    pub point: V3,
    pub normal: V3,
    /// The box struck.
    pub cube: Aabb,
    pub struck: Struck,
}

/// What a node gave the ray: where it stops, that it goes on past it, or
/// the tile a block it met lacks, past which it tells nothing.
enum Met {
    Hit(Hit),
    On,
    Lacks(Tile),
}

/// Which block a ray stops at: the first that fills, as an eye sees; or the
/// first that does not, as what is climbed out of ends.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Until {
    Fills,
    Air,
}

/// What a descent carries along the ray: what it stops at, the first tile
/// lacked, whether any node was answered, and the decor struck.
struct Walk<'a, 'r> {
    ray: &'r Ray,
    detail: Detail,
    until: Until,
    lack: Option<Tile>,
    answered: bool,
    strike: Option<Strike<'a>>,
}

/// Where the ray is within `b` between `t0` and `t1`: its entry and exit,
/// and the axis it enters across.
fn slab(ray: &Ray, b: &Aabb, (t0, t1): (f64, f64)) -> Option<(f64, f64, usize)> {
    let (enter, leave, axis) = b.span_along(ray.origin, ray.direction)?;
    let (enter, leave) = (enter.max(t0), leave.min(t1));
    (enter <= leave).then_some((enter, leave, axis))
}

/// The axis across which the ray leaves `b`: the first of its far faces it
/// reaches.
fn leaves(ray: &Ray, b: &Aabb) -> usize {
    let at = |k: usize| match ray.direction[k] {
        d if d > 0.0 => (b.max[k] - ray.origin[k]) / d,
        d if d < 0.0 => (b.min[k] - ray.origin[k]) / d,
        _ => f64::INFINITY,
    };
    (0..3).min_by(|&a, &b| at(a).total_cmp(&at(b))).unwrap_or(0)
}

/// A decor struck, at its base cell's box: a decor is stood on nowhere.
fn decor_hit(ray: &Ray, t: f64, i: Instance) -> Hit {
    let base = match i.source {
        Source::Ground(cell) => cube(GROUND.cell(), cell),
        Source::Object(e) => cube(e.zoom(), e.at()),
    };
    Hit {
        t,
        point: ray.at(t),
        normal: ray.direction.map(|v| -v),
        cube: base,
        struck: Struck::Decor(i),
    }
}

impl<'a> World<'a> {
    /// The first block `ray` meets within its reach that fills, at the zoom
    /// `detail` asks where it meets it, or the decor struck nearer. What the
    /// ray starts inside is not met, so an eye under the ground is not
    /// stopped where it stands. A block met coarser than held is the lack;
    /// one not held at all is the lack too, past which the ray tells nothing.
    /// Decor, which is no block, is struck at the leaf cells it passes.
    pub fn first_hit(&self, ray: &Ray, detail: Detail) -> Answer<Option<Hit>> {
        let (met, strike) = self.along(ray, detail, Until::Fills, self.strike(ray));
        let decor = strike.and_then(|s| s.best).filter(|&(d, _)| met.value().flatten().is_none_or(|h: Hit| d < h.t));
        match (decor, met) {
            (Some((d, i)), Answer::Nothing(l)) => Answer::Coarser(Some(decor_hit(ray, d, i)), l),
            (Some((d, i)), met) => met.map(|_| Some(decor_hit(ray, d, i))),
            (None, met) => met,
        }
    }

    /// The first block along `ray` that `until` stops at, at the zoom `detail`
    /// asks where it meets it: down the pyramid from the world's roots, the
    /// cubes of zoom 0 it crosses, the two of the world's height (`descend`).
    pub(super) fn along(&self, ray: &Ray, detail: Detail, until: Until, strike: Option<Strike<'a>>) -> (Answer<Option<Hit>>, Option<Strike<'a>>) {
        let mut walk = Walk {
            ray,
            detail,
            until,
            lack: None,
            answered: false,
            strike,
        };
        let mut met = None;
        let mut root = ray.origin.map(|v| (v / units(0)).floor() as i64);
        let mut t = 0.0;
        while t < ray.reach {
            let Some(span) = slab(ray, &cube(0, root), (t, ray.reach)) else { break };
            match self.descend(&mut walk, &mut self.start((0, root)), span) {
                Met::Hit(h) => {
                    met = Some(h);
                    break;
                }
                Met::Lacks(l) => {
                    walk.lack.get_or_insert(l);
                    break;
                }
                // On to the root across the face the ray leaves by.
                Met::On => {
                    let k = leaves(ray, &cube(0, root));
                    root[k] += ray.direction[k].signum() as i64;
                    t = span.1;
                }
            }
        }
        let answer = match (met, walk.lack, walk.answered) {
            (h, None, _) => Answer::Exact(h),
            (Some(h), Some(l), _) => Answer::Coarser(Some(h), l),
            (None, Some(l), true) => Answer::Coarser(None, l),
            (None, Some(l), false) => Answer::Nothing(l),
        };
        (answer, walk.strike)
    }

    /// The first block in `at`'s node the walk stops at, the ray within it
    /// from `t0` to `t1`, entering across `axis`. Coarser than the zoom its
    /// distance asks, the node is asked whether anything under it fills at
    /// the zooms the ray asks within it: nothing, and the ray goes on past it
    /// whole, or for air stops at once; else its children the ray meets,
    /// nearest first, each handed what this one found. At that zoom, the
    /// block (`stop`).
    fn descend(&self, walk: &mut Walk<'a, '_>, at: &mut At, span @ (t0, t1, _): (f64, f64, usize)) -> Met {
        // Only touched, at a face or an edge, as the face a ray leaves from
        // where it starts: not entered, nothing in it met.
        if t1 <= t0 {
            return Met::On;
        }
        self.visit();
        let ray = walk.ray;
        let (zoom, c) = at.node;
        let whole = cube(zoom, c);
        // Decor is no block: a leaf cell the ray passes within where decor
        // is drawn as held strikes what stands near it.
        if zoom == GROUND.cell() {
            if let Some(s) = walk.strike.as_mut().filter(|s| t0 < s.held) {
                self.strike_near(s, ray, [c[0], c[1]]);
            }
        }
        // The zooms the ray asks within it: at its point nearest the eye, and
        // as far as where the ray leaves it.
        let zoom_at = |p: V3| walk.detail.at(len3(&sub3(&p, &ray.origin)));
        let asked = [zoom_at(ray.at(t1)), zoom_at(whole.nearest(ray.origin))];
        let answer = match self.node(at, asked) {
            Answer::Nothing(l) => return Met::Lacks(l),
            Answer::Coarser(i, l) => {
                walk.lack.get_or_insert(l);
                i
            }
            Answer::Exact(i) => i,
        };
        walk.answered = true;
        // No block under it fills, but decor, which is no block, may stand
        // in it: down to the leaf cells, where it is struck.
        let decor = zoom < GROUND.cell() && walk.strike.as_ref().is_some_and(|s| t0 < s.held && self.may_strike(s, at.node));
        match answer {
            In::Empty if !decor => match walk.until {
                Until::Fills => Met::On,
                Until::Air => self.stop(walk, at, span, None),
            },
            In::Block(b) => self.stop(walk, at, span, b),
            In::Empty | In::Split => {
                let mut kids = [(0.0, 0.0, 0, 0); 8];
                let mut n = 0;
                for k in 0..8 {
                    if let Some((enter, leave, axis)) = slab(ray, &cube(zoom + 1, child_of(c, k)), (t0, t1)) {
                        kids[n] = (enter, leave, axis, k);
                        n += 1;
                    }
                }
                kids[..n].sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
                for &(enter, leave, axis, k) in &kids[..n] {
                    match self.descend(walk, &mut self.child(at, k), (enter, leave, axis)) {
                        Met::On => {}
                        met => return met,
                    }
                }
                Met::On
            }
        }
    }

    /// What the walk does at `at`'s node, `block` what it is at its zoom,
    /// none for air or void and for a node nothing under fills: for an eye,
    /// stops where it fills, but not in the block it starts in; for what is
    /// climbed out of, stops where it does not.
    fn stop(&self, walk: &Walk<'a, '_>, at: &At, (t0, _, axis): (f64, f64, usize), block: Option<Block>) -> Met {
        let ray = walk.ray;
        let (zoom, c) = at.node;
        let whole = cube(zoom, c);
        let fills = block.is_some_and(|b| b.fills());
        let stops = match walk.until {
            Until::Fills => fills && !(0..3).all(|k| whole.min[k] <= ray.origin[k] && ray.origin[k] < whole.max[k]),
            Until::Air => !fills,
        };
        if !stops {
            return Met::On;
        }
        let mut normal = [0.0; 3];
        normal[axis] = if ray.direction[axis] > 0.0 { -1.0 } else { 1.0 };
        let struck = match block {
            Some(Block::Placed(_)) => self.objects.placed(zoom, wrap_cell(zoom, c)).map_or(Struck::Objects, |e| Struck::Object(e.id())),
            _ => Struck::Ground,
        };
        Met::Hit(Hit {
            t: t0,
            point: ray.at(t0),
            normal,
            cube: whole,
            struck,
        })
    }
}
