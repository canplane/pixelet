// pxl-core/domain/query.test.rs
//
// The query's laws, checked over a leaf tile and the tiles of its line, and
// over objects written into a store.

use pxl_spec::asset::AssetId;
use pxl_spec::asset::cube::{self, Ground, Sentinel};
use pxl_spec::bake::constant::{OBJECT_COARSEST, OBJECT_FINEST};
use pxl_spec::bake::tile::terrain::{GROUND_COLUMN, GroundCells, TerrainTile};
use pxl_spec::ground::GroundId;
use pxl_spec::space::coord::world::idx_of;
use pxl_spec::space::coord::zoom::{LEAF_CELL_ZOOM, LEAF_ZOOM, MIN_ZOOM, UNIT_ZOOM, Zoom, units};
use pxl_spec::space::layer::lattice::{self, GROUND, Grid, TILE_CELLS};

use super::*;
use crate::domain::layer::instance::Instance;
use crate::domain::layer::{Answer, Base as _};
use crate::domain::observer::{self, ObserverPose};
use pxl_spec::space::coord::mat4;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;
use pxl_spec::space::layer::quadrant::Quadrant;

use crate::domain::layer::object::store::{self, Store};
use crate::domain::layer::tile_cache::TileCache;

use crate::domain::layer::object::store::Write;
use crate::domain::layer::terrain::clipmap::test;
use pxl_engine::world::placed_in;
use pxl_spec::asset::decor::Decor;
use pxl_spec::bake::separation::Separation;
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::hierarchy::morton;
use pxl_spec::space::layer::area;
use pxl_spec::space::layer::area::Area;

const CELL_CNT: usize = GROUND_COLUMN;
const SPAN: usize = TILE_CELLS as usize;

fn top() -> Quadrant {
    Quadrant::new(LEAF_ZOOM - 1, [40, -20])
}

/// A leaf as a client holds it: its tile with its lines, and each cell's
/// asset id and elevation as the clip levels hold them.
#[derive(Clone)]
struct Leaf {
    tile: TerrainTile,
    asset_ids: Vec<GroundId>,
    ele: Vec<i16>,
}

/// A tile's line: the tile and each tile over it, up to the coarsest zoom
/// the source supplies whole.
fn line(p: Quadrant) -> impl Iterator<Item = Quadrant> {
    std::iter::successors(Some(p), |q| (q.zoom > MIN_ZOOM).then(|| q.parent()))
}

/// Makes the world's tile cache one holding `tiles` and, over each tile, its
/// line up to the coarsest zoom the source supplies whole, as the floor holds
/// it all around the eye (`domain/layer/floor.rs`). Here a tile's coarser
/// pictures are the same ground, each marking its children held. Their cells
/// are held in the clip levels around `center`, in units, and the eye stands
/// over `center` (`look_from`).
fn resident(center: [f64; 2], tiles: &[(Quadrant, Leaf)]) {
    let mut tile_cache = TileCache::default();
    let mut admitted: Vec<Quadrant> = Vec::new();
    let mut cells = Vec::new();
    for (
        p,
        Leaf {
            tile,
            asset_ids,
            ele,
        },
    ) in tiles
    {
        for (k, q) in line(*p).enumerate() {
            if admitted.contains(&q) {
                continue;
            }
            admitted.push(q);
            cells.push((q, asset_ids.clone(), ele.clone(), tile.separation.clone()));
            let picture = match k {
                0 => tile.clone(),
                _ => TerrainTile {
                    occupancy: Occupancy::of_slots([true; 4]),
                    ..tile.clone()
                },
            };
            tile_cache
                .admit(q, |_| None, |_| Ok::<_, ()>(picture))
                .unwrap();
        }
    }
    // Every tile is asked for, as the floor asks for what it holds. A tile
    // held but not asked for is only cache, which answers nothing.
    let asks: fnv::FnvHashMap<Quadrant, ()> = admitted.iter().map(|&q| (q, ())).collect();
    tile_cache.ask(&asks);
    test::hold(center, &cells);
    *TerrainTile::tile_cache() = tile_cache;
    look_from([center[0], center[1], 64.0]);
}

/// `World::step_slide` with the eye stood at `eye`.
pub(crate) fn step_slide_from(world: &World, from: &Q3, to: &Q3, eye: &Q3, walled: bool) -> Q3 {
    look_from([eye[0], eye[1], eye[2]].map(|v| q3::to_real(i64::from(v))));
    world.step_slide(from, to, eye, walled)
}

/// Stands the eye at `eye`. From 64 units over ground near the world's
/// floor it draws that ground's leaf cells under it.
pub(crate) fn look_from(eye: V3) {
    observer::set_pose(Some(ObserverPose {
        eye,
        rot: mat4::new(),
    }));
}

/// A leaf as a client holds it once its cells are back, with each cell's
/// elevation and asset id given.
fn drawn(ele: Vec<i16>, ground: GroundCells, separation: Separation) -> Leaf {
    Leaf {
        tile: TerrainTile::of(Vec::new(), Vec::new().into(), separation, Occupancy::LEAF),
        asset_ids: ground.into_owned(),
        ele,
    }
}

fn point(x: f64, y: f64) -> Q3 {
    world::point(x, y, 0.0)
}

fn down(at: [f64; 2]) -> Ray {
    Ray {
        origin: [at[0], at[1], 10_000.0],
        dir: [0.0, 0.0, -1.0],
        t_max: 20_000.0,
    }
}

/// The world with `store` for what is placed, over the ground `resident`
/// holds.
pub(crate) fn placed(store: Store) -> World {
    *store::object_mut() = store;
    World::now()
}

/// Takes the brick of leaf cells over `at` away, as one filled and not yet
/// handed over, so a coarser clip level stands in for the ground there.
pub(crate) fn withhold_at(at: [f64; 2]) {
    test::withhold(LEAF_CELL_ZOOM, at.map(|v| libm::floor(v) as i32));
}

pub(crate) fn write(store: &mut Store, corner: V3, side: Zoom, state: u32) {
    let at = world::point(corner[0], corner[1], corner[2]);
    store.write([Write {
        at: morton::key_of::<3>(side, idx_of(side, &at)),
        asset_id: state,
        ext: None,
    }]);
}

#[test]
fn no_surface_placed_stands() {
    let leaf = top().children()[0];
    let void = drawn(
        vec![0; CELL_CNT],
        vec![Sentinel::Void as u8; SPAN * SPAN].into(),
        Default::default(),
    );
    let (x0, y0, _) = leaf.rect();
    let at = [x0 + 10.5, y0 + 10.5];
    resident(at, &[(leaf, void)]);

    let access = placed(Store::default());
    assert_eq!(
        access
            .surface_of(
                GROUND.leaf_cell_zoom(),
                GROUND.cell_of(LEAF_ZOOM, &point(at[0], at[1])).node().1
            )
            .val()
            .unwrap(),
        None
    );
    assert_eq!(
        access.surface_support(&point(at[0], at[1])),
        Answer::Exact(None)
    );
    assert_eq!(access.first_hit(&down(at)).val().flatten(), None);

    let mut store = Store::default();
    let red = cube::Object::Red as AssetId;
    write(
        &mut store,
        [x0 + 10.0, y0 + 10.0, 5.0],
        OBJECT_COARSEST,
        red,
    );
    let access = placed(store);
    assert_eq!(
        access.surface_support(&point(at[0], at[1])),
        Answer::Exact(Some(6.0))
    );
    let hit = access.first_hit(&down(at)).val().flatten().unwrap();
    assert!(matches!(hit.kind, HitKind::Object(_)), "{hit:?}");
}

/// A deck stands on its own, not on the ground under it: over a void cell
/// the query meets its slab where the renderer draws it.
#[test]
fn deck_over_void_met() {
    use pxl_spec::bake::separation::{Builder, Form, POINT_SCALE};
    let leaf = top().children()[0];
    let mut separation = Builder::default();
    separation.push(
        [([0.0, 10.5], 10, 0), ([40.0, 10.5], 10, 0)],
        4 * POINT_SCALE as u8,
        Ground::ResidentialRoad as u8,
        Form::Bridge,
        0,
    );
    let void = drawn(
        vec![0; CELL_CNT],
        vec![Sentinel::Void as u8; SPAN * SPAN].into(),
        separation.finish(),
    );
    let (x0, y0, _) = leaf.rect();
    let at = [x0 + 20.5, y0 + 10.5];
    resident(at, &[(leaf, void)]);
    let access = placed(Store::default());
    assert_eq!(
        access.surface_support(&point(at[0], at[1])),
        Answer::Exact(Some(10.0))
    );
    let hit = access.first_hit(&down(at)).val().flatten().unwrap();
    assert_eq!(hit.point[2], 10.0, "{hit:?}");
}

/// A ray whose frame runs on past the antimeridian meets the ground and the
/// objects there, which are stored under the world's canonical positions.
#[test]
fn ray_across_antimeridian() {
    let n = 1i32 << LEAF_ZOOM;
    let flat = || {
        let cells = vec![Ground::Land as u8; SPAN * SPAN];
        drawn(vec![5; CELL_CNT], cells.into(), Default::default())
    };
    let tile = |x: i32| Quadrant::new(LEAF_ZOOM, [x, 0]);
    let seam = world::ROUND / 2.0;
    resident(
        [seam, 100.5],
        &[(tile(n / 2 - 1), flat()), (tile(n / 2), flat())],
    );
    let top = placed(Store::default())
        .surface_of(
            GROUND.leaf_cell_zoom(),
            GROUND
                .cell_of(LEAF_ZOOM, &point(-seam + 20.5, 100.5))
                .node()
                .1,
        )
        .val()
        .flatten()
        .unwrap()
        .top;

    let mut store = Store::default();
    let red = cube::Object::Red as AssetId;
    write(&mut store, [-seam + 40.0, 100.0, top], OBJECT_COARSEST, red);
    let access = placed(store);

    let ground = access
        .first_hit(&down([seam + 20.5, 100.5]))
        .exact()
        .flatten()
        .unwrap();
    assert_eq!(
        [ground.point[0], ground.point[2]],
        [seam + 20.5, top],
        "{ground:?}"
    );

    let along = Ray {
        origin: [seam - 30.0, 100.5, top + 0.5],
        dir: [1.0, 0.0, 0.0],
        t_max: 200.0,
    };
    let hit = access.first_hit(&along).exact().flatten().unwrap();
    assert!(matches!(hit.kind, HitKind::Object(_)), "{hit:?}");
    assert_eq!(hit.point[0], seam + 40.0, "{hit:?}");
}

pub(crate) fn flat_at_five() -> [f64; 2] {
    let leaf = top().children()[0];
    let flat = drawn(
        vec![5; CELL_CNT],
        vec![Ground::Land as u8; SPAN * SPAN].into(),
        Default::default(),
    );
    let (x0, y0, _) = leaf.rect();
    resident([x0 + 10.0, y0 + 10.0], &[(leaf, flat)]);
    [x0 + 10.0, y0 + 10.0]
}

/// An eye over the world's top casts from where it stands, not from the
/// top: its ray falls through what is over the world and meets the ground
/// where its line does.
#[test]
fn ray_from_above_meets_ground() {
    let at = flat_at_five();
    let access = placed(Store::default());
    let top = access.first_hit(&down(at)).val().flatten().unwrap().point[2];
    let high = 2.0 * world::BOUND.max[2];
    let over = Ray {
        origin: [at[0], at[1], high],
        dir: [0.0, 0.0, -1.0],
        t_max: 2.0 * high,
    };
    let met = access.first_hit(&over);
    assert!(matches!(met, Answer::Exact(_)), "{met:?}");
    let hit = met.val().flatten().unwrap();
    assert_eq!([hit.point[2], hit.t], [top, high - top], "{hit:?}");
}

/// A dig is met as the cells asked fold it, and nothing finer. At the
/// leaves' cells, a cell dug in part keeps the eighths left as one box of
/// their volume about the cell's center, whose top a ray meets
/// (`answer::side`). A cell dug whole is air, and the ground under it is
/// met. A body reads the cell as columns at the finest zoom placed in it
/// and stands on what is left as it is held, such as the half cubes over a
/// dug lower half.
#[test]
fn dig_met_as_folded() {
    use pxl_content::cave::hollow;
    use pxl_engine::answer::side;
    use pxl_spec::share::EIGHTH_CNT;
    let [x, y] = flat_at_five();
    // A column with no cave under the dig, so the ground under it is met.
    let clear = |x: f64| hollow((LEAF_CELL_ZOOM, [x, y, 3.0].map(|v| libm::floor(v) as i32))) == 0;
    let x = (0..).map(|i| x + f64::from(i)).find(|&x| clear(x)).unwrap();
    let half = units(OBJECT_COARSEST + 1);
    let dug = |n: usize| {
        let mut store = Store::default();
        for k in 0..n {
            let [dx, dy, dz] = [k & 1, k >> 1 & 1, k >> 2 & 1].map(|b| b as f64 * half);
            write(
                &mut store,
                [x + dx, y + dy, 4.0 + dz],
                OBJECT_COARSEST + 1,
                Sentinel::Air as AssetId,
            );
        }
        store
    };
    for (n, stands) in [(1, 5.0), (3, 5.0), (4, 5.0), (8, 4.0)] {
        let left = EIGHTH_CNT - n as u32;
        let top = if left == 0 {
            4.0
        } else {
            4.5 + f64::from(side(left)) / 2.0
        };
        let store = dug(n);
        let access = placed(store);
        let at = [x + half / 2.0, y + half / 2.0];
        // From here the eye draws the leaves' cells, the dig folded into one.
        look_from([at[0], at[1], 64.0]);
        let hit = access.first_hit(&down(at)).exact().flatten().unwrap();
        // The top is checked as `cast::top` gives it: a share of the cell,
        // in f32.
        assert!(
            (hit.cube.max[2] - top).abs() < 1e-6,
            "{n} of 8 dug: {hit:?}"
        );
        // From near, the eye reads the dig as it is held.
        look_from([at[0], at[1], 8.0]);
        assert_eq!(
            access.surface_support(&point(at[0], at[1])),
            Answer::Exact(Some(stands)),
            "{n} of 8 dug"
        );
    }
}

/// The west edge of the coarsest tile of the fixture's line (`resident`):
/// east of it a tile of the line holds the ground, the leaf or a coarser one
/// standing in, and west of it none does.
fn held_edge() -> f64 {
    line(top()).last().unwrap().rect().0
}

/// A ray that reaches ground no tile holds answers a lack, whatever answered
/// before it, and one that ends short of there does not.
#[test]
fn a_ray_into_ground_not_held_lacks() {
    let [x, y] = flat_at_five();
    let world = placed(Store::default());
    let to_edge = x + 0.5 - held_edge();
    let west = |z: f64, reach: f64| Ray {
        origin: [x + 0.5, y + 0.5, z],
        dir: [-1.0, 0.0, 0.0],
        t_max: reach,
    };
    let short = world.first_hit(&west(100.5, to_edge - 1.0));
    let past = world.first_hit(&west(100.5, to_edge + 1.0));
    assert!(!matches!(short, Answer::Lack(_)), "{short:?}");
    assert!(matches!(past, Answer::Lack(_)), "{past:?}");
}

/// A ray that asks the leaves' zoom over ground a coarser tile stands in for
/// answers coarser, whatever the size of the node it passes there, and one
/// that stays over the leaf held answers exactly.
#[test]
fn a_ray_over_ground_stood_in_for_answers_coarser() {
    let [x, y] = flat_at_five();
    let world = placed(Store::default());
    let west = |reach: f64| Ray {
        origin: [x + 0.5, y + 0.5, 100.5],
        dir: [-1.0, 0.0, 0.0],
        t_max: reach,
    };
    assert_eq!(world.first_hit(&west(8.0)), Answer::Exact(None));
    let past = world.first_hit(&west(300.0));
    assert!(matches!(past, Answer::Coarser(None, _)), "{past:?}");
}

/// The climb reads ground stood in for and ground not held apart: over
/// ground a coarser tile stands in for it answers the floor, and where no
/// tile holds the ground nothing fills, so it answers no floor; either way
/// with the tile lacked.
#[test]
fn no_floor_where_ground_is_not_held() {
    let [x, y] = flat_at_five();
    let world = placed(Store::default());
    let edge = held_edge();
    let z = 5.5;
    look_from([x - 100.5, y + 0.5, 8.0]);
    let floor = world.floor_under(&world::point(x - 100.5, y + 0.5, z));
    assert_eq!(floor.val().flatten(), Some(5.0), "{floor:?}");
    assert!(floor.lack().is_some(), "{floor:?}");
    look_from([edge - 10.5, y + 0.5, 8.0]);
    let floor_past = world.floor_under(&world::point(edge - 10.5, y + 0.5, z));
    assert_eq!(floor_past.val().flatten(), None, "{floor_past:?}");
    assert!(floor_past.lack().is_some(), "{floor_past:?}");
}

/// A half cube on the ground, finer than the leaves' cells, is met whole by a
/// ray where the eye is near, which asks the zoom the cube is placed at there.
#[test]
fn half_cube_met() {
    let [x, y] = flat_at_five();
    let half = units(OBJECT_COARSEST + 1);
    let mut store = Store::default();
    let red = cube::Object::Red as AssetId;
    write(&mut store, [x, y, 5.0], OBJECT_COARSEST + 1, red);
    let access = placed(store);
    let at = [x + half / 2.0, y + half / 2.0];
    let from_near = Ray {
        origin: [at[0], at[1], 8.0],
        dir: [0.0, 0.0, -1.0],
        t_max: 8.0,
    };
    look_from(from_near.origin);
    let hit = access.first_hit(&from_near).exact().flatten().unwrap();
    assert!(matches!(hit.kind, HitKind::Object(_)), "{hit:?}");
    assert_eq!(
        [hit.point[2], hit.cube.extent()[0]],
        [5.0 + half, half],
        "{hit:?}"
    );
}

/// A body near the eye stands on what is placed as it is held, wherever in
/// its cell it lies and however small: its cell is read as columns at the
/// finest zoom placed in it.
#[test]
fn body_stands_any_tier() {
    let [x, y] = flat_at_five();
    let red = cube::Object::Red as AssetId;
    for zoom in OBJECT_COARSEST..=OBJECT_FINEST {
        let (cell, side) = (units(GROUND.leaf_cell_zoom()), units(zoom));
        let mut store = Store::default();
        // The cube sits in the cell's far corner, away from its middle.
        write(
            &mut store,
            [x + cell - side, y + cell - side, 5.0],
            zoom,
            red,
        );
        let access = placed(store);
        look_from([x + 0.1, y + 0.1, 6.5]);
        let at = point(x + 0.1, y + 0.1);
        assert_eq!(
            access.surface_support(&at),
            Answer::Exact(Some(5.0 + side)),
            "a cube of z{zoom}"
        );
    }
}

/// In a stack with a cell left out, what stands in the gap stands on the
/// cube under it.
#[test]
fn stack_gap_floored() {
    let [x, y] = flat_at_five();
    let mut store = Store::default();
    let red = cube::Object::Red as AssetId;
    for z in [5.0, 7.0] {
        write(&mut store, [x, y, z], OBJECT_COARSEST, red);
    }
    let access = placed(store);
    let gap = world::point(x + 0.5, y + 0.5, 6.5);
    assert_eq!(access.floor_under(&gap), Answer::Exact(Some(6.0)));
    assert_eq!(access.surface_support(&gap), Answer::Exact(Some(6.0)));
}

/// A ray that goes on from where the last ray from its point ended meets what
/// a ray from the top meets.
#[test]
fn ray_goes_on_from_the_last() {
    use super::ray::Until;
    use crate::domain::layer::tile_cache::Path;

    let [x, y] = flat_at_five();
    let mut store = Store::default();
    let red = cube::Object::Red as AssetId;
    for z in [5.0, 7.0] {
        write(&mut store, [x, y, z], OBJECT_COARSEST, red);
    }
    let access = placed(store);
    let ray = |z: f64, dir: f64| Ray {
        origin: [x + 0.5, y + 0.5, z],
        dir: [0.0, 0.0, dir],
        t_max: 64.0,
    };
    for z in [3.5, 5.5, 6.0, 6.5, 7.0, 7.5, 9.5] {
        let (mut path, mut descent) = (Path::default(), None);
        access.along(
            &ray(z, 1.0),
            Until::Transparent,
            None,
            &mut path,
            &mut descent,
        );
        assert!(descent.is_some());
        let on = access.along(&ray(z, -1.0), Until::Opaque, None, &mut path, &mut descent);
        let top = access.along(
            &ray(z, -1.0),
            Until::Opaque,
            None,
            &mut Path::default(),
            &mut None,
        );
        let met = |a: Answer<Option<Hit>>| a.val().flatten().map(|h| (h.t, h.cube));
        assert_eq!(met(on), met(top), "from {z}");
    }
}

/// A cell coarser than what is placed in it is read as the cast draws it
/// there: one column, whatever is placed finer. From over it, from inside
/// the box it is drawn as, from that box's top, or from over it within the
/// cell's own cube, its floor is that box's top (`cast::top`), where a ray at that zoom meets
/// it.
#[test]
fn coarse_cell_floored_as_drawn() {
    let [x, y] = flat_at_five();
    let mut store = Store::default();
    let red = cube::Object::Red as AssetId;
    write(&mut store, [x, y, 5.0], OBJECT_COARSEST, red);
    write(&mut store, [x + 1.0, y, 5.0], OBJECT_FINEST, red);
    let access = placed(store);
    let zoom = LEAF_ZOOM - 1;
    let at = [x + 0.5, y + 0.5];
    // From here the eye draws the ground there with the cells of `zoom`.
    look_from([at[0], at[1], 128.0]);
    let drawn = access
        .first_hit(&down(at))
        .exact()
        .flatten()
        .unwrap()
        .cube
        .max[2];
    let side = units(GROUND.cell_zoom(zoom));
    let cube_top = libm::ceil(drawn / side) * side;
    assert!(drawn < cube_top, "the cell is filled short of its cube");
    for z in [drawn + 3.0, drawn - 0.25, (drawn + cube_top) / 2.0] {
        // The leaves' cells were not read, so the answer is a coarser one.
        let floor = access.floor_under(&world::point(at[0], at[1], z));
        assert!(
            matches!(floor, Answer::Coarser(Some(top), _) if top == drawn),
            "from {z}: {floor:?}"
        );
    }
    // A cell keeping one eighth of its cube tops at three quarters of its
    // side, a height a position holds: from exactly there too.
    write(store::object_mut(), [x, y, 8.0], OBJECT_COARSEST, red);
    let top = access
        .first_hit(&down(at))
        .exact()
        .flatten()
        .unwrap()
        .cube
        .max[2];
    assert_eq!(top, 9.5);
    assert_eq!(
        access
            .floor_under(&world::point(at[0], at[1], top))
            .val()
            .flatten(),
        Some(top)
    );
}

/// Quake III's step-slide, a column at a time. When blocked, the body
/// climbs the blocker as far as its own column is open over its head, then
/// settles. When a ceiling comes first it stops there, and a blocked axis is
/// slid along.
#[test]
fn body_climbs_or_slides() {
    let [x, y] = flat_at_five();
    let mut store = Store::default();
    let red = cube::Object::Red as AssetId;
    // A wall three cubes high stands in the column east of the body's.
    for z in [5.0, 6.0, 7.0] {
        write(&mut store, [x + 1.0, y, z], OBJECT_COARSEST, red);
    }
    let from = world::point(x + 0.5, y + 0.5, over(5.0));
    let floor = |p: Q3| q3::to_real(i64::from(p[2])) - units(GROUND.leaf_cell_zoom()) / 2.0;
    let east = world::point(x + 1.5, y + 0.5, 0.0);
    let access = placed(store);
    let to = step_slide_from(&access, &from, &east, &from, true);
    assert_eq!(
        (q3::to_real(i64::from(to[0])), floor(to)),
        (x + 1.5, 8.0),
        "climbed onto the wall"
    );
    // A ceiling two cells over the body stops the climb before it tops the
    // wall.
    write(store::object_mut(), [x, y, 7.0], OBJECT_COARSEST, red);
    let to = step_slide_from(&access, &from, &east, &from, true);
    assert!(
        q3::to_real(i64::from(to[0])) < x + 1.0,
        "stopped at the wall"
    );
    assert_eq!(floor(to), 5.0);
    // Going north-east, the east is slid along and the north is walked.
    let to = step_slide_from(
        &access,
        &from,
        &world::point(x + 1.5, y + 1.5, 0.0),
        &from,
        true,
    );
    assert!(
        q3::to_real(i64::from(to[0])) < x + 1.0,
        "slid along the wall"
    );
    assert_eq!((q3::to_real(i64::from(to[1])), floor(to)), (y + 1.5, 5.0));
    // The body shows out to where the view stops drawing cells a zoom
    // coarser than the leaves', as a decor of its cell is drawn, and for an
    // eye that near it walks, over the cells that eye draws. For an eye that far, or
    // toward an end that far from the eye, it does not: it goes where it is
    // sent, through the wall, its height not known.
    let edge = lattice::draw_dist(GROUND, LEAF_ZOOM - 1);
    let over_it = |height: f64| world::point(x + 0.5, y + 0.5, over(5.0) + height);
    assert!(!is_unknown(&step_slide_from(
        &access,
        &from,
        &east,
        &over_it(edge * 0.99),
        true
    )));
    assert_eq!(
        step_slide_from(&access, &from, &east, &over_it(edge), true),
        unknown(&east)
    );
    let far = world::point(x + 0.5 + edge, y + 0.5, 0.0);
    assert_eq!(
        step_slide_from(&access, &from, &far, &from, true),
        unknown(&far)
    );
}

/// A body passes under a ceiling only where its box stands clear: a
/// passage lower than the box (pxl-spec `BODY_BOX`) stops it, and one as
/// high lets it through.
#[test]
fn a_passage_lower_than_a_body_stops_it() {
    let [x, y] = flat_at_five();
    let red = cube::Object::Red as AssetId;
    let from = world::point(x + 0.5, y + 0.5, over(5.0));
    let east = world::point(x + 1.5, y + 0.5, 0.0);
    let height = support::body_height_u();
    for (gap, passes) in [(height - 1.0, false), (height, true)] {
        let mut store = Store::default();
        write(&mut store, [x + 1.0, y, 5.0 + gap], OBJECT_COARSEST, red);
        let to = step_slide_from(&placed(store), &from, &east, &from, true);
        assert_eq!(
            q3::to_real(i64::from(to[0])) > x + 1.0,
            passes,
            "a passage {gap} high"
        );
    }
}

/// A body claims its box over the leaf cell its feet are in: a cube at its
/// head is in that room, and one beside its head is not.
#[test]
fn a_body_claims_its_box() {
    let s = units(GROUND.leaf_cell_zoom());
    let room = body_room(&world::point(10.5 * s, -3.5 * s, over(5.0 * s)));
    let cube_at = |x: f64, y: f64, z: f64| Aabb {
        min: [x * s, y * s, z * s],
        max: [(x + 1.0) * s, (y + 1.0) * s, (z + 1.0) * s],
    };
    assert!(room.shares(cube_at(10.0, -4.0, 5.0)), "its feet");
    assert!(room.shares(cube_at(10.0, -4.0, 6.0)), "its head");
    assert!(!room.shares(cube_at(10.0, -4.0, 7.0)), "over its head");
    assert!(!room.shares(cube_at(11.0, -4.0, 6.0)), "beside its head");
}

/// A move that is not walked asks the area of its two ends alone. One
/// walled and within it stays where it is rather than leave; one not walled
/// goes; and one already past the area goes on freely.
#[test]
fn body_not_shown_keeps_to_the_area() {
    use crate::sys::io::source;
    use pxl_spec::bake::publication::FORMAT;
    use pxl_spec::space::coord::zoom::SHARD_ZOOM;
    source::declare(MIN_ZOOM, &FORMAT, ["0103300"]).unwrap();
    let world = placed(Store::default());
    let side = units(SHARD_ZOOM);
    let middle = |[i, j]: [i32; 2]| {
        world::point(
            (f64::from(i) + 0.5) * side,
            (f64::from(j) + 0.5) * side,
            over(0.0),
        )
    };
    // A shard the source serves, and going east from it the first two it
    // does not.
    let served = (-128..128)
        .flat_map(|i| (-128..128).map(move |j| [i, j]))
        .find(|&c| !world.past(&middle(c)))
        .unwrap();
    let east = (1..)
        .find(|&k| world.past(&middle([served[0] + k, served[1]])))
        .unwrap();
    let [inside, outside, further] =
        [0, east, east + 1].map(|k| middle([served[0] + k, served[1]]));
    assert!(world.within(&inside) && !world.within(&outside));
    // The ends lie a shard apart, farther than a body shows for any
    // eye: here one beside the start.
    let eye = inside;
    assert_eq!(
        step_slide_from(&world, &inside, &outside, &eye, true),
        inside
    );
    assert_eq!(
        step_slide_from(&world, &inside, &outside, &eye, false),
        unknown(&outside)
    );
    assert_eq!(
        step_slide_from(&world, &outside, &further, &eye, true),
        unknown(&further)
    );
    // A body that cannot be stood, no ground held under it, is walled the
    // same.
    let lost = unknown(&inside);
    assert_eq!(step_slide_from(&world, &lost, &outside, &eye, true), lost);
}

/// A height picks the floor nearest it: the ground under a deck or the deck,
/// whichever is closer. A solid with no clear cell over it is no floor.
#[test]
fn height_nearest_floor() {
    let [x, y] = flat_at_five();
    let mut store = Store::default();
    let red = cube::Object::Red as AssetId;
    // A deck a cube thick has its top at 10, with the ground at 5 under it.
    write(&mut store, [x, y, 9.0], OBJECT_COARSEST, red);
    let access = placed(store);
    let near = |z: f64| access.support_nearest(observer::eye(), &world::point(x + 0.5, y + 0.5, z));
    assert_eq!(
        near(6.0),
        Answer::Exact(Some(5.0)),
        "the ground under the deck"
    );
    assert_eq!(near(12.0), Answer::Exact(Some(10.0)), "the deck");
    // A cube right under the deck's cube has no clear cell over its top.
    write(store::object_mut(), [x, y, 8.0], OBJECT_COARSEST, red);
    assert_eq!(
        access.support_nearest(observer::eye(), &world::point(x + 0.5, y + 0.5, 9.0)),
        Answer::Exact(Some(10.0))
    );
}

#[test]
fn box_placed_matches_store() {
    let unit = UNIT_ZOOM;
    let mut store = Store::default();
    let corners: [V3; 5] = [
        [3.0, 4.0, 5.0],
        [3.0, 5.0, 5.0],
        [10.0, 4.0, 2.0],
        [40.0, 4.0, 5.0],
        [3.0, 4.0, 30.0],
    ];
    for (k, &c) in corners.iter().enumerate() {
        write(&mut store, c, unit, 100 + k as u32);
    }
    // The box spans the first three's cells; the last two lie past it.
    let cell = |c: V3| idx_of(unit, &world::point(c[0], c[1], c[2]));
    let (a, b, c) = (cell(corners[0]), cell(corners[1]), cell(corners[2]));
    let lo = [a[0].min(c[0]), a[1].min(b[1]), c[2].min(a[2])];
    let hi = [a[0].max(c[0]), a[1].max(b[1]), c[2].max(a[2])];
    let lanes = store.columns();
    let mut asked = Vec::new();
    let mut placed = placed_in(lo, hi);
    loop {
        let (at, word, more) = placed.next(&lanes);
        if !more {
            break;
        }
        asked.push((at, word & 0xffff));
    }
    let mut in_store = Vec::new();
    let space = Aabb {
        min: [3.0, 4.0, 2.0],
        max: [11.0, 6.0, 6.0],
    };
    store.entries_in(&space, &mut |e| {
        if e.zoom == unit {
            in_store.push((e.at, e.asset_id));
        }
    });
    asked.sort_unstable();
    in_store.sort_unstable();
    assert_eq!(asked.len(), 3, "{asked:?}");
    assert_eq!(asked, in_store);
}

/// A brick handed after the clip levels were folded without it is read with
/// what it holds once a frame has handed it, since the hand folds them again
/// (domain/layer/terrain/clipmap.rs `upload`). A spike a cell wide in the
/// brick, over ground at five and taller than a node over the brick, is met
/// where a body's ray down it met it when the brick was held from the
/// start.
#[test]
fn late_brick_spike() {
    use crate::domain::layer::terrain::clipmap::clip_read;
    use crate::domain::layer::terrain::clipmap::test::{hand, withhold};
    use pxl_engine::clipmap as cells;
    use pxl_engine::clipmap::{BrickSource, CLIP_SIZE, Lookup};
    use pxl_spec::brick::BRICK;
    use pxl_spec::space::coord::zoom::LEAF_CELL_ZOOM;
    let leaf = top().children()[0];
    let mut ele = vec![5; CELL_CNT];
    ele[200 * SPAN + 300] = 400;
    let spiked = drawn(
        ele,
        vec![Ground::Land as u8; SPAN * SPAN].into(),
        Default::default(),
    );
    let (x0, y0, _) = leaf.rect();
    resident([x0 + 300.0, y0 + 200.0], &[(leaf, spiked)]);
    let world = placed(Store::default());
    // Find the spike's column in the clip level of the eye's leaves.
    let (first, _) = clip_read().origin(LEAF_CELL_ZOOM);
    let top = |g: [i32; 2]| {
        cells::column_top(cells::cell(&clip_read(), Lookup::NONE, LEAF_CELL_ZOOM, g).1)
    };
    let spike = (0..CLIP_SIZE as i32 * CLIP_SIZE as i32)
        .map(|k| {
            let w = CLIP_SIZE as i32;
            [
                first[0] * BRICK as i32 + k % w,
                first[1] * BRICK as i32 + k / w,
            ]
        })
        .find(|&g| top(g) == 400)
        .expect("the spike held");
    let side = units(GROUND.leaf_cell_zoom());
    let at = [
        (f64::from(spike[0]) + 0.5) * side,
        (f64::from(spike[1]) + 0.5) * side,
    ];
    let met = |access: &World| access.first_hit(&down(at)).val().flatten().unwrap().point[2];
    let before = met(&world);
    withhold(LEAF_CELL_ZOOM, spike);
    hand();
    assert_eq!(met(&world), before, "the spike met");
}

/// What the ground generates stands where it was generated, whatever is put
/// over it or in place of the cell under it, air or a cube, since it
/// generates on the base (pxl-content `generation::placement`).
#[test]
fn generated_decor_stays() {
    let [x, y] = flat_at_five();
    let near = |store: Store| {
        let world = placed(store);
        let space = Aabb {
            min: [x - 8.0, y - 8.0, world::BOUND.min[2]],
            max: [x + 8.0, y + 8.0, world::BOUND.max[2]],
        };
        world.generated_over(&space)
    };
    let bare = near(Store::default());
    let tree = *bare.first().expect("a decor generates about the eye");
    let [cx, cy, cz] = tree.generated_in().unwrap();
    let column = |dz: i32| [f64::from(cx), f64::from(cy), f64::from(cz + dz)];

    let mut store = Store::default();
    let red = cube::Object::Red as AssetId;
    write(&mut store, column(6), OBJECT_COARSEST, red);
    assert_eq!(near(store), bare, "a cube over it moves nothing");

    for (under, what) in [(Sentinel::Air as AssetId, "a carve"), (red, "a cube")] {
        let mut store = Store::default();
        write(&mut store, column(-1), OBJECT_COARSEST, under);
        assert_eq!(near(store), bare, "{what} under it moves nothing");
    }
}

/// Only an edit in its own box, air or a cube, takes what the ground
/// generates away. An edit under it or over it leaves it generated
/// (pxl-content `generation::DecorQuery::edited`).
#[test]
fn generated_decor_edit_removes() {
    let [x, y] = flat_at_five();
    let world = placed(Store::default());
    let space = Aabb {
        min: [x - 8.0, y - 8.0, world::BOUND.min[2]],
        max: [x + 8.0, y + 8.0, world::BOUND.max[2]],
    };
    let at = |tree: &Instance| {
        let [cx, cy, cz] = tree.generated_in().unwrap();
        (Decor::of(tree.id), [cx, cy, cz])
    };
    let generates_on = |store: Store, decor: &Decor, [cx, cy, _]: [i32; 3]| {
        let world = placed(store);
        scatter::generated_at(&mut scatter::DecorWorld::new(&world), decor, [cx, cy], true)
            .map(|(_, generates)| generates)
    };
    // Pick a decor that generates on bare ground with its box whole.
    let (decor, [cx, cy, cz]) = world
        .generated_over(&space)
        .iter()
        .map(at)
        .find(|(d, c)| generates_on(Store::default(), d, *c) == Some(true))
        .expect("a decor generates about the eye");
    let generates = |store: Store| generates_on(store, decor, [cx, cy, cz]);
    let red = cube::Object::Red as AssetId;
    for (dz, val, what, still) in [
        (0, Sentinel::Air as AssetId, "air in its box", false),
        (0, red, "a cube in its box", false),
        (-1, Sentinel::Air as AssetId, "air under it", true),
        (-1, red, "a cube under it", true),
        (6, red, "a cube over it", true),
    ] {
        let mut store = Store::default();
        let at = [f64::from(cx), f64::from(cy), f64::from(cz + dz)];
        write(&mut store, at, OBJECT_COARSEST, val);
        assert_eq!(generates(store), Some(still), "{what}");
    }
}

/// What the ground generates is struck only for the decors the frame generates
/// at the leaves' clip level. Where the frame generates none, as with the
/// decor setting off (domain/view/lens.rs `decor_zooms`), the pointer's ray
/// goes through where a decor would stand and reaches the ground under it
/// (`World::first_seen`).
#[test]
fn generated_decor_struck_where_generated() {
    use pxl_spec::asset::decor::DECORS;
    let [x, y] = flat_at_five();
    let world = placed(Store::default());
    let space = Aabb {
        min: [x - 8.0, y - 8.0, world::BOUND.min[2]],
        max: [x + 8.0, y + 8.0, world::BOUND.max[2]],
    };
    let generates = |i: &Instance| {
        let [cx, cy, _] = i.generated_in().unwrap();
        let decor = Decor::of(i.id);
        scatter::generated_at(&mut scatter::DecorWorld::new(&world), decor, [cx, cy], true)
            .is_some_and(|(_, generates)| generates)
    };
    let tree = world
        .generated_over(&space)
        .into_iter()
        .find(generates)
        .expect("a decor generates about the eye");
    let b = decor::claim(&tree).unwrap();
    let ray = Ray {
        origin: [
            (b.min[0] + b.max[0]) / 2.0,
            (b.min[1] + b.max[1]) / 2.0,
            b.max[2] + 1.0,
        ],
        dir: [0.0, 0.0, -1.0],
        t_max: 20.0,
    };
    let met = |decors: u32| {
        world
            .first_seen(&ray, decors)
            .exact()
            .flatten()
            .expect("the ground under it")
            .kind
    };
    let leaves = (1 << DECORS.len()) - 1;
    assert!(
        matches!(met(leaves), HitKind::Decor(i) if i.generated_in() == tree.generated_in()),
        "{:?}",
        met(leaves)
    );
    let none = 0;
    assert!(!matches!(met(none), HitKind::Decor(_)), "{:?}", met(none));
    let body = world.first_hit(&ray).exact().flatten().unwrap();
    assert_eq!(body.kind, HitKind::Ground, "a body passes it");
}

/// A decor whose base sinks under its cell is struck at the box it is drawn
/// as, the claim's, and not the cells it claims, which are raised to its cell.
#[test]
fn sunk_decor_struck_at_its_drawn_box() {
    use super::ray::Viewer;
    use pxl_content::generation::Prop;
    use pxl_spec::asset::decor::DECORS;
    use pxl_spec::space::coord::q3::FRAC_W;
    let sunk = scatter::instance(
        &DECORS[0],
        &Prop {
            variant: 0,
            turn: 0,
            at: [100, 200, 5],
            sink: 1 << (FRAC_W - 1),
        },
    );
    let b = decor::claim(&sunk).unwrap();
    assert_eq!(b.min[2], 4.5);
    let ray = Ray {
        origin: [100.5, 200.5, 30.0],
        dir: [0.0, 0.0, -1.0],
        t_max: 40.0,
    };
    let mut viewer = Viewer {
        ray: &ray,
        decors: u32::MAX,
        met: None,
    };
    assert!(viewer.sees(&sunk));
    let met = viewer.met.unwrap();
    assert_eq!((met.cube, met.t), (b, 30.0 - b.max[2]));
}

/// A decor someone placed is the same to whatever asks as one the ground
/// grew: every cell of its box answers it, the eye stops at its box and a
/// body passes it. A ray that starts inside the box does not meet it.
#[test]
fn placed_decor_answers_its_box() {
    use pxl_engine::answer::PLACED_ASSET;
    use pxl_spec::asset::decor::DECORS;
    let [x, y] = flat_at_five();
    let [cx, cy] = [x.floor(), y.floor()];
    let tree = DECORS[0];
    let mut store = Store::default();
    write(&mut store, [cx, cy, 5.0], OBJECT_COARSEST, tree.id);
    let world = placed(store);
    let cell = [cx as i32, cy as i32, 5];
    let word = PLACED_ASSET | tree.id;
    assert_eq!(world.asset((LEAF_CELL_ZOOM, cell)).exact(), Some(word));
    let shot = |z: f64, dir: f64| Ray {
        origin: [cx + 0.5, cy + 0.5, z],
        dir: [0.0, 0.0, dir],
        t_max: 40.0,
    };
    let seen = world
        .first_seen(&shot(30.0, -1.0), 0)
        .exact()
        .flatten()
        .unwrap();
    assert!(matches!(seen.kind, HitKind::Decor(i) if i.generated_in().is_none()));
    assert_eq!(seen.normal, [0.0, 0.0, 1.0]);
    let body = world.first_hit(&shot(30.0, -1.0));
    assert_eq!(body.exact().flatten().unwrap().kind, HitKind::Ground);
    let from_inside = world
        .first_seen(&shot(5.5, -1.0), 0)
        .exact()
        .flatten()
        .unwrap();
    assert_eq!(from_inside.kind, HitKind::Ground);
    let stands = world.floor_under(&world::point(cx + 0.5, cy + 0.5, 6.5));
    assert_eq!(
        stands.exact().flatten(),
        Some(5.0),
        "a body stands under it"
    );
}

/// A service area of the shards at `SHARD_ZOOM` in column `.0` and east of
/// it, in every row, with their nines laid as a declaration lays them
/// (`area::nine`).
struct EastOf(i32);

impl Area for EastOf {
    fn ends(&self) -> bool {
        true
    }
    fn nine(&self, key: u32) -> u32 {
        use pxl_spec::space::coord::zoom::SHARD_ZOOM;
        // This undoes `area::shard_key`, which interleaves X's and Y's bits.
        let mut c = [0i32; 2];
        for b in 0..SHARD_ZOOM {
            c[0] |= ((key >> (2 * b) & 1) as i32) << b;
            c[1] |= ((key >> (2 * b + 1) & 1) as i32) << b;
        }
        area::nine(|n| n[0] >= self.0, c)
    }
}

/// The service area's edge is the wall's columns, a leaf cell thick. A leaf
/// cell one in from the wall is the area's and editable; the wall's column
/// and a cell past it are neither (`World::within`, `World::editable`).
#[test]
fn area_edge_leaf_cell_thick() {
    use pxl_spec::space::coord::zoom::SHARD_ZOOM;
    let s = LEAF_ZOOM - SHARD_ZOOM;
    // The westmost leaf of a served shard whose west neighbor is not served.
    let leaf = Quadrant::new(LEAF_ZOOM, [5 << s, 3 << s]);
    let area = EastOf(5);
    let flat = drawn(
        vec![5; CELL_CNT],
        vec![Ground::Land as u8; SPAN * SPAN].into(),
        Default::default(),
    );
    let (x0, y0, _) = leaf.rect();
    resident([x0 + 10.0, y0 + 10.0], &[(leaf, flat)]);
    let world = placed(Store::default());
    let cell = units(GROUND.leaf_cell_zoom());
    // For the leaf cell `i` cells east of the leaf's west edge: whether it
    // is the area's, and whether a cube of it on the ground is editable.
    let column = |i: f64| {
        let (x, y) = (x0 + i * cell, y0 + 10.0 * cell);
        let space = Aabb {
            min: [x, y, 5.0],
            max: [x + cell, y + cell, 5.0 + cell],
        };
        (
            limit::within_in(&area, &point(x + 0.5 * cell, y + 0.5 * cell)),
            world.editable_in(&area, &space),
        )
    };
    assert_eq!(column(1.0), (true, true), "one in from the wall");
    assert_eq!(column(0.0), (false, false), "the wall's column");
    assert_eq!(column(-1.0), (false, false), "past the wall");
}

/// A decor from another version's catalog, read from a saved file, is
/// unknown: it has no instance, and the core does not stop.
#[test]
fn stored_decor_past_the_catalog_is_unknown() {
    use crate::domain::layer::object::asset::Asset;
    use crate::domain::layer::object::meta::decor_instance;
    use pxl_spec::asset::id::IDX_BITS;
    use pxl_spec::space::layer::octant::Octant;
    let [x, y] = flat_at_five();
    let past: AssetId = (1 << IDX_BITS) | ((1 << IDX_BITS) - 1);
    let mut store = Store::default();
    write(&mut store, [x, y, 5.0], OBJECT_COARSEST, past);
    let at = Octant::holding(OBJECT_COARSEST, &world::point(x, y, 5.0));
    let entry = placed(store).entry_at(at).unwrap();
    assert_eq!(entry.asset_id, past);
    assert_eq!(entry.asset(), Asset::Unknown);
    assert!(decor_instance(&entry).is_none());
}

/// The area's progress the UI shows while the ground loads is the share of
/// the tiles asked for that show.
#[test]
fn shown_share_of_what_is_asked() {
    flat_at_five();
    let tile_cache = TerrainTile::tile_cache();
    assert!(tile_cache.is_shown());
    assert_eq!(tile_cache.shown_share(), 1.0);
}
