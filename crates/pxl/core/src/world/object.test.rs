// pxl_core/world/object.test.rs

use super::*;

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
