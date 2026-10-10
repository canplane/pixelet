// pxl-kernel/query.test.rs
//
// The query's laws over a leaf tile's four children folded into their
// parent as the bake folds them, and objects written into a store.

use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::bake::constant::{OBJECT_COARSEST, OBJECT_FINEST};
use pxl_spec::bake::tile::terrain::Ground;
use pxl_spec::block::cube::{self, ground, sentinel};
use pxl_spec::block::object::{cube_id, Cube};
use pxl_spec::space::coord::pvf3::ivec;
use pxl_spec::space::layer::lattice::TILE_CELLS;

use super::*;
use crate::layer::Answer;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

use crate::layer::object::store::Store;
use crate::layer::residency::Residency;
use pxl_spec::space::coord::zoom::{index_of, units, Zoom};

use crate::layer::object::store::Write;
use crate::layer::residency::{Candidate, Victim};
use crate::serial;

const PADDED: usize = ELE_PADDED_SPAN as usize;
const SPAN: usize = TILE_CELLS as usize;

/// The fixture holds fewer tiles than there are frames.
struct Keep;

impl Victim for Keep {
    fn victim(&self, _: &[Candidate]) -> Option<usize> {
        None
    }
}

fn top() -> Tile {
    Tile::new(LEAF_ZOOM - 1, ivec::gather(40, -20, 0))
}

fn resident(held: &[(Tile, TerrainTile)]) -> Residency<TerrainTile> {
    let mut residency = Residency::default();
    for (p, tile) in held {
        residency.admit(*p, &Keep, |_| Ok::<_, ()>(tile.clone())).unwrap();
    }
    residency
}

fn point(x: f64, y: f64) -> Pvf3 {
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
    let void = TerrainTile::leaf(
        vec![0; PADDED * PADDED],
        Ground {
            cells: vec![sentinel::void; SPAN * SPAN].into(),
        },
        Default::default(),
    );
    let residency = resident(&[(leaf, void)]);
    let (x0, y0, _) = leaf.rect();
    let at = [x0 + 10.5, y0 + 10.5];

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
    separation.push([([0.0, 10.5], 10), ([40.0, 10.5], 10)], 4 * POINT_SCALE as u8, ground::residential_road, Form::Bridge, 0);
    let void = TerrainTile::leaf(
        vec![0; PADDED * PADDED],
        Ground {
            cells: vec![sentinel::void; SPAN * SPAN].into(),
        },
        separation.finish(),
    );
    let residency = resident(&[(leaf, void)]);
    let (x0, y0, _) = leaf.rect();
    let at = [x0 + 20.5, y0 + 10.5];
    let store = Store::default();
    let access = World::new(&residency, &store);
    assert_eq!(access.surface_support(&point(at[0], at[1])), Answer::Exact(Some(10.0)));
    let hit = access.first_hit(&down(at), Detail::Zoom(LEAF_ZOOM)).value().flatten().unwrap();
    assert_eq!(hit.point[2], 10.0, "{hit:?}");
}

/// A ray in the frame that runs on round the world meets, past the
/// antimeridian, the ground and the objects stored under the lane's copy.
#[test]
fn a_ray_across_the_antimeridian_meets_what_stands_past_it() {
    let _g = serial();
    let n = pxl_spec::space::coord::world::around(LEAF_ZOOM);
    let flat = || {
        let cells = vec![ground::land; SPAN * SPAN];
        TerrainTile::leaf(vec![5; PADDED * PADDED], Ground { cells: cells.into() }, Default::default())
    };
    let tile = |x: i64| pxl_spec::space::layer::tile::tile_at(LEAF_ZOOM, [x, 0]);
    let residency = resident(&[(tile(n / 2 - 1), flat()), (tile(n / 2), flat())]);
    let seam = pxl_spec::space::coord::world::ROUND / 2.0;
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
    let flat = TerrainTile::leaf(
        vec![5; PADDED * PADDED],
        Ground {
            cells: vec![ground::land; SPAN * SPAN].into(),
        },
        Default::default(),
    );
    let (x0, y0, _) = leaf.rect();
    (resident(&[(leaf, flat)]), [x0 + 10.0, y0 + 10.0])
}

/// A quarter of a cell dug a cell deep is stood on and struck at its bottom,
/// the rest of the cell at the surface: the query answers at what is placed's
/// own size, as a chunk drawing it as held does.
#[test]
fn a_cell_dug_in_part_is_met_where_each_part_is_drawn() {
    use crate::layer::object::store::AIR;
    let _g = serial();
    let (residency, [x, y]) = flat_at_five();
    let mut store = Store::default();
    for z in [4.0, 4.5] {
        write(&mut store, [x, y, z], OBJECT_COARSEST + 1, AIR);
    }
    let access = World::new(&residency, &store);
    for (at, top) in [([x + 0.25, y + 0.25], 4.0), ([x + 0.75, y + 0.75], 5.0)] {
        assert_eq!(access.surface_support(&point(at[0], at[1])), Answer::Exact(Some(top)), "{at:?}");
        let hit = access.first_hit(&down(at), Detail::Zoom(LEAF_ZOOM)).exact().flatten().unwrap();
        assert_eq!(hit.point[2], top, "{hit:?}");
    }
}

/// A cell dug at the finest zoom is met there, not folded to a coarser one.
#[test]
fn a_cell_dug_at_the_finest_is_met_as_held() {
    use crate::layer::object::store::AIR;
    let _g = serial();
    let (residency, [x, y]) = flat_at_five();
    let mut store = Store::default();
    let side = units(OBJECT_FINEST);
    write(&mut store, [x, y, 5.0 - side], OBJECT_FINEST, AIR);
    let access = World::new(&residency, &store);
    for (at, top) in [([x + side / 2.0, y + side / 2.0], 5.0 - side), ([x + 0.5, y + 0.5], 5.0)] {
        assert_eq!(access.surface_support(&point(at[0], at[1])), Answer::Exact(Some(top)), "{at:?}");
        let hit = access.first_hit(&down(at), Detail::Zoom(LEAF_ZOOM)).exact().flatten().unwrap();
        assert_eq!(hit.point[2], top, "{hit:?}");
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
    assert_eq!(access.floor_under(&gap), Answer::Exact(Some(6.0)));
    assert_eq!(access.ceiling_over(&gap), Answer::Exact(Some(7.0)));
    assert_eq!(access.surface_support(&gap), Answer::Exact(Some(6.0)));
}
