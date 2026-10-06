// pxl-kernel/world/access.test.rs
//
// The query's laws over a leaf tile's four children folded into their
// parent as the bake folds them, and objects written into a store.

use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::bake::format::Ground;
use pxl_spec::cell::cube_types::{self, ground, sentinel};
use pxl_spec::cell::object::{placeable_cube_num_id, PlaceableCube};
use pxl_spec::space::pvf3::ivec;
use pxl_spec::space::pyramid::TILE_CELLS;

use super::*;
use crate::layer::object::store::Write;
use crate::serial;
use crate::world::residency::{Candidate, Victim};
use pxl_spec::space::zoom::Zoom;

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
    Tile::new(LEAF_Z - 1, ivec::gather(40, -20, 0))
}

fn resident(held: &[(Tile, Terrain)]) -> Residency<Terrain> {
    let mut residency = Residency::default();
    for (p, tile) in held {
        residency.admit(*p, &Keep, |_| Ok::<_, ()>(tile.clone())).unwrap();
    }
    residency
}

fn point(x: f64, y: f64) -> Pvf3 {
    projection::project(x, y, 0.0)
}

fn down(at: [f64; 2]) -> Ray {
    Ray {
        origin: [at[0], at[1], 10_000.0],
        direction: [0.0, 0.0, -1.0],
        reach: 20_000.0,
    }
}

fn write(store: &mut Store, corner: V3, side: Zoom, state: u32) {
    let at = projection::project(corner[0], corner[1], corner[2]);
    store.write([Write {
        zoom: side,
        at: index_of(side, &at),
        state,
    }]);
}

#[test]
fn where_the_terrain_gives_no_surface_only_what_is_placed_stands() {
    let _g = serial();
    let leaf = top().children()[0];
    let void = Terrain::leaf(
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
    let access = Access::new(&residency, &empty);
    assert_eq!(access.at(&point(at[0], at[1]), LEAF_Z).value().unwrap().top(), None);
    assert_eq!(access.surface_support(&point(at[0], at[1])), Answer::Exact(None));
    assert_eq!(access.first_hit(&down(at), Detail::Zoom(LEAF_Z)).value().flatten(), None);

    let mut store = Store::default();
    let red = placeable_cube_num_id(PlaceableCube::Block(cube_types::Block::red));
    write(&mut store, [x0 + 10.0, y0 + 10.0, 5.0], CELL, red);
    let access = Access::new(&residency, &store);
    assert_eq!(access.surface_support(&point(at[0], at[1])), Answer::Exact(Some(6.0)));
    let hit = access.first_hit(&down(at), Detail::Zoom(LEAF_Z)).value().flatten().unwrap();
    assert!(matches!(hit.struck, Struck::Object(_)), "{hit:?}");
}

/// A deck stands on its own, not on the ground under it: over a void cell
/// the query meets its slab where the renderer draws it.
#[test]
fn a_deck_over_no_surface_is_met_where_it_is_drawn() {
    use pxl_spec::bake::deck::{Builder, Form, POINT_SCALE};
    let _g = serial();
    let leaf = top().children()[0];
    let mut decks = Builder::default();
    decks.push([([0.0, 10.5], 10), ([40.0, 10.5], 10)], 4 * POINT_SCALE as u8, ground::residential_road, Form::Deck, 0);
    let void = Terrain::leaf(
        vec![0; PADDED * PADDED],
        Ground {
            cells: vec![sentinel::void; SPAN * SPAN].into(),
        },
        decks.finish(),
    );
    let residency = resident(&[(leaf, void)]);
    let (x0, y0, _) = leaf.rect();
    let at = [x0 + 20.5, y0 + 10.5];
    let store = Store::default();
    let access = Access::new(&residency, &store);
    assert_eq!(access.surface_support(&point(at[0], at[1])), Answer::Exact(Some(10.0)));
    let hit = access.first_hit(&down(at), Detail::Zoom(LEAF_Z)).value().flatten().unwrap();
    assert_eq!(hit.point[2], 10.0, "{hit:?}");
}

/// A ray in the frame that runs on round the world meets, past the
/// antimeridian, the ground and the objects stored under the lane's copy.
#[test]
fn a_ray_across_the_antimeridian_meets_what_stands_past_it() {
    let _g = serial();
    let n = pxl_spec::space::zoom::around(LEAF_Z);
    let flat = || {
        let cells = vec![ground::land; SPAN * SPAN];
        Terrain::leaf(vec![5; PADDED * PADDED], Ground { cells: cells.into() }, Default::default())
    };
    let tile = |x: i64| pxl_spec::space::pyramid::tile_at(LEAF_Z, [x, 0]);
    let residency = resident(&[(tile(n / 2 - 1), flat()), (tile(n / 2), flat())]);
    let seam = pxl_spec::space::zoom::ROUND / 2.0;
    let top = Access::new(&residency, &Store::default()).at(&point(-seam + 20.5, 100.5), LEAF_Z).value().unwrap().top().unwrap();

    let mut store = Store::default();
    let red = placeable_cube_num_id(PlaceableCube::Block(cube_types::Block::red));
    write(&mut store, [-seam + 40.0, 100.0, top], CELL, red);
    let access = Access::new(&residency, &store);

    let ground = access.first_hit(&down([seam + 20.5, 100.5]), Detail::Zoom(LEAF_Z)).exact().flatten().unwrap();
    assert_eq!([ground.point[0], ground.point[2]], [seam + 20.5, top], "{ground:?}");

    let along = Ray {
        origin: [seam - 30.0, 100.5, top + 0.5],
        direction: [1.0, 0.0, 0.0],
        reach: 200.0,
    };
    let hit = access.first_hit(&along, Detail::Zoom(LEAF_Z)).exact().flatten().unwrap();
    assert!(matches!(hit.struck, Struck::Object(_)), "{hit:?}");
    assert_eq!(hit.point[0], seam + 40.0, "{hit:?}");
}
