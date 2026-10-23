// pxl-kernel/layer/terrain/window.test.rs
//
// The windows as a test holds them: about a centre, each square of the
// tiles given present at once, its cells as a GPU would hand them back.

use super::*;
use pxl_spec_core::ground::word;

/// A tile as `hold` takes it: a cell's own look and its height, row by row
/// from its first, and its lines.
pub(crate) type HeldTile = (Tile, Vec<GroundId>, Vec<i16>, pxl_spec::bake::separation::Separation);

/// The eye's and the focus's windows about `centre`, in units, X then Y,
/// each square the tiles `held` cover present: per tile, a cell's own look
/// and its height, row by row from its first, and its lines; each square's
/// columns, its tops and its ways' runs as its fill writes them
/// (`fill_columns`), and its levels as a GPU folds them (pxl-spec-core
/// `fill::level`), read back.
pub(crate) fn hold(centre: [f64; 2], held: &[HeldTile]) {
    PLANES.set(Planes {
        cells: vec![0; (SQUARES * CELLS_LANE_BYTES / 4) as usize].leak(),
        joined: vec![0; window::EYE_JOINED_WORDS as usize].leak(),
        records: vec![0; (window::READS * pxl_spec_core::ways::tree::RECORDS_BYTES / 4) as usize].leak(),
        pool: vec![0; 16 * pxl_spec_core::ways::tree::POOL_BLOCKS as usize].leak(),
    });
    // The pool's blocks, each square's tree taken after the one before's, as
    // the GPU's stack hands them, block 0 none's.
    let mut next = 1;
    WINDOWS_STATE.set(fresh());
    let w = windows();
    for win in 0..WINDOWS {
        let (centre_of, cell) = layout(win);
        if centre_of == Centre::Destination {
            continue;
        }
        let first = window::origin(cell, cell_at(cell, centre));
        w.slots[(SQUARES + win) as usize] = Slot { square: first, present: 1, ticket: 0 };
        for s in 0..SLOTS {
            let i = win * SLOTS + s;
            let square = wanted(cell, first, s);
            w.slots[i as usize].square = square;
            let (t, place) = tile_of(cell, square);
            let Some((_, looks_of, heights, separation)) = held.iter().find(|(p, _, _, _)| *p == t) else {
                continue;
            };
            let side = GROUND.tile_cells();
            let at = |k: u32| {
                let [x, y] = window::of_index(k);
                ((place[1] * SQUARE + y) * side + place[0] * SQUARE + x) as usize
            };
            let (mut scratch, mut lines) = (vec![(0, 0); window::WAYS_SQUARE_SEGMENTS as usize], vec![0; window::LINES_WORDS as usize]);
            let n = pxl_spec::bake::tile::terrain::square_lines(t, separation, place, &mut scratch, &mut lines);
            let laid = pxl_spec::bake::tile::terrain::Laid {
                cell,
                place,
                lines: &lines[..n],
                looks: |k: u32| word([u32::from(looks_of[at(k)]); 4]),
                height: |c: [i32; 2]| i32::from(heights[at(window::index_of([c[0].clamp(0, SQUARE as i32 - 1) as u32, c[1].clamp(0, SQUARE as i32 - 1) as u32]))]),
            };
            let (mut lane, mut scratch) = (vec![0u32; window::SQUARE_WORDS as usize], vec![0u32; pxl_spec_core::ways::tree::SCRATCH_WORDS as usize]);
            fill_columns(&laid, |k| i32::from(heights[at(k)]), &mut lane, &mut scratch);
            for k in 0..window::LAID_WORDS {
                let half = |j: u32| window::laid_half(u32::from(looks_of[at(2 * k + j)]), 0);
                lane[(window::LAID_AT + k) as usize] = half(0) | half(1) << 16;
            }
            for j in 0..=LEVELS {
                for y in 0..SQUARE >> j {
                    for x in 0..SQUARE >> j {
                        pxl_spec_core::fill::level(&mut lane, 0, &pxl_spec_core::fill::Scratch { words: &scratch, at: 0 }, cell, j, [x, y]);
                    }
                }
            }
            // Its ways' tree into the pool's mirror, its root word naming it.
            let of = pxl_spec_core::ways::tree::Of {
                cell,
                at: [wrap_index(cell - SQUARE_SHIFT, square[0]), square[1]],
            };
            let [taken, _] = pxl_spec_core::fill::tree(&lane, &scratch, of, planes().pool, next);
            assert_ne!(taken, u32::MAX, "a square's tree past the pool");
            lane[window::ROOT_AT as usize] = 8 * next;
            next += taken;
            let at = (i * CELLS_LANE_BYTES / 4) as usize;
            planes().cells[at..at + window::BACK_WORDS as usize].copy_from_slice(&lane);
            w.slots[i as usize].present = 1;
        }
    }
}

/// A square's columns as a GPU's fill writes them (pxl-spec-core
/// `fill::heights`, `fill::ways`), over cells whose tops are given
/// (`top`) rather than drawn off samples: each column's word, its top, into
/// `lane`; and its spans and those beside it off its lines (`ways::around`)
/// read along z as runs (`ways::runs`), into its `scratch` from its first,
/// none past `RUNS_SQUARE`.
fn fill_columns(sq: &impl window::Square, top: impl Fn(u32) -> i32, lane: &mut [u32], scratch: &mut [u32]) {
    use pxl_spec_core::window::{COLUMNS_AT, FIRSTS_AT, RUNS_AT, RUNS_SQUARE};
    let mut runs = 0;
    for i in 0..SQUARE_CELLS {
        lane[(COLUMNS_AT + i) as usize] = top(i) as u32 & 0xffff;
        scratch[(FIRSTS_AT + i) as usize] = runs;
        let [x, y] = window::of_index(i);
        let around = pxl_spec_core::ways::around(sq, [x as i32, y as i32]).0;
        runs += pxl_spec_core::ways::runs(&around, sq.cell(), |k, w| {
            if runs + k < RUNS_SQUARE {
                scratch[(RUNS_AT + runs + k) as usize] = w;
            }
        });
    }
    scratch[(FIRSTS_AT + SQUARE_CELLS) as usize] = runs;
}
