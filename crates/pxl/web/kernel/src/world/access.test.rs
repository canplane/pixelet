// pxl-kernel/world/access.test.rs
//
// The query's laws, over a leaf page's four children folded into their
// parent as the bake folds them, and objects written into a store:
//
// - an answer is exact only from the page asked, and a coarser one names the
//   page it lacked;
// - the ground a ray meets is the ground the paint draws there;
// - a ray passed over a region by its index lane never passes over ground:
//   nothing along it before what it meets stands above it;
// - a page resident over the ones a ray reads changes no answer, only how
//   soon the ray passes;
// - a placed object is met at its own box, and air takes the ground out;
// - the objects walked along the ray answer as they did asked by boxes;
// - a ray does not meet the ground of the column it starts inside;
// - a query leaves residency as it found it.

use pxl_render::mesh::paint::terrain::adapter::{Around, ChunkSample, PageSource, PageView};
use pxl_render::mesh::voxel_scan::apron_col;
use pxl_spec::bake::constant::{ELE_PADDED_SPAN, OBJECT_FINEST};
use pxl_spec::bake::format::{Ground, GroundChannelKey};
use pxl_spec::bake::pack::with_channel;
use pxl_spec::cell::cube_types;
use pxl_spec::cell::object::{placeable_cube_num_id, PlaceableCube};
use pxl_spec::space::pvf3::ivec;
use pxl_spec::space::pyramid::{Folds, CHUNK_CELLS, PAGE_CELLS};

use super::*;
use crate::global::serial;
use crate::layer::object::store::{At, Write, AIR};
use crate::world::paging::{Candidate, Victim};

const PADDED: usize = ELE_PADDED_SPAN as usize;
const SPAN: usize = PAGE_CELLS as usize;

/// Gives up no frame: the fixture holds fewer pages than there are.
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

    fn unit(&mut self) -> f64 {
        self.below(1 << 20) as f64 / (1 << 20) as f64
    }
}

/// A leaf of rough relief — steps and spikes, which the interpolation
/// overshoots most — over ground part water, which sits a step lower.
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

/// The open ocean, as the bake makes it.
fn zero_page() -> Terrain {
    let cells = vec![with_channel(0, GroundChannelKey::LAND, cube_types::land::sea); SPAN * SPAN];
    Terrain::leaf(vec![0; PADDED * PADDED], Ground { cells: cells.into() })
}

/// The parent the fixture's leaves fold into.
fn top() -> LevelPage {
    LevelPage::new(1, ivec::gather(40, -20, 0))
}

/// Four rough leaves and the parent folded from them, the ocean around.
fn pages() -> (Vec<(LevelPage, Terrain)>, Terrain) {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let leaves: Vec<(LevelPage, Terrain)> = top().children().iter().map(|&c| (c, rough_leaf(&mut rng))).collect();
    let zero = zero_page();
    let parent = Terrain::fold_page(top(), |p| Some(leaves.iter().find(|(q, _)| *q == p).map_or(&zero, |(_, t)| t))).unwrap();
    (leaves, parent)
}

/// Residency holding `held`.
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

#[test]
fn an_answer_is_exact_only_from_the_page_asked_and_a_coarser_one_names_that_page() {
    let _g = serial();
    let (leaves, parent) = pages();
    let store = Store::new();
    let mut rng = Rng(7);

    let none = resident(&[]);
    for at in points(&mut rng) {
        let asked = GROUND.cell_of(0, &at).page();
        assert_eq!(
            Access::new(&none, &store).at(&at, 0),
            Answer::Nothing(Lack {
                layer: Category::terrain,
                page: asked
            })
        );
    }

    let mut held = vec![(top(), parent)];
    held.push(leaves[0].clone());
    let paging = resident(&held);
    let access = Access::new(&paging, &store);
    for at in points(&mut rng) {
        let asked = GROUND.cell_of(0, &at).page();
        match access.at(&at, 0) {
            Answer::Exact(g) => {
                assert_eq!(asked, leaves[0].0, "exact from a page over the one asked");
                assert_eq!(g.top, leaf_top(&leaves[0].1, &at));
            }
            Answer::Coarser(_, lack) => {
                assert_ne!(asked, leaves[0].0, "coarser where the page asked is resident");
                assert_eq!(
                    lack,
                    Lack {
                        layer: Category::terrain,
                        page: asked
                    }
                );
            }
            Answer::Nothing(_) => panic!("nothing where a page over it is resident"),
        }
        assert!(matches!(access.at(&at, 1), Answer::Exact(_)), "the parent is the page asked at its own level");
        // The bound a descent is decided by is exact and lacking alike.
        let [kx, ky, _] = L2.index_of(&at);
        assert_eq!(access.relief(asked, (L2, [kx, ky])).lack().is_some(), asked != leaves[0].0);
    }
}

/// A ray straight down onto `at`, from far above anything the fixture holds.
fn down(at: [f64; 2]) -> Ray {
    Ray {
        origin: [at[0], at[1], 10_000.0],
        direction: [0.0, 0.0, -1.0],
        reach: 20_000.0,
    }
}

#[test]
fn the_ground_a_ray_meets_is_the_ground_the_paint_draws() {
    let _g = serial();
    let (leaves, parent) = pages();
    let store = Store::new();
    // A chunk of the first leaf, read from the leaf and from the parent over
    // it, as the builder reads a chunk from whichever it is drawn from.
    let (leaf_page, leaf) = &leaves[0];
    let (x0, y0, _) = leaf_page.rect();
    for (held, src, n) in [(vec![(top(), parent.clone()), leaves[0].clone()], leaf, 1u32), (vec![(top(), parent.clone())], &parent, 2)] {
        let paging = resident(&held);
        let access = Access::new(&paging, &store);
        let first = GROUND.cell_of(0, &point(x0, y0));
        let mut sample = ChunkSample::default();
        let own = PageSource {
            page: PageView {
                cells: &src.ground.cells,
                ele: &src.ele,
            },
            origin: first.in_page_above(src_level(n)),
            n,
        };
        sample.read(&Around { own, beside: [None; 8] }, [0, 0], 0);
        for cy in (0..CHUNK_CELLS).step_by(7) {
            for cx in (0..CHUNK_CELLS).step_by(5) {
                let ray = down([x0 + cx as f64 + 0.5, y0 + cy as f64 + 0.5]);
                let hit = access.first_hit(&ray, Detail::Level(0)).value().flatten().expect("a ray down meets the ground");
                assert_eq!(hit.struck, Struck::Ground);
                assert_eq!(hit.point[2], sample.ele[apron_col(cx, cy) as usize] as f64 * U_PER_CELL, "({cx}, {cy}) read {n} a side");
                assert_eq!(hit.normal, [0.0, 0.0, 1.0]);
            }
        }
    }
}

/// The level a page `n` of a chunk's cells a side is over the chunk's.
fn src_level(n: u32) -> u32 {
    n.trailing_zeros()
}

#[test]
fn a_ray_passed_over_by_the_index_passes_over_no_ground() {
    let _g = serial();
    let (leaves, parent) = pages();
    let store = Store::new();
    let mut rng = Rng(0x5eed);
    let (x0, y0, side) = top().rect();
    for held in [vec![(top(), parent.clone())], vec![(top(), parent.clone()), leaves[1].clone(), leaves[2].clone()]] {
        let paging = resident(&held);
        let access = Access::new(&paging, &store);
        let top_at = |p: &V3| access.at(&point(p[0], p[1]), 0).value().map(|g| g.top);
        for _ in 0..120 {
            let from = [x0 + rng.unit() * side, y0 + rng.unit() * side];
            let ground = top_at(&[from[0], from[1], 0.0]).unwrap();
            let (azimuth, dip) = (rng.unit() * std::f64::consts::TAU, 0.02 + rng.unit() * 0.4);
            let ray = Ray {
                origin: [from[0], from[1], ground + 1.0 + rng.unit() * 120.0],
                direction: [azimuth.cos() * dip.cos(), azimuth.sin() * dip.cos(), -dip.sin()],
                reach: 400.0,
            };
            let answer = access.first_hit(&ray, Detail::Level(0));
            let hit = answer.value().flatten();
            let until = hit.map_or(ray.reach, |h| h.t);
            // Every point before what is met stands at or over the drawn
            // ground under it, where there is any.
            let mut s = 0.0;
            while s < until - 1e-6 {
                let p = ray.at(s);
                if let Some(top) = top_at(&p) {
                    assert!(p[2] >= top - 1e-9, "passed under the ground at {s} before meeting it at {until}: {} < {top}", p[2]);
                }
                s += 0.1;
            }
            if let Some(h) = hit {
                // Just past the face it came through, in the cell it met.
                let inside = ray.at(h.t + 1e-6);
                assert!(inside[2] <= top_at(&inside).unwrap() + 1e-6, "met above the ground");
                // Met on ground read from a page over the one asked is never
                // exact.
                let asked = GROUND.cell_of(0, &point(inside[0], inside[1])).page();
                if !held.iter().any(|(p, _)| *p == asked) {
                    assert!(answer.lack().is_some(), "exact from a page coarser than asked: {ray:?}");
                }
            }
        }
    }
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

#[test]
fn a_placed_object_is_met_at_its_own_box_and_air_takes_the_ground_out() {
    let _g = serial();
    let (leaves, parent) = pages();
    let paging = resident(&[(top(), parent), leaves[0].clone()]);
    let (x0, y0, _) = leaves[0].0.rect();
    let (cx, cy) = (x0 + 10.0, y0 + 20.0);
    let mut store = Store::new();
    let bare = Access::new(&paging, &Store::new()).at(&point(cx, cy), 0).value().unwrap().top;

    // A quarter-cell cube, set in from its cell's corner, a cell over the
    // ground.
    let side = OBJECT_FINEST.up(1);
    let q = side.side();
    let corner = [cx + q, cy + 2.0 * q, bare + U_PER_CELL + q];
    write(&mut store, corner, side, placeable_cube_num_id(PlaceableCube::Block(cube_types::Block::red)));
    let access = Access::new(&paging, &store);
    let hit = access.first_hit(&down([corner[0] + q / 2.0, corner[1] + q / 2.0]), Detail::Level(0)).value().flatten().unwrap();
    assert!(matches!(hit.struck, Struck::Object(_)), "{hit:?}");
    assert_eq!((hit.box_min, hit.box_max), (corner, corner.map(|v| v + q)));
    assert_eq!(hit.point[2], corner[2] + q);
    // Beside it in the same cell, the cell is the cube's: nothing is met
    // there until the ground's cell under it.
    let beside = access.first_hit(&down([cx + 0.9, cy + 0.9]), Detail::Level(0)).value().flatten().unwrap();
    assert_eq!((beside.struck, beside.point[2]), (Struck::Ground, bare));

    // Air in the ground's top cell: the ray goes down to the cell under it.
    let (ax, ay) = (x0 + 30.5, y0 + 40.5);
    let ground = access.at(&point(ax, ay), 0).value().unwrap().top;
    write(&mut store, [ax, ay, ground - U_PER_CELL], CELL, AIR);
    let access = Access::new(&paging, &store);
    let hit = access.first_hit(&down([ax, ay]), Detail::Level(0)).value().flatten().unwrap();
    assert_eq!((hit.struck, hit.point[2]), (Struck::Ground, ground - U_PER_CELL));
}

/// A page resident over the ones a ray reads only lets it pass sooner: with
/// the leaves' parent and its parent held over them, every ray meets what it
/// meets without them — the same thing at the same place, as exactly, lacking
/// the same page — high over the ground, grazing its slopes, and over and
/// under objects standing apart from it. The rays stay over the leaves, the
/// ground both residencies read at the level asked.
#[test]
fn a_page_over_the_ground_read_changes_no_answer() {
    let _g = serial();
    let (leaves, parent) = pages();
    let zero = zero_page();
    let over = top().parent();
    let grand = Terrain::fold_page(over, |p| Some(if p == top() { &parent } else { &zero })).unwrap();
    let (x0, y0, side) = top().rect();
    let mut rng = Rng(0xb0c5);
    let mut store = Store::new();
    for _ in 0..16 {
        let corner = [x0 + rng.unit() * side, y0 + rng.unit() * side, 60.0 + rng.unit() * 300.0];
        write(&mut store, corner, CELL, placeable_cube_num_id(PlaceableCube::Block(cube_types::Block::red)));
    }
    let bare = resident(&leaves);
    let stacked = resident(&leaves.iter().cloned().chain([(top(), parent.clone()), (over, grand)]).collect::<Vec<_>>());
    let (without, with) = (Access::new(&bare, &store), Access::new(&stacked, &store));
    let mut met = 0;
    for _ in 0..600 {
        let from = [x0 + rng.unit() * side, y0 + rng.unit() * side];
        let (azimuth, dip) = (rng.unit() * std::f64::consts::TAU, if rng.below(2) == 0 { 0.001 + rng.unit() * 0.05 } else { rng.unit() * 1.2 - 0.3 });
        let mut ray = Ray {
            origin: [from[0], from[1], 20.0 + rng.unit() * 2_000.0],
            direction: [azimuth.cos() * dip.cos(), azimuth.sin() * dip.cos(), -dip.sin()],
            reach: 6_000.0,
        };
        ray.reach = ray.reach.min(exit(&ray, [x0, y0], side) - 1e-3);
        let (a, b) = (without.first_hit(&ray, Detail::Level(0)), with.first_hit(&ray, Detail::Level(0)));
        assert_eq!(a.lack(), b.lack(), "{ray:?}");
        assert_eq!(a.exact().is_some(), b.exact().is_some(), "{ray:?}");
        match (a.value().flatten(), b.value().flatten()) {
            (None, None) => {}
            (Some(h), Some(k)) => {
                met += 1;
                assert_eq!((h.struck, h.box_min, h.box_max, h.normal), (k.struck, k.box_min, k.box_max, k.normal), "{ray:?}");
                assert!((h.t - k.t).abs() < 1e-6, "{ray:?}: met at {} and at {}", h.t, k.t);
            }
            (h, k) => panic!("{ray:?}: met {h:?} without the pages over and {k:?} with them"),
        }
    }
    assert!(met > 50, "too few rays met anything to say much: {met}");
}

#[test]
fn a_query_leaves_residency_as_it_found_it() {
    let _g = serial();
    let (leaves, parent) = pages();
    let paging = resident(&[(top(), parent), leaves[3].clone()]);
    let store = Store::new();
    let before = (paging.revision(), paging.frames_used());
    let access = Access::new(&paging, &store);
    let mut rng = Rng(11);
    for at in points(&mut rng) {
        let _ = access.at(&at, 0);
        let p = [pvf3::to_real(pvf3::X(&at)), pvf3::to_real(pvf3::Y(&at))];
        let _ = access.first_hit(&down(p), Detail::Level(0));
        let _ = access.relief(GROUND.cell_of(0, &at).page(), GROUND.cell_of(0, &at).page().node());
    }
    assert_eq!(before, (paging.revision(), paging.frames_used()));
    for (p, _) in [(top(), ()), (leaves[3].0, ())] {
        assert!(paging.asked(p).is_none(), "a query asked for a page");
    }
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

    // Five cubes of the cell's eight: the bottom four and one over them.
    let mut store = Store::new();
    for [dx, dy, dz] in [[0, 0, 0], [1, 0, 0], [0, 1, 0], [1, 1, 0], [0, 0, 1]] {
        write(&mut store, [bx + dx as f64, by + dy as f64, bz + dz as f64], CELL, red);
    }
    // And a lone cube a few cells over.
    let (lx, ly) = (bx + 4.0 * cell, by);
    write(&mut store, [lx, ly, bz], CELL, red);
    let access = Access::new(&paging, &store);

    // Over the corner of the cell no cube's top reaches, the level meets the
    // cell's top where z16 meets the cube under it.
    let at = [bx + 1.5, by + 1.5];
    let fine = access.first_hit(&down(at), Detail::Level(0)).value().flatten().unwrap();
    assert_eq!(fine.point[2], bz + 1.0, "{fine:?}");
    let hit = access.first_hit(&down(at), Detail::Level(level)).value().flatten().unwrap();
    assert!(matches!(hit.struck, Struck::Objects), "{hit:?}");
    assert_eq!((hit.box_min, hit.box_max), ([bx, by, bz], [bx + cell, by + cell, bz + cell]));
    assert_eq!(hit.point[2], bz + cell);

    // The lone cube folds away: the ray goes down to the ground, as without it.
    let alone = [lx + 0.5, ly + 0.5];
    let hit = access.first_hit(&down(alone), Detail::Level(level)).value().flatten().unwrap();
    let ground = Access::new(&paging, &Store::new()).first_hit(&down(alone), Detail::Level(level)).value().flatten().unwrap();
    assert_eq!((hit.struck, hit.point), (Struck::Ground, ground.point));
}

/// A ray that starts inside the ground does not meet the column it starts
/// in: straight down it meets nothing, and across it meets the next
/// column's ground at the wall it comes in through.
#[test]
fn a_ray_starting_inside_the_ground_does_not_meet_its_own_column() {
    let _g = serial();
    let (leaves, parent) = pages();
    let paging = resident(&[(top(), parent), leaves[0].clone()]);
    let store = Store::new();
    let access = Access::new(&paging, &store);
    let (x0, y0, _) = leaves[0].0.rect();
    let (x, y) = (x0 + 10.5, y0 + 20.5);
    let ground = |x: f64| access.at(&point(x, y), 0).value().unwrap().top;
    let z = ground(x).min(ground(x + 1.0)) - 3.5;
    let down = Ray {
        origin: [x, y, z],
        direction: [0.0, 0.0, -1.0],
        reach: 50.0,
    };
    assert_eq!(access.first_hit(&down, Detail::Level(0)).exact(), Some(None));
    let across = Ray {
        origin: [x, y, z],
        direction: [1.0, 0.0, 0.0],
        reach: 5.0,
    };
    let hit = access.first_hit(&across, Detail::Level(0)).exact().flatten().unwrap();
    assert_eq!((hit.struck, hit.t, hit.normal), (Struck::Ground, 0.5, [-1.0, 0.0, 0.0]));
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

/// A ray meets what the walk before it met: the objects found down their
/// index along the ray and the ground walked only as far as the nearest of
/// them answer as the terrain's walk did with the objects asked by boxes at
/// each step — the same thing at the same place, as exactly, lacking the
/// same page. Over ground part held and part not, so lacks fall before,
/// among and after what is met; with objects on the ground, floating over
/// it and stacked in one column, of every size from a sub-cell's to a few
/// cells', air among them; along rays grazing the ground, level, straight
/// down and at random, at z16, over it and at the level each distance asks.
#[test]
fn the_objects_walked_along_the_ray_answer_as_the_boxes_did() {
    let _g = serial();
    let (leaves, parent) = pages();
    let paging = resident(&[(top(), parent), leaves[0].clone(), leaves[2].clone()]);
    let (x0, y0, side) = top().rect();
    let mut rng = Rng(0x51de);
    let red = placeable_cube_num_id(PlaceableCube::Block(cube_types::Block::red));
    let bare = Store::new();
    let mut store = Store::new();
    let sizes = [OBJECT_FINEST.up(1), CELL, CELL.up(1), CELL.up(2)];
    let mut placed = Vec::new();
    for k in 0..400 {
        let (x, y) = (x0 + rng.unit() * side, y0 + rng.unit() * side);
        let ground = Access::new(&paging, &bare).at(&point(x, y), 0).value().map_or(0.0, |g| g.top);
        let z = match k % 4 {
            0 => ground - U_PER_CELL,
            1 => ground + rng.unit() * 8.0,
            _ => ground + 20.0 + rng.unit() * 200.0,
        };
        let s = sizes[rng.below(sizes.len() as u64) as usize];
        let state = if rng.below(6) == 0 { AIR } else { red };
        // A cube over a cell is its cells, since a write is a cell's at most.
        let n = if s > CELL { CELL.count_in(s) as i64 } else { 1 };
        for [dx, dy, dz] in (0..n * n * n).map(|i| [i % n, i / n % n, i / (n * n)]) {
            write(&mut store, [x + dx as f64, y + dy as f64, z + dz as f64], s.min(CELL), state);
        }
        placed.push([x, y, z]);
        if k % 10 == 0 {
            for dz in 1..4 {
                write(&mut store, [x, y, z + dz as f64 * 7.0], CELL, red);
            }
        }
    }
    let access = Access::new(&paging, &store);
    let before = before::Before(&access);
    let mut met = [0u32; 2];
    for n in 0..1_000 {
        let from = [x0 + rng.unit() * side, y0 + rng.unit() * side];
        let azimuth = rng.unit() * std::f64::consts::TAU;
        let direction = match n % 5 {
            0 => [0.0, 0.0, -1.0],
            1 => [azimuth.cos(), azimuth.sin(), 0.0],
            2 => [1.0, 0.0, -1e-3],
            _ => {
                let dip = if n % 2 == 0 { 0.001 + rng.unit() * 0.05 } else { rng.unit() * 1.2 - 0.3 };
                [azimuth.cos() * dip.cos(), azimuth.sin() * dip.cos(), -dip.sin()]
            }
        };
        let mut ray = Ray {
            origin: [from[0], from[1], 5.0 + rng.unit() * 600.0],
            direction,
            reach: 3_000.0,
        };
        // Every other ray toward something placed, from where it was going
        // to start.
        if n % 2 == 1 {
            let [x, y, z] = placed[rng.below(placed.len() as u64) as usize];
            let to = [x + 0.1 - ray.origin[0], y + 0.1 - ray.origin[1], z + 0.1 - ray.origin[2]];
            let len = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt();
            ray.direction = to.map(|v| v / len);
        }
        let detail = match n % 4 {
            0 | 1 => Detail::Level(0),
            2 => Detail::Level(1 + rng.below(2) as u32),
            _ => Detail::Distance(Tau::FINEST),
        };
        // A ray that starts inside the ground passed its column whole before
        // where no object's bound reached the column, and met the next cell
        // down where one did; it now passes the column whole either way.
        if access.at(&point(ray.origin[0], ray.origin[1]), 0).value().is_some_and(|g| ray.origin[2] < g.top) {
            continue;
        }
        let (a, b) = (before.first_hit(&ray, detail), access.first_hit(&ray, detail));
        assert_eq!(a.lack(), b.lack(), "{ray:?} {detail:?}");
        assert_eq!(a.exact().is_some(), b.exact().is_some(), "{ray:?} {detail:?}");
        match (a.value().flatten(), b.value().flatten()) {
            (None, None) => {}
            (Some(h), Some(k)) => {
                met[usize::from(h.struck != Struck::Ground)] += 1;
                assert_eq!((h.struck, h.box_min, h.box_max, h.normal), (k.struck, k.box_min, k.box_max, k.normal), "{ray:?} {detail:?}");
                assert!((h.t - k.t).abs() < 1e-6, "{ray:?} {detail:?}: met at {} and at {}", h.t, k.t);
            }
            (h, k) => panic!("{ray:?} {detail:?}: met {h:?} before and {k:?} now"),
        }
    }
    assert!(met[0] > 100 && met[1] > 100, "too few met the ground or the objects to say much: {met:?}");
}
