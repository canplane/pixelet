// pxl-kernel/layer/object/store.test.rs

use pxl_spec::bake::constant::{BAND_CELLS, CELL, CELL_STEPS};
use pxl_spec::cell::cube_types::Block;
use pxl_spec::space::indexer::L2_IDX;
use pxl_spec::space::pvf3::{fvec, ivec};
use pxl_spec::space::xy::TO_XY;

use super::*;
use crate::global::serial;
use crate::layer::object::size::DECOR_SIZE;

// Far-apart corners per test: the store is kernel-global across the binary.
fn corner(x: i64, y: i64, z: i64) -> Pvf3 {
    Pvf3 { i: ivec::gather(x, y, z), f: 0 }
}

fn in_cell(c: &Pvf3) -> Vec<OccupiedEntry> {
    let [ix, iy, iz] = CELL.index_of(c);
    get_objects_in_cell(ix, iy, iz)
}

/// A column's revision moves on a write to it, and on nothing else: not
/// another column's write, and not the eraser's previews, which change what
/// is drawn and not what is stored.
#[test]
fn a_column_s_revision_moves_on_its_own_writes_and_not_on_a_preview() {
    let _g = serial();
    let c = corner(100_020, 0, 0);
    let col_idx = L2_IDX(TO_XY(c.i));
    let elsewhere = L2_IDX(TO_XY(ivec::gather(1_000_000, 1_000_000, 0)));
    let (before, other) = (object_col_revision(col_idx), object_col_revision(elsewhere));
    place_decor(&c, DECOR_SIZE, DecorId::tree);
    assert!(object_col_revision(col_idx) > before, "its own write moved it");
    assert_eq!(object_col_revision(elsewhere), other, "another column's did not");
    let (placed, all) = (object_col_revision(col_idx), objects_revision());
    set_virtual_hide(Some(in_cell(&c).remove(0).at));
    set_virtual_carve_preview(&corner(100_021, 0, 0), DECOR_SIZE);
    set_virtual_hide(None);
    clear_virtual_carve_preview();
    assert_eq!((object_col_revision(col_idx), objects_revision()), (placed, all), "a preview moved nothing stored");
}

#[test]
fn object_store_persistence_a_save_is_placement_records_per_page_and_loading_it_puts_every_object_back_where_it_was() {
    let _g = serial();
    let a = corner(5000, 7000, 12);
    let b = Pvf3 {
        i: ivec::gather(5001, 7000, 12),
        f: fvec::gather(fvec::from_real(0.25), 0, 0),
    };
    place(&a, 1.0, PlaceableCube::Block(Block::red), false);
    place(&b, 0.25, PlaceableCube::Block(Block::blue), false);
    place_decor(
        &Pvf3 {
            i: ivec::gather(5000, 7001, 12),
            f: 0,
        },
        DECOR_SIZE,
        DecorId::tree,
    );
    let shape = |o: &OccupiedEntry| (o.i, o.f, o.size.to_bits(), matches!(o.kind, EntryKind::Decor { .. }));
    // The three cells the test places in and the ones around them.
    let get_objects_near = |at: &Pvf3| {
        let [cx, cy, cz] = CELL.index_of(at);
        let near = (-1..=1).flat_map(|dx| (-1..=1).flat_map(move |dy| (-1..=1).map(move |dz| [cx + dx, cy + dy, cz + dz])));
        near.flat_map(|[x, y, z]| get_objects_in_cell(x, y, z)).collect::<Vec<_>>()
    };
    let before: Vec<_> = get_objects_near(&a).iter().map(shape).collect();
    assert_eq!(before.len(), 3);
    let blob = encode_objects().unwrap();
    for o in get_objects_near(&a) {
        remove(o.at);
    }
    assert_eq!(get_objects_near(&a).len(), 0);
    decode_objects(&blob).unwrap();
    assert_eq!(get_objects_near(&a).iter().map(shape).collect::<Vec<_>>(), before);
    assert_eq!(encode_objects().unwrap(), blob);
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

/// Deterministic edits on every run.
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

/// Whole cells in band 0 of one column, and what the store keeps folded at
/// `level`, with the column's corner in that level's cells.
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

fn cube_at(store: &mut Store, [x, y, z]: [i64; 3], state: u32) {
    store.write([write_of(&corner(x, y, z), 1.0, state)]);
}

/// The index may give a box with only air in it, never none where an entry
/// is; checked after every edit, over boxes of every size across zero.
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

/// Painted as sub-cells, the mesher would draw a box each.
#[test]
fn a_paint_finer_than_a_cube_takes_it_whole() {
    let mut store = Store::new();
    let col_idx = ivec::gather(1, -1, 0);
    let step = OBJECT_FINEST.count_in(CELL);
    store.write(
        [Write {
            col_idx,
            band_idx: 0,
            at: [3 * step, 4 * step, 5 * step],
            scale: CELL,
            state: red(),
        }]
        .into_iter(),
    );
    for target in [OBJECT_FINEST.up(1), CELL.down(1)] {
        let mut out = Vec::new();
        store.fold_entries(col_idx, target, None, &mut |band, at, scale, v| out.push((band, at, scale, v)));
        assert_eq!(out, [(0, [3 * step, 4 * step, 5 * step], CELL, red())]);
    }
}

fn round_trip(origin: [f64; 2], seed: u64) {
    let mut rng = Rng(seed);
    kernel().objects.clear();
    let red = placeable_cube_num_id(PlaceableCube::Block(Block::red));
    let blue = placeable_cube_num_id(PlaceableCube::Block(Block::blue));
    let decor = decor_num_id(DecorId::tree);
    let mut writes = Vec::new();
    for k in 0..3_000 {
        let scale = OBJECT_FINEST.up((rng.next() % 4) as i32);
        let side = scale.side();
        let at = [0, 1, 2].map(|_| (((rng.next() % 1_200) as f64 - 600.0) * STEP_UNITS / side).floor() * side);
        let at = if k % 7 == 0 { [at[0], at[1], at[2] + side] } else { at };
        let value = rng.pick(&[red, red, blue, AIR, decor, VOID]);
        let mut coord = Pvf3::default();
        pxl_spec::space::pvf3::projection::project(&mut coord, origin[0] + at[0], origin[1] + at[1], at[2]);
        writes.push(write_of(&coord, side, value));
    }
    // Five of the eight bands about the origin filled by a majority at every
    // level, so there is something to fold over a band too.
    let band = BAND_CELLS;
    let mut at = Pvf3::default();
    pxl_spec::space::pvf3::projection::project(&mut at, origin[0], origin[1], 0.0);
    let col_idx = L2_IDX(TO_XY(at.i));
    let col = [ivec::X(col_idx), ivec::Y(col_idx)];
    for slot in 0..5 {
        let [x, y, z] = child_of([-1i64; 3], slot);
        let state = if slot % 2 == 0 { red } else { blue };
        writes.extend(fill(ivec::gather(col[0] + x, col[1] + y, 0), z as BandZ, [0; 3], band, state));
    }
    apply(writes);
    let before = held(&kernel().objects);
    let over = before.1.iter().filter(|((scale, _), _)| *scale > BAND).count();
    assert!(
        before.0.len() > 1_000 && over > 0 && before.3.len() > 1_000,
        "{origin:?}: too little held to say much: {} leaves, {over} folds over a band",
        before.0.len()
    );
    let blob = encode_objects().unwrap();
    kernel().objects.clear();
    decode_objects(&blob).unwrap();
    let after = held(&kernel().objects);
    assert_eq!(before.0, after.0, "{origin:?}: the leaves");
    assert_eq!(before.1, after.1, "{origin:?}: the folds over a band");
    assert_eq!(before.2, after.2, "{origin:?}: the bounds");
    assert_eq!(before.3, after.3, "{origin:?}: the folds under a band");
    kernel().objects.clear();
}
