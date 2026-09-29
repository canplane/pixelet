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
// - a query leaves residency as it found it.

use pxl_render::mesh::paint::terrain::adapter::{ChunkSample, PageView};
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
        assert_eq!(access.extent(asked, (L2, [kx, ky])).lack().is_some(), asked != leaves[0].0);
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
        sample.read(
            &PageView {
                cells: &src.ground.cells,
                ele: &src.ele,
            },
            first.in_page_above(src_level(n)),
            n,
            0,
        );
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
    let before = (paging.revision(), paging.frames_used(), paging.leases().count());
    let access = Access::new(&paging, &store);
    let mut rng = Rng(11);
    for at in points(&mut rng) {
        let _ = access.at(&at, 0);
        let p = [pvf3::to_real(pvf3::X(&at)), pvf3::to_real(pvf3::Y(&at))];
        let _ = access.first_hit(&down(p), Detail::Level(0));
        let _ = access.extent(GROUND.cell_of(0, &at).page(), GROUND.cell_of(0, &at).page().node());
    }
    assert_eq!(before, (paging.revision(), paging.frames_used(), paging.leases().count()));
    for (p, _) in [(top(), ()), (leaves[3].0, ())] {
        assert!(paging.asked(p).is_none(), "a query asked for a page");
    }
}
