// pxl-kernel/layer/object/store.test.rs

//
// A col_idx's changes are read as its revisions moving, which is what
// world/chunk/source.rs follows.

use pxl_spec::bake::constant::{BAND_CELLS, CELL, CELL_STEPS};
use pxl_spec::cell::cube_types::Block;
use pxl_spec::space::pvf3::ivec;

use pxl_spec::space::hierarchy::slot_of;

use super::*;

// Distinct, far-apart corners per test so leftover state from one test never
// collides with another: the store is kernel-global across the test binary.
fn corner(x: i64, y: i64, z: i64) -> Pvf3 {
    Pvf3 { i: ivec::gather(x, y, z), f: 0 }
}

fn entries(store: &Store, col_idx: IVec) -> Vec<(i32, u32, u32)> {
    let Some(e) = store.entries(col_idx) else { return Vec::new() };
    (0..e.band.len()).map(|k| (e.band[k], e.pos[k], e.state[k])).collect()
}

fn write(store: &mut Store, col_idx: IVec, band_idx: BandZ, at: [u32; 3], scale: Scale, state: u32) -> BTreeSet<(IVec, BandZ)> {
    store.write([Write { col_idx, band_idx, at, scale, state }])
}

#[test]
fn a_write_is_read_back_and_its_entries_are_made_within_it() {
    let mut store = Store::new();
    assert_eq!(write(&mut store, 7, 0, [8, 16, 24], CELL, 3), BTreeSet::from([(7, 0)]));
    assert_eq!(store.read(7, 0, [8, 16, 24]), 3);
    assert_eq!(store.read(7, 0, [15, 23, 31]), 3, "a cell leaf covers its eight steps");
    assert_eq!(store.read(7, 0, [16, 16, 24]), VOID);
    assert_eq!(entries(&store, 7), vec![(0, pack_pos(8, 16, 24, CELL), 3)]);
    let cell = store.cell(7, 1, 2, 3).expect("the cell is found");
    assert_eq!((cell.start, cell.count, cell.solid), (0, 1, 1));
    assert!(store.cell(7, 2, 2, 3).is_none());
}

#[test]
fn a_void_write_prunes_the_band_and_the_chunk() {
    let mut store = Store::new();
    write(&mut store, 7, 2, [0, 0, 0], OBJECT_FINEST, AIR);
    assert_eq!(store.cols().count(), 1);
    let cell = store.cell(7, 0, 0, 2 * BAND_CELLS as i32).expect("the cell is found");
    assert_eq!(cell.solid, 0, "AIR does not occupy");
    assert_eq!(write(&mut store, 7, 2, [0, 0, 0], OBJECT_FINEST, VOID), BTreeSet::from([(7, 2)]));
    assert_eq!(store.cols().count(), 0);
    assert_eq!(entries(&store, 7), vec![]);
    assert!(store.cell(7, 0, 0, 2 * BAND_CELLS as i32).is_none());
    assert!(write(&mut store, 7, 2, [0, 0, 0], OBJECT_FINEST, VOID).is_empty(), "erasing nothing changes nothing");
}

#[test]
fn a_merged_block_comes_back_as_its_cells_each_found_where_it_lies() {
    let mut store = Store::new();
    // A 4×4×4 block of whole cells folds into one leaf two levels above the
    // cell; its cells come back one entry each, in the lanes' own order.
    let writes = (0..64u32).map(|k| {
        let [x, y, z] = at_walk_order::<3>(k as u64, 2);
        Write {
            col_idx: 1,
            band_idx: -1,
            at: [x * CELL_STEPS, y * CELL_STEPS, z * CELL_STEPS],
            scale: CELL,
            state: 5,
        }
    });
    store.write(writes);
    let found = entries(&store, 1);
    assert_eq!(found.len(), 64);
    assert!(found.iter().all(|&(b, p, s)| b == -1 && s == 5 && unpack_pos(p).3 == CELL));
    for (k, &(_, p, _)) in found.iter().enumerate() {
        let (x, y, z, _) = unpack_pos(p);
        let cell = store
            .cell(1, x / CELL_STEPS, y / CELL_STEPS, -(BAND_CELLS as i32) + (z / CELL_STEPS) as i32)
            .expect("every cell is found");
        assert_eq!((cell.start, cell.count), (k as u32, 1));
    }
}

#[test]
fn the_lanes_are_in_band_and_walk_order_so_a_cell_s_entries_are_one_run() {
    let mut store = Store::new();
    // Sub-cell leaves in two cells and a whole cell between them, over two
    // bands, written out of order.
    let finest = |x: u32, y: u32, z: u32| Write {
        col_idx: 3,
        band_idx: 0,
        at: [x, y, z],
        scale: OBJECT_FINEST,
        state: 2,
    };
    store.write([
        Write { band_idx: 1, ..finest(8, 0, 0) },
        finest(9, 1, 0),
        finest(8, 0, 0),
        Write { scale: CELL, ..finest(0, 8, 0) },
        finest(1, 0, 0),
        finest(0, 0, 1),
    ]);
    let found = entries(&store, 3);
    let order: Vec<(BandZ, u64)> = found
        .iter()
        .map(|&(b, p, _)| {
            let (x, y, z, _) = unpack_pos(p);
            (b, walk_order([x / CELL_STEPS, y / CELL_STEPS, z / CELL_STEPS], BAND_CELL_SHAMT))
        })
        .collect();
    assert!(order.is_sorted(), "{order:?}");
    assert_eq!(store.cell(3, 0, 0, 0).map(|c| c.count), Some(2));
    assert_eq!(store.cell(3, 1, 0, 0).map(|c| c.count), Some(2));
    assert_eq!(store.cell(3, 0, 1, 0).map(|c| c.count), Some(1));
    assert_eq!(store.cell(3, 1, 0, BAND_CELLS as i32).map(|c| c.count), Some(1));
}

/// xorshift64: the same edits on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn pick<T: Copy>(&mut self, of: &[T]) -> T {
        of[(self.next() % of.len() as u64) as usize]
    }
}

/// The writes that set, of the block `cells` a side from `from` in band
/// `band_idx` of column `col_idx`, five of every eight children down to a
/// whole cell to `state`: the least that makes the block `state` at every
/// level by a strict majority, whatever the ties.
fn fill(col_idx: IVec, band_idx: BandZ, from: [u32; 3], cells: u32, state: u32) -> Vec<Write> {
    fn go(corner: [u32; 3], side: u32, write: &mut dyn FnMut([u32; 3])) {
        if side == 1 {
            return write(corner);
        }
        for slot in 0..5 {
            let c = child_of([0i64; 3], slot);
            go([0, 1, 2].map(|a| corner[a] + c[a] as u32 * side / 2), side / 2, write);
        }
    }
    let step = OBJECT_FINEST.count_in(CELL);
    let mut out = Vec::new();
    go(from, cells, &mut |c| {
        out.push(Write {
            col_idx,
            band_idx,
            at: c.map(|v| v * step),
            scale: CELL,
            state,
        })
    });
    out
}

/// The objects' fold done the long way, as a chunk once did at every build:
/// every entry of `columns` but `hide`, each at its own size, folded from
/// the finest up a level at a time to `target` — what the store's folds
/// are held to.
fn fold_entries<'a>(columns: impl IntoIterator<Item = Entries<'a>>, target: Scale, hide: Option<EntryAt>) -> HashMap<[i64; 3], u32> {
    let mut native: HashMap<Scale, Vec<([i64; 3], u32)>> = HashMap::new();
    for e in columns {
        let corner = FROM_L2_IDX(e.col_idx);
        let (x0, y0) = (OBJECT_FINEST.index(ivec::X(corner)), OBJECT_FINEST.index(ivec::Y(corner)));
        for k in 0..e.band.len() {
            let (band, pos, state) = (e.band[k], e.pos[k], e.state[k]);
            if !object_value_is_voxel(state) || hide == Some(e.at(k)) {
                continue;
            }
            let (x, y, z, own) = unpack_pos(pos);
            let (x, y) = (x0 + x as i64, y0 + y as i64);
            let z = band as i64 * OBJECT_FINEST.count_in(BAND) as i64 + z as i64;
            // An entry is a cell or finer, and a target a cell or coarser.
            assert!(own <= target);
            let at = |v: i64| v.div_euclid(OBJECT_FINEST.count_in(own) as i64);
            native.entry(own).or_default().push(([at(x), at(y), at(z)], state));
        }
    }
    let mut cells: HashMap<[i64; 3], u32> = HashMap::new();
    let mut scale = OBJECT_FINEST;
    loop {
        cells.extend(native.remove(&scale).unwrap_or_default());
        if scale >= target {
            return cells;
        }
        let mut parents: HashMap<[i64; 3], [u32; 8]> = HashMap::new();
        for (c, state) in cells.drain() {
            parents.entry(parent_of(c)).or_insert([0; 8])[slot_of(c)] = state;
        }
        let parent = scale.up(1);
        cells.extend(parents.into_iter().filter_map(|(p, v)| {
            let m = majority(v, parent.above(CELL));
            (m != 0).then_some((p, m))
        }));
        scale = parent;
    }
}

/// `cells` — (x, y, z, value), a whole cell each in band 0 of one column —
/// written into a store, and what it keeps folded at `level` over that
/// column; with the column's corner in that level's cells.
fn folded(cells: &[(u32, u32, u32, u32)], level: u32) -> (HashMap<[i64; 3], u32>, [i64; 2]) {
    let mut store = Store::new();
    let col_idx = ivec::gather(1, -1, 0);
    let step = OBJECT_FINEST.count_in(CELL);
    store.write(cells.iter().map(|&(x, y, z, state)| Write {
        col_idx,
        band_idx: 0,
        at: [x * step, y * step, z * step],
        scale: CELL,
        state,
    }));
    let target = GROUND.scale(level);
    let per = target.count_in(BAND) as i64;
    let corner = [per, -per];
    let mut out = HashMap::new();
    store.fold_at(target, corner, [corner[0] + per, corner[1] + per], None, &mut out);
    (out, corner)
}

fn red() -> u32 {
    placeable_cube_num_id(PlaceableCube::Block(Block::red))
}

#[test]
fn a_cell_folds_to_itself_at_level_0() {
    let (out, [x, y]) = folded(&[(3, 4, 5, red())], 0);
    assert_eq!(out.get(&[x + 3, y + 4, 5]), Some(&red()));
    assert_eq!(out.len(), 1);
}

#[test]
fn a_lone_cube_goes_at_the_first_fold_and_a_slab_of_half_stays() {
    assert!(folded(&[(0, 0, 0, red())], 1).0.is_empty());
    let (out, [x, y]) = folded(&[(0, 0, 0, red()), (1, 0, 0, red()), (0, 1, 0, red()), (1, 1, 0, red())], 1);
    assert_eq!(out.get(&[x, y, 0]), Some(&red()));
}

#[test]
fn a_solid_block_keeps_folding_while_it_fills_its_parent() {
    let cube: Vec<_> = (0..64).map(|k| (k % 4, k / 4 % 4, k / 16, red())).collect();
    let (out, [x, y]) = folded(&cube, 2);
    assert_eq!(out.get(&[x, y, 0]), Some(&red()));
    assert_eq!(out.len(), 1);
}

#[test]
fn air_is_a_value_and_folds_like_one() {
    let (out, [x, y]) = folded(&[(0, 0, 0, AIR), (1, 0, 0, AIR), (0, 1, 0, AIR), (1, 1, 0, AIR), (0, 0, 1, AIR)], 1);
    assert_eq!(out.get(&[x, y, 0]), Some(&AIR));
}

/// Every entry's box, in units.
fn entry_boxes(store: &Store) -> Vec<Aabb> {
    let mut out = Vec::new();
    for &col_idx in store.columns.keys() {
        let e = store.entries(col_idx).unwrap();
        let corner = FROM_L2_IDX(col_idx);
        for k in 0..e.band.len() {
            let (x, y, z, own) = unpack_pos(e.pos[k]);
            let min = [
                ivec::X(corner) as f64 + x as f64 * STEP_UNITS,
                ivec::Y(corner) as f64 + y as f64 * STEP_UNITS,
                e.band[k] as f64 * BAND_UNITS + z as f64 * STEP_UNITS,
            ];
            out.push(Aabb {
                min,
                max: min.map(|v| v + own.side()),
            });
        }
    }
    out
}

/// A whole cell of `state` with its corner at (x, y, z) units.
fn cube_at(store: &mut Store, [x, y, z]: [i64; 3], state: u32) {
    store.write([write_of(&corner(x, y, z), 1.0, state)]);
}

/// Where the objects in a box are is never missed and never past the box:
/// the box of every part of every entry inside it lies within what the
/// index gives, which lies within the box, and none is given only where
/// no entry is inside. The index may give a box where no entry is — a
/// band's bound, cut to the box, holds the air between its entries — but
/// never none where one is. After every edit, over boxes of every size,
/// across zero on every axis, where the coarsest cubes meet.
#[test]
fn where_the_objects_in_a_box_are_is_never_missed_and_never_past_it() {
    let mut rng = Rng(0x5851_f42d_4c95_7f2d);
    let mut store = Store::new();
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
                (Some(t), Some(g)) => assert!(within(&t, &g) && within(&g, &query), "step {step}: {t:?} in {g:?} in {query:?}"),
                (Some(t), None) => panic!("step {step}: {t:?} in {query:?} missed"),
                (None, Some(g)) => assert!(within(&g, &query), "step {step}: {g:?} past {query:?}"),
                (None, None) => {}
            }
        }
    }
    assert!(met > 1_000, "only {met} boxes held an entry");
}

/// No axis is the index's own: the same objects with their axes turned,
/// asked the same box turned, give the same box turned — whatever the
/// store keeps them by, a column across and a band up.
#[test]
fn turning_the_axes_turns_where_the_objects_are_and_nothing_else() {
    let mut rng = Rng(0x2c1b_3c6d_a5b4_9f1e);
    let red = placeable_cube_num_id(PlaceableCube::Block(Block::red));
    let turn = |[x, y, z]: [i64; 3]| [y, z, x];
    let turn_box = |b: Aabb| Aabb {
        min: [b.min[1], b.min[2], b.min[0]],
        max: [b.max[1], b.max[2], b.max[0]],
    };
    let mut stores = [Store::new(), Store::new(), Store::new()];
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
