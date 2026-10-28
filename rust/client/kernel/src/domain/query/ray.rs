// pxl-kernel/domain/query/ray.rs

//! The first cell a ray meets, at the zoom its distance asks
//! (doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md): the
//! descent is pxl-spec-core's (`cast::first`), the one a GPU's cast runs too,
//! and the world answers it here from everything it holds, each node handed
//! what its parent found (`Walker`). What only the CPU answers, decor,
//! rides on that answer.

use super::decor::Strike;
use super::node::{coarsest, At};
use super::world::World;
use crate::domain::layer::instance::{Instance, Source};
use crate::domain::layer::residency::Lines;
use crate::domain::layer::{Answer, Base};
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world::{origin, wrap_cell};
use pxl_spec::space::coord::zoom::{cube, units, Zoom};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::Tile;
use pxl_spec_core::answer::{self, In, Kind};
pub(super) use pxl_spec_core::cast::Until;
use pxl_spec_core::cast::{self, Detail, Ends, Query, Spacing};
use pxl_spec_core::support::{Footing, Line};

/// The zooms a cell is asked at: no coarser than a tile draws and no finer
/// than what is placed is held.
pub fn asked() -> [Zoom; 2] {
    [coarsest(), OBJECT_FINEST]
}

/// The cells of tiles at the zoom `tile` wherever they are, as the descent
/// reads a level of detail (pxl-spec-core `cast::Detail`): a range of that
/// zoom's cells alone, held to the zooms a cell is asked at (`asked`).
pub fn at_zoom(tile: Zoom) -> Detail {
    let [lo, hi] = asked();
    let z = GROUND.cell_zoom(tile).clamp(lo, hi);
    Detail {
        tau: 1.0,
        range: [z, z],
        eye: [0.0; 3],
        spacing: Spacing::NONE,
    }
}

/// What is held, as finely as it is held: the ground at its cells, which
/// answer alike asked finer, and what is placed at its own. Where the eye
/// draws it coarser is the asker's to judge by `by_distance`
/// (domain/control/interaction.rs `Picked`).
pub fn held() -> Detail {
    let [_, finest] = asked();
    Detail {
        tau: 1.0,
        range: [finest, finest],
        eye: [0.0; 3],
        spacing: Spacing::NONE,
    }
}

/// The camera grid's cells at each distance from the origin at `tau`
/// (`Tau::cell_at`), whose cell, drawn whole, spans at least one cell of
/// the grid from any direction: the rule that sizes every drawn cell.
pub fn by_distance(tau: Tau) -> Detail {
    Detail {
        tau: tau.scale() as f32,
        range: asked(),
        eye: [0.0; 3],
        spacing: Spacing::NONE,
    }
}

/// The cell zoom the eye's rays draw at `at`: the camera grid's rule at its
/// distance (`Tau::cell_at`), no finer than the ground's cells and, once the
/// source declares its top, no coarser than a tile draws.
pub fn cell_at(eye: V3, tau: Tau, at: V3) -> Zoom {
    let v = v3::sub(&at, &eye);
    let cell = tau.cell_at(v3::dot(&v, &v)).min(GROUND.cell());
    TerrainTile::top().map_or(cell, |top| cell.max(GROUND.grid_zoom(top)))
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

    fn root(&mut self, node: [i32; 3]) -> At {
        self.world.start((0, node))
    }

    /// The child is the one its index's low bits name.
    fn child(&mut self, at: &At, _: Zoom, node: [i32; 3]) -> At {
        self.world.child(
            at,
            (node[0] & 1 | (node[1] & 1) << 1 | (node[2] & 1) << 2) as usize,
        )
    }

    /// The world's answer (`World::node`), the first lack kept; a node
    /// nothing under fills is air its size. Decor fills no cell: a leaf cell
    /// the ray passes within where decor is drawn as held strikes what
    /// stands near it, and a node no cell under fills is still gone into
    /// where decor may stand in it, down to the leaf cells.
    fn node(
        &mut self,
        at: &mut At,
        _: Zoom,
        _: [i32; 3],
        asked: [Zoom; 2],
        t0: f32,
    ) -> In<[u32; 2]> {
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
        let answer = match self.world.node(at, asked, &mut self.lines) {
            Answer::Nothing(l) => {
                self.lack.get_or_insert(l);
                return In {
                    kind: Kind::Lacks,
                    asset: answer::AIR,
                };
            }
            Answer::Coarser(i, l) => {
                self.lack.get_or_insert(l);
                i
            }
            Answer::Exact(i) => i,
        };
        self.answered = true;
        let decor = zoom < GROUND.cell()
            && self.strike.as_ref().is_some_and(|(_, s)| {
                t0 < s.held && self.world.may_strike(s, at.node, at.handed.shard)
            });
        match answer.kind {
            // Air coarser than asked, gone into where decor may stand in it;
            // and a split entered whole, its box the world's, not the decor
            // standing over it.
            Kind::Cell if zoom < asked[1] && decor => In {
                kind: Kind::Split,
                asset: answer::BOX_WHOLE,
            },
            Kind::Split if decor => In {
                kind: Kind::Split,
                asset: answer::BOX_WHOLE,
            },
            _ => answer,
        }
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
    /// The first cell `ray` meets within its reach that fills, at the zoom
    /// `detail` asks where it meets it, or the decor struck nearer. What the
    /// ray starts inside is not met, so an eye under the ground is not
    /// stopped where it stands. A cell met coarser than held is the lack;
    /// one not held at all is the lack too, past which the ray tells nothing.
    /// Decor, which fills no cell, is struck at the leaf cells it passes.
    pub fn first_hit(&self, ray: &Ray, detail: Detail) -> Answer<Option<Hit>> {
        let (met, strike) = self.along(ray, detail, Until::Opaque, self.strike(ray));
        let decor = strike
            .and_then(|s| s.best)
            .filter(|&(d, _)| met.value().flatten().is_none_or(|h: Hit| d < h.t));
        match (decor, met) {
            (Some((d, i)), Answer::Nothing(l)) => Answer::Coarser(Some(decor_hit(ray, d, i)), l),
            (Some((d, i)), met) => met.map(|_| Some(decor_hit(ray, d, i))),
            (None, met) => met,
        }
    }

    /// The first cell along `ray` that `until` stops at, at the zoom `detail`
    /// asks where it meets it (pxl-spec-core `cast::first`), from the ray's
    /// origin, over the world where it stands there (pxl-spec `world::origin`).
    pub(super) fn along(
        &self,
        ray: &Ray,
        detail: Detail,
        until: Until,
        strike: Option<Strike<'a>>,
    ) -> (Answer<Option<Hit>>, Option<Strike<'a>>) {
        let mut walker = Walker {
            world: self,
            lack: None,
            answered: false,
            strike: strike.map(|s| (ray, s)),
            lines: Lines::default(),
        };
        let o = ray.origin;
        let end = ray.at(ray.reach);
        self.read([o[0], o[1]], [end[0], end[1]], detail.range[0]);
        let from = cast::Ray::new(ray.direction.map(|v| v as f32), ray.reach as f32);
        let q = origin(o[0], o[1], o[2]);
        let met = cast::first(&mut walker, q, &from, detail, until);
        let met = (met.ends == Ends::Stop).then(|| self.hit(ray, q, met.stop));
        let answer = match (met, walker.lack, walker.answered) {
            (h, None, _) => Answer::Exact(h),
            (Some(h), Some(l), _) => Answer::Coarser(Some(h), l),
            (None, Some(l), true) => Answer::Coarser(None, l),
            (None, Some(l), false) => Answer::Nothing(l),
        };
        (answer, walker.strike.map(|(_, s)| s))
    }

    /// The footing down column `g` at `cell` (pxl-spec-core
    /// `support::footing`): the top of the cell met as drawn, with what the
    /// world lacked on the way, none where nothing a ray cannot see through
    /// is met; and the footing itself, whose line the cells about it are
    /// asked off (`asset_about`).
    pub(super) fn footing(
        &self,
        cell: Zoom,
        g: [i32; 2],
    ) -> (Answer<Option<(i32, f32)>>, Footing<At>) {
        let mut walker = self.walker();
        let side = units(cell);
        let middle = g.map(|v| (f64::from(v) + 0.5) * side);
        self.read(middle, middle, cell);
        let f = pxl_spec_core::support::footing(&mut walker, cell, g);
        let met = (f.ends == Ends::Stop).then_some(f.top);
        let answer = match (met, walker.lack, walker.answered) {
            (m, None, _) => Answer::Exact(m),
            (Some(m), Some(l), _) => Answer::Coarser(Some(m), l),
            (None, Some(l), true) => Answer::Coarser(None, l),
            (None, Some(l), false) => Answer::Nothing(l),
        };
        (answer, f)
    }

    /// What is in the cell `c` of `zoom`, asked off the line `l` (pxl-spec-core
    /// `support::asset`): from the deepest node of it holding the cell,
    /// never again from the top; nothing and the tile where the world lacks
    /// it.
    pub(super) fn asset_about(&self, l: &Line<At>, zoom: Zoom, c: [i32; 3]) -> Answer<u32> {
        let mut walker = self.walker();
        let value = pxl_spec_core::support::asset(&mut walker, l, zoom, c);
        match walker.lack {
            None => Answer::Exact(value),
            Some(t) => Answer::Nothing(t),
        }
    }

    /// A walk with nothing met yet, striking no decor.
    fn walker(&self) -> Walker<'_, 'a> {
        Walker {
            world: self,
            lack: None,
            answered: false,
            strike: None,
            lines: Lines::default(),
        }
    }

    /// Where the descent from `q`, the ray's origin as a position names it,
    /// stopped, in units: its node's box put back on the unit cell holding
    /// the origin, so it lies on the origin's lap, inset to the box its cell
    /// keeps.
    fn hit(&self, ray: &Ray, q: Q3, s: cast::Stop<At>) -> Hit {
        let (zoom, c) = s.at.node;
        let side = units(zoom);
        let unit = ray.origin.map(libm::floor);
        // The corner's Q3 steps from the origin's unit cell, exact: the roots
        // apart, its place in its root, less the origin's unit cell's there.
        let n = s.node;
        let steps = |k: usize| {
            (i64::from(n.roots[k]) << 32) + i64::from(n.corner[k]) - i64::from(q[k] as u32 & !0xff)
        };
        let min: V3 = std::array::from_fn(|k| unit[k] + q3::to_real(steps(k)));
        let axis = s.axis as usize;
        let mut normal = [0.0; 3];
        normal[axis] = if ray.direction[axis] > 0.0 { -1.0 } else { 1.0 };
        let struck = match answer::placed(s.asset[0]) {
            true => self
                .objects
                .placed(zoom, wrap_cell(zoom, c), s.at.handed.placed)
                .map_or(Struck::Objects, |e| Struck::Object(e.id())),
            false => Struck::Ground,
        };
        // A cell kept short of its cube is the box it is drawn as, about
        // its cube's center (pxl-spec-core `cast::kept_bounds`).
        let (lo, hi) = cast::kept_bounds(([0.0; 3], [1.0; 3]), s.asset[0]);
        let t = f64::from(s.t);
        Hit {
            t,
            point: ray.at(t),
            normal,
            cube: Aabb {
                min: std::array::from_fn(|k| min[k] + side * f64::from(lo[k])),
                max: std::array::from_fn(|k| min[k] + side * f64::from(hi[k])),
            },
            struck,
        }
    }
}
