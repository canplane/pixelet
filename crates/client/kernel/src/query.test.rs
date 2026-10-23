// pxl-kernel/query.test.rs
//
// The query's laws over a leaf tile's four children folded into their
// parent as the bake folds them, and objects written into a store.

use pxl_spec::bake::constant::OBJECT_COARSEST;
use pxl_spec::bake::tile::terrain::Ground;
use pxl_spec::block::cube::{self, ground, sentinel};
use pxl_spec::block::object::{cube_id, Cube};
use pxl_spec::space::layer::lattice::{GROUND, TILE_CELLS};

use super::*;
use crate::layer::Answer;
use pxl_spec::bake::tile::terrain::{GroundId, TerrainTile, GROUND_LANE};
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

use crate::layer::object::store::Store;
use crate::layer::residency::Residency;
use pxl_spec::space::coord::zoom::{index_of, units, Zoom};

use crate::layer::object::store::Write;
use crate::layer::residency::{Candidate, Victim};
use crate::serial;

const CELLS: usize = GROUND_LANE;
const SPAN: usize = TILE_CELLS as usize;

/// The fixture holds fewer tiles than there are frames.
struct Keep;

impl Victim for Keep {
    fn victim(&self, _: &[Candidate]) -> Option<usize> {
        None
    }
}

fn top() -> Tile {
    Tile::new(LEAF_ZOOM - 1, [40, -20])
}

/// A leaf as a client holds it: its lines, and its cells, each its own look
/// and its height, as its windows hold them once a GPU drew them.
#[derive(Clone)]
struct Leaf {
    tile: TerrainTile,
    looks: Vec<GroundId>,
    heights: Vec<i16>,
}

/// The tiles held, and over each its line up to the coarsest zoom the
/// source supplies whole, as the floor holds it all around the eye
/// (`layer/residency/floor.rs`): a tile's coarser pictures here are the
/// same ground, each marking its children held; their cells held in the
/// windows about `centre`, in units.
fn resident(centre: [f64; 2], held: &[(Tile, Leaf)]) -> Residency<TerrainTile> {
    use pxl_spec::bake::tile::Occupancy;
    let mut residency = Residency::default();
    let mut admitted: Vec<Tile> = Vec::new();
    let mut cells = Vec::new();
    for (p, Leaf { tile, looks, heights }) in held {
        let line = std::iter::successors(Some(*p), |q| (q.zoom > GROUND.top()).then(|| q.parent()));
        for (k, q) in line.enumerate() {
            if admitted.contains(&q) {
                continue;
            }
            admitted.push(q);
            cells.push((q, looks.clone(), heights.clone(), tile.separation.clone()));
            let picture = match k {
                0 => tile.clone(),
                _ => TerrainTile {
                    occupancy: Occupancy::of_slots([true; 4]),
                    ..tile.clone()
                },
            };
            residency.admit(q, &Keep, |_| Ok::<_, ()>(picture)).unwrap();
        }
    }
    // Asked for, as the floor asks for what it holds: a tile held but not
    // asked is cache, which answers nothing.
    let asks: fnv::FnvHashMap<Tile, crate::io::demand::Ask> = admitted.iter().map(|&q| (q, crate::io::demand::Ask::PLAIN)).collect();
    residency.ask(&asks);
    // Their cells as a GPU would hand them back, at once.
    crate::layer::terrain::window::test::hold(centre, &cells);
    residency
}

/// A leaf as a client holds it once its cells are back: each cell's height and look given.
fn drawn(heights: Vec<i16>, ground: Ground, separation: pxl_spec::bake::separation::Separation) -> Leaf {
    Leaf {
        tile: TerrainTile::of(Vec::new(), Ground { cells: Vec::new().into() }, separation, pxl_spec::bake::tile::Occupancy::LEAF),
        looks: ground.cells.into_owned(),
        heights,
    }
}

fn point(x: f64, y: f64) -> Q3 {
    world::point(x, y, 0.0)
}

fn down(at: [f64; 2]) -> Ray {
    Ray {
        origin: [at[0], at[1], 10_000.0],
        direction: [0.0, 0.0, -1.0],
        reach: 20_000.0,
    }
}

fn write(store: &mut Store, corner: V3, side: Zoom, state: u32) {
    let at = world::point(corner[0], corner[1], corner[2]);
    store.write([Write {
        key: pxl_spec::space::hierarchy::morton::key_of::<3>(side, index_of(side, &at)),
        id: state,
        ext: None,
    }]);
}

#[test]
fn where_the_terrain_gives_no_surface_only_what_is_placed_stands() {
    let _g = serial();
    let leaf = top().children()[0];
    let void = drawn(
        vec![0; CELLS],
        Ground {
            cells: vec![sentinel::void; SPAN * SPAN].into(),
        },
        Default::default(),
    );
    let (x0, y0, _) = leaf.rect();
    let at = [x0 + 10.5, y0 + 10.5];
    let residency = resident(at, &[(leaf, void)]);

    let empty = Store::default();
    let access = World::new(&residency, &empty);
    assert_eq!(access.surface(&point(at[0], at[1]), LEAF_ZOOM).value().unwrap(), None);
    assert_eq!(access.surface_support(&point(at[0], at[1])), Answer::Exact(None));
    assert_eq!(access.first_hit(&down(at), Detail::Zoom(LEAF_ZOOM)).value().flatten(), None);

    let mut store = Store::default();
    let red = cube_id(Cube::Object(cube::Object::red));
    write(&mut store, [x0 + 10.0, y0 + 10.0, 5.0], OBJECT_COARSEST, red);
    let access = World::new(&residency, &store);
    assert_eq!(access.surface_support(&point(at[0], at[1])), Answer::Exact(Some(6.0)));
    let hit = access.first_hit(&down(at), Detail::Zoom(LEAF_ZOOM)).value().flatten().unwrap();
    assert!(matches!(hit.struck, Struck::Object(_)), "{hit:?}");
}

/// A deck stands on its own, not on the ground under it: over a void cell
/// the query meets its slab where the renderer draws it.
#[test]
fn a_deck_over_no_surface_is_met_where_it_is_drawn() {
    use pxl_spec::bake::separation::{Builder, Form, POINT_SCALE};
    let _g = serial();
    let leaf = top().children()[0];
    let mut separation = Builder::default();
    separation.push([([0.0, 10.5], 10, 0), ([40.0, 10.5], 10, 0)], 4 * POINT_SCALE as u8, ground::residential_road, Form::Bridge, 0);
    let void = drawn(
        vec![0; CELLS],
        Ground {
            cells: vec![sentinel::void; SPAN * SPAN].into(),
        },
        separation.finish(),
    );
    let (x0, y0, _) = leaf.rect();
    let at = [x0 + 20.5, y0 + 10.5];
    let residency = resident(at, &[(leaf, void)]);
    let store = Store::default();
    let access = World::new(&residency, &store);
    assert_eq!(access.surface_support(&point(at[0], at[1])), Answer::Exact(Some(10.0)));
    let hit = access.first_hit(&down(at), Detail::Zoom(LEAF_ZOOM)).value().flatten().unwrap();
    assert_eq!(hit.point[2], 10.0, "{hit:?}");
}

/// A ray in the frame that runs on round the world meets, past the
/// antimeridian, the ground and the objects stored under the world's name for them.
#[test]
fn a_ray_across_the_antimeridian_meets_what_stands_past_it() {
    let _g = serial();
    let n = pxl_spec::space::coord::world::around(LEAF_ZOOM);
    let flat = || {
        let cells = vec![ground::land; SPAN * SPAN];
        drawn(vec![5; CELLS], Ground { cells: cells.into() }, Default::default())
    };
    let tile = |x: i64| pxl_spec::space::layer::tile::tile_at(LEAF_ZOOM, [x, 0]);
    let seam = pxl_spec::space::coord::world::ROUND / 2.0;
    let residency = resident([seam, 100.5], &[(tile(n / 2 - 1), flat()), (tile(n / 2), flat())]);
    let top = World::new(&residency, &Store::default()).surface(&point(-seam + 20.5, 100.5), LEAF_ZOOM).value().flatten().unwrap().top;

    let mut store = Store::default();
    let red = cube_id(Cube::Object(cube::Object::red));
    write(&mut store, [-seam + 40.0, 100.0, top], OBJECT_COARSEST, red);
    let access = World::new(&residency, &store);

    let ground = access.first_hit(&down([seam + 20.5, 100.5]), Detail::Zoom(LEAF_ZOOM)).exact().flatten().unwrap();
    assert_eq!([ground.point[0], ground.point[2]], [seam + 20.5, top], "{ground:?}");

    let along = Ray {
        origin: [seam - 30.0, 100.5, top + 0.5],
        direction: [1.0, 0.0, 0.0],
        reach: 200.0,
    };
    let hit = access.first_hit(&along, Detail::Zoom(LEAF_ZOOM)).exact().flatten().unwrap();
    assert!(matches!(hit.struck, Struck::Object(_)), "{hit:?}");
    assert_eq!(hit.point[0], seam + 40.0, "{hit:?}");
}

fn flat_at_five() -> (Residency<TerrainTile>, [f64; 2]) {
    let leaf = top().children()[0];
    let flat = drawn(
        vec![5; CELLS],
        Ground {
            cells: vec![ground::land; SPAN * SPAN].into(),
        },
        Default::default(),
    );
    let (x0, y0, _) = leaf.rect();
    (resident([x0 + 10.0, y0 + 10.0], &[(leaf, flat)]), [x0 + 10.0, y0 + 10.0])
}

/// A dig is met as the cells asked fold it, nothing finer: at the leaves' cells
/// a cell dug less than half is whole, its top met by a ray and stood on by a
/// walker, and a cell dug half or more is air, the ground under it met.
#[test]
fn a_dig_is_met_as_the_cells_asked_fold_it() {
    use crate::layer::object::store::AIR;
    let _g = serial();
    let (residency, [x, y]) = flat_at_five();
    let half = units(OBJECT_COARSEST + 1);
    let dug = |n: usize| {
        let mut store = Store::default();
        for k in 0..n {
            let [dx, dy, dz] = [k & 1, k >> 1 & 1, k >> 2 & 1].map(|b| b as f64 * half);
            write(&mut store, [x + dx, y + dy, 4.0 + dz], OBJECT_COARSEST + 1, AIR);
        }
        store
    };
    for (n, top) in [(1, 5.0), (3, 5.0), (4, 4.0), (8, 4.0)] {
        let store = dug(n);
        let access = World::new(&residency, &store);
        let at = [x + half / 2.0, y + half / 2.0];
        let hit = access.first_hit(&down(at), Detail::Zoom(LEAF_ZOOM)).exact().flatten().unwrap();
        assert_eq!(hit.point[2], top, "{n} of 8 dug: {hit:?}");
        assert_eq!(access.surface_support(&point(at[0], at[1])), Answer::Exact(Some(top)), "{n} of 8 dug");
    }
}

/// In a stack with a cell left out, what stands in the gap stands on the
/// cube under it, and the cube over it is its ceiling.
#[test]
fn a_gap_in_a_stack_is_floored_and_roofed_by_its_cubes() {
    let _g = serial();
    let (residency, [x, y]) = flat_at_five();
    let mut store = Store::default();
    let red = cube_id(Cube::Object(cube::Object::red));
    for z in [5.0, 7.0] {
        write(&mut store, [x, y, z], OBJECT_COARSEST, red);
    }
    let access = World::new(&residency, &store);
    let gap = world::point(x + 0.5, y + 0.5, 6.5);
    assert_eq!(access.floor_under(&gap, LEAF_ZOOM), Answer::Exact(Some(6.0)));
    assert_eq!(access.ceiling_over(&gap), Answer::Exact(Some(7.0)));
    assert_eq!(access.surface_support(&gap), Answer::Exact(Some(6.0)));
}

/// Quake III's step-slide, a column at a time: blocked, the walker climbs the
/// blocker as far as its own column is open over its head, then settles; with
/// a ceiling first it stops there, and an axis that is blocked is slid along.
#[test]
fn a_walker_climbs_what_blocks_it_under_open_sky_and_slides_under_a_ceiling() {
    let _g = serial();
    let (residency, [x, y]) = flat_at_five();
    let mut store = Store::default();
    let red = cube_id(Cube::Object(cube::Object::red));
    // A wall three cubes high in the column east of the walker's.
    for z in [5.0, 6.0, 7.0] {
        write(&mut store, [x + 1.0, y, z], OBJECT_COARSEST, red);
    }
    let from = world::point(x + 0.5, y + 0.5, over(5.0));
    let floor = |p: Q3| q3::to_real(i64::from(p[2])) - units(GROUND.cell()) / 2.0;
    let east = world::point(x + 1.5, y + 0.5, 0.0);
    {
        let access = World::new(&residency, &store);
        let to = access.step_slide(&from, &east, &from, true).unwrap();
        assert_eq!((q3::to_real(i64::from(to[0])), floor(to)), (x + 1.5, 8.0), "climbed onto the wall");
    }
    // A roof over the walker two cells up: the climb meets it first.
    write(&mut store, [x, y, 7.0], OBJECT_COARSEST, red);
    let access = World::new(&residency, &store);
    let to = access.step_slide(&from, &east, &from, true).unwrap();
    assert!(q3::to_real(i64::from(to[0])) < x + 1.0, "stopped at the wall");
    assert_eq!(floor(to), 5.0);
    // Going north-east, the east is slid along and the north is walked.
    let to = access.step_slide(&from, &world::point(x + 1.5, y + 1.5, 0.0), &from, true).unwrap();
    assert!(q3::to_real(i64::from(to[0])) < x + 1.0, "slid along the wall");
    assert_eq!((q3::to_real(i64::from(to[1])), floor(to)), (y + 1.5, 5.0));
}

/// A height picks the floor nearest it: the ground under a deck, or the deck,
/// whichever its height is closer to; a solid with no clear cell over it is
/// no floor.
#[test]
fn a_height_stands_on_the_floor_nearest_it() {
    let _g = serial();
    let (residency, [x, y]) = flat_at_five();
    let mut store = Store::default();
    let red = cube_id(Cube::Object(cube::Object::red));
    // A deck a cube thick, its top at 10, with the ground at 5 under it.
    write(&mut store, [x, y, 9.0], OBJECT_COARSEST, red);
    let access = World::new(&residency, &store);
    let near = |z: f64| access.support_nearest(&world::point(x + 0.5, y + 0.5, z));
    assert_eq!(near(6.0), Answer::Exact(Some(5.0)), "the ground under the deck");
    assert_eq!(near(12.0), Answer::Exact(Some(10.0)), "the deck");
    // A cube right under the deck's: its top has no clear cell over it.
    write(&mut store, [x, y, 8.0], OBJECT_COARSEST, red);
    let access = World::new(&residency, &store);
    assert_eq!(access.support_nearest(&world::point(x + 0.5, y + 0.5, 9.0)), Answer::Exact(Some(10.0)));
}
