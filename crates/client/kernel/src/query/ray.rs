// pxl-kernel/query/ray.rs

//! The first block a ray meets, at the zoom its distance asks
//! (doc/decision/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md): the
//! descent is pxl-portable's (`cast::first`), the one a GPU's cast runs too,
//! and the world answers it here from everything it holds, each node handed
//! what its parent found (`Walker`). What only the CPU answers — the ways,
//! what is placed, decor — rides on that answer.

use super::decor::Strike;
use super::node::{coarsest, At, In};
use super::world::World;
use crate::layer::instance::{Instance, Source};
use crate::layer::object::store::EntryAt;
use crate::layer::Answer;
pub(super) use pxl_portable::cast::Until;
use pxl_portable::cast::{self, Ends, Kind, Query};
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::block::Block;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::{point, wrap_cell};
use pxl_spec::space::coord::zoom::{cube, units, Zoom};
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

/// The zooms a block is asked at: no coarser than a tile draws and no finer
/// than what is placed is held.
pub fn asked() -> [Zoom; 2] {
    [coarsest(), OBJECT_FINEST]
}

impl Detail {
    /// The rule as the descent reads it: a stated tile zoom's cells, or the
    /// camera grid's at each distance (`Tau::cell_at`), whose block, drawn
    /// whole, spans at least one cell of the grid from any direction.
    /// A stated zoom is a range of that zoom alone.
    fn portable(self) -> cast::Detail {
        let [lo, hi] = asked();
        match self {
            Detail::Zoom(z) => {
                let z = GROUND.cell_zoom(z).clamp(lo, hi);
                cast::Detail { tau: 1.0, range: [z, z] }
            }
            Detail::Distance(tau) => cast::Detail {
                tau: tau.scale() as f32,
                range: [lo, hi],
            },
        }
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
    /// What the block met is, as the world answers it at its zoom; none for
    /// decor, which is no block, and for what is climbed out of into air.
    pub block: Option<Block>,
}

/// The world as the descent asks it, with what only the CPU carries along
/// a ray: the decor struck, the first tile lacked, and whether any node was
/// answered.
struct Walker<'w, 'a> {
    world: &'w World<'a>,
    ray: &'w Ray,
    lack: Option<Tile>,
    answered: bool,
    strike: Option<Strike<'a>>,
}

impl<'a> Query for Walker<'_, 'a> {
    type At = At;
    type Block = Option<Block>;
    const AIR: Option<Block> = None;

    fn root(&mut self, node: [i32; 3]) -> At {
        self.world.start((0, node.map(i64::from)))
    }

    /// The child is the one its index's low bits name.
    fn child(&mut self, at: &At, _: Zoom, node: [i32; 3]) -> At {
        self.world.child(at, (node[0] & 1 | (node[1] & 1) << 1 | (node[2] & 1) << 2) as usize)
    }

    /// The world's answer (`World::node`), the first lack kept; a node
    /// nothing under fills is air its size. Decor is no block: a leaf cell
    /// the ray passes within where decor is drawn as held strikes what
    /// stands near it, and a node no block under fills is still gone into
    /// where decor may stand in it, down to the leaf cells.
    fn node(&mut self, at: &mut At, _: Zoom, _: [i32; 3], asked: [Zoom; 2], t0: f32) -> cast::In<Option<Block>> {
        let (zoom, c) = at.node;
        let t0 = f64::from(t0);
        if zoom == GROUND.cell() {
            if let Some(s) = self.strike.as_mut().filter(|s| t0 < s.held) {
                self.world.strike_near(s, self.ray, [c[0], c[1]]);
            }
        }
        let answer = match self.world.node(at, asked) {
            Answer::Nothing(l) => {
                self.lack.get_or_insert(l);
                return cast::In { kind: Kind::Lacks, block: None };
            }
            Answer::Coarser(i, l) => {
                self.lack.get_or_insert(l);
                i
            }
            Answer::Exact(i) => i,
        };
        self.answered = true;
        let decor = zoom < GROUND.cell() && self.strike.as_ref().is_some_and(|s| t0 < s.held && self.world.may_strike(s, at.node));
        match answer {
            In::Empty if !decor => cast::In { kind: Kind::Block, block: None },
            In::Empty | In::Split => cast::In { kind: Kind::Split, block: None },
            In::Block(b) => cast::In { kind: Kind::Block, block: b },
        }
    }

    fn fills(&self, block: &Option<Block>) -> bool {
        block.is_some_and(|b| b.fills())
    }
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
        block: None,
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
    /// asks where it meets it (pxl-portable `cast::first`), from the ray's
    /// origin as a position names it.
    pub(super) fn along(&self, ray: &Ray, detail: Detail, until: Until, strike: Option<Strike<'a>>) -> (Answer<Option<Hit>>, Option<Strike<'a>>) {
        let mut walker = Walker {
            world: self,
            ray,
            lack: None,
            answered: false,
            strike,
        };
        let o = ray.origin;
        let from = cast::Ray::of(point(o[0], o[1], o[2]), ray.direction.map(|v| v as f32), ray.reach as f32);
        let met = cast::first(&mut walker, &from, detail.portable(), until);
        if let Some(v) = self.visits {
            v.set(v.get() + u64::from(met.visits));
        }
        let met = (met.ends == Ends::Stop).then(|| self.hit(ray, met.stop));
        let answer = match (met, walker.lack, walker.answered) {
            (h, None, _) => Answer::Exact(h),
            (Some(h), Some(l), _) => Answer::Coarser(Some(h), l),
            (None, Some(l), true) => Answer::Coarser(None, l),
            (None, Some(l), false) => Answer::Nothing(l),
        };
        (answer, walker.strike)
    }

    /// Where the descent stopped, in units: its node's box put back on the
    /// unit cell holding the origin, so it lies on the origin's lap.
    fn hit(&self, ray: &Ray, s: cast::Stop<At, Option<Block>>) -> Hit {
        let (zoom, c) = s.at.node;
        let side = units(zoom);
        let unit = ray.origin.map(libm::floor);
        // A corner counts units, or its own cells where they are finer.
        let per = if zoom > pxl_portable::UNIT_ZOOM { side } else { 1.0 };
        let min: V3 = std::array::from_fn(|k| unit[k] + f64::from(s.place.corner[k]) * per);
        let axis = s.axis as usize;
        let mut normal = [0.0; 3];
        normal[axis] = if ray.direction[axis] > 0.0 { -1.0 } else { 1.0 };
        let struck = match s.block {
            Some(Block::Placed(_)) => self.objects.placed(zoom, wrap_cell(zoom, c)).map_or(Struck::Objects, |e| Struck::Object(e.id())),
            _ => Struck::Ground,
        };
        let t = f64::from(s.t);
        Hit {
            t,
            point: ray.at(t),
            normal,
            cube: Aabb { min, max: min.map(|v| v + side) },
            struck,
            block: s.block,
        }
    }
}
