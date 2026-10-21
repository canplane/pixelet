// pxl-kernel/query/ray.rs

//! The first block a ray meets, at the zoom its distance asks
//! (doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md): the
//! descent is pxl-portable's (`cast::first`), the one a GPU's cast runs too,
//! and the world answers it here from everything it holds, each node handed
//! what its parent found (`Walker`). What only the CPU answers, decor,
//! rides on that answer.

use super::decor::Strike;
use super::node::{coarsest, At};
use super::world::World;
use crate::layer::instance::{Instance, Source};
use crate::layer::residency::Lines;
use crate::layer::Answer;
use pxl_portable::answer::{self, In, Kind};
pub(super) use pxl_portable::cast::Until;
use pxl_portable::cast::{self, Detail, Ends, Query};
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::{point, wrap_cell};
use pxl_spec::space::coord::zoom::{cube, units, Zoom};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::Tile;

/// The zooms a block is asked at: no coarser than a tile draws and no finer
/// than what is placed is held.
pub fn asked() -> [Zoom; 2] {
    [coarsest(), OBJECT_FINEST]
}

/// The cells of tiles at the zoom `tile` wherever they are, as the descent
/// reads a level of detail (pxl-portable `cast::Detail`): a range of that
/// zoom's cells alone, held to the zooms a block is asked at (`asked`).
pub fn at_zoom(tile: Zoom) -> Detail {
    let [lo, hi] = asked();
    let z = GROUND.cell_zoom(tile).clamp(lo, hi);
    Detail { tau: 1.0, range: [z, z] }
}

/// The camera grid's cells at each distance from the origin at `tau`
/// (`Tau::cell_at`), whose block, drawn whole, spans at least one cell of
/// the grid from any direction: the rule that sizes every drawn cell.
pub fn by_distance(tau: Tau) -> Detail {
    Detail {
        tau: tau.scale() as f32,
        range: asked(),
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

/// The world as the descent asks it, with what only the CPU carries along
/// a ray: the decor struck, the first tile lacked, and whether any node was
/// answered.
/// The walk's query as a type, for what reads its line after it
/// (pxl-portable `cast::Line::held`).
pub(super) type Walking = Walker<'static, 'static>;

/// A footing as `World::footing` answers it.
pub(super) struct Footing {
    pub block: u32,
    pub top: i32,
    /// What its walk holds of its line (pxl-portable `cast::Line`).
    pub line: cast::Line<At, At>,
    pub lines: Lines,
}

pub(super) struct Walker<'w, 'a> {
    world: &'w World<'a>,
    lack: Option<Tile>,
    answered: bool,
    /// The decor struck along the ray, where the walk strikes any.
    strike: Option<(&'w Ray, Strike<'a>)>,
    /// The ground's lines the walk read, each node's stepped from them
    /// (`World::node`).
    lines: Lines,
}

impl<'a> Query for Walker<'_, 'a> {
    type At = At;
    /// Kept whole: the CPU's walk holds its stack where it is cheap.
    type Kept = At;

    fn root(&mut self, node: [i32; 3]) -> At {
        self.world.start((0, node))
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
    fn node(&mut self, at: &mut At, _: Zoom, _: [i32; 3], asked: [Zoom; 2], t0: f32) -> In<[u32; 2]> {
        let (zoom, c) = at.node;
        let t0 = f64::from(t0);
        // What is placed and where the ground stands about it, off what was
        // about its parent, while decor is struck there.
        if let Some((_, s)) = self.strike.as_mut() {
            s.about.visit(&self.world.objects.lanes(), at.node);
            if t0 < s.held && s.about_zoom(zoom) == zoom {
                s.about.read_columns(self.world, (zoom, [c[0], c[1]]));
            }
        }
        if zoom == GROUND.cell() {
            if let Some((ray, s)) = self.strike.as_mut().filter(|(_, s)| t0 < s.held) {
                self.world.strike_near(s, ray, (zoom, c));
            }
        }
        let (answer, block) = match self.world.node(at, asked, &mut self.lines) {
            Answer::Nothing(l) => {
                self.lack.get_or_insert(l);
                return In {
                    kind: Kind::Lacks,
                    block: answer::AIR,
                };
            }
            Answer::Coarser(i, l) => {
                self.lack.get_or_insert(l);
                i
            }
            Answer::Exact(i) => i,
        };
        self.answered = true;
        let decor = zoom < GROUND.cell() && self.strike.as_ref().is_some_and(|(_, s)| t0 < s.held && self.world.may_strike(s, at.node, at.shard));
        match answer.kind {
            // Air coarser than asked, gone into where decor may stand in it.
            Kind::Block if !block && decor => In {
                kind: Kind::Split,
                block: [answer::slab_word(answer::ANY), 0],
            },
            _ => answer,
        }
    }

    fn keep(at: &At) -> At {
        *at
    }

    fn unkeep(kept: At, _: &At, _: Zoom) -> At {
        kept
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
            lack: None,
            answered: false,
            strike: strike.map(|s| (ray, s)),
            lines: Lines::default(),
        };
        let o = ray.origin;
        let from = cast::Ray::new(ray.direction.map(|v| v as f32), ray.reach as f32);
        let met = cast::first(&mut walker, &cast::Origin::of(point(o[0], o[1], o[2])), &from, detail, until);
        let met = (met.ends == Ends::Stop).then(|| self.hit(ray, met.stop));
        let answer = match (met, walker.lack, walker.answered) {
            (h, None, _) => Answer::Exact(h),
            (Some(h), Some(l), _) => Answer::Coarser(Some(h), l),
            (None, Some(l), true) => Answer::Coarser(None, l),
            (None, Some(l), false) => Answer::Nothing(l),
        };
        (answer, walker.strike.map(|(_, s)| s))
    }

    /// The footing down column `g` at `cell` (pxl-portable
    /// `scatter::footing`): the block met and the block over it, where the
    /// world holds the column as its tiles say; none where nothing fills or
    /// a tile under the ray is lacked or only a coarser one stands in. With
    /// what its walk holds of its line down to it, and the ground's lines it
    /// read, which what is asked about it steps from (`Footing`).
    pub(super) fn footing(&self, cell: Zoom, g: [i32; 2]) -> Option<Footing> {
        let mut walker = Walker {
            world: self,
            lack: None,
            answered: false,
            strike: None,
            lines: Lines::default(),
        };
        let ((block, top, met), line) = pxl_portable::scatter::footing(&mut walker, cell, g);
        (met && walker.lack.is_none()).then_some(Footing {
            block,
            top,
            line,
            lines: walker.lines,
        })
    }

    /// Where the descent stopped, in units: its node's box put back on the
    /// unit cell holding the origin, so it lies on the origin's lap.
    fn hit(&self, ray: &Ray, s: cast::Stop<At>) -> Hit {
        let (zoom, c) = s.at.node;
        let side = units(zoom);
        let unit = ray.origin.map(libm::floor);
        // A corner counts units, or its own cells where they are finer.
        let per = if zoom > pxl_portable::UNIT_ZOOM { side } else { 1.0 };
        let min: V3 = std::array::from_fn(|k| unit[k] + f64::from(s.place.corner[k]) * per);
        let axis = s.axis as usize;
        let mut normal = [0.0; 3];
        normal[axis] = if ray.direction[axis] > 0.0 { -1.0 } else { 1.0 };
        let struck = match answer::placed(s.block[0]) {
            true => self.objects.placed(zoom, wrap_cell(zoom, c), s.at.placed).map_or(Struck::Objects, |e| Struck::Object(e.id())),
            false => Struck::Ground,
        };
        let t = f64::from(s.t);
        Hit {
            t,
            point: ray.at(t),
            normal,
            cube: Aabb { min, max: min.map(|v| v + side) },
            struck,
        }
    }
}
