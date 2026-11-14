// pxl-core/domain/layer/object/store.test.rs

use pxl_spec::asset::cube::{Object, Sentinel};
use pxl_spec::share::eighths;
use pxl_spec::space::coord::zoom::{LEAF_ZOOM, SHARD_ZOOM, UNIT_ZOOM};
use pxl_spec::space::hierarchy::morton::key_of;
use pxl_spec::space::layer::lattice::GROUND;

use fnv::FnvHashMap;

use super::*;
use pxl_engine::object::{self, Cut};
use pxl_spec::bake::constant::OBJECT_COARSEST;
use xpute_kit::math::rng::SplitMix64;

fn entry(store: &Store, zoom: Zoom, at: [i32; 3]) -> Option<Entry> {
    store.entry_at(zoom, at, store.cursor((zoom, at)))
}

fn id_of(zoom: Zoom, at: [i32; 3]) -> u128 {
    key_of::<3>(zoom, at)
}

#[test]
fn write_undo_round_trip() {
    crate::on_core(|| {
        let mut store = Store::default();
        for at in [[8, 16, 24], [-3, 5, -7]] {
            assert!(!store
                .write([Write {
                    at: id_of(OBJECT_COARSEST, at),
                    asset_id: 3,
                    ext: None
                }])
                .is_empty());
            let finest = at.map(|v| v << (OBJECT_FINEST - OBJECT_COARSEST));
            assert!(
                {
                    let slot = store.cursor((OBJECT_FINEST, finest));
                    slot.holds()
                        && !slot.is_folded()
                        && object::val(&store.columns(), slot, Cut::NONE, OBJECT_FINEST, finest)
                            == 3
                },
                "a cell leaf covers its steps"
            );
            let mut met = Vec::new();
            let slot = store.cursor((OBJECT_COARSEST, at));
            store.each_in(OBJECT_COARSEST, at, slot, &mut |e| met.push(*e));
            assert_eq!(
                met,
                vec![Entry {
                    zoom: OBJECT_COARSEST,
                    at,
                    asset_id: 3,
                    ext: 0
                }]
            );
            store.write([Write {
                at: id_of(OBJECT_COARSEST, at),
                asset_id: Sentinel::Void as u32,
                ext: None,
            }]);
            let mut left = 0;
            store.each(&mut |_| left += 1);
            assert_eq!(left, 0);
            assert!(
                store
                    .write([Write {
                        at: id_of(OBJECT_COARSEST, at),
                        asset_id: Sentinel::Void as u32,
                        ext: None,
                    }])
                    .is_empty(),
                "erasing nothing changes nothing"
            );
        }
    });
}

/// A seeded generator, so the random edits are the same on every run.
struct Rng(SplitMix64);

impl Rng {
    fn next(&mut self) -> u64 {
        u64::from(self.0.next_u32())
    }

    fn pick<T: Copy>(&mut self, of: &[T]) -> T {
        of[(self.next() % of.len() as u64) as usize]
    }
}

/// A node from a unit to a kilometer wide, within `radius` units of the origin.
fn some_node(rng: &mut Rng, radius: f64) -> Node<3> {
    let zoom: Zoom = rng.pick(&[24, 21, 18, 16, 14]);
    let at = [0, 1, 2].map(|_| {
        ((rng.next() % (2 * radius as u64)) as f64 - radius).div_euclid(units(zoom)) as i32
    });
    (zoom, at)
}

/// What the store's folded tree holds at each cell: its value and how many
/// eighths of the cell's cube are filled (pxl-spec `share::eighths`).
type Content = FnvHashMap<[i32; 3], (u32, u32)>;

/// Writes `cells`, whole cells offset from a fixed corner, and reads back
/// through the store's query what the folded tree holds at `zoom`'s cells,
/// wherever it holds anything. Also returns the corner in that zoom's cells.
fn folded(cells: &[(i32, i32, i32, u32)], zoom: Zoom) -> (Content, [i32; 2]) {
    let mut store = Store::default();
    let base = [64, -64];
    store.write(cells.iter().map(|&(x, y, z, state)| Write {
        at: id_of(OBJECT_COARSEST, [base[0] + x, base[1] + y, z]),
        asset_id: state,
        ext: None,
    }));
    let target = GROUND.cell_zoom(zoom);
    let corner = base.map(|v| v >> (GROUND.leaf_cell_zoom() - target));
    let per = 64 >> (GROUND.leaf_cell_zoom() - target);
    let mut out = FnvHashMap::default();
    for z in -4..8 {
        for y in corner[1]..corner[1] + per {
            for x in corner[0]..corner[0] + per {
                let slot = store.cursor((target, [x, y, z]));
                let v = object::val(&store.columns(), slot, Cut::NONE, target, [x, y, z]);
                let eighths = eighths(object::fill_share(&store.columns(), slot));
                if v != Sentinel::Void as u32 && eighths > 0 {
                    out.insert([x, y, z], (v, eighths));
                }
            }
        }
    }
    (out, corner)
}

fn red() -> u32 {
    Object::Red as AssetId
}

#[test]
fn leaf_cell_folds_to_itself() {
    crate::on_core(|| {
        let (out, [x, y]) = folded(&[(3, 4, 5, red())], LEAF_ZOOM);
        assert_eq!(out.get(&[x + 3, y + 4, 5]), Some(&(red(), 8)));
        assert_eq!(out.len(), 1);
    });
}

#[test]
fn lone_cube_fold() {
    crate::on_core(|| {
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
    });
}

#[test]
fn solid_block_folds() {
    crate::on_core(|| {
        let cube: Vec<_> = (0..64).map(|k| (k % 4, k / 4 % 4, k / 16, red())).collect();
        let (out, [x, y]) = folded(&cube, LEAF_ZOOM - 2);
        assert_eq!(out.get(&[x, y, 0]), Some(&(red(), 8)));
        assert_eq!(out.len(), 1);
    });
}

#[test]
fn air_folds_as_val() {
    crate::on_core(|| {
        let air = Sentinel::Air as u32;
        let (out, [x, y]) = folded(
            &[
                (0, 0, 0, air),
                (1, 0, 0, air),
                (0, 1, 0, air),
                (1, 1, 0, air),
                (0, 0, 1, air),
            ],
            LEAF_ZOOM - 1,
        );
        assert_eq!(out.get(&[x, y, 0]), Some(&(air, 5)));
    });
}

fn cube_at(store: &mut Store, [x, y, z]: [i32; 3], state: u32) {
    store.write([Write::at(Octant::new(UNIT_ZOOM, [x, y, z]), state)]);
}

#[test]
fn axis_turn() {
    crate::on_core(|| {
        let mut rng = Rng(SplitMix64::new(0x2c1b_3c6d_a5b4_9f1e));
        let red = Object::Red as AssetId;
        let turn = |[x, y, z]: [i32; 3]| [y, z, x];
        let turn_box = |b: Aabb| Aabb {
            min: [b.min[1], b.min[2], b.min[0]],
            max: [b.max[1], b.max[2], b.max[0]],
        };
        let mut stores = [Store::default(), Store::default(), Store::default()];
        for _ in 0..300 {
            let at = [0, 1, 2].map(|_| (rng.next() % 300) as i32 - 150);
            let state = rng.pick(&[red, red, Sentinel::Air as u32, Sentinel::Void as u32]);
            cube_at(&mut stores[0], at, state);
            cube_at(&mut stores[1], turn(at), state);
            cube_at(&mut stores[2], turn(turn(at)), state);
        }
        let bound = |store: &Store, node: Node<3>| store.cursor(node).holds().then(|| region(node));
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
    });
}

#[test]
fn column_extent() {
    crate::on_core(|| {
        let mut rng = Rng(SplitMix64::new(0x7d3a_51e9_c2b4_0f86));
        let red = Object::Red as AssetId;
        let mut store = Store::default();
        for _ in 0..300 {
            let at = [
                (rng.next() % 64) as i32,
                (rng.next() % 64) as i32,
                (rng.next() % 300) as i32 - 150,
            ];
            cube_at(&mut store, at, rng.pick(&[red, red, Sentinel::Air as u32]));
        }
        // Each leaf's extent along z in the column. A leaf finer than the
        // column's zoom counts as the whole cube of that zoom it lies in.
        let extent = |(zoom, [x, y]): Node<2>| {
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
            assert_eq!(store.column(&mut walk, node), extent(node), "{node:?}");
        }
    });
}

fn marker_text(text: &str) -> Option<Ext> {
    Some(Ext::MarkerText(text.to_string()))
}

#[test]
fn leaf_words_freed_on_erase() {
    crate::on_core(|| {
        use pxl_spec::asset::marker::MARKER;
        let mut store = Store::default();
        let (zoom, at) = (OBJECT_COARSEST, [5, 7, 2]);
        store.write([Write {
            at: id_of(zoom, at),
            asset_id: MARKER,
            ext: marker_text("cafe"),
        }]);
        let e = entry(&store, zoom, at).expect("placed");
        assert_eq!(store.ext(&e), Some(&Ext::MarkerText("cafe".into())));
        store.write([Write {
            at: id_of(zoom, at),
            asset_id: Sentinel::Void as u32,
            ext: None,
        }]);
        assert!(entry(&store, zoom, at).is_none());
        // The freed slot is the next one taken, so no record outlived its leaf.
        store.write([Write {
            at: id_of(zoom, at),
            asset_id: MARKER,
            ext: marker_text("bar"),
        }]);
        let e = entry(&store, zoom, at).expect("placed again");
        assert_eq!(e.ext, 1);
        assert_eq!(store.ext(&e), Some(&Ext::MarkerText("bar".into())));
    });
}

#[test]
fn marker_text_change_cube() {
    crate::on_core(|| {
        use pxl_spec::asset::marker::MARKER;
        let mut store = Store::default();
        let (zoom, at) = (OBJECT_COARSEST, [5, 7, 2]);
        let put = |store: &mut Store, text: &str| {
            store.write([Write {
                at: id_of(zoom, at),
                asset_id: MARKER,
                ext: marker_text(text),
            }])
        };
        assert_eq!(put(&mut store, "cafe").len(), 1);
        // The old record's slot is reused, so the leaf's word stays the same,
        // and the cube must still count as changed.
        assert_eq!(put(&mut store, "bar").len(), 1);
        let e = entry(&store, zoom, at).expect("placed");
        assert_eq!(store.ext(&e), Some(&Ext::MarkerText("bar".into())));
    });
}

#[test]
fn eight_markers_stay() {
    crate::on_core(|| {
        use pxl_spec::asset::marker::MARKER;
        let mut store = Store::default();
        let parent = [4, 4, 2];
        let writes: Vec<Write> = (0..8)
            .map(|k| Write {
                at: id_of(
                    OBJECT_COARSEST + 1,
                    object::child_at(OBJECT_COARSEST, parent, k),
                ),
                asset_id: MARKER,
                ext: marker_text(&format!("m{k}")),
            })
            .collect();
        store.write(writes);
        let mut met = 0;
        store.each(&mut |e| {
            assert_eq!(e.zoom, OBJECT_COARSEST + 1);
            met += 1;
        });
        assert_eq!(met, 8);
    });
}

#[test]
fn whole_shard_meets_leaves() {
    crate::on_core(|| {
        let mut store = Store::default();
        let at = [5, 7, 2];
        store.write([Write {
            at: id_of(OBJECT_COARSEST, at),
            asset_id: 3,
            ext: None,
        }]);
        // The node of the whole shard, which is the root of the shard's own
        // tree.
        let shard = [
            at[0] >> (OBJECT_COARSEST - SHARD_ZOOM),
            at[1] >> (OBJECT_COARSEST - SHARD_ZOOM),
            0,
        ];
        let mut met = 0;
        let slot = store.cursor((SHARD_ZOOM, shard));
        store.each_in(SHARD_ZOOM, shard, slot, &mut |_| met += 1);
        assert_eq!(met, 1);
        assert!(slot.holds());
    });
}
