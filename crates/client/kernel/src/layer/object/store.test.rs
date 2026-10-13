// pxl-kernel/layer/object/store.test.rs

use pxl_spec::block::cube::Object;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::zoom::{base_of, UNIT_ZOOM};
use pxl_spec::space::layer::lattice::GROUND;

use super::*;
use crate::layer::object::pyramid::Held;
use crate::layer::Query;
use pxl_spec::bake::constant::OBJECT_COARSEST;
use pxl_spec::block::object::{cube_id, Cube};
use pxl_spec::space::layer::tile::LEAF_ZOOM;
use xpute_kit::math::rng::SplitMix64;

fn corner(x: i64, y: i64, z: i64) -> Q3 {
    base_of(UNIT_ZOOM, [x, y, z])
}

#[test]
fn a_write_is_read_back_and_undone_leaves_nothing_on_either_side_of_zero() {
    let mut store = Store::default();
    for at in [[8, 16, 24], [-3, 5, -7]] {
        assert!(!store
            .write([Write {
                key: key(OBJECT_COARSEST, at),
                id: 3,
                ext: None
            }])
            .is_empty());
        let finest = at.map(|v| v << (OBJECT_FINEST - OBJECT_COARSEST));
        assert!(
            matches!(store.cursor((OBJECT_FINEST, finest), OBJECT_FINEST), Held::Leaf { leaf, .. } if leaf.id == 3),
            "a cell leaf covers its steps"
        );
        let mut met = Vec::new();
        store.each_in(OBJECT_COARSEST, at, &mut |e| met.push(*e));
        assert_eq!(
            met,
            vec![Entry {
                zoom: OBJECT_COARSEST,
                at,
                state: 3,
                ext: 0
            }]
        );
        store.write([Write {
            key: key(OBJECT_COARSEST, at),
            id: VOID,
            ext: None,
        }]);
        let mut left = 0;
        store.each(&mut |_| left += 1);
        assert_eq!(left, 0);
        assert!(
            store
                .write([Write {
                    key: key(OBJECT_COARSEST, at),
                    id: VOID,
                    ext: None,
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

/// A node from a unit to a kilometer wide, within `reach` units of the origin.
fn some_node(rng: &mut Rng, reach: f64) -> Node<3> {
    let zoom: Zoom = rng.pick(&[24, 21, 18, 16, 14]);
    let at = [0, 1, 2].map(|_| ((rng.next() % (2 * reach as u64)) as f64 - reach).div_euclid(units(zoom)) as i64);
    (zoom, at)
}

/// Whole cells from a corner, and what the objects' pyramid holds at
/// `zoom`'s cells, read through the store's query, with the corner in that
/// zoom's cells.
fn folded(cells: &[(i64, i64, i64, u32)], zoom: Zoom) -> (FnvHashMap<[i64; 3], u32>, [i64; 2]) {
    let mut store = Store::default();
    let base = [64, -64];
    store.write(cells.iter().map(|&(x, y, z, state)| Write {
        key: key(OBJECT_COARSEST, [base[0] + x, base[1] + y, z]),
        id: state,
        ext: None,
    }));
    let target = GROUND.cell_zoom(zoom);
    let corner = base.map(|v| v >> (GROUND.cell() - target));
    let per = 64 >> (GROUND.cell() - target);
    let mut out = FnvHashMap::default();
    for z in -4..8 {
        for y in corner[1]..corner[1] + per {
            for x in corner[0]..corner[0] + per {
                let held = store.cursor((target, [x, y, z]), target);
                if let Some(v) = held.value(None) {
                    out.insert([x, y, z], v);
                }
            }
        }
    }
    (out, corner)
}

fn red() -> u32 {
    cube_id(Cube::Object(Object::red))
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

fn cube_at(store: &mut Store, [x, y, z]: [i64; 3], state: u32) {
    store.write([Write::at(&corner(x, y, z), 1.0, state)]);
}

#[test]
fn turning_the_axes_turns_where_the_objects_are_and_nothing_else() {
    let mut rng = Rng(SplitMix64::new(0x2c1b_3c6d_a5b4_9f1e));
    let red = cube_id(Cube::Object(Object::red));
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
    let bound = |store: &Store, (zoom, at): Node<3>| store.bounds((zoom, at), zoom).value().flatten().map(|b| b.aabb);
    for _ in 0..500 {
        let (zoom, at) = some_node(&mut rng, 200.0);
        let given = bound(&stores[0], (zoom, at));
        assert_eq!(bound(&stores[1], (zoom, turn(at))), given.map(turn_box), "{at:?} at z{zoom}");
        assert_eq!(bound(&stores[2], (zoom, turn(turn(at)))), given.map(|b| turn_box(turn_box(b))), "{at:?} at z{zoom}");
    }
}

fn words(text: &str) -> Option<Ext> {
    Some(Ext::MarkerText(text.to_string()))
}

#[test]
fn a_leaf_keeps_its_words_and_an_erase_frees_them() {
    use pxl_spec::block::object::MARKER;
    let mut store = Store::default();
    let (zoom, at) = (OBJECT_COARSEST, [5, 7, 2]);
    store.write([Write {
        key: key(zoom, at),
        id: MARKER,
        ext: words("cafe"),
    }]);
    let e = store.placed(zoom, at).expect("placed");
    assert_eq!(store.ext(&e), Some(&Ext::MarkerText("cafe".into())));
    store.write([Write {
        key: key(zoom, at),
        id: VOID,
        ext: None,
    }]);
    assert!(store.placed(zoom, at).is_none());
    // The freed slot is the next one kept: nothing outlived its leaf.
    store.write([Write {
        key: key(zoom, at),
        id: MARKER,
        ext: words("bar"),
    }]);
    let e = store.placed(zoom, at).expect("placed again");
    assert_eq!(e.ext, 1);
    assert_eq!(store.ext(&e), Some(&Ext::MarkerText("bar".into())));
}

#[test]
fn eight_markers_filling_a_cell_stay_eight() {
    use pxl_spec::block::object::MARKER;
    let mut store = Store::default();
    let parent = [4, 4, 2];
    let writes: Vec<Write> = (0..8)
        .map(|k| Write {
            key: key(OBJECT_COARSEST + 1, child_cube(OBJECT_COARSEST, parent, k)),
            id: MARKER,
            ext: words(&format!("m{k}")),
        })
        .collect();
    store.write(writes);
    let mut met = 0;
    store.each(&mut |e| {
        assert_eq!(e.zoom, OBJECT_COARSEST + 1);
        met += 1;
    });
    assert_eq!(met, 8);
}

#[test]
fn a_shard_read_whole_meets_its_leaves() {
    let mut store = Store::default();
    let at = [5, 7, 2];
    store.write([Write {
        key: key(OBJECT_COARSEST, at),
        id: 3,
        ext: None,
    }]);
    // The node of a whole shard: its tree's root, read through the region.
    let shard = [at[0] >> (OBJECT_COARSEST - SHARD_ZOOM), at[1] >> (OBJECT_COARSEST - SHARD_ZOOM), 0];
    let mut met = 0;
    store.read((SHARD_ZOOM, shard), SHARD_ZOOM).value().expect("read").entries(&mut |_| met += 1);
    assert_eq!(met, 1);
    assert!(store.bounds::<3>((SHARD_ZOOM, shard), SHARD_ZOOM).value().flatten().is_some());
    assert!(store.holds(SHARD_ZOOM, shard));
}
