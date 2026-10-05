// pxl-kernel/world/access.test.rs
//
// The query's laws over a leaf page's four children folded into their
// parent as the bake folds them, and objects written into a store.

use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::bake::format::Ground;
use pxl_spec::cell::cube_types::{self, ground, sentinel};
use pxl_spec::cell::object::{placeable_cube_num_id, PlaceableCube};
use pxl_spec::space::pvf3::ivec;
use pxl_spec::space::pyramid::{Folds, PAGE_CELLS};
use xpute_core::math::rng::SplitMix64;

use super::*;
use crate::layer::object::store::{At, Write};
use crate::serial;
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

struct Rng(SplitMix64);

impl Rng {
    fn below(&mut self, n: u64) -> i64 {
        (u64::from(self.0.next_u32()) % n) as i64
    }
}

/// Steps and spikes, which the interpolation overshoots most, over ground
/// part water a step lower.
fn rough_leaf(rng: &mut Rng) -> Terrain {
    let ele = (0..PADDED * PADDED).map(|_| if rng.below(6) == 0 { rng.below(300) - 40 } else { rng.below(12) } as i16).collect();
    let cells = (0..SPAN * SPAN)
        .map(|_| {
            if rng.below(4) == 0 {
                ground::water
            } else if rng.below(3) == 0 {
                ground::land
            } else {
                ground::park
            }
        })
        .collect::<Vec<_>>();
    Terrain::leaf(ele, Ground { cells: cells.into() }, Default::default())
}

fn zero_page() -> Terrain {
    let cells = vec![ground::water; SPAN * SPAN];
    Terrain::leaf(vec![0; PADDED * PADDED], Ground { cells: cells.into() }, Default::default())
}

fn top() -> Page {
    Page::new(GROUND_Z - 1, ivec::gather(40, -20, 0))
}

fn pages() -> (Vec<(Page, Terrain)>, Terrain) {
    let mut rng = Rng(SplitMix64::new(0x9e37_79b9_7f4a_7c15));
    let leaves: Vec<(Page, Terrain)> = top().children().iter().map(|&c| (c, rough_leaf(&mut rng))).collect();
    let zero = zero_page();
    let parent = Terrain::fold_page(top(), |p| Some(leaves.iter().find(|(q, _)| *q == p).map_or(&zero, |(_, t)| t))).unwrap();
    (leaves, parent)
}

fn resident(held: &[(Page, Terrain)]) -> Paging<Terrain> {
    let mut paging = Paging::default();
    for (p, page) in held {
        paging.admit(*p, &Keep, |_| Ok::<_, ()>(page.clone())).unwrap();
    }
    paging
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

fn write(store: &mut Store, corner: V3, side: Scale, state: u32) {
    let at = projection::project(corner[0], corner[1], corner[2]);
    let a = At::of_cube(&at, side);
    store.write([Write {
        col_idx: a.col_idx,
        band_idx: a.band,
        at: a.step,
        scale: side,
        state,
    }]);
}

/// Above z16 a cell the objects fold solid is met at its own box; a lone
/// cube the fold takes away is passed to the ground.
#[test]
fn a_zooms_ray_meets_the_objects_in_the_cells_the_zoom_draws_from() {
    let _g = serial();
    let (leaves, parent) = pages();
    let paging = resident(&[(top(), parent), leaves[0].clone()]);
    let (x0, y0, _) = leaves[0].0.rect();
    let z = GROUND_Z - 1;
    let cell = GROUND.side_at(z).side();
    let red = placeable_cube_num_id(PlaceableCube::Block(cube_types::Block::red));
    let (bx, by) = ((x0 / cell).floor() * cell + 4.0 * cell, (y0 / cell).floor() * cell + 4.0 * cell);
    let bare = Access::new(&paging, &Store::default()).at(&point(bx, by), z).value().unwrap().top().unwrap();
    let bz = ((bare / cell).floor() + 3.0) * cell;

    // Five of the cell's eight: the bottom four and one over them.
    let mut store = Store::default();
    for [dx, dy, dz] in [[0, 0, 0], [1, 0, 0], [0, 1, 0], [1, 1, 0], [0, 0, 1]] {
        write(&mut store, [bx + dx as f64, by + dy as f64, bz + dz as f64], CELL, red);
    }
    let (lx, ly) = (bx + 4.0 * cell, by);
    write(&mut store, [lx, ly, bz], CELL, red);
    let access = Access::new(&paging, &store);

    // Over the corner no cube's top reaches, the zoom still meets the cell's top.
    let at = [bx + 1.5, by + 1.5];
    let fine = access.first_hit(&down(at), Detail::Zoom(GROUND_Z)).value().flatten().unwrap();
    assert_eq!(fine.point[2], bz + 1.0, "{fine:?}");
    let hit = access.first_hit(&down(at), Detail::Zoom(z)).value().flatten().unwrap();
    assert!(matches!(hit.struck, Struck::Objects), "{hit:?}");
    assert_eq!(
        hit.cube,
        Aabb {
            min: [bx, by, bz],
            max: [bx + cell, by + cell, bz + cell]
        }
    );
    assert_eq!(hit.point[2], bz + cell);

    let alone = [lx + 0.5, ly + 0.5];
    let hit = access.first_hit(&down(alone), Detail::Zoom(z)).value().flatten().unwrap();
    let ground = Access::new(&paging, &Store::default()).first_hit(&down(alone), Detail::Zoom(z)).value().flatten().unwrap();
    assert_eq!((hit.struck, hit.point), (Struck::Ground, ground.point));
}

/// Past the coverage the terrain gives no surface: nothing stands and no ray
/// stops there, and what is placed there stands as anywhere.
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
    let paging = resident(&[(leaf, void)]);
    let (x0, y0, _) = leaf.rect();
    let at = [x0 + 10.5, y0 + 10.5];

    let empty = Store::default();
    let access = Access::new(&paging, &empty);
    assert_eq!(access.at(&point(at[0], at[1]), GROUND_Z).value().unwrap().top(), None);
    assert_eq!(access.surface_support(&point(at[0], at[1])), Answer::Exact(None));
    assert_eq!(access.first_hit(&down(at), Detail::Zoom(GROUND_Z)).value().flatten(), None);

    let mut store = Store::default();
    let red = placeable_cube_num_id(PlaceableCube::Block(cube_types::Block::red));
    write(&mut store, [x0 + 10.0, y0 + 10.0, 5.0], CELL, red);
    let access = Access::new(&paging, &store);
    assert_eq!(access.surface_support(&point(at[0], at[1])), Answer::Exact(Some(6.0)));
    let hit = access.first_hit(&down(at), Detail::Zoom(GROUND_Z)).value().flatten().unwrap();
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
    decks.push([([0.0, 10.5], 10), ([40.0, 10.5], 10)], 4 * POINT_SCALE as u8, ground::residential_road, Form::Deck);
    let void = Terrain::leaf(
        vec![0; PADDED * PADDED],
        Ground {
            cells: vec![sentinel::void; SPAN * SPAN].into(),
        },
        decks.finish(),
    );
    let paging = resident(&[(leaf, void)]);
    let (x0, y0, _) = leaf.rect();
    let at = [x0 + 20.5, y0 + 10.5];
    let store = Store::default();
    let access = Access::new(&paging, &store);
    assert_eq!(access.surface_support(&point(at[0], at[1])), Answer::Exact(Some(10.0)));
    let hit = access.first_hit(&down(at), Detail::Zoom(GROUND_Z)).value().flatten().unwrap();
    assert_eq!(hit.point[2], 10.0, "{hit:?}");
}
