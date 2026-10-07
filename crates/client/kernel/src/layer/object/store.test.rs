// pxl-kernel/layer/object/store.test.rs

use pxl_spec::cell::cube_types::Block;
use pxl_spec::space::coord::pvf3::ivec;
use pxl_spec::space::layer::lattice::GROUND;

use super::*;
use pxl_spec::bake::constant::OBJECT_COARSEST;
use pxl_spec::cell::object::{placeable_cube_num_id, PlaceableCube};
use pxl_spec::space::layer::tile::LEAF_ZOOM;
use xpute_core::math::rng::SplitMix64;

fn corner(x: i64, y: i64, z: i64) -> Pvf3 {
    Pvf3 { i: ivec::gather(x, y, z), f: 0 }
}

#[test]
fn a_write_is_read_back_and_undone_leaves_nothing_on_either_side_of_zero() {
    let mut store = Store::default();
    for at in [[8, 16, 24], [-3, 5, -7]] {
        assert!(!store.write([Write { key: key(OBJECT_COARSEST, at), id: 3 }]).is_empty());
        assert_eq!(
            store.node(OBJECT_FINEST, first_descendant_of(at, u32::from(OBJECT_FINEST - OBJECT_COARSEST))).0.state,
            3,
            "a cell leaf covers its steps"
        );
        let mut met = Vec::new();
        store.each_in(OBJECT_COARSEST, at, &mut |e| met.push(*e));
        assert_eq!(met, vec![Entry { zoom: OBJECT_COARSEST, at, state: 3 }]);
        store.write([Write {
            key: key(OBJECT_COARSEST, at),
            id: VOID,
        }]);
        let mut left = 0;
        store.each(&mut |_| left += 1);
        assert_eq!(left, 0);
        assert!(
            store
                .write([Write {
                    key: key(OBJECT_COARSEST, at),
                    id: VOID
                }])
                .is_empty(),
            "erasing nothing changes nothing"
        );
    }
}

/// Deterministic edits on every run.
struct Rng(SplitMix64);

impl Rng {
    fn next(&mut self) -> u64 {
        u64::from(self.0.next_u32())
    }

    fn pick<T: Copy>(&mut self, of: &[T]) -> T {
        of[(self.next() % of.len() as u64) as usize]
    }
}

/// Whole cells from a corner, and what the store keeps folded at `zoom`'s
/// cells, with the corner in that zoom's cells.
fn folded(cells: &[(i64, i64, i64, u32)], zoom: Zoom) -> (HashMap<[i64; 3], u32>, [i64; 2]) {
    let mut store = Store::default();
    let base = [64, -64];
    store.write(cells.iter().map(|&(x, y, z, state)| Write {
        key: key(OBJECT_COARSEST, [base[0] + x, base[1] + y, z]),
        id: state,
    }));
    let target = GROUND.cell_zoom(zoom);
    let corner = base.map(|v| v >> (GROUND.cell() - target));
    let per = 64 >> (GROUND.cell() - target);
    let mut out = HashMap::new();
    store.fold_at(target, corner, [corner[0] + per, corner[1] + per], None, &mut out).unwrap();
    (out, corner)
}

fn red() -> u32 {
    placeable_cube_num_id(PlaceableCube::Block(Block::red))
}

#[test]
fn a_cell_folds_to_itself_on_a_leaf() {
    let (out, [x, y]) = folded(&[(3, 4, 5, red())], LEAF_ZOOM);
    assert_eq!(out.get(&[x + 3, y + 4, 5]), Some(&red()));
    assert_eq!(out.len(), 1);
}

#[test]
fn a_lone_cube_goes_at_the_first_fold_and_a_slab_of_half_stays() {
    assert!(folded(&[(0, 0, 0, red())], LEAF_ZOOM - 1).0.is_empty());
    let (out, [x, y]) = folded(&[(0, 0, 0, red()), (1, 0, 0, red()), (0, 1, 0, red()), (1, 1, 0, red())], LEAF_ZOOM - 1);
    assert_eq!(out.get(&[x, y, 0]), Some(&red()));
}

#[test]
fn a_solid_block_keeps_folding_while_it_fills_its_parent() {
    let cube: Vec<_> = (0..64).map(|k| (k % 4, k / 4 % 4, k / 16, red())).collect();
    let (out, [x, y]) = folded(&cube, LEAF_ZOOM - 2);
    assert_eq!(out.get(&[x, y, 0]), Some(&red()));
    assert_eq!(out.len(), 1);
}

#[test]
fn air_is_a_value_and_folds_like_one() {
    let (out, [x, y]) = folded(&[(0, 0, 0, AIR), (1, 0, 0, AIR), (0, 1, 0, AIR), (1, 1, 0, AIR), (0, 0, 1, AIR)], LEAF_ZOOM - 1);
    assert_eq!(out.get(&[x, y, 0]), Some(&AIR));
}

fn entry_boxes(store: &Store) -> Vec<Aabb> {
    let all = Aabb {
        min: [-1e7, -1e7, REACH.min[2]],
        max: [1e7, 1e7, REACH.max[2]],
    };
    let mut out = Vec::new();
    store.entries_in(&all, &mut |e| {
        if e.state != VOID {
            out.push(e.cube());
        }
    });
    out
}

fn cube_at(store: &mut Store, [x, y, z]: [i64; 3], state: u32) {
    store.write([Write::at(&corner(x, y, z), 1.0, state)]);
}

/// The index may give a box with only air in it, never none where an entry
/// is; checked after every edit, over boxes of every size across zero.
#[test]
fn where_the_objects_in_a_box_are_is_never_missed_and_never_past_it() {
    let mut rng = Rng(SplitMix64::new(0x5851_f42d_4c95_7f2d));
    let mut store = Store::default();
    let red = placeable_cube_num_id(PlaceableCube::Block(Block::red));
    let mut met = 0;
    for step in 0..200 {
        let at = [0, 1, 2].map(|_| (rng.next() % 400) as i64 - 200);
        cube_at(&mut store, at, rng.pick(&[red, red, AIR, VOID]));
        let boxes = entry_boxes(&store);
        for _ in 0..20 {
            let side = rng.pick(&[1.0, 7.5, 64.0, 150.0, 1000.0]);
            let min = [0, 1, 2].map(|_| (rng.next() % 500) as f64 - 250.0);
            let query = Aabb { min, max: min.map(|v| v + side) };
            let truth = boxes.iter().filter_map(|b| b.meet(query)).reduce(Aabb::join);
            let given = store.bound_in(&query);
            met += truth.is_some() as u32;
            match (truth, given) {
                (Some(t), Some(g)) => assert!(g.contains(t) && query.contains(g), "step {step}: {t:?} in {g:?} in {query:?}"),
                (Some(t), None) => panic!("step {step}: {t:?} in {query:?} missed"),
                (None, Some(g)) => assert!(query.contains(g), "step {step}: {g:?} past {query:?}"),
                (None, None) => {}
            }
        }
    }
    assert!(met > 1_000, "only {met} boxes held an entry");
}

#[test]
fn turning_the_axes_turns_where_the_objects_are_and_nothing_else() {
    let mut rng = Rng(SplitMix64::new(0x2c1b_3c6d_a5b4_9f1e));
    let red = placeable_cube_num_id(PlaceableCube::Block(Block::red));
    let turn = |[x, y, z]: [i64; 3]| [y, z, x];
    let turn_box = |b: Aabb| Aabb {
        min: [b.min[1], b.min[2], b.min[0]],
        max: [b.max[1], b.max[2], b.max[0]],
    };
    let mut stores = [Store::default(), Store::default(), Store::default()];
    for _ in 0..300 {
        let at = [0, 1, 2].map(|_| (rng.next() % 300) as i64 - 150);
        let state = rng.pick(&[red, red, AIR, VOID]);
        cube_at(&mut stores[0], at, state);
        cube_at(&mut stores[1], turn(at), state);
        cube_at(&mut stores[2], turn(turn(at)), state);
    }
    for _ in 0..500 {
        let side = rng.pick(&[3.0, 64.0, 100.0, 400.0]);
        let min = [0, 1, 2].map(|_| (rng.next() % 400) as f64 - 200.0);
        let query = Aabb { min, max: min.map(|v| v + side) };
        let given = stores[0].bound_in(&query);
        assert_eq!(stores[1].bound_in(&turn_box(query)), given.map(turn_box), "{query:?}");
        assert_eq!(stores[2].bound_in(&turn_box(turn_box(query))), given.map(|b| turn_box(turn_box(b))), "{query:?}");
    }
}
