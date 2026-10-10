// pxl-kernel/query/decor.rs

//! Decor as the world answers it: what the ground grows, evaluated cell by
//! cell as the chunks grow it (`grown_at`), and what objects place, each
//! struck by its model. A ray strikes it as the ground walk passes, only
//! where the chunks draw it as held, at the leaf cells: a fold strikes none of
//! it. The walk follows the detail rule, not the chunk the view chose, which
//! may differ at a chunk's edge.

use pxl_spec::block::decor::{Decor, DECORS};
use pxl_spec::block::stand::Block;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::pvf3::pvf3;
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::zoom::{base_of, index_of, units, Zoom};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

use super::ray::{Detail, Ray};
use super::support::block_in;
use super::world::World;
use crate::decor::strike::{reach, strike};
use crate::layer::instance::{Instance, Source};
use crate::layer::object::meta::placed;
use crate::layer::terrain::scatter::grown_at;
use crate::layer::terrain::Source as TerrainSource;
use crate::layer::Query;
use crate::model::registry::Models;

/// How decor is struck: against its models, where the chunks drawing at `tau` hold it.
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
    around: i64,
    /// How far along the ray decor is drawn as held.
    pub(super) held: f64,
    /// The column met last, whose neighbors are struck already.
    last: Option<[i64; 2]>,
    /// The leaf tile read last, which the next cell most likely lies in.
    tile: Option<(Tile, TerrainSource<'a>)>,
    pub(super) best: Option<(f64, Instance)>,
}

impl<'a> World<'a> {
    /// Whether the ground grows `decor` standing in the leaf block `at`, as
    /// the tile zoom `zoom`'s chunks draw it: the block void, and the one
    /// under it ground `decor` grows on. Air placed there or under it is no void.
    pub fn grows(&self, decor: &Decor, [x, y, z]: [i64; 3], zoom: Zoom) -> bool {
        let half = units(GROUND.cell()) / 2.0;
        let base = base_of(GROUND.cell(), [x, y, z]);
        let center = pvf3::gather(pvf3::X(&base) + pvf3::from_real(half), pvf3::Y(&base) + pvf3::from_real(half), pvf3::Z(&base));
        // The block and the one under it, from one composition of their column.
        let (at, under) = (pvf3::to_real(pvf3::Z(&base)) + half, pvf3::to_real(pvf3::Z(&base)) - half);
        let blocks = self.runs_at(&center, zoom, |runs, _| [at, under].map(|z| block_in(runs, z)));
        matches!(blocks.value(), Some([None, Some(Block::Ground(g))]) if decor.allows(g))
    }

    /// What the ground grows of `decor` standing in the leaf cell `at` lies
    /// in; none where it grows nothing there or its leaf tile is not resident.
    pub fn grown(&self, decor: &Decor, at: &Pvf3) -> Option<Instance> {
        let [x, y, z] = index_of(GROUND.cell(), at);
        let (source, place) = self.leaf_source([x, y])?;
        grown_at(decor, &source.view(), [x, y], place).filter(|i| i.grown_in().is_some_and(|c| c[2] == z))
    }

    /// The leaf tile holding the leaf cell `cell`, and the cell's place in it.
    fn leaf_source(&self, [x, y]: [i64; 2]) -> Option<(TerrainSource<'a>, [u32; 2])> {
        let leaf = GROUND.cell_of(LEAF_ZOOM, &base_of(GROUND.cell(), [x, y, 0]));
        let (zoom, [tx, ty]) = leaf.tile().node();
        Some((self.terrain.read((zoom, [tx, ty, 0]), GROUND.grid_zoom(zoom)).exact()?, leaf.in_tile()))
    }

    /// The strike a ray carries, where decor is struck at all.
    pub(super) fn strike(&self, ray: &Ray) -> Option<Strike<'a>> {
        let Grown { models, tau } = self.decor?;
        let held = match Detail::Distance(tau).at(0.0) {
            (LEAF_ZOOM, _, until) => until.min(ray.reach),
            _ => return None,
        };
        let reach = reach(models);
        Some(Strike {
            models,
            reach,
            around: reach.ceil() as i64,
            held,
            last: None,
            tile: None,
            best: None,
        })
    }

    /// Strikes the decor standing near the leaf column `cell` the walk is in:
    /// those not near the column met before it, since a walk only moves on.
    pub(super) fn strike_near(&self, s: &mut Strike<'a>, ray: &Ray, cell: [i64; 2]) {
        let (r, last) = (s.around, s.last.replace(cell));
        let near = |a: [i64; 2], b: [i64; 2]| (a[0] - b[0]).abs() <= r && (a[1] - b[1]).abs() <= r;
        // Placed decor stands where the ray's height over the held span, widened by the reach, can meet it.
        let (za, zb) = (ray.at(0.0)[2], ray.at(s.held)[2]);
        let (z0, z1) = (za.min(zb) - s.reach, za.max(zb) + s.reach);
        for y in cell[1] - r..=cell[1] + r {
            for x in cell[0] - r..=cell[0] + r {
                if last.is_some_and(|l| near([x, y], l)) {
                    continue;
                }
                self.grown_near(s, ray, [x, y]);
                let column = Aabb {
                    min: [x as f64, y as f64, z0],
                    max: [(x + 1) as f64, (y + 1) as f64, z1],
                };
                self.objects.entries_in(&column, &mut |e| {
                    if let Some(i) = placed(e).filter(|i| column.holds(i.at)) {
                        meet(s, ray, i);
                    }
                });
            }
        }
    }

    fn grown_near(&self, s: &mut Strike<'a>, ray: &Ray, cell: [i64; 2]) {
        let leaf = GROUND.cell_of(LEAF_ZOOM, &base_of(GROUND.cell(), [cell[0], cell[1], 0]));
        let source = match s.tile {
            Some((t, source)) if t == leaf.tile() => source,
            _ => {
                let Some((source, _)) = self.leaf_source(cell) else { return };
                s.tile = Some((leaf.tile(), source));
                source
            }
        };
        let view = source.view();
        for decor in &DECORS {
            let Some(i) = grown_at(decor, &view, cell, leaf.in_tile()) else { continue };
            let Source::Ground(at) = i.source else { continue };
            if self.grows(decor, at, LEAF_ZOOM) {
                meet(s, ray, i);
            }
        }
    }
}

fn meet(s: &mut Strike, ray: &Ray, i: Instance) {
    if let Some(t) = strike(s.models, &i, ray.origin, ray.direction, s.best.map_or(s.held, |b| b.0)) {
        s.best = Some((t, i));
    }
}
