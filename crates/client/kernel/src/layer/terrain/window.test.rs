// pxl-kernel/layer/terrain/window.test.rs
//
// The windows as a test holds them: about a centre, each square of the
// tiles given present at once, its cells as a GPU would hand them back.

use super::*;

/// A tile as `hold` takes it: a cell's own look and its height, row by row
/// from its first, and its ways' bound.
pub(crate) type HeldTile = (Tile, Vec<GroundId>, Vec<i16>, Vec<[i16; 2]>);

/// The eye's and the focus's windows about `centre`, in units, X then Y,
/// each square the tiles `held` cover present: per tile, a cell's own look
/// and its height, row by row from its first, and its ways' bound; each
/// square's levels joined over its cells and its ways as a GPU joins them
/// (pxl-portable `window::join`).
pub(crate) fn hold(centre: [f64; 2], held: &[HeldTile]) {
    PLANES.set(Planes {
        cells: vec![0; (SQUARES * CELLS_LANE_BYTES) as usize].leak(),
        looks: vec![0; (SQUARES * LOOK_LANE_BYTES) as usize].leak(),
        back: vec![0; SQUARES as usize].leak(),
    });
    WINDOWS_STATE.set(Windows {
        windows: core::array::from_fn(|w| Window {
            cell: layout(w as u32).1,
            first: None,
        }),
        slots: [Slot {
            square: [i32::MIN; 2],
            state: State::Absent,
        }; SQUARES as usize],
        ticket: 0,
        table: Vec::new(),
        pinned: Vec::new(),
    });
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
            let Some((_, looks_of, heights, ways)) = held.iter().find(|(p, _, _, _)| *p == t) else { continue };
            let side = GROUND.tile_cells();
            let at = |k: u32| ((place[1] * SQUARE + k / SQUARE) * side + place[0] * SQUARE + k % SQUARE) as usize;
            for (k, l) in looks(i).iter_mut().enumerate() {
                *l = looks_of[at(k as u32)];
            }
            let mut words = vec![0u32; window::BACK_WORDS as usize];
            for k in 0..SQUARE_CELLS {
                words[(k / 2) as usize] |= (heights[at(k)] as u16 as u32) << (16 * (k % 2));
            }
            let from = (window::HEIGHT_WORDS + window::LEVEL_WORDS) as usize;
            window::square_ways(|e| ways[e as usize].map(i32::from), place, &mut words[from..]);
            let write = |words: &[u32]| {
                let c = &mut planes().cells[(i * CELLS_LANE_BYTES) as usize..];
                for (k, v) in words.iter().enumerate() {
                    c[4 * k..4 * k + 4].copy_from_slice(&v.to_le_bytes());
                }
            };
            write(&words);
            w.slots[i as usize].state = State::Present;
            for j in 1..=LEVELS {
                let n = SQUARE >> j;
                for y in 0..n {
                    for x in 0..n {
                        words[(window::HEIGHT_WORDS + window::level_at(j) + y * n + x) as usize] = window::join(&Windowed, cell, i, j, [x, y]) as u32;
                    }
                }
                write(&words);
            }
        }
    }
}
