// pxl_core/module/world/object/store.test.rs

//
// A col_idx's changes are read as its revisions moving, which is what
// chunk_node.rs follows.

use pxl_shared::spec::cell::cube_types::Block;
use pxl_shared::spec::space::indexer::{L2_IDX, L2_IDX_IN_L3, L3_IDX};
use pxl_shared::spec::space::pvf3::{fvec, ivec};
use pxl_shared::spec::space::xy::TO_XY;

use super::*;
use crate::module::world::object::collision::find_overlapping_object;
use crate::module::world::object::size::DECOR_SIZE;
use crate::sys::global::serial;

// Distinct, far-apart corners per test so leftover state from one test never
// collides with another: the store is kernel-global across the test binary.
fn corner(x: i64, y: i64, z: i64) -> PVF3 {
    PVF3 { i: ivec::gather(x, y, z), f: 0 }
}

fn in_cell(c: &PVF3) -> Vec<OccupiedEntry> {
    let [ix, iy, iz] = cell_index(c);
    get_objects_in_cell(ix, iy, iz)
}

#[test]
fn object_store_place_decor_stores_a_real_decor_entry_not_a_color() {
    let _g = serial();
    let c = corner(100_001, 0, 0);
    place_decor(&c, DECOR_SIZE, DecorId::tree);
    let entries = in_cell(&c);
    assert_eq!(entries.len(), 1);
    assert!(matches!(entries[0].kind, EntryKind::Decor { decor_id: DecorId::tree }));
}

#[test]
fn object_store_place_decor_is_retrievable_by_its_own_address_same_path_a_solid_cube_uses() {
    let _g = serial();
    let c = corner(100_002, 0, 0);
    place_decor(&c, DECOR_SIZE, DecorId::tree);
    let entry = in_cell(&c).remove(0);
    let fetched = get_placed_object(entry.at).expect("fetched by its address");
    assert!(matches!(fetched.kind, EntryKind::Decor { .. }));
}

#[test]
fn object_store_place_decor_blocks_a_second_placement_at_the_exact_same_corner() {
    let _g = serial();
    let c = corner(100_003, 0, 0);
    place_decor(&c, DECOR_SIZE, DecorId::tree);
    let blocked = find_overlapping_object(&c, DECOR_SIZE, 0).expect("blocked");
    assert!(matches!(blocked.kind, EntryKind::Decor { .. }));
}

#[test]
fn object_store_place_decor_a_solid_cube_still_blocks_placement_the_same_way_a_decor_entry_does() {
    let _g = serial();
    let c = corner(100_004, 0, 0);
    place(&c, DECOR_SIZE, PlaceableCube::Block(Block::red), false);
    let blocked = find_overlapping_object(&c, DECOR_SIZE, 0).expect("blocked");
    assert!(matches!(blocked.kind, EntryKind::Solid { .. }));
}

#[test]
fn object_store_remove_erases_a_decor_entry_the_same_way_it_erases_a_solid_cube() {
    let _g = serial();
    let c = corner(100_005, 0, 0);
    place_decor(&c, DECOR_SIZE, DecorId::tree);
    let entry = in_cell(&c).remove(0);
    remove(entry.at);
    assert_eq!(in_cell(&c).len(), 0);
    assert!(get_placed_object(entry.at).is_none());
}

#[test]
fn object_store_column_key_an_object_above_one_l2_span_still_belongs_to_its_xy_column() {
    // The renderer asks for a col_idx by an XY page index and a z = 0 page
    // offset. A cell key that let its own Z through put everything above
    // absolute z = 64u under a key nothing ever queried.
    let _g = serial();
    let (x, y) = (4096 * 3, 4096 * 5);
    let ground = corner(x, y, 10);
    let high = corner(x, y, 200);
    place(&ground, DECOR_SIZE, PlaceableCube::Block(Block::red), false);
    place(&high, DECOR_SIZE, PlaceableCube::Block(Block::blue), false);
    let col_idx = TO_XY(ground.i);
    let key = object_col_idx_of(L3_IDX(col_idx), L2_IDX_IN_L3(col_idx));
    let mut zs = Vec::new();
    each_col_entry(key, &mut |e| zs.push(ivec::Z(e.corner().i)), false);
    zs.sort_unstable();
    assert_eq!(zs, vec![10, 200]);
    // The renderer's own spelling of the same key — a page offset it builds
    // with z = 0 — resolves to the same col_idx.
    assert_eq!(object_col_idx_of(L3_IDX(col_idx), ivec::gather(0, 0, 0) | L2_IDX_IN_L3(col_idx)), key);
}

#[test]
fn object_store_band_tree_a_2_2_2_block_of_one_colour_reads_back_as_eight_cells() {
    // A band tree merges a uniform block of whole cells into one larger leaf
    // when it compacts; everything downstream assumes an entry is one cell.
    let _g = serial();
    let x0 = 4096 * 7;
    let mut corners = Vec::new();
    for dz in 0..2 {
        for dy in 0..2 {
            for dx in 0..2 {
                corners.push(corner(x0 + dx, dy, dz));
            }
        }
    }
    for c in &corners {
        place(&c, DECOR_SIZE, PlaceableCube::Block(Block::red), false);
    }
    for c in &corners {
        let found = in_cell(c);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].size, 1.0);
    }
    let first = in_cell(&corners[0]).remove(0);
    remove(first.at);
    assert_eq!(in_cell(&corners[0]).len(), 0);
    assert_eq!(in_cell(&corners[7]).len(), 1);
}

#[test]
fn object_store_band_tree_clearing_a_chunk_s_last_object_frees_the_chunk() {
    let _g = serial();
    let c = corner(4096 * 9, 4096 * 2, 3);
    let before = object_store_stats().0;
    place(&c, DECOR_SIZE, PlaceableCube::Block(Block::blue), false);
    assert_eq!(object_store_stats().0, before + 1);
    let entry = in_cell(&c).remove(0);
    remove(entry.at);
    assert_eq!(object_store_stats().0, before);
}

#[test]
fn object_store_entries_a_column_walk_s_chunk_local_corner_agrees_with_the_absolute_one() {
    let _g = serial();
    let x0 = 4096 * 11 + 70; // not on a chunk boundary
    let c = corner(x0, 4096 * 4 + 5, 130);
    place(&c, DECOR_SIZE, PlaceableCube::Block(Block::red), false);
    let col_idx = TO_XY(c.i);
    let base = object_col_idx_of(L3_IDX(col_idx), L2_IDX_IN_L3(col_idx));
    let mut seen = 0;
    each_col_entry(
        base,
        &mut |e| {
            seen += 1;
            assert_eq!(ivec::Z(e.corner().i), 130);
            assert_eq!(e.lx, (x0 % 64) as f64);
            assert_eq!(e.ly, 5.0);
            assert_eq!(e.lz, 130.0);
        },
        false,
    );
    assert_eq!(seen, 1);
}

#[test]
fn object_store_cells_occupancy_follows_placement_and_removal_and_air_does_not_occupy() {
    let _g = serial();
    let c = corner(4096 * 13, 0, 2);
    let [ix, iy, iz] = cell_index(&c);
    assert!(!cell_is_occupied(ix, iy, iz));
    place(&c, DECOR_SIZE, PlaceableCube::Block(Block::red), false);
    assert!(cell_is_occupied(ix, iy, iz));
    let entry = get_objects_in_cell(ix, iy, iz).remove(0);
    remove(entry.at);
    assert!(!cell_is_occupied(ix, iy, iz));
    let air = corner(4096 * 13, 0, 5);
    place_air(&air, DECOR_SIZE);
    let [ax, ay, az] = cell_index(&air);
    assert!(!cell_is_occupied(ax, ay, az));
}

#[test]
fn object_store_revisions_a_column_s_revisions_move_on_its_own_changes_on_the_right_channel() {
    let _g = serial();
    let c = corner(100_020, 0, 0);
    let col_idx = L2_IDX(TO_XY(c.i));
    let elsewhere = L2_IDX(TO_XY(ivec::gather(1_000_000, 1_000_000, 0)));
    let (objects0, decor0, other0) = (object_col_revision(col_idx), decor_col_revision(col_idx), object_col_revision(elsewhere));
    place_decor(&c, DECOR_SIZE, DecorId::tree);
    assert!(decor_col_revision(col_idx) > decor0, "decor channel moved");
    assert_eq!(object_col_revision(elsewhere), other0, "another col_idx did not");
    let objects1 = object_col_revision(col_idx);
    let solid = corner(100_021, 0, 0);
    place(&solid, DECOR_SIZE, PlaceableCube::Block(Block::red), false);
    assert!(
        object_col_revision(L2_IDX(TO_XY(solid.i))) > if L2_IDX(TO_XY(solid.i)) == col_idx { objects1 } else { objects0 },
        "objects channel moved"
    );
}

#[test]
fn object_store_persistence_a_save_is_placement_records_per_page_and_loading_it_puts_every_object_back_where_it_was() {
    let _g = serial();
    let a = corner(5000, 7000, 12);
    let b = PVF3 {
        i: ivec::gather(5001, 7000, 12),
        f: fvec::gather(fvec::from_real(0.25), 0, 0),
    };
    place(&a, 1.0, PlaceableCube::Block(Block::red), false);
    place(&b, 0.25, PlaceableCube::Block(Block::blue), false);
    place_decor(
        &PVF3 {
            i: ivec::gather(5000, 7001, 12),
            f: 0,
        },
        DECOR_SIZE,
        DecorId::tree,
    );
    let shape = |o: &OccupiedEntry| (o.i, o.f, o.size.to_bits(), matches!(o.kind, EntryKind::Decor { .. }));
    let before: Vec<_> = get_objects_near(&a, 1.0).iter().map(shape).collect();
    assert_eq!(before.len(), 3);
    let blob = encode_objects().unwrap();
    for o in get_objects_near(&a, 1.0) {
        remove(o.at);
    }
    assert_eq!(get_objects_near(&a, 1.0).len(), 0);
    decode_objects(&blob).unwrap();
    assert_eq!(get_objects_near(&a, 1.0).iter().map(shape).collect::<Vec<_>>(), before);
    assert_eq!(encode_objects().unwrap(), blob);
}

fn entries(store: &Store, col_idx: IVec) -> Vec<(i32, u32, u32)> {
    let Some(lanes) = store.chunk_lanes(col_idx) else { return Vec::new() };
    // SAFETY: the lanes are the chunk's own, read before any change.
    unsafe {
        let n = lanes.count as usize;
        let b = core::slice::from_raw_parts(lanes.band_at, n);
        let p = core::slice::from_raw_parts(lanes.pos_at, n);
        let s = core::slice::from_raw_parts(lanes.state_at, n);
        (0..n).map(|k| (b[k], p[k], s[k])).collect()
    }
}

#[test]
fn a_write_is_read_back_and_derived_as_one_entry() {
    let mut store = Store::new();
    store.write(7, 0, [8, 16, 24], CELL_LEVEL, 3);
    assert_eq!(store.read(7, 0, [8, 16, 24]), 3);
    assert_eq!(store.read(7, 0, [15, 23, 31]), 3, "a cell leaf covers its eight steps");
    assert_eq!(store.read(7, 0, [16, 16, 24]), VOID);
    store.reindex(7);
    assert_eq!(entries(&store, 7), vec![(0, pack_pos(8, 16, 24, CELL_LEVEL), 3)]);
    let cell = store.cell(7, 1, 2, 3).expect("the cell is indexed");
    assert_eq!((cell.start, cell.count, cell.solid), (0, 1, 1));
    let (_, n) = store.take_touched();
    assert_eq!(n, 1);
}

#[test]
fn a_void_write_prunes_the_band_and_the_chunk() {
    let mut store = Store::new();
    store.write(7, 2, [0, 0, 0], MIN_LEVEL, AIR);
    store.reindex(7);
    assert_eq!(store.chunk_count(), 1);
    let cell = store.cell(7, 0, 0, 2 * BAND_CELLS as i32).expect("the cell is indexed");
    assert_eq!(cell.solid, 0, "AIR does not occupy");
    store.write(7, 2, [0, 0, 0], MIN_LEVEL, VOID);
    store.reindex(7);
    assert_eq!((store.chunk_count(), store.band_count(), store.cell_count()), (0, 0, 0));
    assert_eq!(entries(&store, 7), vec![]);
    let (at, n) = store.take_touched();
    assert_eq!(n, 1, "the closed chunk's cell is reported once");
    // SAFETY: the rows were just written and stay until the next change.
    let row = unsafe { core::slice::from_raw_parts(at, 3) };
    assert_eq!(row, &[0, 0, 2 * BAND_CELLS as i32]);
}

#[test]
fn eight_cells_of_one_colour_merge_and_come_back_as_eight_entries() {
    let mut store = Store::new();
    for z in 0..2 {
        for y in 0..2 {
            for x in 0..2 {
                store.write(1, -1, [x * 8, y * 8, z * 8], CELL_LEVEL, 5);
            }
        }
    }
    store.reindex(1);
    let found = entries(&store, 1);
    assert_eq!(found.len(), 8);
    assert!(found.iter().all(|&(b, p, s)| b == -1 && s == 5 && (p >> (3 * POS_BITS)) == (CELL_LEVEL - MIN_LEVEL) as u32));
    assert_eq!(store.cell_count(), 8);
    assert_eq!(store.cell(1, 1, 1, -(BAND_CELLS as i32) + 1).map(|c| c.count), Some(1));
}

#[test]
fn closing_a_chunk_moves_the_last_into_its_place() {
    let mut store = Store::new();
    store.write(10, 0, [0, 0, 0], CELL_LEVEL, 1);
    store.write(20, 0, [0, 0, 0], CELL_LEVEL, 1);
    store.write(30, 0, [0, 0, 0], CELL_LEVEL, 1);
    store.write(10, 0, [0, 0, 0], CELL_LEVEL, VOID);
    assert_eq!((store.chunk_key(0), store.chunk_key(1)), (30, 20));
    assert_eq!(store.read(30, 0, [0, 0, 0]), 1);
    assert_eq!(store.read(20, 0, [0, 0, 0]), 1);
}
