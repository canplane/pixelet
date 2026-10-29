// pxl-kernel/domain/layer/object/store.test.rs

use pxl_spec::asset::cube::Object;
use pxl_spec::space::coord::zoom::UNIT_ZOOM;
use pxl_spec::space::layer::lattice::GROUND;

use fnv::FnvHashMap;
use pxl_spec::space::coord::zoom::SHARD_ZOOM;
use pxl_spec::space::hierarchy::morton::child_cube;

use super::*;
use crate::domain::layer::Query;
use pxl_spec::asset::cube::{cube_id, Cube};
use pxl_spec::bake::constant::OBJECT_COARSEST;
use pxl_spec::space::layer::tile::LEAF_ZOOM;
use pxl_spec_core::overwrite::{self, Cut};
use xpute_kit::math::rng::SplitMix64;

#[test]
fn write_undo_round_trip() {
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
            {
                let held = store.cursor((OBJECT_FINEST, finest), OBJECT_FINEST);
                held.holds()
                    && !held.is_folded()
                    && overwrite::val(&store.lanes(), held, Cut::NONE, OBJECT_FINEST, finest) == 3
            },
            "a cell leaf covers its steps"
        );
        let mut met = Vec::new();
        store
            .read((OBJECT_COARSEST, at))
            .entries(&mut |e| met.push(*e));
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
    let at = [0, 1, 2]
        .map(|_| ((rng.next() % (2 * reach as u64)) as f64 - reach).div_euclid(units(zoom)) as i32);
    (zoom, at)
}

/// What the objects' pyramid holds at a cell: its value and the eighths of
/// its cube it keeps (pxl-spec-core `answer::eighths`).
type Held = FnvHashMap<[i32; 3], (u32, u32)>;

/// Whole cells from a corner, and what the objects' pyramid holds at
/// `zoom`'s cells, read through the store's query, where it keeps any, and
/// the corner in that zoom's cells.
fn folded(cells: &[(i32, i32, i32, u32)], zoom: Zoom) -> (Held, [i32; 2]) {
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
                let v = overwrite::val(&store.lanes(), held, Cut::NONE, target, [x, y, z]);
                let kept = pxl_spec_core::answer::eighths(overwrite::filled(&store.lanes(), held));
                if v != VOID && kept > 0 {
                    out.insert([x, y, z], (v, kept));
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
fn leaf_cell_folds_to_itself() {
    let (out, [x, y]) = folded(&[(3, 4, 5, red())], LEAF_ZOOM);
    assert_eq!(out.get(&[x + 3, y + 4, 5]), Some(&(red(), 8)));
    assert_eq!(out.len(), 1);
}

#[test]
fn lone_cube_fold() {
    let (out, [x, y]) = folded(&[(0, 0, 0, red())], LEAF_ZOOM - 1);
    assert_eq!(out.get(&[x, y, 0]), Some(&(red(), 1)));
    assert!(folded(&[(0, 0, 0, red())], LEAF_ZOOM - 2).0.is_empty());
    let (out, [x, y]) = folded(
        &[
            (0, 0, 0, red()),
            (1, 0, 0, red()),
            (0, 1, 0, red()),
            (1, 1, 0, red()),
        ],
        LEAF_ZOOM - 1,
    );
    assert_eq!(out.get(&[x, y, 0]), Some(&(red(), 4)));
}

#[test]
fn solid_block_folds() {
    let cube: Vec<_> = (0..64).map(|k| (k % 4, k / 4 % 4, k / 16, red())).collect();
    let (out, [x, y]) = folded(&cube, LEAF_ZOOM - 2);
    assert_eq!(out.get(&[x, y, 0]), Some(&(red(), 8)));
    assert_eq!(out.len(), 1);
}

#[test]
fn air_folds_as_val() {
    let (out, [x, y]) = folded(
        &[
            (0, 0, 0, AIR),
            (1, 0, 0, AIR),
            (0, 1, 0, AIR),
            (1, 1, 0, AIR),
            (0, 0, 1, AIR),
        ],
        LEAF_ZOOM - 1,
    );
    assert_eq!(out.get(&[x, y, 0]), Some(&(AIR, 5)));
}

fn cube_at(store: &mut Store, [x, y, z]: [i32; 3], state: u32) {
    store.write([Write::at(EntryAt::new(UNIT_ZOOM, [x, y, z]), state)]);
}

#[test]
fn axis_turn() {
    let mut rng = Rng(SplitMix64::new(0x2c1b_3c6d_a5b4_9f1e));
    let red = cube_id(Cube::Object(Object::red));
    let turn = |[x, y, z]: [i32; 3]| [y, z, x];
    let turn_box = |b: Aabb| Aabb {
        min: [b.min[1], b.min[2], b.min[0]],
        max: [b.max[1], b.max[2], b.max[0]],
    };
    let mut stores = [Store::default(), Store::default(), Store::default()];
    for _ in 0..300 {
        let at = [0, 1, 2].map(|_| (rng.next() % 300) as i32 - 150);
        let state = rng.pick(&[red, red, AIR, VOID]);
        cube_at(&mut stores[0], at, state);
        cube_at(&mut stores[1], turn(at), state);
        cube_at(&mut stores[2], turn(turn(at)), state);
    }
    let bound = |store: &Store, (zoom, at): Node<3>| store.bounds((zoom, at), zoom).val().flatten();
    for _ in 0..500 {
        let (zoom, at) = some_node(&mut rng, 200.0);
        let given = bound(&stores[0], (zoom, at));
        assert_eq!(
            bound(&stores[1], (zoom, turn(at))),
            given.map(turn_box),
            "{at:?} at z{zoom}"
        );
        assert_eq!(
            bound(&stores[2], (zoom, turn(turn(at)))),
            given.map(|b| turn_box(turn_box(b))),
            "{at:?} at z{zoom}"
        );
    }
}

#[test]
fn column_reach() {
    let mut rng = Rng(SplitMix64::new(0x7d3a_51e9_c2b4_0f86));
    let red = cube_id(Cube::Object(Object::red));
    let mut store = Store::default();
    for _ in 0..300 {
        let at = [
            (rng.next() % 64) as i32,
            (rng.next() % 64) as i32,
            (rng.next() % 300) as i32 - 150,
        ];
        cube_at(&mut store, at, rng.pick(&[red, red, AIR]));
    }
    // Each leaf's reach along z in the column, as the cube of the column's
    // zoom it lies in where it is finer.
    let reach = |(zoom, [x, y]): Node<2>| {
        let mut z: Option<[f64; 2]> = None;
        store.each(&mut |e| {
            let (k, at) = (
                zoom.min(e.zoom),
                e.at.map(|v| v >> e.zoom.saturating_sub(zoom)),
            );
            if [at[0], at[1]] == [x >> (zoom - k), y >> (zoom - k)] {
                let s = [f64::from(at[2]) * units(k), f64::from(at[2] + 1) * units(k)];
                z = Some(z.map_or(s, |z| [z[0].min(s[0]), z[1].max(s[1])]));
            }
        });
        let whole = region((zoom, [x, y]));
        z.map(|z| Aabb {
            min: [whole.min[0], whole.min[1], z[0]],
            max: [whole.max[0], whole.max[1], z[1]],
        })
    };
    let mut walk = Walk::default();
    for _ in 0..400 {
        let zoom = rng.pick(&[
            UNIT_ZOOM - 6,
            UNIT_ZOOM - 3,
            UNIT_ZOOM - 1,
            UNIT_ZOOM,
            UNIT_ZOOM + 1,
        ]);
        let side = 64 >> UNIT_ZOOM.saturating_sub(zoom) << zoom.saturating_sub(UNIT_ZOOM);
        let node = (
            zoom,
            [0, 1].map(|_| (rng.next() % side.max(1) as u64) as i32),
        );
        assert_eq!(store.column(&mut walk, node), reach(node), "{node:?}");
    }
}

fn words(text: &str) -> Option<Ext> {
    Some(Ext::MarkerText(text.to_string()))
}

#[test]
fn leaf_words_freed_on_erase() {
    use pxl_spec::asset::marker::MARKER;
    let mut store = Store::default();
    let (zoom, at) = (OBJECT_COARSEST, [5, 7, 2]);
    store.write([Write {
        key: key(zoom, at),
        id: MARKER,
        ext: words("cafe"),
    }]);
    let e = store.read((zoom, at)).placed().expect("placed");
    assert_eq!(store.ext(&e), Some(&Ext::MarkerText("cafe".into())));
    store.write([Write {
        key: key(zoom, at),
        id: VOID,
        ext: None,
    }]);
    assert!(store.read((zoom, at)).placed().is_none());
    // The freed slot is the next one kept: nothing outlived its leaf.
    store.write([Write {
        key: key(zoom, at),
        id: MARKER,
        ext: words("bar"),
    }]);
    let e = store.read((zoom, at)).placed().expect("placed again");
    assert_eq!(e.ext, 1);
    assert_eq!(store.ext(&e), Some(&Ext::MarkerText("bar".into())));
}

#[test]
fn marker_words_change_cube() {
    use pxl_spec::asset::marker::MARKER;
    let mut store = Store::default();
    let (zoom, at) = (OBJECT_COARSEST, [5, 7, 2]);
    let put = |store: &mut Store, text: &str| {
        store.write([Write {
            key: key(zoom, at),
            id: MARKER,
            ext: words(text),
        }])
    };
    assert_eq!(put(&mut store, "cafe").len(), 1);
    // The old record's slot is the one kept, so the word is the same.
    assert_eq!(put(&mut store, "bar").len(), 1);
    let e = store.read((zoom, at)).placed().expect("placed");
    assert_eq!(store.ext(&e), Some(&Ext::MarkerText("bar".into())));
}

#[test]
fn eight_markers_stay() {
    use pxl_spec::asset::marker::MARKER;
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
fn whole_shard_meets_leaves() {
    let mut store = Store::default();
    let at = [5, 7, 2];
    store.write([Write {
        key: key(OBJECT_COARSEST, at),
        id: 3,
        ext: None,
    }]);
    // The node of a whole shard: its tree's root, read through the region.
    let shard = [
        at[0] >> (OBJECT_COARSEST - SHARD_ZOOM),
        at[1] >> (OBJECT_COARSEST - SHARD_ZOOM),
        0,
    ];
    let mut met = 0;
    store.read((SHARD_ZOOM, shard)).entries(&mut |_| met += 1);
    assert_eq!(met, 1);
    assert!(store
        .bounds::<3>((SHARD_ZOOM, shard), SHARD_ZOOM)
        .val()
        .flatten()
        .is_some());
}
