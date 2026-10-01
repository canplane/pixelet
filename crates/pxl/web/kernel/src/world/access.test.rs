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

use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::bake::format::{Ground, GroundChannelKey};
use pxl_spec::bake::pack::with_channel;
use pxl_spec::cell::cube_types;
use pxl_spec::cell::object::{placeable_cube_num_id, PlaceableCube};
use pxl_spec::space::pvf3::ivec;
use pxl_spec::space::pyramid::{Folds, PAGE_CELLS};

use super::*;
use crate::global::serial;
use crate::layer::object::store::{At, Write};
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

/// A ray straight down onto `at`, from far above anything the fixture holds.
fn down(at: [f64; 2]) -> Ray {
    Ray {
        origin: [at[0], at[1], 10_000.0],
        direction: [0.0, 0.0, -1.0],
        reach: 20_000.0,
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
