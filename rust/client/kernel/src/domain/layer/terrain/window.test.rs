// pxl-kernel/domain/layer/terrain/window.test.rs
//
// The windows as a test holds them: about a centre, each square of the
// tiles given present at once, its cells as a GPU would hand them back.

use super::*;
use pxl_spec_core::ground::word;
use pxl_spec_core::overwrite::Lanes;

/// A tile as `hold` takes it: a cell's own look and its height, row by row
/// from its first, and its lines.
pub(crate) type HeldTile = (
    Tile,
    Vec<GroundId>,
    Vec<i16>,
    pxl_spec::bake::separation::Separation,
);

/// What `hold` leaves held: the squares' words up to their last that is
/// not zero, the pool's families taken as records (pxl-spec-core
/// `way::tree::lay`), the slots and the berths.
struct Built {
    cells: Vec<u32>,
    records: Vec<u32>,
    slots: Vec<Slot>,
    berths: Vec<Berth>,
}

/// What windows are built over: a centre and the tiles held.
type Over = ([f64; 2], Vec<HeldTile>);

/// The windows built this run, by what they were built over: the same are
/// built once, and each `hold` after takes a copy.
static BUILT: std::sync::Mutex<Vec<(Over, Built)>> = std::sync::Mutex::new(Vec::new());

const CELLS_WORDS: usize = (SQUARES * CELLS_LANE_BYTES / 4) as usize;
const POOL_WORDS: usize = 16 * pxl_spec_core::way::tree::POOL_FAMILIES as usize;

/// The pool's mirror holding `records`, a square's most at a time.
fn mirrored(records: &[u32]) {
    use pxl_spec_core::way::tree::{RECORD_WORDS, SQUARE_FAMILIES};
    let m = mirror();
    m.clear();
    for families in records.chunks((SQUARE_FAMILIES * RECORD_WORDS) as usize) {
        let mut laid = vec![(families.len() / RECORD_WORDS as usize) as u32];
        laid.extend_from_slice(families);
        assert_eq!(
            m.lay(&laid),
            Ok(()),
            "the test's pool past the mirror's cap"
        );
    }
}

/// The eye's and the focus's windows about `centre`, in units, X then Y,
/// each square the tiles `held` cover present in its berth, the focus's
/// windows the eye's finest two again: per tile, a cell's own look
/// and its height, row by row from its first, and its lines; each square's
/// columns, its tops and its ways' spans as its fill keeps them
/// (`fill_columns`), and its levels as a GPU folds them (pxl-spec-core
/// `fill::level`), read back. Built once a run for a centre and its tiles
/// (`BUILT`); each hold is its own copy, whatever a test does to it.
pub(crate) fn hold(centre: [f64; 2], held: &[HeldTile]) {
    let mut built = BUILT.lock().unwrap_or_else(|e| e.into_inner());
    let k = match built
        .iter()
        .position(|((c, h), _)| *c == centre && h.as_slice() == held)
    {
        Some(k) => k,
        None => {
            let records = build(centre, held);
            let upto = |words: &[u32]| {
                words[..words.iter().rposition(|&w| w != 0).map_or(0, |i| i + 1)].to_vec()
            };
            let b = Built {
                cells: upto(planes().cells),
                records,
                slots: windows().slots.to_vec(),
                berths: windows().berths.to_vec(),
            };
            built.push(((centre, held.to_vec()), b));
            built.len() - 1
        }
    };
    let b = &built[k].1;
    let of = |words: &[u32], len: usize| {
        let mut lane = vec![0; len];
        lane[..words.len()].copy_from_slice(words);
        lane.leak()
    };
    PLANES.set(Planes {
        cells: of(&b.cells, CELLS_WORDS),
    });
    mirrored(&b.records);
    WINDOWS_STATE.set(fresh());
    windows().slots.copy_from_slice(&b.slots);
    windows().berths.copy_from_slice(&b.berths);
    fold(windows());
}

/// The square at `cell` holding cell `g` absent, a fill on its way to it,
/// and the windows folded without it, as a turn's step leaves them before
/// it lands: its berth and that fill's ticket, which `on_landed` takes.
pub(crate) fn unland(cell: Zoom, g: [i32; 2]) -> (u32, u32) {
    let w = windows();
    let b = berth_of(w, cell, window::square_of(g).0).expect("held");
    (w.berths[b as usize].present, w.berths[b as usize].ticket) = (false, 7);
    for i in of_slot(b % SLOTS) {
        show(w, i);
    }
    fold(w);
    (b, 7)
}

/// The device gone, as a turn's step finds it: every square's cells given
/// up (`berth`).
pub(crate) fn lose() {
    berth(windows(), true);
}

/// `hold`'s windows, built: the families their trees took, as records.
fn build(centre: [f64; 2], held: &[HeldTile]) -> Vec<u32> {
    PLANES.set(Planes {
        cells: vec![0; CELLS_WORDS].leak(),
    });
    // The trees built into a pool in one lane, as the GPU's.
    let mut pool = vec![0; POOL_WORDS];
    // The pool's families, each square's tree taken after the one before's, as
    // the GPU's stack hands them, family 0 none's.
    let mut next = 1;
    WINDOWS_STATE.set(fresh());
    let w = windows();
    for win in 0..WINDOWS {
        let (centre_of, cell) = layout(win);
        if centre_of == Centre::Destination {
            continue;
        }
        let first = window::origin(cell, cell_at(cell, centre));
        w.slots[(SQUARES + win) as usize] = Slot {
            square: first,
            present: 1,
            ticket: 0,
        };
        for s in 0..SLOTS {
            w.slots[(win * SLOTS + s) as usize].square = wanted(cell, first, s);
        }
    }
    berth(w, false);
    for b in 0..SQUARES {
        let Berth { cell, square, .. } = w.berths[b as usize];
        if w.berths[b as usize].is_free() {
            continue;
        }
        let (t, place) = tile_of(cell, square);
        let Some((_, looks_of, heights, separation)) = held.iter().find(|(p, _, _, _)| *p == t)
        else {
            continue;
        };
        let side = GROUND.tile_cells();
        let at = |k: u32| {
            let [x, y] = window::of_index(k);
            ((place[1] * SQUARE + y) * side + place[0] * SQUARE + x) as usize
        };
        let (mut segments, mut lines) = (
            vec![(0, 0); window::WAYS_SQUARE_SEGMENTS as usize],
            vec![0; window::LINES_WORDS as usize],
        );
        let n = pxl_spec::bake::tile::terrain::square_lines(
            t,
            separation,
            place,
            &mut segments,
            &mut lines,
        );
        let laid = pxl_spec::bake::tile::terrain::Laid {
            cell,
            place,
            lines: &lines[..n],
            looks: |k: u32| word([u32::from(looks_of[at(k)]); 4]),
            height: |c: [i32; 2]| {
                i32::from(
                    heights[at(window::index_of([
                        c[0].clamp(0, SQUARE as i32 - 1) as u32,
                        c[1].clamp(0, SQUARE as i32 - 1) as u32,
                    ]))],
                )
            },
        };
        let (mut lane, mut scratch) = (
            vec![0u32; window::SQUARE_WORDS as usize],
            vec![0u32; pxl_spec_core::way::tree::SCRATCH_WORDS as usize],
        );
        fill_columns(
            &laid,
            |k| i32::from(heights[at(k)]),
            &mut lane,
            &mut scratch,
        );
        for k in 0..window::LAID_WORDS {
            let half = |j: u32| window::laid_half(u32::from(looks_of[at(2 * k + j)]), 0);
            lane[(window::LAID_AT + k) as usize] = half(0) | half(1) << 16;
        }
        for j in 0..=LEVELS {
            for y in 0..SQUARE >> j {
                for x in 0..SQUARE >> j {
                    pxl_spec_core::fill::level(
                        &mut lane,
                        0,
                        &pxl_spec_core::fill::Scratch {
                            words: &scratch,
                            at: 0,
                        },
                        cell,
                        j,
                        [x, y],
                    );
                }
            }
        }
        // Its ways' tree into the pool's mirror, its root word naming it.
        let [taken, _, root] = pxl_spec_core::fill::tree(&lane, &scratch, cell, &mut pool, next);
        assert_ne!(taken, u32::MAX, "a square's tree past the pool");
        lane[window::ROOT_AT as usize] = root;
        next += taken;
        let at = (b * CELLS_LANE_BYTES / 4) as usize;
        planes().cells[at..at + window::BACK_WORDS as usize].copy_from_slice(&lane);
        w.berths[b as usize].present = true;
    }
    for i in 0..SQUARES {
        show(w, i);
    }
    // Every family taken, a record each: its id, its heap words, its data.
    let p = pxl_spec_core::way::tree::Read(&pool);
    let mut records = Vec::new();
    for b in 1..next {
        records.push(b);
        records.extend((0..8).map(|k| p.heap(8 * b + k)));
        records.extend((0..8).map(|k| p.data(8 * b + k)));
    }
    records
}

/// A square's columns as a GPU's fill writes them (pxl-spec-core
/// `fill::heights`, `fill::spans`), over cells whose tops are given
/// (`top`) rather than drawn off samples: each column's word, its top, into
/// `lane`; and the spans of its columns and its ring off its lines
/// (`fill::keep`) into its `scratch`.
fn fill_columns(
    sq: &impl window::Square,
    top: impl Fn(u32) -> i32,
    lane: &mut [u32],
    scratch: &mut [u32],
) {
    for i in 0..SQUARE_CELLS {
        lane[(window::COLUMNS_AT + i) as usize] = top(i) as u32 & 0xffff;
    }
    for i in 0..window::SPANS_COLUMNS {
        pxl_spec_core::fill::keep(sq, scratch, 0, i);
    }
}

/// The eye's windows laid out about `eye` and the focus's about `focus`, in
/// units, X then Y, as `step` lays a window that moved, no destination's;
/// their squares berthed (`berth`), what was held before kept.
fn lay(w: &mut Windows, eye: [f64; 2], focus: [f64; 2]) {
    for win in 0..WINDOWS {
        let (centre_of, cell) = layout(win);
        let p = match centre_of {
            Centre::Eye => eye,
            Centre::Focus => focus,
            Centre::Destination => continue,
        };
        let first = window::origin(cell, cell_at(cell, p));
        w.slots[(SQUARES + win) as usize] = Slot {
            square: first,
            present: 1,
            ticket: 0,
        };
        for s in 0..SLOTS {
            w.slots[(win * SLOTS + s) as usize].square = wanted(cell, first, s);
        }
    }
    berth(w, false);
}

/// Every square held present, as if each had landed, its cells as they
/// came back, all zero unless a test held its own (`hold`): the windows
/// fold over them wherever they are laid again (`berth`).
fn land_all(w: &mut Windows) {
    PLANES.get(|| Planes {
        cells: vec![0; CELLS_WORDS].leak(),
    });
    for b in w.berths.iter_mut().filter(|b| !b.is_free()) {
        b.present = true;
    }
    for i in 0..SQUARES {
        show(w, i);
    }
}

/// A leaf cell's middle, in units.
fn leaf(c: [f64; 2]) -> [f64; 2] {
    c.map(|v| (v + 0.5) * units(LEAF_CELL))
}

/// The berths holding a square at `cell`.
fn held_at(w: &Windows, cell: Zoom) -> usize {
    w.berths.iter().filter(|b| b.cell == cell).count()
}

/// The square window `win` wants in slot `s`.
fn want(w: &Windows, win: u32, s: u32) -> [i32; 2] {
    w.slots[(win * SLOTS + s) as usize].square
}

const CENTRE: [f64; 2] = [37.0 * 4096.0 + 100.0, 11.0 * 4096.0 + 200.0];

#[test]
fn shared_square_held_once() {
    let _g = crate::sys::serial();
    WINDOWS_STATE.set(fresh());
    let w = windows();
    lay(w, leaf(CENTRE), leaf(CENTRE));
    assert_eq!(
        w.berths.iter().filter(|b| !b.is_free()).count(),
        (EYE_WINDOWS * SLOTS) as usize,
        "the focus's windows take no berth"
    );
    for focus in EYE_WINDOWS..EYE_WINDOWS + 2 {
        let cell = layout(focus).1;
        let eye = cell - FIRST_CELL;
        for s in 0..SLOTS {
            assert_eq!(want(w, focus, s), want(w, eye, s));
            assert_eq!(
                berth_of(w, cell, want(w, focus, s)),
                Some(eye * SLOTS + s),
                "in the eye's own berth"
            );
        }
    }
    land_all(w);
    for focus in EYE_WINDOWS..EYE_WINDOWS + 2 {
        for s in 0..SLOTS {
            let row = w.slots[(focus * SLOTS + s) as usize];
            assert!(
                row.holds(row.square) && row.berth() == (layout(focus).1 - FIRST_CELL) * SLOTS + s,
                "the focus's rows name the eye's berths"
            );
        }
    }
}

#[test]
fn left_square_kept() {
    let _g = crate::sys::serial();
    WINDOWS_STATE.set(fresh());
    let w = windows();
    let (eye, focus) = (LEAF_CELL - FIRST_CELL, EYE_WINDOWS + 1);
    lay(w, leaf(CENTRE), leaf(CENTRE));
    land_all(w);
    // The eye a square on at the leaves: its window there leaves a column
    // the focus's still wants, and wants a column the focus's does not.
    let on = leaf([CENTRE[0] + f64::from(SQUARE), CENTRE[1]]);
    lay(w, on, leaf(CENTRE));
    assert_eq!(held_at(w, LEAF_CELL), (SLOTS + window::ACROSS) as usize);
    for s in 0..SLOTS {
        let q = want(w, focus, s);
        assert!(
            Windowed.present(LEAF_CELL, q).1,
            "the focus's squares stay present"
        );
        assert!(w.slots[(focus * SLOTS + s) as usize].holds(q));
    }
    let new: Vec<u32> = (0..SLOTS)
        .filter(|&s| !w.slots[(eye * SLOTS + s) as usize].holds(want(w, eye, s)))
        .collect();
    assert_eq!(
        new.len(),
        window::ACROSS as usize,
        "the eye's new column alone is absent"
    );
    for &s in &new {
        let b = berth_of(w, LEAF_CELL, want(w, eye, s)).unwrap();
        assert!(
            w.berths[b as usize].is_absent() && b != eye * SLOTS + s,
            "in a free berth, the eye's own still holding the focus's square"
        );
    }
    // Back where it was, the eye finds the column it left present.
    land_all(w);
    lay(w, leaf(CENTRE), leaf(CENTRE));
    for s in 0..SLOTS {
        assert!(
            w.slots[(eye * SLOTS + s) as usize].holds(want(w, eye, s)),
            "not filled again"
        );
    }
    assert_eq!(
        held_at(w, LEAF_CELL),
        SLOTS as usize,
        "the column no window wants is given up"
    );
}

#[test]
fn apart_windows_own_berths() {
    let _g = crate::sys::serial();
    WINDOWS_STATE.set(fresh());
    let w = windows();
    lay(
        w,
        leaf(CENTRE),
        leaf([CENTRE[0] + 100.0 * f64::from(SQUARE), CENTRE[1]]),
    );
    for win in 0..EYE_WINDOWS + 2 {
        for s in 0..SLOTS {
            assert_eq!(
                berth_of(w, layout(win).1, want(w, win, s)),
                Some(win * SLOTS + s)
            );
        }
    }
}

#[test]
fn landing_shows_everywhere() {
    let _g = crate::sys::serial();
    // The fold after the landing reads the squares' cells.
    PLANES.set(Planes {
        cells: vec![0; CELLS_WORDS].leak(),
    });
    WINDOWS_STATE.set(fresh());
    let w = windows();
    let (eye, focus) = (LEAF_CELL - FIRST_CELL, EYE_WINDOWS + 1);
    lay(w, leaf(CENTRE), leaf(CENTRE));
    let q = want(w, eye, 0);
    let b = berth_of(w, LEAF_CELL, q).unwrap();
    w.berths[b as usize].ticket = 7;
    for i in of_slot(0) {
        show(w, i);
    }
    on_landed(b, 6, Landing::Refused);
    assert_eq!(
        w.berths[b as usize].ticket, 7,
        "another fill's landing lays nothing"
    );
    on_landed(b, 7, Landing::Built(&[0]));
    refold();
    for win in [eye, focus] {
        let row = w.slots[(win * SLOTS) as usize];
        assert!(row.holds(q) && row.berth() == b);
    }
    // Refused while a window wants it, then given up with the last.
    w.berths[b as usize] = Berth {
        present: false,
        ticket: REFUSED,
        ..w.berths[b as usize]
    };
    let far = leaf([CENTRE[0] + 100.0 * f64::from(SQUARE), CENTRE[1]]);
    lay(w, leaf(CENTRE), far);
    assert_eq!(
        w.berths[b as usize].ticket, REFUSED,
        "the eye still wants it"
    );
    assert!(
        !berth_wants(w, tile_of(LEAF_CELL, q).0, b),
        "nor is its tile kept for it"
    );
    lay(w, far, far);
    assert_eq!(berth_of(w, LEAF_CELL, q), None);
}

#[test]
fn heap_wait_rehanded() {
    use crate::sys::boot::set_heap_stat;
    use core::sync::atomic::{AtomicU32, Ordering};
    use xpute_guest::mem::heap::HeapStat;
    static RELEASES: AtomicU32 = AtomicU32::new(0);
    let _g = crate::sys::serial();
    set_heap_stat(|| HeapStat {
        releases: RELEASES.load(Ordering::Relaxed),
        ..HeapStat::default()
    });
    WINDOWS_STATE.set(fresh());
    let w = windows();
    lay(w, leaf(CENTRE), leaf(CENTRE));
    let (eye, focus) = (LEAF_CELL - FIRST_CELL, EYE_WINDOWS + 1);
    let q = want(w, eye, 0);
    let b = berth_of(w, LEAF_CELL, q).unwrap();
    // As `on_landed` leaves it where the heap refused the mirror its chunk.
    (w.berths[b as usize].ticket, w.heap) = (WAITING, Some(Refused::now()));
    assert!(!wake(w), "no page given back: it waits");
    lay(w, leaf(CENTRE), leaf(CENTRE));
    assert_eq!(
        w.berths[b as usize].ticket, WAITING,
        "unhanded while it waits"
    );
    assert!(
        !berth_wants(w, tile_of(LEAF_CELL, q).0, b),
        "nor its tile kept for it"
    );
    RELEASES.fetch_add(1, Ordering::Relaxed);
    assert!(wake(w) && w.heap.is_none());
    berth(w, false);
    assert!(
        w.berths[b as usize].is_absent() && berth_wants(w, tile_of(LEAF_CELL, q).0, b),
        "absent, to be handed again"
    );
    for win in [eye, focus] {
        assert_eq!(
            w.slots[(win * SLOTS) as usize].ticket,
            0,
            "its rows show it"
        );
    }
    set_heap_stat(HeapStat::default);
}
