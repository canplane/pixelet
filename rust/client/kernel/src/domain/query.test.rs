// pxl-kernel/domain/query.test.rs
//
// The query's laws over a leaf tile's four children folded into their
// parent as the bake folds them, and objects written into a store.

use pxl_spec::asset::cube::{self, ground, sentinel};
use pxl_spec::asset::cube::{cube_id, Cube};
use pxl_spec::bake::constant::{OBJECT_COARSEST, OBJECT_FINEST};
use pxl_spec::bake::tile::terrain::Ground;
use pxl_spec::space::layer::lattice::{GROUND, TILE_CELLS};

use super::*;
use crate::domain::layer::Answer;
use pxl_spec::bake::tile::terrain::{GroundId, TerrainTile, GROUND_LANE};
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM, TOP_ZOOM};

use crate::domain::layer::object::store::Store;
use crate::domain::layer::residency::Residency;
use pxl_spec::space::coord::zoom::{index_of, units, Zoom};

use crate::domain::layer::object::store::Write;
use crate::sys::serial;
use pxl_spec_core::answer::{In, Kind};

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
/// (`domain/layer/residency/floor.rs`): a tile's coarser pictures here are the
/// same ground, each marking its children held; their cells held in the
/// windows about `centre`, in units.
fn resident(centre: [f64; 2], held: &[(Tile, Leaf)]) -> Residency<TerrainTile> {
    resident_to(centre, held, TOP_ZOOM)
}

/// `resident`, each tile's line climbed only to `top`: past the tiles at
/// `top`, nothing is held, and the windows there lack.
fn resident_to(centre: [f64; 2], held: &[(Tile, Leaf)], top: Zoom) -> Residency<TerrainTile> {
    use pxl_spec::bake::tile::Occupancy;
    let mut residency = Residency::default();
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
        let line = std::iter::successors(Some(*p), |q| (q.zoom > top).then(|| q.parent()));
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
            residency
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
    residency.ask(&asks);
    // Their cells as a GPU would hand them back, at once.
    crate::domain::layer::terrain::window::test::hold(centre, &cells);
    residency
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
    let residency = resident(at, &[(leaf, void)]);

    let empty = Store::default();
    let access = World::new(&residency, &empty);
    assert_eq!(
        access
            .surface(&point(at[0], at[1]), LEAF_ZOOM)
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
    let access = World::new(&residency, &store);
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
    let residency = resident(at, &[(leaf, void)]);
    let store = Store::default();
    let access = World::new(&residency, &store);
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
    let residency = resident(
        [seam, 100.5],
        &[(tile(n / 2 - 1), flat()), (tile(n / 2), flat())],
    );
    let top = World::new(&residency, &Store::default())
        .surface(&point(-seam + 20.5, 100.5), LEAF_ZOOM)
        .value()
        .flatten()
        .unwrap()
        .top;

    let mut store = Store::default();
    let red = cube_id(Cube::Object(cube::Object::red));
    write(&mut store, [-seam + 40.0, 100.0, top], OBJECT_COARSEST, red);
    let access = World::new(&residency, &store);

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
    let (residency, at) = flat_at_five();
    let store = Store::default();
    let access = World::new(&residency, &store);
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
    let (residency, [x, y]) = flat_at_five();
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
        let access = World::new(&residency, &store);
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
    let (residency, [x, y]) = flat_at_five();
    let half = units(OBJECT_COARSEST + 1);
    let mut store = Store::default();
    let red = cube_id(Cube::Object(cube::Object::red));
    write(&mut store, [x, y, 5.0], OBJECT_COARSEST + 1, red);
    let access = World::new(&residency, &store);
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
    let (residency, [x, y]) = flat_at_five();
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
        let access = World::new(&residency, &store);
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
    let (residency, [x, y]) = flat_at_five();
    let mut store = Store::default();
    let red = cube_id(Cube::Object(cube::Object::red));
    for z in [5.0, 7.0] {
        write(&mut store, [x, y, z], OBJECT_COARSEST, red);
    }
    let access = World::new(&residency, &store);
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
        let to = access.step_slide(&from, &east, &from, true, true).unwrap();
        assert_eq!(
            (q3::to_real(i64::from(to[0])), floor(to)),
            (x + 1.5, 8.0),
            "climbed onto the wall"
        );
    }
    // A roof over the walker two cells up: the climb meets it first.
    write(&mut store, [x, y, 7.0], OBJECT_COARSEST, red);
    let access = World::new(&residency, &store);
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
    let (residency, [x, y]) = flat_at_five();
    let mut store = Store::default();
    let red = cube_id(Cube::Object(cube::Object::red));
    // A deck a cube thick, its top at 10, with the ground at 5 under it.
    write(&mut store, [x, y, 9.0], OBJECT_COARSEST, red);
    let access = World::new(&residency, &store);
    let near = |z: f64| access.support_nearest(&world::point(x + 0.5, y + 0.5, z));
    assert_eq!(
        near(6.0),
        Answer::Exact(Some(5.0)),
        "the ground under the deck"
    );
    assert_eq!(near(12.0), Answer::Exact(Some(10.0)), "the deck");
    // A cube right under the deck's: its top has no clear cell over it.
    write(&mut store, [x, y, 8.0], OBJECT_COARSEST, red);
    let access = World::new(&residency, &store);
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
    let (residency, [x, y]) = flat_at_five();
    let store = Store::default();
    let read = changed::Columns::default();
    let access = World::new(&residency, &store).reading(&read);
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

/// Ground whose height at `[x, y]` units is `flat`, a slope of `ramp` along
/// X from the leaf's first cell, and hills `hill` high `wave` units apart,
/// each zoom's cell its mean over the cell (`Relief::mean`), so the coarser
/// cells are the finer folded, as a bake folds them.
#[derive(Clone, Copy)]
struct Relief {
    flat: f64,
    ramp: f64,
    hill: f64,
    wave: f64,
}

impl Relief {
    /// The mean height over a cell `w` units wide about `[x, y]`, `x0` the
    /// leaf's first: a sine's mean over a cell is its middle's scaled by
    /// `sin(a) / a` at half the cell's phase.
    fn mean(&self, x0: f64, [x, y]: [f64; 2], w: f64) -> f64 {
        let k = self.wave / std::f64::consts::TAU;
        let a = w / (2.0 * k);
        let s = |v: f64| libm::sin(v / k) * libm::sin(a) / a;
        self.flat + self.ramp * (x - x0) + self.hill * s(x) * s(y)
    }
}

/// `relief`'s ground under the leaf and every tile over it to `top`, each
/// its own cells, the ways `ways` over the leaf alone, held about `centre`.
fn relieved(
    leaf: Tile,
    relief: Relief,
    ways: pxl_spec::bake::separation::Separation,
    centre: [f64; 2],
    top: Zoom,
) -> Residency<TerrainTile> {
    use pxl_spec::bake::tile::Occupancy;
    let x0 = leaf.rect().0;
    let line: Vec<Tile> =
        std::iter::successors(Some(leaf), |q| (q.zoom > top).then(|| q.parent())).collect();
    let held: Vec<(Tile, Leaf)> = line
        .iter()
        .rev()
        .map(|&q| {
            let (qx, qy, side) = q.rect();
            let w = side / SPAN as f64;
            let heights = (0..CELLS)
                .map(|i| {
                    let at = [
                        qx + ((i % SPAN) as f64 + 0.5) * w,
                        qy + ((i / SPAN) as f64 + 0.5) * w,
                    ];
                    libm::rint(relief.mean(x0, at, w)) as i16
                })
                .collect();
            let (separation, occupancy) = if q == leaf {
                (ways.clone(), Occupancy::LEAF)
            } else {
                (Default::default(), Occupancy::of_slots([true; 4]))
            };
            let tile = TerrainTile::of(
                Vec::new(),
                Ground {
                    cells: Vec::new().into(),
                },
                separation,
                occupancy,
            );
            let looks = vec![ground::land; SPAN * SPAN];
            (
                q,
                Leaf {
                    tile,
                    looks,
                    heights,
                },
            )
        })
        .collect();
    resident_to(centre, &held, top)
}

/// The world as the cast asks it, with nothing the CPU alone strikes: what
/// a GPU's cast meets (pxl-shader world.rs `World`).
struct Asking<'w, 'a> {
    world: &'w World<'a>,
    lines: crate::domain::layer::residency::Lines,
}

impl Asking<'_, '_> {
    fn answer(&mut self, at: &mut super::node::At, asked: [Zoom; 2]) -> In<[u32; 2]> {
        match self.world.node(at, asked, &mut self.lines) {
            Answer::Nothing(_) => In {
                kind: Kind::Lacks,
                block: pxl_spec_core::answer::AIR,
            },
            Answer::Coarser(i, _) | Answer::Exact(i) => i,
        }
    }
}

impl pxl_spec_core::cast::Query for Asking<'_, '_> {
    type At = super::node::At;
    fn root(&mut self, node: [i32; 3]) -> Self::At {
        self.world.start((0, node))
    }
    fn child(&mut self, at: &Self::At, _: Zoom, node: [i32; 3]) -> Self::At {
        let k = node[0] & 1 | (node[1] & 1) << 1 | (node[2] & 1) << 2;
        self.world.child(at, k as usize)
    }
    fn node(
        &mut self,
        at: &mut Self::At,
        _: Zoom,
        _: [i32; 3],
        asked: [Zoom; 2],
        _: f32,
    ) -> In<[u32; 2]> {
        self.answer(at, asked)
    }
}

/// Where `ray` from the origin enters the box `p`, in units, no nearer
/// than the origin and short of its reach; none where it passes it.
fn enters(ray: &pxl_spec_core::cast::Ray, p: &pxl_spec_core::proxy::Proxy) -> Option<f64> {
    let (mut t0, mut t1) = (0.0f64, f64::from(ray.reach));
    for a in 0..3 {
        let (d, lo, hi) = (
            f64::from(ray.direction[a]),
            f64::from(p.lo[a]),
            f64::from(p.hi[a]),
        );
        if d == 0.0 {
            if !(lo <= 0.0 && 0.0 < hi) {
                return None;
            }
            continue;
        }
        let (a, b) = (lo / d, hi / d);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
    }
    (t1 > t0).then_some(t0)
}

/// Where `ray` from the origin, inside the box `p`, leaves it short of its
/// reach; none where it reaches its reach first.
fn leaves(ray: &pxl_spec_core::cast::Ray, p: &pxl_spec_core::proxy::Proxy) -> Option<f64> {
    let mut t1 = f64::from(ray.reach);
    for a in 0..3 {
        let d = f64::from(ray.direction[a]);
        if d != 0.0 {
            let (lo, hi) = (f64::from(p.lo[a]), f64::from(p.hi[a]));
            t1 = t1.min((lo / d).max(hi / d));
        }
    }
    (t1 < f64::from(ray.reach)).then_some(t1)
}

/// The nearest a ray from `o` enters any box of the proxy's under the node
/// `n` (pxl-spec-core `proxy`), nearer than `best`, each node asked through
/// the world's one query with nothing handed down, as a pass building the
/// boxes would ask it; and the nodes asked.
#[allow(clippy::too_many_arguments)]
fn front(
    q: &mut Asking,
    o: [i32; 3],
    ray: &pxl_spec_core::cast::Ray,
    detail: pxl_spec_core::cast::Detail,
    coarser: Zoom,
    n: pxl_spec_core::cast::Node,
    best: &mut f64,
    asked: &mut u32,
) {
    use pxl_spec_core::proxy::{self, BOX, UNDER};
    let (zooms, node, cut) = proxy::asked(o, detail, coarser, &n);
    if enters(ray, &node).is_none_or(|t| t >= *best) {
        return;
    }
    *asked += 1;
    let mut at = q.world.start((n.zoom, n.index(o)));
    let i = q.answer(&mut at, zooms);
    let p = proxy::of(o, &n, node, cut, i, floored(q, o, detail, &n, &i));
    match p.says {
        BOX => {
            if let Some(t) = enters(ray, &p) {
                *best = best.min(t);
            }
        }
        UNDER if enters(ray, &p).is_some() => {
            // As the GPU descends it: as many zooms at once as no box lies
            // between (`proxy::steps`).
            let k = proxy::steps(o, detail, coarser, &n);
            let mut children: Vec<(f64, pxl_spec_core::cast::Node)> = (0..1 << (3 * k))
                .map(|d| proxy::descendant(&n, k, d))
                .filter_map(|c| {
                    let (_, b, _) = proxy::asked(o, detail, coarser, &c);
                    enters(ray, &b).map(|t| (t, c))
                })
                .collect();
            children.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, c) in children {
                front(q, o, ray, detail, coarser, c, best, asked);
            }
        }
        _ => {}
    }
}

/// The boxes the proxy draws under the node `n` whose nearest point lies
/// within `reach` of the origin, counted into `boxes`: what a pass would
/// instance, the frustum aside; and of them, those whose top is their
/// slab's, about one a column.
fn drawn_boxes(
    q: &mut Asking,
    o: [i32; 3],
    detail: pxl_spec_core::cast::Detail,
    coarser: Zoom,
    n: pxl_spec_core::cast::Node,
    reach: f32,
    boxes: &mut [u32; 2],
) {
    use pxl_spec_core::proxy::{self, BOX, UNDER};
    let (zooms, node, cut) = proxy::asked(o, detail, coarser, &n);
    let near = (0..3)
        .map(|a| {
            let v = node.lo[a].max(0.0).min(node.hi[a]);
            v * v
        })
        .sum::<f32>();
    if near > reach * reach {
        return;
    }
    let mut at = q.world.start((n.zoom, n.index(o)));
    let i = q.answer(&mut at, zooms);
    let p = proxy::of(o, &n, node, cut, i, floored(q, o, detail, &n, &i));
    match p.says {
        BOX => {
            boxes[0] += 1;
            boxes[1] += u32::from(p.hi[2] < node.hi[2]);
        }
        UNDER => {
            let k = proxy::steps(o, detail, coarser, &n);
            for d in 0..1 << (3 * k) {
                let c = proxy::descendant(&n, k, d);
                drawn_boxes(q, o, detail, coarser, c, reach, boxes);
            }
        }
        _ => {}
    }
}

/// The floor of the node `n` where it is a split (`proxy::floor`), off the
/// ground as a node's reading holds it; none for any other.
fn floored(
    q: &Asking,
    o: [i32; 3],
    detail: pxl_spec_core::cast::Detail,
    n: &pxl_spec_core::cast::Node,
    i: &In<[u32; 2]>,
) -> i32 {
    if i.kind != Kind::Split {
        return pxl_spec_core::ground::UNBOUNDED[0];
    }
    let mut lines = Default::default();
    let mut held = super::node::Reading::new(q.world, &mut lines);
    pxl_spec_core::proxy::floor(&mut held, o, detail, n)
}

/// A sorted sample's share `p` (0 to 1), none of none.
fn quantile(v: &[f64], p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v[((v.len() - 1) as f64 * p) as usize]
}

/// The proxy's boxes are conservative: over flat, rolling and steep ground,
/// with a bridge, a tunnel and cubes placed, the cut built from the seeds
/// over the eye's coarsest window (`proxy::seed`), every eye ray enters a
/// box, or leaves that window (`proxy::coverage`), no later than the cast
/// stops (`cast::first`, the GPU's own descent, its world without decor); a
/// ray that does neither meets nothing; and a walk started there stops
/// where the walk from the eye does. How loose the boxes are is printed, by
/// how many zooms coarser than the camera grid's they are drawn
/// (`proxy::cut`): `cargo test-host -- proxy --nocapture`.
#[test]
fn a_proxy_s_box_is_entered_no_later_than_the_cast_stops() {
    use crate::domain::layer::object::store::AIR;
    use crate::domain::layer::terrain::window::Windowed;
    use pxl_spec::bake::separation::{Builder, Form, POINT_SCALE};
    use pxl_spec_core::cast::{self, Detail, Ends, Node, Rooted, Until};
    use pxl_spec_core::proxy;
    let _g = serial();
    let leaf = top().children()[0];
    let (x0, y0, side) = leaf.rect();
    let centre = [x0 + side / 2.0, y0 + side / 2.0];
    let ways = |deck: i16| {
        let mut b = Builder::default();
        b.push(
            [([180.0, 250.5], deck, 0), ([330.0, 250.5], deck, 0)],
            4 * POINT_SCALE as u8,
            ground::residential_road,
            Form::Bridge,
            0,
        );
        b.push(
            [
                ([256.5, 120.0], deck - 30, 0),
                ([256.5, 220.0], deck - 30, 0),
            ],
            4 * POINT_SCALE as u8,
            ground::residential_road,
            Form::Tunnel,
            6,
        );
        b.finish()
    };
    let grounds = [
        (
            "flat",
            Relief {
                flat: 5.0,
                ramp: 0.0,
                hill: 0.0,
                wave: 64.0,
            },
            20,
            TOP_ZOOM,
        ),
        (
            "rolling",
            Relief {
                flat: 20.0,
                ramp: 0.05,
                hill: 8.0,
                wave: 96.0,
            },
            40,
            TOP_ZOOM,
        ),
        (
            "steep",
            Relief {
                flat: 60.0,
                ramp: 0.3,
                hill: 40.0,
                wave: 48.0,
            },
            140,
            TOP_ZOOM,
        ),
        // Held only to z12's tiles: past their windows' reach, the lack.
        (
            "steep, z12 up",
            Relief {
                flat: 60.0,
                ramp: 0.3,
                hill: 40.0,
                wave: 48.0,
            },
            140,
            12,
        ),
    ];
    let detail = Detail {
        tau: 1.0,
        range: asked(),
        eye: [0.0; 3],
    };
    let red = cube_id(Cube::Object(cube::Object::red));
    let mut rng = 0x9e37_79b9_7f4a_7c15u64;
    let mut random = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng >> 11) as f64 / (1u64 << 53) as f64
    };
    const RAYS: usize = 200;
    const COARSER: [Zoom; 3] = [1, 2, 3];
    for (name, relief, deck, held) in grounds {
        let residency = relieved(leaf, relief, ways(deck), centre, held);
        let mut store = Store::default();
        let face = relief.mean(x0, centre, 1.0);
        // A tower and a half cube by the eye.
        for z in 0..6 {
            write(
                &mut store,
                [
                    centre[0] + 12.0,
                    centre[1] + 3.0,
                    libm::floor(face) + f64::from(z),
                ],
                OBJECT_COARSEST,
                red,
            );
        }
        write(
            &mut store,
            [centre[0] - 7.0, centre[1] - 9.0, libm::floor(face) + 1.0],
            OBJECT_COARSEST + 1,
            red,
        );
        // A shaft dug 24 deep, three cells square: a ray down it meets ground
        // under every face about it.
        let shaft = [centre[0] - 20.0, centre[1] + 15.0];
        let mouth = libm::floor(relief.mean(x0, shaft, 1.0));
        for k in 0..9 * 24 {
            let at = [
                shaft[0] + f64::from(k % 3),
                shaft[1] + f64::from(k / 3 % 3),
                mouth - 24.0 + f64::from(k / 9),
            ];
            write(&mut store, at, OBJECT_COARSEST, AIR);
        }
        let world = World::new(&residency, &store);
        // An eye down the shaft is not in the open: it has no proxy.
        let down_the_shaft = world::origin(shaft[0] + 1.5, shaft[1] + 1.5, mouth - 20.0);
        assert!(!proxy::aired(&Windowed, down_the_shaft), "{name}");
        let mut q = Asking {
            world: &world,
            lines: Default::default(),
        };
        for over in [2.0, 20.0, 200.0] {
            // Over its own leaf cell's face, as its column holds it.
            let own = libm::rint(relief.mean(x0, centre.map(|v| libm::floor(v) + 0.5), 1.0));
            let eye = [centre[0], centre[1], own + over];
            let o = world::origin(eye[0], eye[1], eye[2]);
            assert!(proxy::aired(&Windowed, o), "{name} +{over}");
            let (first, laid) = proxy::covered(&Windowed);
            assert!(laid, "{name} +{over}: the eye's coarsest window laid out");
            let cover = proxy::coverage(o, first);
            let mut gaps: Vec<Vec<(f64, f64, f64)>> = vec![Vec::new(); COARSER.len()];
            let (mut stops, mut lacks, mut sky) = (0, 0, 0);
            let mut visits = [0u64; 2];
            let mut empty = [0u32; COARSER.len()];
            for _ in 0..RAYS {
                let az = random() * std::f64::consts::TAU;
                let el = -1.3 + 1.35 * random();
                let d = [
                    (libm::cos(el) * libm::cos(az)) as f32,
                    (libm::cos(el) * libm::sin(az)) as f32,
                    libm::sin(el) as f32,
                ];
                let ray = cast::Ray::new(d, 3000.0);
                let met = cast::first(&mut q, o, &ray, detail, Until::Fills);
                match met.ends {
                    Ends::Stop => stops += 1,
                    Ends::Lacks => lacks += 1,
                    Ends::On => sky += 1,
                }
                for (k, &coarser) in COARSER.iter().enumerate() {
                    let mut best = f64::INFINITY;
                    let mut asked = 0;
                    for k in 0..proxy::SEEDS {
                        let seed = proxy::seed(o, first, k);
                        front(
                            &mut q, o, &ray, detail, coarser, seed, &mut best, &mut asked,
                        );
                    }
                    // Past the window the cast lacks: a ray starts no later
                    // than it leaves it.
                    if let Some(t) = leaves(&ray, &cover) {
                        best = best.min(t);
                    }
                    if best.is_infinite() {
                        assert_eq!(
                            met.ends,
                            Ends::On,
                            "{name} +{over}, {coarser} coarser, {d:?}: no box on a ray that stops"
                        );
                    }
                    if met.ends == Ends::On {
                        // A box over nothing: a pixel the ray walks from it to its reach.
                        empty[k] += u32::from(best.is_finite());
                        continue;
                    }
                    let stop = f64::from(met.stop.t);
                    assert!(
                        best <= stop + 1e-4 * stop.max(1.0),
                        "{name} +{over}, {coarser} coarser, {d:?}: the box at {best} past the stop at {stop}, {:?}",
                        met.stop.node
                    );
                    if met.ends != Ends::Stop {
                        continue;
                    }
                    // Started at the box's front, from the root, as a pixel would.
                    let at = cast::Query::root(&mut q, Node::ROOT.index(o));
                    let from = cast::walk(
                        &mut q,
                        o,
                        &ray,
                        detail,
                        Until::Fills,
                        (Node::ROOT, at, best as f32),
                        &Rooted(at),
                        &mut (),
                    );
                    assert_eq!(
                        (from.ends, from.stop.node),
                        (met.ends, met.stop.node),
                        "{name} +{over}, {coarser} coarser, {d:?}: a walk from the box stops elsewhere"
                    );
                    if k == 0 {
                        visits[0] += u64::from(met.visits);
                    }
                    if coarser == 2 {
                        visits[1] += u64::from(from.visits);
                    }
                    let block = units(met.stop.node.zoom);
                    gaps[k].push((stop - best, (stop - best) / block, f64::from(asked)));
                }
            }
            eprintln!(
                "{name} +{over}: {stops} stop, {lacks} lack, {sky} nothing; visits from the eye {:.1}, from the box 2 coarser {:.1}",
                visits[0] as f64 / stops.max(1) as f64,
                visits[1] as f64 / stops.max(1) as f64,
            );
            if over == 20.0 {
                let mut boxes = [[0u32; 2]; COARSER.len()];
                // Three coarser alone, as a test's time allows: where the
                // windows lack, one and two count seven and fifty times as many.
                for (k, &coarser) in COARSER.iter().enumerate().skip(2) {
                    for s in 0..proxy::SEEDS {
                        let seed = proxy::seed(o, first, s);
                        drawn_boxes(&mut q, o, detail, coarser, seed, 3000.0, &mut boxes[k]);
                    }
                }
                eprintln!(
                    "  boxes within 3000 units and the slab's tops of them, 3 coarser: {:?}",
                    boxes[2]
                );
            }
            for (k, g) in gaps.iter_mut().enumerate() {
                let mut units_ = g.iter().map(|v| v.0).collect::<Vec<_>>();
                let mut blocks = g.iter().map(|v| v.1).collect::<Vec<_>>();
                units_.sort_by(f64::total_cmp);
                blocks.sort_by(f64::total_cmp);
                let asked = g.iter().map(|v| v.2).sum::<f64>() / g.len().max(1) as f64;
                eprintln!(
                    "  {} coarser: gap p50 {:.2} p90 {:.2} max {:.2} units; in the stop's blocks p50 {:.2} p90 {:.2} max {:.2}; nodes asked {asked:.0}; boxes over nothing {}",
                    COARSER[k],
                    quantile(&units_, 0.5),
                    quantile(&units_, 0.9),
                    quantile(&units_, 1.0),
                    quantile(&blocks, 0.5),
                    quantile(&blocks, 0.9),
                    quantile(&blocks, 1.0),
                    empty[k],
                );
            }
        }
    }
}
