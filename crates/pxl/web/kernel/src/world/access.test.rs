// pxl-kernel/world/access.test.rs
//
// The query's laws over a leaf page's four children folded into their
// parent as the bake folds them, and objects written into a store.

use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::bake::format::{Ground, GroundChannelKey};
use pxl_spec::bake::pack::with_channel;
use pxl_spec::cell::cube_types;
use pxl_spec::cell::object::{placeable_cube_num_id, PlaceableCube};
use pxl_spec::space::pvf3::ivec;
use pxl_spec::space::pyramid::{Folds, PAGE_CELLS};

use super::*;
use crate::global::serial;
use crate::layer::object::store::{At, Write};
use crate::world::paging::{Candidate, Victim};

const PADDED: usize = ELE_PADDED_SPAN as usize;
const SPAN: usize = PAGE_CELLS as usize;

/// The fixture holds fewer pages than there are frames.
struct Keep;

impl Victim for Keep {
    fn victim(&self, _: &[Candidate]) -> Option<usize> {
        None
    }
}

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> i64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n) as i64
    }
}

/// Steps and spikes, which the interpolation overshoots most, over ground
/// part water a step lower.
fn rough_leaf(rng: &mut Rng) -> Terrain {
    let ele = (0..PADDED * PADDED).map(|_| if rng.below(6) == 0 { rng.below(300) - 40 } else { rng.below(12) } as i16).collect();
    let cells = (0..SPAN * SPAN)
        .map(|_| {
            let land = if rng.below(3) == 0 { cube_types::land::bare } else { cube_types::land::green_light };
            let water = if rng.below(4) == 0 { cube_types::water::river } else { 0 };
            with_channel(with_channel(0, GroundChannelKey::LAND, land), GroundChannelKey::WATER, water)
        })
        .collect::<Vec<_>>();
    Terrain::leaf(ele, Ground { cells: cells.into() })
}

fn zero_page() -> Terrain {
    let cells = vec![with_channel(0, GroundChannelKey::LAND, cube_types::land::sea); SPAN * SPAN];
    Terrain::leaf(vec![0; PADDED * PADDED], Ground { cells: cells.into() })
}

fn top() -> LevelPage {
    LevelPage::new(1, ivec::gather(40, -20, 0))
}

fn pages() -> (Vec<(LevelPage, Terrain)>, Terrain) {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let leaves: Vec<(LevelPage, Terrain)> = top().children().iter().map(|&c| (c, rough_leaf(&mut rng))).collect();
    let zero = zero_page();
    let parent = Terrain::fold_page(top(), |p| Some(leaves.iter().find(|(q, _)| *q == p).map_or(&zero, |(_, t)| t))).unwrap();
    (leaves, parent)
}

fn resident(held: &[(LevelPage, Terrain)]) -> Paging<Terrain> {
    let mut paging = Paging::default();
    for (p, page) in held {
        paging.admit(*p, &Keep, |_| Ok::<_, ()>(page.clone())).unwrap();
    }
    paging
}

fn point(x: f64, y: f64) -> Pvf3 {
    let mut at = Pvf3::default();
    projection::project(&mut at, x, y, 0.0);
    at
}

/// Points spread over the parent's ground.
fn points(rng: &mut Rng) -> Vec<Pvf3> {
    let (x0, y0, side) = top().rect();
    (0..400).map(|_| point(x0 + rng.unit() * side, y0 + rng.unit() * side)).collect()
}

/// The ground a leaf draws at its own cell, asked of the page directly.
fn leaf_top(leaf: &Terrain, at: &Pvf3) -> f64 {
    let [tx, ty] = GROUND.cell_of(0, at).in_page();
    ground_ele_at(&leaf.ele, Some(GroundLanes::of(&leaf.ground)), tx as f64 + 0.5, ty as f64 + 0.5) as f64 * U_PER_CELL
}

/// A ray straight down onto `at`, from far above anything the fixture holds.
fn down(at: [f64; 2]) -> Ray {
    Ray {
        origin: [at[0], at[1], 10_000.0],
        direction: [0.0, 0.0, -1.0],
        reach: 20_000.0,
    }
}

/// The level a page `n` of a chunk's cells a side is over the chunk's.
fn src_level(n: u32) -> u32 {
    n.trailing_zeros()
}

/// Writes `state` in a cube of `side` with its corner at `corner`.
fn write(store: &mut Store, corner: V3, side: Scale, state: u32) {
    let mut at = Pvf3::default();
    projection::project(&mut at, corner[0], corner[1], corner[2]);
    let a = At::of_cube(&at, side);
    store.write([Write {
        col_idx: a.col_idx,
        band_idx: a.band,
        at: a.step,
        scale: side,
        state,
    }]);
}

/// A ray asking a level over z16 meets the objects in the cells the level
/// draws from: a cell the objects fold into solid is met at its own box,
/// over the corners of it no cube fills too, while a lone cube the fold
/// takes away is passed down to the ground under it.
#[test]
fn a_level_s_ray_meets_the_objects_in_the_cells_the_level_draws_from() {
    let _g = serial();
    let (leaves, parent) = pages();
    let paging = resident(&[(top(), parent), leaves[0].clone()]);
    let (x0, y0, _) = leaves[0].0.rect();
    let level = 1;
    let cell = GROUND.scale(level).side();
    let red = placeable_cube_num_id(PlaceableCube::Block(cube_types::Block::red));
    let (bx, by) = ((x0 / cell).floor() * cell + 4.0 * cell, (y0 / cell).floor() * cell + 4.0 * cell);
    let bare = Access::new(&paging, &Store::new()).at(&point(bx, by), level).value().unwrap().top;
    let bz = ((bare / cell).floor() + 3.0) * cell;

    // Five of the cell's eight: the bottom four and one over them.
    let mut store = Store::new();
    for [dx, dy, dz] in [[0, 0, 0], [1, 0, 0], [0, 1, 0], [1, 1, 0], [0, 0, 1]] {
        write(&mut store, [bx + dx as f64, by + dy as f64, bz + dz as f64], CELL, red);
    }
    let (lx, ly) = (bx + 4.0 * cell, by);
    write(&mut store, [lx, ly, bz], CELL, red);
    let access = Access::new(&paging, &store);

    // Over the corner no cube's top reaches, the level still meets the cell's top.
    let at = [bx + 1.5, by + 1.5];
    let fine = access.first_hit(&down(at), Detail::Level(0)).value().flatten().unwrap();
    assert_eq!(fine.point[2], bz + 1.0, "{fine:?}");
    let hit = access.first_hit(&down(at), Detail::Level(level)).value().flatten().unwrap();
    assert!(matches!(hit.struck, Struck::Objects), "{hit:?}");
    assert_eq!((hit.box_min, hit.box_max), ([bx, by, bz], [bx + cell, by + cell, bz + cell]));
    assert_eq!(hit.point[2], bz + cell);

    let alone = [lx + 0.5, ly + 0.5];
    let hit = access.first_hit(&down(alone), Detail::Level(level)).value().flatten().unwrap();
    let ground = Access::new(&paging, &Store::new()).first_hit(&down(alone), Detail::Level(level)).value().flatten().unwrap();
    assert_eq!((hit.struck, hit.point), (Struck::Ground, ground.point));
}

/// The ray's walk as it was before the objects were asked along the ray:
/// the terrain's traversal with the objects' bound asked by boxes at each
/// step. The oracle the walk down the objects' index is held to.
mod before {
    use pxl_spec::space::pvf3::REACH;
    use pxl_spec::space::v3::Aabb;

    use super::*;

    pub struct Before<'a>(pub &'a Access<'a>);

    impl<'a> core::ops::Deref for Before<'a> {
        type Target = Access<'a>;
        fn deref(&self) -> &Access<'a> {
            self.0
        }
    }

    struct Region {
        /// The square's corner and side, in units.
        base: [f64; 2],
        side: f64,
        /// The page at the level asked.
        page: LevelPage,
        /// The index lane's lowest and highest over it, and whether it is read
        /// from the page asked; none where nothing over it is resident.
        terrain: Option<((f64, f64), bool)>,
        /// The lowest and highest the objects under it reach.
        objects: Option<(f64, f64)>,
    }

    impl Before<'_> {
        /// The first thing `ray` meets within its reach, the ground drawn at the
        /// level `detail` says along it: the region the ray is in is passed over
        /// whole where it stays above everything the index lane and the objects'
        /// reach say can stand there, and walked cell column by cell column of
        /// its level where it does not. A region read from a page coarser than
        /// asked, whose bound the ray enters, is the lack — the nearest one, since
        /// what is under it could be met before anything past it — and a region
        /// nothing over is resident for is one too, whatever the ray's height,
        /// since nothing bounds it.
        ///
        /// A region ends where the level changes, so a walk is in the cells of
        /// the level at its start, which is the finest over it.
        ///
        /// Before a region of the level asked is read, the pages over it that are
        /// resident are asked, coarsest first, whether the ray passes over all
        /// they hold (`clear`): an index cell of a page `k` levels up is `2^k`
        /// of the level's across, so ground far under the ray is passed in as
        /// many tests as there are levels over it, not a region at a time. A
        /// page over it that is not resident proves nothing and is passed by; it
        /// never lets the ray pass over what it would have held. What a ray
        /// costs is the regions and cells it visits, which is fewest far over
        /// the ground and most along a slope it grazes.
        pub fn first_hit(&self, ray: &Ray, detail: Detail) -> Answer<Option<Hit>> {
            let mut lack: Option<Lack> = None;
            let mut answered = false;
            let mut t = 0.0;
            while t < ray.reach {
                let (level, until) = detail.at(t);
                if let Some(out) = self.clear(ray, t, level, until) {
                    answered = true;
                    t = out;
                    continue;
                }
                let r = self.region(&ray.at(t + NUDGE), level);
                let out = exit(ray, r.base, r.side).min(until).max(t + NUDGE).min(ray.reach);
                let low = ray.at(t)[2].min(ray.at(out)[2]);
                let terrain_under = r.terrain.is_some_and(|((_, hi), _)| low <= hi);
                let objects_under = r.objects.is_some_and(|(_, hi)| low <= hi);
                match r.terrain {
                    Some((_, exact)) => {
                        answered = true;
                        if terrain_under && !exact {
                            lack.get_or_insert(Lack {
                                layer: Category::terrain,
                                page: r.page,
                            });
                        }
                    }
                    None => {
                        lack.get_or_insert(Lack {
                            layer: Category::terrain,
                            page: r.page,
                        });
                    }
                }
                if terrain_under || objects_under {
                    if let Some(hit) = self.walk(ray, &r, t, out, level) {
                        return match lack {
                            Some(l) => Answer::Coarser(Some(hit), l),
                            None => Answer::Exact(Some(hit)),
                        };
                    }
                }
                t = out;
            }
            match (lack, answered) {
                (None, _) => Answer::Exact(None),
                (Some(l), true) => Answer::Coarser(None, l),
                (Some(l), false) => Answer::Nothing(l),
            }
        }

        /// Where the ray, at `t` and asking at `level` until `until`, is proved
        /// to pass over everything under an index cell of a resident page over
        /// `level`: the far edge of the largest such cell, or none. The ground
        /// is solid under its surface, so it is passed over only above its
        /// highest; objects stand in their span with air under it, so they are
        /// passed under too. An index cell bounds what is drawn at its page's
        /// level and every level under it, and the ray asks at `level` up to
        /// `until` only, so no cell carries it past there.
        fn clear(&self, ray: &Ray, t: f64, level: u32, until: f64) -> Option<f64> {
            let p = ray.at(t + NUDGE);
            let mut at = Pvf3::default();
            projection::project(&mut at, p[0], p[1], 0.0);
            (level + 1..=TOP_LEVEL).rev().find_map(|lv| {
                let body = self.terrain.get(GROUND.cell_of(lv, &at).page())?;
                let e = EXTENT.cell_of(lv, &at);
                let [_, hi] = body.index.extent()[e.index() as usize];
                let scale = e.scale();
                let [ix, iy, _] = scale.index_of(&at);
                let base = scale.base_of([ix, iy, 0]);
                let base = [pvf3::to_real(pvf3::X(&base)), pvf3::to_real(pvf3::Y(&base))];
                let out = exit(ray, base, scale.side()).min(until).min(ray.reach).max(t + NUDGE);
                let (za, zb) = (ray.at(t)[2], ray.at(out)[2]);
                let over_ground = za.min(zb) > hi as f64 * U_PER_CELL;
                let passed = Aabb {
                    min: [base[0], base[1], za.min(zb)],
                    max: [base[0] + scale.side(), base[1] + scale.side(), za.max(zb)],
                };
                let clear_of_objects = self.objects.bound_in(&passed).is_none();
                (over_ground && clear_of_objects).then_some(out)
            })
        }

        /// The region a point lies in, for a ray asking at `level`.
        fn region(&self, p: &V3, level: u32) -> Region {
            let mut at = Pvf3::default();
            projection::project(&mut at, p[0], p[1], 0.0);
            let page = GROUND.cell_of(level, &at).page();
            let page_scale = L3.up(level as i32);
            let (scale, terrain) = match self.drawn_from(page) {
                Some((src, body)) => {
                    // The index cell of the page it is drawn from, or the page
                    // asked where that is the smaller: either way one bound and
                    // one page across it.
                    let e = EXTENT.cell_of(src.level, &at);
                    let [lo, hi] = body.index.extent()[e.index() as usize];
                    let exact = self.terrain.source(page).is_some();
                    (e.scale().min(page_scale), Some(((lo as f64 * U_PER_CELL, hi as f64 * U_PER_CELL), exact)))
                }
                None => (page_scale, None),
            };
            let [ix, iy, _] = scale.index_of(&at);
            let base = scale.base_of([ix, iy, 0]);
            let base = [pvf3::to_real(pvf3::X(&base)), pvf3::to_real(pvf3::Y(&base))];
            let over = Aabb {
                min: [base[0], base[1], REACH.min[2]],
                max: [base[0] + scale.side(), base[1] + scale.side(), REACH.max[2]],
            };
            Region {
                base,
                side: scale.side(),
                page,
                terrain,
                objects: self.objects.bound_in(&over).map(|b| (b.min[2], b.max[2])),
            }
        }

        /// The first thing met between `t0` and `t1` along the ray inside region
        /// `r`, cell column by cell column of `level` in the order the ray
        /// crosses them (Amanatides–Woo). The walk stops at the region's edge by
        /// the cells it holds and not by `t1`: a step's distance and the edge's
        /// are summed two ways, and a column past the edge taken for inside would
        /// be read as the region's, exact or not. A region is at least a cell of
        /// its level across: its index cell is the page's it is read from, at
        /// that level or over it.
        fn walk(&self, ray: &Ray, r: &Region, t0: f64, t1: f64, level: u32) -> Option<Hit> {
            let side = GROUND.scale(level).side();
            let across = (r.side / side) as i64;
            let first = r.base.map(|v| (v / side).floor() as i64);
            let inside = |c: [i64; 2]| (0..2).all(|k| (first[k]..first[k] + across).contains(&c[k]));
            let [dx, dy, _] = ray.direction;
            let start = ray.at(t0 + NUDGE);
            let mut cell = [(start[0] / side).floor() as i64, (start[1] / side).floor() as i64];
            let axis = |d: f64, c: i64, o: f64| -> (f64, f64) {
                if d.abs() < 1e-12 {
                    (f64::INFINITY, f64::INFINITY)
                } else {
                    (side / d.abs(), ((c + i64::from(d > 0.0)) as f64 * side - o) / d)
                }
            };
            let (delta_x, mut next_x) = axis(dx, cell[0], ray.origin[0]);
            let (delta_y, mut next_y) = axis(dy, cell[1], ray.origin[1]);
            let mut from = t0;
            while from < t1 && inside(cell) {
                let to = next_x.min(next_y).min(t1);
                let hit = if level == 0 { self.column(ray, cell, from, to) } else { self.folded(ray, cell, from, to, level) };
                if hit.is_some() {
                    return hit;
                }
                if next_x < next_y {
                    cell[0] += if dx > 0.0 { 1 } else { -1 };
                    next_x += delta_x;
                } else {
                    cell[1] += if dy > 0.0 { 1 } else { -1 };
                    next_y += delta_y;
                }
                from = to;
            }
            None
        }

        /// The first thing met in z16 cell column `cell` while the ray is over
        /// it, between `t0` and `t1`: the ground under its drawn top, and where
        /// something is placed in the column at the heights the ray passes, one
        /// cell of height at a time, since a cell holding a voxel entry is its
        /// objects' and not the ground's. What the ray starts inside is not met.
        fn column(&self, ray: &Ray, [ix, iy]: [i64; 2], t0: f64, t1: f64) -> Option<Hit> {
            let side = CELL.side();
            let base = CELL.base_of([ix, iy, 0]);
            let top = self.ground(GROUND.cell_of(0, &base)).value().map(|g| g.top);
            let (za, zb) = (ray.at(t0)[2], ray.at(t1)[2]);
            let [kx, ky, _] = L2.index_of(&base);
            let passed = Aabb {
                min: [ix as f64 * side, iy as f64 * side, za.min(zb)],
                max: [(ix + 1) as f64 * side, (iy + 1) as f64 * side, za.max(zb)],
            };
            let placed = self.objects.bound_in(&passed).map(|b| (b.min[2], b.max[2]));
            let ground = |t: f64, cz: i64, normal: V3| {
                let min = [ix as f64 * side, iy as f64 * side, cz as f64 * side];
                Hit {
                    t,
                    point: ray.at(t),
                    normal,
                    box_min: min,
                    box_max: min.map(|v| v + side),
                    struck: Struck::Ground,
                }
            };
            let dz = ray.direction[2];
            let Some((lo, hi)) = placed else {
                // Nothing placed where the ray passes: the ground alone, met at
                // the column's side or through its top.
                let top = top?;
                if za < top {
                    return (t0 > 0.0).then(|| ground(t0, (za / side).floor() as i64, wall_normal(ray, t0, side)));
                }
                if zb < top {
                    let t = (top - ray.origin[2]) / dz;
                    return Some(ground(t, (top / side).ceil() as i64 - 1, [0.0, 0.0, 1.0]));
                }
                return None;
            };
            // Only the heights something can be met at are stepped through: from
            // the highest of what is placed and the ground's top down, and up to
            // the highest of what is placed.
            let ceiling = hi.max(top.unwrap_or(f64::NEG_INFINITY));
            let at_z = |z: f64| (z - ray.origin[2]) / dz;
            let mut from = t0;
            if dz < 0.0 && za > ceiling {
                from = at_z(ceiling);
            } else if dz > 0.0 && za < lo && top.is_none_or(|top| za >= top) {
                from = at_z(lo);
            }
            let end = if dz > 0.0 && top.is_none_or(|top| za >= top) { t1.min(at_z(hi)) } else { t1 };
            let col_idx = L2_IDX(TO_XY(base.i));
            let corner = [(kx * cells_across()) as f64 * side, (ky * cells_across()) as f64 * side];
            let first = (ray.at(from)[2] / side).floor() as i64;
            let mut cz = first;
            loop {
                let leave = if dz.abs() < 1e-12 {
                    t1
                } else {
                    let face = if dz > 0.0 { (cz + 1) as f64 * side } else { cz as f64 * side };
                    at_z(face).min(t1)
                };
                let entries = self.cell_entries(col_idx, [ix - kx * cells_across(), iy - ky * cells_across()], cz);
                if !entries.is_empty() {
                    if let Some(hit) = object_hit(ray, &entries, corner) {
                        return Some(hit);
                    }
                } else if top.is_some_and(|top| (cz as f64) * side < top) && from > 0.0 {
                    // In through the column's side where this is the cell the ray
                    // came into the column at, and through the cell's floor or
                    // ceiling past it.
                    let normal = if from == t0 {
                        wall_normal(ray, from, side)
                    } else {
                        [0.0, 0.0, if dz > 0.0 { -1.0 } else { 1.0 }]
                    };
                    return Some(ground(from, cz, normal));
                }
                if leave >= end {
                    return None;
                }
                cz += if dz > 0.0 { 1 } else { -1 };
                from = leave;
            }
        }

        /// The first thing met in cell column `cell` of `level`, over z16, while
        /// the ray is over it, between `t0` and `t1`: the ground the level draws
        /// under its top, and the objects in the cells the level draws from: a
        /// level over z16 draws its objects folded into its own cells, as the
        /// store keeps them (layer/object/store.rs `fold_cell`), so the first of
        /// those cells the ray passes that paints a material is what is met, the
        /// cell's own box — the ground's cells and the objects' at one
        /// resolution, the one the mesher then merges 2 × 2 × 2 by what each
        /// bucket covers, whichever layer it came from. The cells read are the
        /// ones the ray passes. What the ray starts inside is not met.
        fn folded(&self, ray: &Ray, [ix, iy]: [i64; 2], t0: f64, t1: f64, level: u32) -> Option<Hit> {
            let scale = GROUND.scale(level);
            let side = scale.side();
            let base = scale.base_of([ix, iy, 0]);
            let top = self.ground(GROUND.cell_of(level, &base)).value().map(|g| g.top);
            let (za, zb) = (ray.at(t0)[2], ray.at(t1)[2]);
            let dz = ray.direction[2];
            // Where the ray comes into the heights `lo` to `hi` over the column,
            // and through which face.
            let enter = |lo: f64, hi: f64| -> Option<(f64, V3)> {
                if za >= lo && za <= hi {
                    return (t0 > 0.0).then(|| (t0, wall_normal(ray, t0, side)));
                }
                if za > hi && zb <= hi {
                    return Some(((hi - ray.origin[2]) / dz, [0.0, 0.0, 1.0]));
                }
                if za < lo && zb >= lo {
                    return Some(((lo - ray.origin[2]) / dz, [0.0, 0.0, -1.0]));
                }
                None
            };
            let corner = [ix as f64 * side, iy as f64 * side];
            let ground = top.and_then(|top| enter(f64::NEG_INFINITY, top)).map(|(t, normal)| {
                let p = ray.at(t);
                let cz = ((p[2] - normal[2] * NUDGE) / side).floor();
                Hit {
                    t,
                    point: p,
                    normal,
                    box_min: [corner[0], corner[1], cz * side],
                    box_max: [corner[0] + side, corner[1] + side, (cz + 1.0) * side],
                    struck: Struck::Ground,
                }
            });
            // The cells of the column the ray passes, in the order it passes
            // them; a folded value is drawn solid where it paints a material.
            let (from, to) = ((za / side).floor() as i64, (zb / side).floor() as i64);
            let cells: &mut dyn Iterator<Item = i64> = if from <= to { &mut (from..=to) } else { &mut (to..=from).rev() };
            let objects = cells
                .filter(|&cz| {
                    let v = u32::from(self.objects.folded_value(scale, [ix, iy, cz]));
                    object_value_is_voxel(v) && !object_value_is_air(v)
                })
                .find_map(|cz| {
                    let (lo, hi) = (cz as f64 * side, (cz + 1) as f64 * side);
                    let (t, normal) = enter(lo, hi)?;
                    Some(Hit {
                        t,
                        point: ray.at(t),
                        normal,
                        box_min: [corner[0], corner[1], lo],
                        box_max: [corner[0] + side, corner[1] + side, hi],
                        struck: Struck::Objects,
                    })
                });
            match (ground, objects) {
                (Some(g), Some(o)) => Some(if o.t < g.t { o } else { g }),
                (g, o) => g.or(o),
            }
        }
    }
}

