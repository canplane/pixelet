// pxl-core/domain/query.test.rs
//
// The query's laws, checked over a leaf tile's four children folded into
// their parent as the bake folds them, and over objects written into a store.

use pxl_spec::asset::cube::{self, ground, sentinel};
use pxl_spec::asset::AssetId;
use pxl_spec::bake::constant::{OBJECT_COARSEST, OBJECT_FINEST};
use pxl_spec::bake::tile::terrain::Ground;
use pxl_spec::space::layer::lattice::{Cells, GROUND, TILE_CELLS};

use super::*;
use crate::domain::layer::instance::Instance;
use crate::domain::layer::Answer;
use pxl_spec::bake::tile::terrain::{GroundId, TerrainTile, GROUND_COLUMN};
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;
use pxl_spec::space::layer::quadrant::{Quadrant, LEAF_ZOOM, TOP_ZOOM};

use crate::domain::layer::object::store::Store;
use crate::domain::layer::tile_cache::TileCache;
use pxl_spec::space::coord::zoom::{idx_of, units, Zoom};

use crate::domain::layer::object::store::ObjectQuery as _;
use crate::domain::layer::object::store::Write;
use crate::domain::layer::terrain::clipmap::test;
use crate::sys::serial;
use pxl_engine::area::{self, Area};
use pxl_engine::world::placed_in;
use pxl_spec::asset::decor::Decor;
use pxl_spec::bake::separation::Separation;
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::hierarchy::morton;

const CELLS: usize = GROUND_COLUMN;
const SPAN: usize = TILE_CELLS as usize;

fn top() -> Quadrant {
    Quadrant::new(LEAF_ZOOM - 1, [40, -20])
}

/// A leaf as a client holds it: its tile with its lines, and each cell's
/// palette index and height as the clip levels hold them once a GPU drew
/// them.
#[derive(Clone)]
struct Leaf {
    tile: TerrainTile,
    palette_indices: Vec<GroundId>,
    heights: Vec<i16>,
}

/// A tile cache holding `held` and, over each tile, its line up to the
/// coarsest zoom the source supplies whole, as the floor holds it all around
/// the eye (`domain/layer/tile_cache/floor.rs`). Here a tile's coarser
/// pictures are the same ground, each marking its children held. Their cells
/// are held in the clip levels around `center`, in units.
fn resident(center: [f64; 2], held: &[(Quadrant, Leaf)]) -> TileCache<TerrainTile> {
    use pxl_spec::bake::tile::Occupancy;
    let mut tile_cache = TileCache::default();
    let mut admitted: Vec<Quadrant> = Vec::new();
    let mut cells = Vec::new();
    for (
        p,
        Leaf {
            tile,
            palette_indices,
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
            cells.push((
                q,
                palette_indices.clone(),
                heights.clone(),
                tile.separation.clone(),
            ));
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
    // Their cells are handed back at once, as a GPU would hand them back.
    test::hold(center, &cells);
    tile_cache
}

/// A leaf as a client holds it once its cells are back, with each cell's
/// height and palette index given.
fn drawn(heights: Vec<i16>, ground: Ground, separation: Separation) -> Leaf {
    Leaf {
        tile: TerrainTile::of(
            Vec::new(),
            Ground {
                cells: Vec::new().into(),
            },
            separation,
            Occupancy::LEAF,
        ),
        palette_indices: ground.cells.into_owned(),
        heights,
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

pub(crate) fn write(store: &mut Store, corner: V3, side: Zoom, state: u32) {
    let at = world::point(corner[0], corner[1], corner[2]);
    store.write([Write {
        at: morton::key_of::<3>(side, idx_of(side, &at)),
        id: state,
        ext: None,
    }]);
}

#[test]
fn no_surface_placed_stands() {
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
    assert_eq!(
        access
            .first_hit(&down(at), at_zoom(LEAF_ZOOM))
            .val()
            .flatten(),
        None
    );

    let mut store = Store::default();
    let red = cube::Object::red as AssetId;
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
        .val()
        .flatten()
        .unwrap();
    assert!(matches!(hit.kind, HitKind::Object(_)), "{hit:?}");
}

/// A deck stands on its own, not on the ground under it: over a void cell
/// the query meets its slab where the renderer draws it.
#[test]
fn deck_over_void_met() {
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
        .val()
        .flatten()
        .unwrap();
    assert_eq!(hit.point[2], 10.0, "{hit:?}");
}

/// A ray whose frame runs on past the antimeridian meets the ground and the
/// objects there, which are stored under the world's canonical positions.
#[test]
fn ray_across_antimeridian() {
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
    let tile = |x: i32| Quadrant::new(LEAF_ZOOM, [x, 0]);
    let seam = world::ROUND / 2.0;
    let tile_cache = resident(
        [seam, 100.5],
        &[(tile(n / 2 - 1), flat()), (tile(n / 2), flat())],
    );
    let top = World::new(&tile_cache, &Store::default())
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
    let red = cube::Object::red as AssetId;
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
        dir: [1.0, 0.0, 0.0],
        t_max: 200.0,
    };
    let hit = access
        .first_hit(&along, at_zoom(LEAF_ZOOM))
        .exact()
        .flatten()
        .unwrap();
    assert!(matches!(hit.kind, HitKind::Object(_)), "{hit:?}");
    assert_eq!(hit.point[0], seam + 40.0, "{hit:?}");
}

pub(crate) fn flat_at_five() -> (TileCache<TerrainTile>, [f64; 2]) {
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
fn ray_from_above_meets_ground() {
    let _g = serial();
    let (tile_cache, at) = flat_at_five();
    let store = Store::default();
    let access = World::new(&tile_cache, &store);
    let top = access
        .first_hit(&down(at), at_zoom(LEAF_ZOOM))
        .val()
        .flatten()
        .unwrap()
        .point[2];
    let high = 2.0 * world::BOUNDS.max[2];
    let over = Ray {
        origin: [at[0], at[1], high],
        dir: [0.0, 0.0, -1.0],
        t_max: 2.0 * high,
    };
    let met = access.first_hit(&over, at_zoom(LEAF_ZOOM));
    assert!(matches!(met, Answer::Exact(_)), "{met:?}");
    let hit = met.val().flatten().unwrap();
    assert_eq!([hit.point[2], hit.t], [top, high - top], "{hit:?}");
}

/// A dig is met as the cells asked fold it, and nothing finer. At the
/// leaves' cells, a cell dug in part keeps the eighths left as one box of
/// their volume about the cell's center, whose top a ray meets
/// (`answer::side`). A cell dug whole is air, and the ground under it is
/// met. A walker reads the cell as columns at the finest zoom placed in it
/// and stands on what is left as it is held, such as the half cubes over a
/// dug lower half.
#[test]
fn dig_met_as_folded() {
    use crate::domain::layer::object::store::AIR;
    use pxl_engine::answer::{side, EIGHTHS};
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
        // The top is checked as `cast::top` gives it: a share of the cell,
        // in f32.
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

/// What is held is met as finely as it is held. A half cube on the ground,
/// finer than the leaves' cells, is met whole by a pick, which asks at the
/// zoom the cube is placed at.
#[test]
fn half_cube_met() {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let half = units(OBJECT_COARSEST + 1);
    let mut store = Store::default();
    let red = cube::Object::red as AssetId;
    write(&mut store, [x, y, 5.0], OBJECT_COARSEST + 1, red);
    let access = World::new(&tile_cache, &store);
    let at = [x + half / 2.0, y + half / 2.0];
    let hit = access
        .first_hit(&down(at), finest())
        .exact()
        .flatten()
        .unwrap();
    assert!(matches!(hit.kind, HitKind::Object(_)), "{hit:?}");
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
fn walker_stands_any_tier() {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let red = cube::Object::red as AssetId;
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
fn stack_gap_floored() {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let mut store = Store::default();
    let red = cube::Object::red as AssetId;
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

/// A cell coarser than what is placed in it is read as the cast draws it
/// there: one column, whatever is placed finer. From over it or from inside
/// the box it is drawn as, its floor is that box's top (`cast::top`), where
/// a ray at that zoom meets it.
#[test]
fn coarse_cell_floored_as_drawn() {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let mut store = Store::default();
    let red = cube::Object::red as AssetId;
    write(&mut store, [x, y, 5.0], OBJECT_COARSEST, red);
    write(&mut store, [x + 1.0, y, 5.0], OBJECT_FINEST, red);
    let access = World::new(&tile_cache, &store);
    let zoom = LEAF_ZOOM - 1;
    let at = [x + 0.5, y + 0.5];
    let drawn = access
        .first_hit(&down(at), at_zoom(zoom))
        .exact()
        .flatten()
        .unwrap()
        .cube
        .max[2];
    for z in [drawn + 3.0, drawn - 0.25] {
        assert_eq!(
            access.floor_under(&world::point(at[0], at[1], z), zoom),
            Answer::Exact(Some(drawn)),
            "from {z}"
        );
    }
}

/// Quake III's step-slide, a column at a time. When blocked, the walker
/// climbs the blocker as far as its own column is open over its head, then
/// settles. When a ceiling comes first it stops there, and a blocked axis is
/// slid along.
#[test]
fn walker_climbs_or_slides() {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let mut store = Store::default();
    let red = cube::Object::red as AssetId;
    // A wall three cubes high stands in the column east of the walker's.
    for z in [5.0, 6.0, 7.0] {
        write(&mut store, [x + 1.0, y, z], OBJECT_COARSEST, red);
    }
    let from = world::point(x + 0.5, y + 0.5, over(5.0));
    let floor = |p: Q3| q3::to_real(i64::from(p[2])) - units(GROUND.leaf_cell_zoom()) / 2.0;
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
    // A roof two cells over the walker stops the climb before it tops the
    // wall.
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

/// A height picks the floor nearest it: the ground under a deck or the deck,
/// whichever is closer. A solid with no clear cell over it is no floor.
#[test]
fn height_nearest_floor() {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let mut store = Store::default();
    let red = cube::Object::red as AssetId;
    // A deck a cube thick has its top at 10, with the ground at 5 under it.
    write(&mut store, [x, y, 9.0], OBJECT_COARSEST, red);
    let access = World::new(&tile_cache, &store);
    let near = |z: f64| access.support_nearest(&world::point(x + 0.5, y + 0.5, z));
    assert_eq!(
        near(6.0),
        Answer::Exact(Some(5.0)),
        "the ground under the deck"
    );
    assert_eq!(near(12.0), Answer::Exact(Some(10.0)), "the deck");
    // A cube right under the deck's cube has no clear cell over its top.
    write(&mut store, [x, y, 8.0], OBJECT_COARSEST, red);
    let access = World::new(&tile_cache, &store);
    assert_eq!(
        access.support_nearest(&world::point(x + 0.5, y + 0.5, 9.0)),
        Answer::Exact(Some(10.0))
    );
}

/// A reading reports the columns its queries read, joined: a support reads
/// its own cell, and a walk reads every cell it passes.
#[test]
fn reading_reports_columns() {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let store = Store::default();
    let read = changes::Footprint::default();
    let access = World::new(&tile_cache, &store).reading(&read);
    let cell = units(GROUND.leaf_cell_zoom());
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
fn box_placed_matches_store() {
    let _g = serial();
    let unit = pxl_engine::UNIT_ZOOM;
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
    let mut held = Vec::new();
    let space = Aabb {
        min: [3.0, 4.0, 2.0],
        max: [11.0, 6.0, 6.0],
    };
    store.entries_in(&space, &mut |e| {
        if e.zoom == unit {
            held.push((e.at, e.state));
        }
    });
    asked.sort_unstable();
    held.sort_unstable();
    assert_eq!(asked.len(), 3, "{asked:?}");
    assert_eq!(asked, held);
}

/// A brick handed after the clip levels were folded without it is read with
/// what it holds once a frame has handed it, since the hand folds them again
/// (domain/layer/terrain/clipmap.rs `upload`). A spike a cell wide in the
/// brick, over ground at five and taller than a node over the brick, is met
/// where a walker's ray down it met it when the brick was held from the
/// start. No cell of the brick stands outside the bound read over it at any
/// zoom coarser than a brick.
#[test]
fn late_brick_spike() {
    use crate::domain::layer::terrain::clipmap::test::{hand, unland};
    use crate::domain::layer::terrain::clipmap::CpuClipmap;
    use pxl_engine::clipmap::{self as cells, Bricks, Lookup, BRICK, BRICK_MIPS, CLIP_SIZE};
    use pxl_engine::LEAF_CELL_ZOOM;
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
    // Find the spike's column in the clip level of the eye's leaves.
    let (first, _) = CpuClipmap.origin(LEAF_CELL_ZOOM);
    let top = |g: [i32; 2]| {
        cells::column_top(cells::cell(&CpuClipmap, Lookup::NONE, LEAF_CELL_ZOOM, g).1)
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
    let met = |access: &World| {
        access
            .first_hit(&down(at), at_zoom(LEAF_ZOOM))
            .val()
            .flatten()
            .unwrap()
            .point[2]
    };
    let held = met(&World::new(&tile_cache, &store));
    unland(LEAF_CELL_ZOOM, spike);
    hand();
    assert_eq!(met(&World::new(&tile_cache, &store)), held, "the spike met");
    let q = cells::brick_of(spike).0;
    let mut outside = 0;
    for k in 0..BRICK * BRICK {
        let g = [
            q[0] * BRICK as i32 + (k % BRICK) as i32,
            q[1] * BRICK as i32 + (k / BRICK) as i32,
        ];
        let h = top(g);
        for zoom in TOP_ZOOM..LEAF_CELL_ZOOM - BRICK_MIPS {
            let s = LEAF_CELL_ZOOM - zoom;
            let (b, _, _) = cells::bound_over(
                &CpuClipmap,
                &mut Lookup::NONE.clone(),
                [LEAF_CELL_ZOOM, LEAF_CELL_ZOOM],
                (zoom, [g[0] >> s, g[1] >> s]),
            );
            outside += u32::from(h < b[0][0] || h > b[0][1]);
        }
    }
    assert_eq!(outside, 0, "cells outside the bound over them");
}

/// A lost device takes every brick it held with it. The turn that sees the
/// loss gives up their cells (domain/layer/terrain/clipmap.rs
/// `assign_slots`, as `step` runs it) and folds the clip levels over none,
/// so no node over a brick of the eye's leaves keeps a bound of cells that
/// are gone.
#[test]
fn device_loss_drops_bounds() {
    use crate::domain::layer::terrain::clipmap::{test::lose, CpuClipmap};
    use pxl_engine::clipmap::{self as cells, Bricks, Lookup, BRICK, BRICK_MIPS};
    use pxl_engine::ground::UNBOUNDED;
    use pxl_engine::LEAF_CELL_ZOOM;
    let _g = serial();
    let _held = flat_at_five();
    let (first, _) = CpuClipmap.origin(LEAF_CELL_ZOOM);
    let zoom = LEAF_CELL_ZOOM - BRICK_MIPS - 1;
    let over = || {
        let s = LEAF_CELL_ZOOM - zoom;
        let g = [
            (first[0] * BRICK as i32) >> s,
            (first[1] * BRICK as i32) >> s,
        ];
        let mut place = Lookup::NONE;
        cells::bound_over(
            &CpuClipmap,
            &mut place,
            [LEAF_CELL_ZOOM, LEAF_CELL_ZOOM],
            (zoom, g),
        )
        .0
    };
    assert_ne!(over(), [UNBOUNDED; 2], "held");
    lose();
    assert_eq!(over(), [UNBOUNDED; 2], "over cells gone");
}

/// What the ground generates stands where it was generated, whatever is put
/// over it or in place of the cell under it, air or a cube, since it
/// generates on the world unplaced (pxl-content `generation::placement`).
#[test]
fn generated_decor_stays() {
    use crate::domain::layer::object::store::AIR;
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let near = |store: &Store| {
        let world = World::new(&tile_cache, store);
        let space = Aabb {
            min: [x - 8.0, y - 8.0, world::BOUNDS.min[2]],
            max: [x + 8.0, y + 8.0, world::BOUNDS.max[2]],
        };
        world.generated_over(&space)
    };
    let bare = near(&Store::default());
    let tree = *bare.first().expect("a decor generates about the eye");
    let [cx, cy, cz] = tree.generated_in().unwrap();
    let column = |dz: i32| [f64::from(cx), f64::from(cy), f64::from(cz + dz)];

    let mut store = Store::default();
    let red = cube::Object::red as AssetId;
    write(&mut store, column(6), OBJECT_COARSEST, red);
    assert_eq!(near(&store), bare, "a cube over it moves nothing");

    for (under, what) in [(AIR, "a carve"), (red, "a cube")] {
        let mut store = Store::default();
        write(&mut store, column(-1), OBJECT_COARSEST, under);
        assert_eq!(near(&store), bare, "{what} under it moves nothing");
    }
}

/// Only an edit in its own box, air or a cube, takes what the ground
/// generates away. An edit under it or over it leaves it generated
/// (pxl-engine `generation::DecorQuery::asset`, edited).
#[test]
fn generated_decor_edit_removes() {
    use crate::domain::layer::object::store::AIR;
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let empty = Store::default();
    let world = World::new(&tile_cache, &empty);
    let space = Aabb {
        min: [x - 8.0, y - 8.0, world::BOUNDS.min[2]],
        max: [x + 8.0, y + 8.0, world::BOUNDS.max[2]],
    };
    let at = |tree: &Instance| {
        let [cx, cy, cz] = tree.generated_in().unwrap();
        (Decor::of(u32::from(tree.id)), [cx, cy, cz])
    };
    let generates_on = |store: &Store, decor: &Decor, [cx, cy, _]: [i32; 3]| {
        let world = World::new(&tile_cache, store);
        scatter::generated_at(&mut scatter::DecorWorld::new(&world), decor, [cx, cy], true)
            .map(|(_, generates)| generates)
    };
    // Pick a decor that generates on bare ground with its box whole.
    let (decor, [cx, cy, cz]) = world
        .generated_over(&space)
        .iter()
        .map(at)
        .find(|(d, c)| generates_on(&Store::default(), d, *c) == Some(true))
        .expect("a decor generates about the eye");
    let generates = |store: &Store| generates_on(store, decor, [cx, cy, cz]);
    let red = cube::Object::red as AssetId;
    for (dz, val, what, still) in [
        (0, AIR, "air in its box", false),
        (0, red, "a cube in its box", false),
        (-1, AIR, "air under it", true),
        (-1, red, "a cube under it", true),
        (6, red, "a cube over it", true),
    ] {
        let mut store = Store::default();
        let at = [f64::from(cx), f64::from(cy), f64::from(cz + dz)];
        write(&mut store, at, OBJECT_COARSEST, val);
        assert_eq!(generates(&store), Some(still), "{what}");
    }
}

/// What the ground generates is struck only for the kinds the frame generates
/// at the leaves' clip level. Where the frame generates none, as with the
/// decor setting off (domain/view/lens.rs `decor_zooms`), the pointer's ray
/// goes through where a decor would stand and reaches the ground under it
/// (`World::striking_decor`).
#[test]
fn generated_decor_struck_where_generated() {
    use pxl_engine::clipmap::FIRST_CELL_ZOOM;
    use pxl_engine::LEAF_CELL_ZOOM;
    use pxl_spec::asset::decor::DECORS;
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let empty = Store::default();
    let world = World::new(&tile_cache, &empty);
    let space = Aabb {
        min: [x - 8.0, y - 8.0, world::BOUNDS.min[2]],
        max: [x + 8.0, y + 8.0, world::BOUNDS.max[2]],
    };
    let generates = |i: &Instance| {
        let [cx, cy, _] = i.generated_in().unwrap();
        let decor = Decor::of(u32::from(i.id));
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
    let met = |zooms: [u32; DECORS.len()]| {
        world
            .striking_decor(&zooms)
            .first_hit(&ray, finest())
            .exact()
            .flatten()
            .expect("the ground under it")
            .kind
    };
    let leaves = [1 << (LEAF_CELL_ZOOM - FIRST_CELL_ZOOM); DECORS.len()];
    assert!(
        matches!(met(leaves), HitKind::Decor(i) if i.generated_in() == tree.generated_in()),
        "{:?}",
        met(leaves)
    );
    let none = [0; DECORS.len()];
    assert!(!matches!(met(none), HitKind::Decor(_)), "{:?}", met(none));
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
        use pxl_engine::SHARD_ZOOM;
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
    use pxl_engine::SHARD_ZOOM;
    let _g = serial();
    let s = LEAF_ZOOM - SHARD_ZOOM;
    // The westmost leaf of a served shard whose west neighbor is not served.
    let leaf = Quadrant::new(LEAF_ZOOM, [5 << s, 3 << s]);
    let area = EastOf(5);
    let flat = drawn(
        vec![5; CELLS],
        Ground {
            cells: vec![ground::land; SPAN * SPAN].into(),
        },
        Default::default(),
    );
    let (x0, y0, _) = leaf.rect();
    let tile_cache = resident([x0 + 10.0, y0 + 10.0], &[(leaf, flat)]);
    let store = Store::default();
    let world = World::new(&tile_cache, &store);
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
    use pxl_spec::asset::id::SHAPE_BITS;
    use pxl_spec::space::layer::octant::Octant;
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let past: AssetId = (1 << SHAPE_BITS) | ((1 << SHAPE_BITS) - 1);
    let mut store = Store::default();
    write(&mut store, [x, y, 5.0], OBJECT_COARSEST, past);
    let at = Octant::holding(OBJECT_COARSEST, &world::point(x, y, 5.0));
    let entry = World::new(&tile_cache, &store).entry_at(at).unwrap();
    assert_eq!(entry.state, past);
    assert_eq!(entry.asset(), Asset::Unknown);
    assert!(decor_instance(&entry).is_none());
}

/// The area's progress the UI shows while the ground loads is the share of
/// the tiles asked for that show.
#[test]
fn shown_share_of_what_is_asked() {
    let _g = serial();
    let (tile_cache, _) = flat_at_five();
    assert!(tile_cache.is_shown());
    assert_eq!(tile_cache.shown_share(), 1.0);
}
