// pxl-kernel/layer/object/store.test.rs

//
// A col_idx's changes are read as its revisions moving, which is what
// world/chunk/source.rs follows.

use pxl_spec::bake::constant::{BAND_CELLS, CELL_STEPS};
use pxl_spec::cell::cube_types::Block;
use pxl_spec::space::indexer::L2_IDX;
use pxl_spec::space::pvf3::{fvec, ivec};
use pxl_spec::space::xy::TO_XY;

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

fn write(store: &mut Store, col_idx: IVec, band_idx: BandZ, at: [u32; 3], scale: Scale, state: u32) -> BTreeSet<IVec> {
    store.write([Write { col_idx, band_idx, at, scale, state }])
}

#[test]
fn a_write_is_read_back_and_its_entries_are_made_within_it() {
    let mut store = Store::new();
    assert_eq!(write(&mut store, 7, 0, [8, 16, 24], CELL, 3), BTreeSet::from([7]));
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
    assert_eq!(write(&mut store, 7, 2, [0, 0, 0], OBJECT_FINEST, VOID), BTreeSet::from([7]));
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

/// A chunk above z16 reads a write to a column it draws, in its tiers and
/// its decor, and not one to a column it does not: the revision it reads
/// agrees with what it draws.
#[test]
fn a_level_chunk_moves_with_the_columns_it_draws_and_no_other() {
    use crate::world::chunk::source::{decor_revision_of, objects_revision_of};
    use crate::world::pyramid::chunk::Chunk;
    let _g = serial();
    let c = corner(4096 * 13, 4096 * 5, 3);
    let level = 3;
    let (home, far) = (Chunk::of(level, &c).base().i, Chunk::of(level, &c).beside(5, 0).base().i);
    let before = (objects_revision_of(level, home), decor_revision_of(level, home), objects_revision_of(level, far));
    place(&c, DECOR_SIZE, PlaceableCube::Block(Block::red), false);
    assert!(objects_revision_of(level, home) > before.0, "its tiers moved");
    assert!(decor_revision_of(level, home) > before.1, "its decor moved");
    assert_eq!(objects_revision_of(level, far), before.2, "a chunk that does not draw it did not");
}

/// What stands under a node reaches what the columns under it reach, at
/// every level, however the columns were written and erased: the tree is
/// the union of its columns, remade up the chain a write touches.
#[test]
fn a_node_reaches_what_the_columns_under_it_reach() {
    use pxl_spec::space::hierarchy::ancestor_of;
    use pxl_spec::space::indexer::L2;
    let mut store = Store::new();
    let cols = [(3i64, 5i64, 0, 1), (4, 5, 2, 3), (-7, 5, -1, 0), (40, -9, 1, 2), (3, 6, 0, 5)];
    for &(x, y, band, z) in &cols {
        write(&mut store, ivec::gather(x, y, 0), band, [0, 0, z * CELL_STEPS], CELL, 1);
    }
    write(&mut store, ivec::gather(3, 6, 0), 0, [0, 0, 5 * CELL_STEPS], CELL, VOID);
    for level in 0..6u32 {
        for &(x, y, _, _) in &cols {
            let at = ancestor_of([x, y], level);
            let union = store
                .chunks
                .iter()
                .filter(|(col, _)| ancestor_of([ivec::X(**col), ivec::Y(**col)], level) == at)
                .map(|(_, chunk)| chunk.z)
                .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)));
            assert_eq!(store.reach_over((L2.up(level as i32), at)), union, "level {level} at {at:?}");
        }
    }
}
