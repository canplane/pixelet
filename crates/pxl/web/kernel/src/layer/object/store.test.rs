// pxl-kernel/layer/object/store.test.rs

//
// A col_idx's changes are read as its revisions moving, which is what
// world/chunk/source.rs follows.

use pxl_spec::bake::constant::{BAND_CELLS, CELL, CELL_STEPS};
use pxl_spec::cell::cube_types::Block;
use pxl_spec::space::indexer::L2_IDX;
use pxl_spec::space::pvf3::{fvec, ivec};
use pxl_spec::space::xy::TO_XY;

use pxl_spec::space::hierarchy::slot_of;

use super::*;
use crate::global::serial;
use crate::layer::object::size::DECOR_SIZE;

// Distinct, far-apart corners per test so leftover state from one test never
// collides with another: the store is kernel-global across the test binary.
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

/// A chunk reads a write to a column and a height it draws, in its tiers
/// and its decor, and not one it does not — a chunk beside it, or the chunk
/// over the same column a height up: the revision it reads agrees with what
/// it draws, at z16 and above.
#[test]
fn a_chunk_moves_with_what_it_draws_and_no_other() {
    use crate::world::chunk::source::{decor_revision_of, objects_revision_of};
    use crate::world::pyramid::chunk::Chunk;
    let _g = serial();
    let c = corner(4096 * 13, 4096 * 5, 3);
    for level in [0, 3] {
        let home = Chunk::of(level, &c);
        let (far, above) = (home.beside(5, 0, 0), home.beside(0, 0, 1));
        let before = (objects_revision_of(home), decor_revision_of(home), objects_revision_of(far), objects_revision_of(above));
        place(&c, DECOR_SIZE, PlaceableCube::Block(Block::red), false);
        assert!(objects_revision_of(home) > before.0, "level {level}: its tiers moved");
        assert!(decor_revision_of(home) > before.1, "level {level}: its decor moved");
        assert_eq!(objects_revision_of(far), before.2, "level {level}: a chunk beside it did not");
        assert_eq!(objects_revision_of(above), before.3, "level {level}: the chunk over it did not");
    }
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

/// What the store keeps folded is what the objects' fold makes of every
/// entry, at every level, after every edit: finer writes splitting a leaf,
/// coarser ones claiming a block, erasures, values the fold counts as
/// nothing, columns and bands either side of zero; and with any one entry
/// hidden, what the fold makes of every entry but it.
#[test]
fn what_the_store_keeps_folded_is_what_the_fold_makes_of_its_entries() {
    use pxl_spec::cell::object::{decor_ids, placeable_cubes};

    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let mut store = Store::new();
    let values = [
        VOID,
        AIR,
        placeable_cube_num_id(placeable_cubes[0]),
        placeable_cube_num_id(placeable_cubes[1]),
        placeable_cube_num_id(placeable_cubes[2]),
        decor_num_id(decor_ids[0]),
    ];
    let cols = [ivec::gather(-1, 0, 0), ivec::gather(0, 0, 0), ivec::gather(0, -1, 0), ivec::gather(1, 1, 0)];
    let (lo, hi) = ([-64.0f64, -64.0], [128.0f64, 128.0]);
    let mut mine = HashMap::new();
    let mut held = [false; 4];
    for step in 0..160 {
        let (col_idx, band_idx, state) = (rng.pick(&cols), rng.pick(&[-1, 0, 1]), rng.pick(&values));
        if rng.next().is_multiple_of(4) {
            // A block voted for at every level, over what is there.
            let side = rng.pick(&[4, 8]);
            store.write(fill(col_idx, band_idx, [0, 1, 2].map(|_| rng.pick(&[0, 4, 8])), side, state));
        } else {
            let scale = Scale::of(-3 + (rng.next() % 4) as i32);
            let n = OBJECT_FINEST.count_in(scale) as u64;
            // Crowded into a corner of the band, so votes meet.
            let at = [0, 1, 2].map(|_| ((rng.next() % (128 / n).max(1)) * n) as u32);
            store.write([Write { col_idx, band_idx, at, scale, state }]);
        }
        let hides: Vec<EntryAt> = cols.iter().filter_map(|&c| store.entries(c)).flat_map(|e| (0..e.band.len()).map(move |k| e.at(k))).collect();
        for hide in [None, (!hides.is_empty()).then(|| rng.pick(&hides))] {
            for level in 0..=TOP_LEVEL {
                let target = GROUND.scale(level);
                let side = target.side();
                let (from, to) = (
                    [(lo[0] / side).floor() as i64, (lo[1] / side).floor() as i64],
                    [(hi[0] / side).ceil() as i64, (hi[1] / side).ceil() as i64],
                );
                let oracle = fold_entries(cols.iter().filter_map(|&c| store.entries(c)), target, hide);
                store.fold_at(target, from, to, hide, &mut mine);
                assert_eq!(mine, oracle, "step {step}, level {level}, hiding {hide:?}");
                if (level as usize) < held.len() {
                    held[level as usize] |= !oracle.is_empty();
                }
            }
        }
    }
    assert_eq!(held, [true; 4], "a level held nothing, so nothing of it was compared");
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

/// Past a band, what the store keeps folded is the fold's too: bands filled
/// by a majority at every level, so the coarse cells hold something, then a
/// block of one erased, a band changed and an entry hidden.
#[test]
fn past_a_band_what_the_store_keeps_folded_is_what_the_fold_makes_of_its_entries() {
    use pxl_spec::cell::object::placeable_cubes;

    let (x, y) = (placeable_cube_num_id(placeable_cubes[0]), placeable_cube_num_id(placeable_cubes[1]));
    let cols = [ivec::gather(0, 0, 0), ivec::gather(1, 0, 0), ivec::gather(0, 1, 0), ivec::gather(1, 1, 0)];
    let band = BAND_CELLS;
    let mut store = Store::new();
    for (col, band_idx, state) in [(0, 0, x), (1, 0, x), (2, 0, x), (3, 0, x), (0, 1, x), (1, 1, y)] {
        store.write(fill(cols[col], band_idx, [0; 3], band, state));
    }
    let mut mine = HashMap::new();
    let mut check = |store: &Store, stage: &str| {
        let entry = store.entries(cols[1]).map(|e| e.at(e.band.len() / 2));
        for (hide, levels) in [(None, &[5, 6, 7, 8, TOP_LEVEL][..]), (entry, &[5, 7][..])] {
            for &level in levels {
                let target = GROUND.scale(level);
                let side = target.side();
                let (from, to) = ([0, 0], [(128.0 / side).ceil() as i64, (128.0 / side).ceil() as i64]);
                let oracle = fold_entries(cols.iter().filter_map(|&c| store.entries(c)), target, hide);
                store.fold_at(target, from, to, hide, &mut mine);
                assert_eq!(mine, oracle, "{stage}, level {level}, hiding {hide:?}");
                // One cell of level 7 is filled, and nothing over it.
                assert!(level > 7 || !oracle.is_empty(), "{stage}, level {level}: nothing folded, so nothing compared");
            }
        }
    };
    check(&store, "filled");
    store.write(fill(cols[2], 0, [16, 16, 16], 16, VOID));
    check(&store, "a block erased");
    store.write(fill(cols[3], 0, [0; 3], band, y));
    check(&store, "a band changed");
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

/// What the store holds, whole: every leaf of every band, what the objects
/// fold to at every cell over a band, where they are, and what they fold to
/// at every cell under a band that holds one — in an order that does not
/// depend on how the maps and trees are laid out.
type Held = (
    Vec<(IVec, BandZ, [i32; 3], Scale, u16)>,
    Vec<((Scale, [i64; 3]), u16)>,
    Vec<((Scale, [i64; 3]), [u64; 6])>,
    Vec<((Scale, [i64; 3]), u16)>,
);

fn held(store: &Store) -> Held {
    let mut leaves = Vec::new();
    for (&col_idx, column) in &store.columns {
        for (&band, tree) in &column.bands {
            sparse_tree::walk(tree, &mut |x, y, z, scale, value| {
                if u32::from(value) != VOID {
                    leaves.push((col_idx, band, [x, y, z], scale, value));
                }
            });
        }
    }
    let mut folds: Vec<_> = store.folds.iter().map(|(&k, &v)| (k, v)).collect();
    let mut bounds: Vec<_> = store
        .bounds
        .iter()
        .map(|(&k, b)| (k, [b.min, b.max].concat().try_into().map(|a: [f64; 6]| a.map(f64::to_bits)).unwrap()))
        .collect();
    // Under a band, every cell over a leaf, at every scale from the leaf's
    // to the band's.
    let mut under = Vec::new();
    for &(col_idx, band, [x, y, z], scale, _) in &leaves {
        let per = OBJECT_FINEST.count_in(scale) as i64;
        let mut s = scale;
        while s < BAND {
            let cells = s.count_in(BAND) as i64;
            let within = [x, y, z].map(|c| (i64::from(c) / per) >> s.above(scale));
            let at = [ivec::X(col_idx) * cells + within[0], ivec::Y(col_idx) * cells + within[1], i64::from(band) * cells + within[2]];
            under.push(((s, at), store.folded_value(s, at)));
            s = s.up(1);
        }
    }
    leaves.sort_unstable();
    folds.sort_unstable();
    bounds.sort_unstable();
    under.sort_unstable();
    under.dedup();
    (leaves, folds, bounds, under)
}

/// A save loaded back is the store it was saved from: the same leaves, and
/// so the same folds over and under a band and the same bounds, made again
/// from them. Over cubes of every size a write takes, solid, air and decor,
/// written, written over and taken away again, across zero on every axis
/// and stacked in columns — about the origin, and about Seoul, where a
/// position is far from its page's corner in the world's lanes.
#[test]
fn a_saved_store_loads_back_as_the_store_it_was() {
    let _g = serial();
    for (origin, seed) in [([0.0, 0.0], 0x0b1e_c7ed_5a7e_d00d), ([5_911_552.0, 1_900_544.0], 0x5e01_5eed)] {
        round_trip(origin, seed);
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
