// pxl-kernel/domain/query.test.rs
//
// The query's laws over a leaf tile's four children folded into their
// parent as the bake folds them, and objects written into a store.

use pxl_spec::asset::cube::{self, ground, sentinel};
use pxl_spec::asset::cube::{cube_id, Cube};
use pxl_spec::bake::constant::{OBJECT_COARSEST, OBJECT_FINEST};
use pxl_spec::bake::tile::terrain::Ground;
use pxl_spec::space::layer::lattice::{Cells, GROUND, TILE_CELLS};

use super::*;
use crate::domain::layer::Answer;
use pxl_spec::bake::tile::terrain::{GroundId, TerrainTile, GROUND_LANE};
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM, TOP_ZOOM};

use crate::domain::layer::object::store::Store;
use crate::domain::layer::tile_cache::TileCache;
use pxl_spec::space::coord::zoom::{index_of, units, Zoom};

use crate::domain::layer::object::store::Write;
use crate::sys::serial;

const CELLS: usize = GROUND_LANE;
const SPAN: usize = TILE_CELLS as usize;

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
/// (`domain/layer/tile_cache/floor.rs`): a tile's coarser pictures here are the
/// same ground, each marking its children held; their cells held in the
/// windows about `centre`, in units.
fn resident(centre: [f64; 2], held: &[(Tile, Leaf)]) -> TileCache<TerrainTile> {
    use pxl_spec::bake::tile::Occupancy;
    let mut tile_cache = TileCache::default();
    let mut admitted: Vec<Tile> = Vec::new();
    let mut cells = Vec::new();
    for (
        p,
        Leaf {
            tile,
            looks,
            heights,
        },
    ) in held
    {
        let line = std::iter::successors(Some(*p), |q| (q.zoom > TOP_ZOOM).then(|| q.parent()));
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
            tile_cache
                .admit(q, |_| None, |_| Ok::<_, ()>(picture))
                .unwrap();
        }
    }
    // Asked for, as the floor asks for what it holds: a tile held but not
    // asked is cache, which answers nothing.
    let asks: fnv::FnvHashMap<Tile, crate::sys::io::demand::Ask> = admitted
        .iter()
        .map(|&q| (q, crate::sys::io::demand::Ask::PLAIN))
        .collect();
    tile_cache.ask(&asks);
    // Their cells as a GPU would hand them back, at once.
    crate::domain::layer::terrain::window::test::hold(centre, &cells);
    tile_cache
}

/// A leaf as a client holds it once its cells are back: each cell's height and look given.
fn drawn(
    heights: Vec<i16>,
    ground: Ground,
    separation: pxl_spec::bake::separation::Separation,
) -> Leaf {
    Leaf {
        tile: TerrainTile::of(
            Vec::new(),
            Ground {
                cells: Vec::new().into(),
            },
            separation,
            pxl_spec::bake::tile::Occupancy::LEAF,
        ),
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
    let tile_cache = resident(at, &[(leaf, void)]);

    let empty = Store::default();
    let access = World::new(&tile_cache, &empty);
    assert_eq!(
        access
            .surface_of(
                GROUND.cell(),
                GROUND.cell_of(LEAF_ZOOM, &point(at[0], at[1])).node().1
            )
            .value()
            .unwrap(),
        None
    );
    assert_eq!(
        access.surface_support(&point(at[0], at[1])),
        Answer::Exact(None)
    );
    assert_eq!(
        access
            .first_hit(&down(at), at_zoom(LEAF_ZOOM))
            .value()
            .flatten(),
        None
    );

    let mut store = Store::default();
    let red = cube_id(Cube::Object(cube::Object::red));
    write(
        &mut store,
        [x0 + 10.0, y0 + 10.0, 5.0],
        OBJECT_COARSEST,
        red,
    );
    let access = World::new(&tile_cache, &store);
    assert_eq!(
        access.surface_support(&point(at[0], at[1])),
        Answer::Exact(Some(6.0))
    );
    let hit = access
        .first_hit(&down(at), at_zoom(LEAF_ZOOM))
        .value()
        .flatten()
        .unwrap();
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
    separation.push(
        [([0.0, 10.5], 10, 0), ([40.0, 10.5], 10, 0)],
        4 * POINT_SCALE as u8,
        ground::residential_road,
        Form::Bridge,
        0,
    );
    let void = drawn(
        vec![0; CELLS],
        Ground {
            cells: vec![sentinel::void; SPAN * SPAN].into(),
        },
        separation.finish(),
    );
    let (x0, y0, _) = leaf.rect();
    let at = [x0 + 20.5, y0 + 10.5];
    let tile_cache = resident(at, &[(leaf, void)]);
    let store = Store::default();
    let access = World::new(&tile_cache, &store);
    assert_eq!(
        access.surface_support(&point(at[0], at[1])),
        Answer::Exact(Some(10.0))
    );
    let hit = access
        .first_hit(&down(at), at_zoom(LEAF_ZOOM))
        .value()
        .flatten()
        .unwrap();
    assert_eq!(hit.point[2], 10.0, "{hit:?}");
}

/// A ray in the frame that runs on round the world meets, past the
/// antimeridian, the ground and the objects stored under the world's name for them.
#[test]
fn a_ray_across_the_antimeridian_meets_what_stands_past_it() {
    let _g = serial();
    let n = 1i32 << LEAF_ZOOM;
    let flat = || {
        let cells = vec![ground::land; SPAN * SPAN];
        drawn(
            vec![5; CELLS],
            Ground {
                cells: cells.into(),
            },
            Default::default(),
        )
    };
    let tile = |x: i32| pxl_spec::space::layer::tile::tile_at(LEAF_ZOOM, [x, 0]);
    let seam = pxl_spec::space::coord::world::ROUND / 2.0;
    let tile_cache = resident(
        [seam, 100.5],
        &[(tile(n / 2 - 1), flat()), (tile(n / 2), flat())],
    );
    let top = World::new(&tile_cache, &Store::default())
        .surface_of(
            GROUND.cell(),
            GROUND
                .cell_of(LEAF_ZOOM, &point(-seam + 20.5, 100.5))
                .node()
                .1,
        )
        .value()
        .flatten()
        .unwrap()
        .top;

    let mut store = Store::default();
    let red = cube_id(Cube::Object(cube::Object::red));
    write(&mut store, [-seam + 40.0, 100.0, top], OBJECT_COARSEST, red);
    let access = World::new(&tile_cache, &store);

    let ground = access
        .first_hit(&down([seam + 20.5, 100.5]), at_zoom(LEAF_ZOOM))
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
        direction: [1.0, 0.0, 0.0],
        reach: 200.0,
    };
    let hit = access
        .first_hit(&along, at_zoom(LEAF_ZOOM))
        .exact()
        .flatten()
        .unwrap();
    assert!(matches!(hit.struck, Struck::Object(_)), "{hit:?}");
    assert_eq!(hit.point[0], seam + 40.0, "{hit:?}");
}

fn flat_at_five() -> (TileCache<TerrainTile>, [f64; 2]) {
    let leaf = top().children()[0];
    let flat = drawn(
        vec![5; CELLS],
        Ground {
            cells: vec![ground::land; SPAN * SPAN].into(),
        },
        Default::default(),
    );
    let (x0, y0, _) = leaf.rect();
    (
        resident([x0 + 10.0, y0 + 10.0], &[(leaf, flat)]),
        [x0 + 10.0, y0 + 10.0],
    )
}

/// An eye over the world's top casts from where it stands, not from the
/// top: its ray falls through what is over the world and meets the ground
/// where its line does.
#[test]
fn a_ray_from_over_the_world_meets_the_ground_where_its_line_does() {
    let _g = serial();
    let (tile_cache, at) = flat_at_five();
    let store = Store::default();
    let access = World::new(&tile_cache, &store);
    let top = access
        .first_hit(&down(at), at_zoom(LEAF_ZOOM))
        .value()
        .flatten()
        .unwrap()
        .point[2];
    let high = 2.0 * world::REACH.max[2];
    let over = Ray {
        origin: [at[0], at[1], high],
        direction: [0.0, 0.0, -1.0],
        reach: 2.0 * high,
    };
    let met = access.first_hit(&over, at_zoom(LEAF_ZOOM));
    assert!(matches!(met, Answer::Exact(_)), "{met:?}");
    let hit = met.value().flatten().unwrap();
    assert_eq!([hit.point[2], hit.t], [top, high - top], "{hit:?}");
}

/// A dig is met as the cells asked fold it, nothing finer: at the leaves' cells
/// a cell dug in part keeps the eighths left, a box of their volume about the
/// cell's center whose top a ray meets (`answer::side`), and a cell dug whole
/// is air, the ground under it met. A walker reads the cell as its columns at
/// the finest zoom placed in it, and stands on what is left as it is held:
/// on the half cubes over a dug lower half.
#[test]
fn a_dig_is_met_as_the_cells_asked_fold_it() {
    use crate::domain::layer::object::store::AIR;
    use pxl_spec_core::answer::{side, EIGHTHS};
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let half = units(OBJECT_COARSEST + 1);
    let dug = |n: usize| {
        let mut store = Store::default();
        for k in 0..n {
            let [dx, dy, dz] = [k & 1, k >> 1 & 1, k >> 2 & 1].map(|b| b as f64 * half);
            write(
                &mut store,
                [x + dx, y + dy, 4.0 + dz],
                OBJECT_COARSEST + 1,
                AIR,
            );
        }
        store
    };
    for (n, stands) in [(1, 5.0), (3, 5.0), (4, 5.0), (8, 4.0)] {
        let left = EIGHTHS - n as u32;
        let top = if left == 0 {
            4.0
        } else {
            4.5 + f64::from(side(left)) / 2.0
        };
        let store = dug(n);
        let access = World::new(&tile_cache, &store);
        let at = [x + half / 2.0, y + half / 2.0];
        let hit = access
            .first_hit(&down(at), at_zoom(LEAF_ZOOM))
            .exact()
            .flatten()
            .unwrap();
        // The top as `cast::top` gives it, a share of the cell in f32.
        assert!(
            (hit.cube.max[2] - top).abs() < 1e-6,
            "{n} of 8 dug: {hit:?}"
        );
        assert_eq!(
            access.surface_support(&point(at[0], at[1])),
            Answer::Exact(Some(stands)),
            "{n} of 8 dug"
        );
    }
}

/// What is held is met as finely as it is held: a half cube on the ground,
/// finer than the leaves' cells, is met whole by a pick, which asks it as it
/// is placed.
#[test]
fn a_half_cube_is_met_as_it_is_placed() {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let half = units(OBJECT_COARSEST + 1);
    let mut store = Store::default();
    let red = cube_id(Cube::Object(cube::Object::red));
    write(&mut store, [x, y, 5.0], OBJECT_COARSEST + 1, red);
    let access = World::new(&tile_cache, &store);
    let at = [x + half / 2.0, y + half / 2.0];
    let hit = access
        .first_hit(&down(at), held())
        .exact()
        .flatten()
        .unwrap();
    assert!(matches!(hit.struck, Struck::Object(_)), "{hit:?}");
    assert_eq!(
        [hit.point[2], hit.cube.extent()[0]],
        [5.0 + half, half],
        "{hit:?}"
    );
}

/// A walker stands on what is placed as it is held, wherever in its cell it
/// lies and however small: its cell is read as columns at the finest zoom
/// placed in it.
#[test]
fn a_walker_stands_on_a_cube_of_any_tier_anywhere_in_its_cell() {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let red = cube_id(Cube::Object(cube::Object::red));
    for zoom in OBJECT_COARSEST..=OBJECT_FINEST {
        let (cell, side) = (units(GROUND.cell()), units(zoom));
        let mut store = Store::default();
        // In the cell's far corner, off its middle.
        write(
            &mut store,
            [x + cell - side, y + cell - side, 5.0],
            zoom,
            red,
        );
        let access = World::new(&tile_cache, &store);
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
fn a_gap_in_a_stack_is_floored_by_the_cube_under_it() {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let mut store = Store::default();
    let red = cube_id(Cube::Object(cube::Object::red));
    for z in [5.0, 7.0] {
        write(&mut store, [x, y, z], OBJECT_COARSEST, red);
    }
    let access = World::new(&tile_cache, &store);
    let gap = world::point(x + 0.5, y + 0.5, 6.5);
    assert_eq!(
        access.floor_under(&gap, LEAF_ZOOM),
        Answer::Exact(Some(6.0))
    );
    assert_eq!(access.surface_support(&gap), Answer::Exact(Some(6.0)));
}

/// Quake III's step-slide, a column at a time: blocked, the walker climbs the
/// blocker as far as its own column is open over its head, then settles; with
/// a ceiling first it stops there, and an axis that is blocked is slid along.
#[test]
fn a_walker_climbs_what_blocks_it_under_open_sky_and_slides_under_a_ceiling() {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
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
        let access = World::new(&tile_cache, &store);
        let to = access.step_slide(&from, &east, &from, true, true).unwrap();
        assert_eq!(
            (q3::to_real(i64::from(to[0])), floor(to)),
            (x + 1.5, 8.0),
            "climbed onto the wall"
        );
    }
    // A roof over the walker two cells up: the climb meets it first.
    write(&mut store, [x, y, 7.0], OBJECT_COARSEST, red);
    let access = World::new(&tile_cache, &store);
    let to = access.step_slide(&from, &east, &from, true, true).unwrap();
    assert!(
        q3::to_real(i64::from(to[0])) < x + 1.0,
        "stopped at the wall"
    );
    assert_eq!(floor(to), 5.0);
    // Going north-east, the east is slid along and the north is walked.
    let to = access
        .step_slide(
            &from,
            &world::point(x + 1.5, y + 1.5, 0.0),
            &from,
            true,
            true,
        )
        .unwrap();
    assert!(
        q3::to_real(i64::from(to[0])) < x + 1.0,
        "slid along the wall"
    );
    assert_eq!((q3::to_real(i64::from(to[1])), floor(to)), (y + 1.5, 5.0));
}

/// A height picks the floor nearest it: the ground under a deck, or the deck,
/// whichever its height is closer to; a solid with no clear cell over it is
/// no floor.
#[test]
fn a_height_stands_on_the_floor_nearest_it() {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let mut store = Store::default();
    let red = cube_id(Cube::Object(cube::Object::red));
    // A deck a cube thick, its top at 10, with the ground at 5 under it.
    write(&mut store, [x, y, 9.0], OBJECT_COARSEST, red);
    let access = World::new(&tile_cache, &store);
    let near = |z: f64| access.support_nearest(&world::point(x + 0.5, y + 0.5, z));
    assert_eq!(
        near(6.0),
        Answer::Exact(Some(5.0)),
        "the ground under the deck"
    );
    assert_eq!(near(12.0), Answer::Exact(Some(10.0)), "the deck");
    // A cube right under the deck's: its top has no clear cell over it.
    write(&mut store, [x, y, 8.0], OBJECT_COARSEST, red);
    let access = World::new(&tile_cache, &store);
    assert_eq!(
        access.support_nearest(&world::point(x + 0.5, y + 0.5, 9.0)),
        Answer::Exact(Some(10.0))
    );
}

/// A reading reports the columns its queries read: a support its own
/// cell's, a walk every cell it passes, joined.
#[test]
fn a_reading_reports_the_columns_its_queries_read() {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let store = Store::default();
    let read = changed::Columns::default();
    let access = World::new(&tile_cache, &store).reading(&read);
    let cell = units(GROUND.cell());
    let at = [x + 0.5 * cell, y + 0.5 * cell];
    let c = at.map(|v| libm::floor(v / cell) * cell);
    access.surface_support(&point(at[0], at[1]));
    assert_eq!(read.get(), Some([c, c.map(|v| v + cell)]));
    let from = world::point(at[0], at[1], over(5.0));
    access
        .step_slide(
            &from,
            &world::point(at[0] + 3.0 * cell, at[1], 0.0),
            &from,
            true,
            true,
        )
        .unwrap();
    assert_eq!(read.get(), Some([c, [c[0] + 4.0 * cell, c[1] + cell]]));
}

#[test]
fn what_is_placed_in_a_box_is_what_the_store_holds_there() {
    let _g = serial();
    let unit = pxl_spec_core::UNIT_ZOOM;
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
    // The first three, the box their cells span; the last two past it.
    let cell = |c: V3| index_of(unit, &world::point(c[0], c[1], c[2]));
    let (a, b, c) = (cell(corners[0]), cell(corners[1]), cell(corners[2]));
    let lo = [a[0].min(c[0]), a[1].min(b[1]), c[2].min(a[2])];
    let hi = [a[0].max(c[0]), a[1].max(b[1]), c[2].max(a[2])];
    let lanes = store.lanes();
    let mut asked = Vec::new();
    let mut placed = pxl_spec_core::world::placed_in(lo, hi);
    loop {
        let (at, word, more) = placed.next(&lanes);
        if !more {
            break;
        }
        asked.push((at, word & 0xffff));
    }
    let mut held = Vec::new();
    let space = pxl_spec::space::coord::aabb::Aabb {
        min: [3.0, 4.0, 2.0],
        max: [11.0, 6.0, 6.0],
    };
    store.entries_in(&space, &mut |e| {
        if e.zoom == unit {
            held.push((e.at, u32::from(e.state)));
        }
    });
    asked.sort_unstable();
    held.sort_unstable();
    assert_eq!(asked.len(), 3, "{asked:?}");
    assert_eq!(asked, held);
}

/// A square that lands after the windows were folded without it is read
/// with what it holds once the landings are folded, as the stage folds them
/// (domain/layer/terrain/window.rs `refold`): a spike
/// a cell wide in it, over ground at five and taller than a node over the
/// square, is met where a walker's ray down it met it while the square was
/// held from the start, and no cell of the square stands outside the bound
/// read over it at any zoom over a square.
#[test]
fn a_square_landing_after_the_fold_is_read_with_its_spike() {
    use crate::domain::layer::terrain::window::{on_landed, refold, test::unland, Windowed};
    use pxl_render::gpu::renderer::Landing;
    use pxl_spec_core::window::{self as cells, Found, Squares, LEVELS, SQUARE, WINDOW};
    use pxl_spec_core::LEAF_CELL;
    let _g = serial();
    let leaf = top().children()[0];
    let mut heights = vec![5; CELLS];
    heights[200 * SPAN + 300] = 400;
    let spiked = drawn(
        heights,
        Ground {
            cells: vec![ground::land; SPAN * SPAN].into(),
        },
        Default::default(),
    );
    let (x0, y0, _) = leaf.rect();
    let tile_cache = resident([x0 + 300.0, y0 + 200.0], &[(leaf, spiked)]);
    let store = Store::default();
    // The spike's column in the eye's leaves' window.
    let (first, _) = Windowed.origin(LEAF_CELL);
    let top = |g: [i32; 2]| cells::column_top(cells::cell(&Windowed, Found::NONE, LEAF_CELL, g).1);
    let spike = (0..WINDOW as i32 * WINDOW as i32)
        .map(|k| {
            let w = WINDOW as i32;
            [
                first[0] * SQUARE as i32 + k % w,
                first[1] * SQUARE as i32 + k / w,
            ]
        })
        .find(|&g| top(g) == 400)
        .expect("the spike held");
    let side = units(GROUND.cell());
    let at = [
        (f64::from(spike[0]) + 0.5) * side,
        (f64::from(spike[1]) + 0.5) * side,
    ];
    let met = |access: &World| {
        access
            .first_hit(&down(at), at_zoom(LEAF_ZOOM))
            .value()
            .flatten()
            .unwrap()
            .point[2]
    };
    let held = met(&World::new(&tile_cache, &store));
    let (b, ticket) = unland(LEAF_CELL, spike);
    // As the stage lands a read-back: each landing, then one fold.
    on_landed(b, ticket, Landing::Built(&[0]));
    refold();
    assert_eq!(met(&World::new(&tile_cache, &store)), held, "the spike met");
    let q = cells::square_of(spike).0;
    let mut outside = 0;
    for k in 0..SQUARE * SQUARE {
        let g = [
            q[0] * SQUARE as i32 + (k % SQUARE) as i32,
            q[1] * SQUARE as i32 + (k / SQUARE) as i32,
        ];
        let h = top(g);
        for zoom in TOP_ZOOM..LEAF_CELL - LEVELS {
            let s = LEAF_CELL - zoom;
            let (b, _, _) = cells::bound_over(
                &Windowed,
                &mut Found::NONE.clone(),
                [LEAF_CELL, LEAF_CELL],
                (zoom, [g[0] >> s, g[1] >> s]),
            );
            outside += u32::from(h < b[0][0] || h > b[0][1]);
        }
    }
    assert_eq!(outside, 0, "cells outside the bound over them");
}

/// The device that went took every square it held: the turn that sees it
/// gives up their cells (domain/layer/terrain/window.rs `berth`, as `step`
/// runs it) and folds the windows over none, so no node over a square of
/// the eye's leaves keeps a bound of cells gone.
#[test]
fn a_device_gone_leaves_no_bound_over_the_squares_it_held() {
    use crate::domain::layer::terrain::window::{test::lose, Windowed};
    use pxl_spec_core::ground::UNBOUNDED;
    use pxl_spec_core::window::{self as cells, Found, Squares, LEVELS, SQUARE};
    use pxl_spec_core::LEAF_CELL;
    let _g = serial();
    let _held = flat_at_five();
    let (first, _) = Windowed.origin(LEAF_CELL);
    let zoom = LEAF_CELL - LEVELS - 1;
    let over = || {
        let s = LEAF_CELL - zoom;
        let g = [first[0] * SQUARE as i32 >> s, first[1] * SQUARE as i32 >> s];
        let mut place = Found::NONE;
        cells::bound_over(&Windowed, &mut place, [LEAF_CELL, LEAF_CELL], (zoom, g)).0
    };
    assert_ne!(over(), [UNBOUNDED; 2], "held");
    lose();
    assert_eq!(over(), [UNBOUNDED; 2], "over cells gone");
}

/// What the ground grows stands where it grew whatever is put over it or in
/// place of the cell under it, air or a cube: it grows on the world
/// unplaced (pxl-spec-core `scatter::grown`).
#[test]
fn a_grown_decor_stands_where_it_grew_whatever_is_put_over_or_under_it() {
    use crate::domain::layer::object::store::AIR;
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let near = |store: &Store| {
        let world = World::new(&tile_cache, store);
        let space = pxl_spec::space::coord::aabb::Aabb {
            min: [x - 8.0, y - 8.0, world::REACH.min[2]],
            max: [x + 8.0, y + 8.0, world::REACH.max[2]],
        };
        world.grown_over(&space)
    };
    let bare = near(&Store::default());
    let tree = *bare.first().expect("a decor grows about the eye");
    let [cx, cy, cz] = tree.grown_in().unwrap();
    let column = |dz: i32| [f64::from(cx), f64::from(cy), f64::from(cz + dz)];

    let mut store = Store::default();
    let red = cube_id(Cube::Object(cube::Object::red));
    write(&mut store, column(6), OBJECT_COARSEST, red);
    assert_eq!(near(&store), bare, "a cube over it moves nothing");

    for (under, what) in [(AIR, "a carve"), (red, "a cube")] {
        let mut store = Store::default();
        write(&mut store, column(-1), OBJECT_COARSEST, under);
        assert_eq!(near(&store), bare, "{what} under it moves nothing");
    }
}

/// Only an edit in its own box takes what the ground grows away, air or a
/// cube; one under it or over it leaves it growing (pxl-spec-core
/// `scatter::Grounds::is_edited`).
#[test]
fn a_grown_decor_goes_only_where_an_edit_holds_its_box() {
    use crate::domain::layer::object::store::AIR;
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let empty = Store::default();
    let world = World::new(&tile_cache, &empty);
    let space = pxl_spec::space::coord::aabb::Aabb {
        min: [x - 8.0, y - 8.0, world::REACH.min[2]],
        max: [x + 8.0, y + 8.0, world::REACH.max[2]],
    };
    let tree = *world
        .grown_over(&space)
        .first()
        .expect("a decor grows about the eye");
    let [cx, cy, cz] = tree.grown_in().unwrap();
    let decor = pxl_spec::asset::decor::Decor::of(pxl_spec::asset::DecorId::of(tree.id).unwrap());
    let grows = |store: &Store| {
        let world = World::new(&tile_cache, store);
        super::scatter::grown_at(
            &mut super::scatter::Asked::new(&world),
            decor,
            [cx, cy],
            true,
        )
        .map(|(_, grows)| grows)
    };
    assert_eq!(
        grows(&Store::default()),
        Some(true),
        "it grows on bare ground"
    );
    let red = cube_id(Cube::Object(cube::Object::red));
    for (dz, value, what, kept) in [
        (0, AIR, "air in its box", false),
        (0, red, "a cube in its box", false),
        (-1, AIR, "air under it", true),
        (-1, red, "a cube under it", true),
        (6, red, "a cube over it", true),
    ] {
        let mut store = Store::default();
        let at = [f64::from(cx), f64::from(cy), f64::from(cz + dz)];
        write(&mut store, at, OBJECT_COARSEST, value);
        assert_eq!(grows(&store), Some(kept), "{what}");
    }
}
