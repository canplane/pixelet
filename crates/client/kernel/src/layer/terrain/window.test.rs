// pxl-kernel/layer/terrain/window.test.rs
//
// The windows as a test holds them: about a centre, each square of the
// tiles given present at once, its cells as a GPU would hand them back.

use super::*;

/// A tile as `hold` takes it: a cell's own look and its height, row by row
/// from its first, and its lines.
pub(crate) type HeldTile = (Tile, Vec<GroundId>, Vec<i16>, pxl_spec::bake::separation::Separation);

/// The eye's and the focus's windows about `centre`, in units, X then Y,
/// each square the tiles `held` cover present: per tile, a cell's own look
/// and its height, row by row from its first, and its lines; each square's
/// columns, its tops and its ways' runs as its fill writes them
/// (`fill_columns`), and its levels as a GPU folds them (pxl-portable
/// `fill::level`), read back.
pub(crate) fn hold(centre: [f64; 2], held: &[HeldTile]) {
    PLANES.set(Planes {
        cells: vec![0; (SQUARES * CELLS_LANE_BYTES) as usize].leak(),
        looks: vec![0; (SQUARES * LOOK_LANE_BYTES) as usize].leak(),
        back: vec![0; SQUARES as usize].leak(),
    });
    WINDOWS_STATE.set(fresh());
    let w = windows();
    for win in 0..WINDOWS {
        let (centre_of, cell) = layout(win);
        if centre_of == Centre::Destination {
            continue;
        }
        let first = window::origin(cell, cell_at(cell, centre));
        w.windows[win as usize].first = Some(first);
        for s in 0..SLOTS {
            let i = win * SLOTS + s;
            let square = wanted(cell, first, s);
            w.slots[i as usize].square = square;
            let (t, place) = tile_of(cell, square);
            let Some((_, looks_of, heights, separation)) = held.iter().find(|(p, _, _, _)| *p == t) else {
                continue;
            };
            let side = GROUND.tile_cells();
            let at = |k: u32| ((place[1] * SQUARE + k / SQUARE) * side + place[0] * SQUARE + k % SQUARE) as usize;
            for (k, l) in looks(i).iter_mut().enumerate() {
                *l = looks_of[at(k as u32)];
            }
            let (mut scratch, mut lines) = (vec![(0, 0); window::WAYS_SQUARE_SEGMENTS as usize], vec![0; window::LINES_WORDS as usize]);
            let n = pxl_spec::bake::tile::terrain::square_lines(t, separation, place, &mut scratch, &mut lines);
            let laid = pxl_spec::bake::tile::terrain::Laid {
                cell,
                place,
                lines: &lines[..n],
                looks: |k: u32| word([u32::from(looks_of[at(k)]); 4]),
                height: |c: [i32; 2]| i32::from(heights[at((c[1].clamp(0, SQUARE as i32 - 1) as u32) * SQUARE + c[0].clamp(0, SQUARE as i32 - 1) as u32)]),
            };
            let mut lane = vec![0u32; window::SQUARE_WORDS as usize];
            fill_columns(&laid, |k| i32::from(heights[at(k)]), &mut lane);
            for j in 0..=LEVELS {
                for y in 0..SQUARE >> j {
                    for x in 0..SQUARE >> j {
                        pxl_portable::fill::level(&mut lane, 0, cell, j, [x, y]);
                    }
                }
            }
            let c = &mut planes().cells[(i * CELLS_LANE_BYTES) as usize..];
            for (k, v) in lane[window::COLUMNS_AT as usize..(window::COLUMNS_AT + window::BACK_WORDS) as usize].iter().enumerate() {
                c[4 * k..4 * k + 4].copy_from_slice(&v.to_le_bytes());
            }
            w.slots[i as usize].state = State::Present;
        }
    }
}

/// A square's columns as a GPU's fill writes them (pxl-portable
/// `fill::heights`, `fill::ways`), over cells whose tops are given
/// (`top`) rather than drawn off samples: each column's word, its top and
/// its first run, and its spans off its lines (`ways::column`) read along z
/// as runs (`ways::runs`), into `lane`, none past `RUNS_SQUARE`.
fn fill_columns(sq: &impl window::Square, top: impl Fn(u32) -> i32, lane: &mut [u32]) {
    use pxl_portable::window::{COLUMNS_AT, RUNS_AT, RUNS_SQUARE};
    let mut runs = 0;
    for i in 0..SQUARE_CELLS {
        lane[(COLUMNS_AT + i) as usize] = top(i) as u32 & 0xffff | runs << 16;
        let spans = pxl_portable::ways::column(sq, [(i % SQUARE) as i32, (i / SQUARE) as i32]).0;
        runs += pxl_portable::ways::runs(&spans, sq.cell(), |k, w| {
            if runs + k < RUNS_SQUARE {
                lane[(RUNS_AT + runs + k) as usize] = w;
            }
        });
    }
    lane[(COLUMNS_AT + SQUARE_CELLS) as usize] = runs << 16;
}
