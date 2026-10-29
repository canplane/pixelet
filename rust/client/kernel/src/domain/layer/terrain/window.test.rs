// pxl-kernel/domain/layer/terrain/window.test.rs
//
// The windows as a test holds them: about a center, each brick of the
// tiles given present at once, its cells as a GPU would hand them back.

use super::*;
use crate::sys;
use pxl_spec::bake::separation::Separation;
use pxl_spec::bake::tile::terrain;
use pxl_spec_core::fill;
use pxl_spec_core::ground::word;
use pxl_spec_core::overwrite::Lanes;
use pxl_spec_core::way::tree;
use pxl_spec_core::way::tree::Read;

/// A tile as `hold` takes it: a cell's own look and its height, row by row
/// from its first, and its lines.
pub(crate) type HeldTile = (Tile, Vec<GroundId>, Vec<i16>, Separation);

/// What `hold` leaves held: the bricks' words up to their last that is
/// not zero, the pool's families taken as records (pxl-spec-core
/// `way::tree::lay`), the entries and the slots.
struct Built {
    cells: Vec<u32>,
    records: Vec<u32>,
    entries: Vec<Entry>,
    slots: Vec<Slot>,
}

/// What windows are built over: a center and the tiles held.
type Over = ([f64; 2], Vec<HeldTile>);

/// The windows built this run, by what they were built over: the same are
/// built once, and each `hold` after takes a copy.
static BUILT: std::sync::Mutex<Vec<(Over, Built)>> = std::sync::Mutex::new(Vec::new());

const CELLS_WORDS: usize = (SLOTS * CELLS_LANE_BYTES / 4) as usize;
const POOL_WORDS: usize = 16 * tree::POOL_FAMILIES as usize;

/// The pool's mirror holding `records`, a brick's most at a time.
fn mirrored(records: &[u32]) {
    use pxl_spec_core::way::tree::{BRICK_FAMILIES, RECORD_WORDS};
    let m = mirror();
    m.clear();
    for families in records.chunks((BRICK_FAMILIES * RECORD_WORDS) as usize) {
        let mut laid = vec![(families.len() / RECORD_WORDS as usize) as u32];
        laid.extend_from_slice(families);
        assert_eq!(
            m.lay(&laid),
            Ok(()),
            "the test's pool past the mirror's cap"
        );
    }
}

/// The eye's and the pivot's windows about `centre`, in units, X then Y,
/// each brick the tiles `held` cover present in its slot, the pivot's
/// windows the eye's finest two again: per tile, a cell's own look
/// and its height, row by row from its first, and its lines; each brick's
/// columns, its tops and its ways' spans as its fill keeps them
/// (`fill_columns`), and its levels as a GPU folds them (pxl-spec-core
/// `fill::level`), read back. Built once a run for a center and its tiles
/// (`BUILT`); each hold is its own copy, whatever a test does to it.
pub(crate) fn hold(center: [f64; 2], held: &[HeldTile]) {
    let mut built = BUILT.lock().unwrap_or_else(|e| e.into_inner());
    let k = match built
        .iter()
        .position(|((c, h), _)| *c == center && h.as_slice() == held)
    {
        Some(k) => k,
        None => {
            let records = build(center, held);
            let upto = |words: &[u32]| {
                words[..words.iter().rposition(|&w| w != 0).map_or(0, |i| i + 1)].to_vec()
            };
            let b = Built {
                cells: upto(planes().cells),
                records,
                entries: windows().entries.to_vec(),
                slots: windows().slots.to_vec(),
            };
            built.push(((center, held.to_vec()), b));
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
    windows().entries.copy_from_slice(&b.entries);
    windows().slots.copy_from_slice(&b.slots);
    fold(windows());
}

/// The brick at `cell` holding cell `g` absent, a fill on its way to it,
/// and the windows folded without it, as a turn's step leaves them before
/// it lands: its slot and that fill's ticket, which `on_landed` takes.
pub(crate) fn unland(cell: Zoom, g: [i32; 2]) -> (u32, u32) {
    let w = windows();
    let b = slot_of(w, cell, window::brick_of(g).0).expect("held");
    (w.slots[b as usize].present, w.slots[b as usize].ticket) = (false, 7);
    for i in of_entry(b % ENTRIES) {
        show(w, i);
    }
    fold(w);
    (b, 7)
}

/// The device gone, as a turn's step finds it: every brick's cells given
/// up (`assign_slots`).
pub(crate) fn lose() {
    assign_slots(windows(), true);
}

/// `hold`'s windows, built: the families their trees took, as records.
fn build(center: [f64; 2], held: &[HeldTile]) -> Vec<u32> {
    PLANES.set(Planes {
        cells: vec![0; CELLS_WORDS].leak(),
    });
    // The trees built into a pool in one lane, as the GPU's.
    let mut pool = vec![0; POOL_WORDS];
    // The pool's families, each brick's tree taken after the one before's, as
    // the GPU's stack hands them, family 0 none's.
    let mut next = 1;
    WINDOWS_STATE.set(fresh());
    let w = windows();
    for win in 0..WINDOWS {
        let (center_of, cell) = layout(win);
        if center_of == Center::Destination {
            continue;
        }
        let first = window::origin(cell, cell_at(cell, center));
        w.entries[(SLOTS + win) as usize] = Entry {
            brick: first,
            present: 1,
            ticket: 0,
        };
        for s in 0..ENTRIES {
            w.entries[(win * ENTRIES + s) as usize].brick = wanted(cell, first, s);
        }
    }
    assign_slots(w, false);
    for b in 0..SLOTS {
        let Slot { cell, brick, .. } = w.slots[b as usize];
        if w.slots[b as usize].is_free() {
            continue;
        }
        let (t, place) = tile_of(cell, brick);
        let Some((_, looks_of, heights, separation)) = held.iter().find(|(p, _, _, _)| *p == t)
        else {
            continue;
        };
        let side = GROUND.tile_cells();
        let at = |k: u32| {
            let [x, y] = window::of_idx(k);
            ((place[1] * BRICK + y) * side + place[0] * BRICK + x) as usize
        };
        let (mut segments, mut lines) = (
            vec![(0, 0); window::WAYS_BRICK_SEGMENTS as usize],
            vec![0; window::LINES_WORDS as usize],
        );
        let n = brick_lines(t, separation, place, &mut segments, &mut lines);
        let laid = terrain::Laid {
            cell,
            place,
            lines: &lines[..n],
            looks: |k: u32| word([u32::from(looks_of[at(k)]); 4]),
            height: |c: [i32; 2]| {
                i32::from(
                    heights[at(window::idx_of([
                        c[0].clamp(0, BRICK as i32 - 1) as u32,
                        c[1].clamp(0, BRICK as i32 - 1) as u32,
                    ]))],
                )
            },
        };
        let (mut lane, mut scratch) = (
            vec![0u32; window::BRICK_WORDS as usize],
            vec![0u32; tree::SCRATCH_WORDS as usize],
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
            for y in 0..BRICK >> j {
                for x in 0..BRICK >> j {
                    fill::level(
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
        let [taken, _, root] = fill::tree(&lane, &scratch, cell, &mut pool, next);
        assert_ne!(taken, u32::MAX, "a brick's tree past the pool");
        lane[window::ROOT_AT as usize] = root;
        next += taken;
        let at = (b * CELLS_LANE_BYTES / 4) as usize;
        planes().cells[at..at + window::BACK_WORDS as usize].copy_from_slice(&lane);
        w.slots[b as usize].present = true;
    }
    for i in 0..SLOTS {
        show(w, i);
    }
    // Every family taken, a record each: its id, its heap words, its data.
    let p = Read(&pool);
    let mut records = Vec::new();
    for b in 1..next {
        records.push(b);
        records.extend((0..8).map(|k| p.heap(8 * b + k)));
        records.extend((0..8).map(|k| p.data(8 * b + k)));
    }
    records
}

/// A brick's columns as a GPU's fill writes them (pxl-spec-core
/// `fill::heights`, `fill::spans`), over cells whose tops are given
/// (`top`) rather than drawn off samples: each column's word, its top, into
/// `lane`; and the spans of its columns and its ring off its lines
/// (`fill::keep`) into its `scratch`.
fn fill_columns(
    sq: &impl window::Brick,
    top: impl Fn(u32) -> i32,
    lane: &mut [u32],
    scratch: &mut [u32],
) {
    for i in 0..BRICK_CELLS {
        lane[(window::COLUMNS_AT + i) as usize] = top(i) as u32 & 0xffff;
    }
    for i in 0..window::SPANS_COLUMNS {
        fill::keep(sq, scratch, 0, i);
    }
}

/// The eye's windows laid out about `eye` and the pivot's about `pivot`, in
/// units, X then Y, as `step` lays a window that moved, no destination's;
/// their bricks slotted (`assign_slots`), what was held before kept.
fn lay(w: &mut Windows, eye: [f64; 2], pivot: [f64; 2]) {
    for win in 0..WINDOWS {
        let (center_of, cell) = layout(win);
        let p = match center_of {
            Center::Eye => eye,
            Center::Pivot => pivot,
            Center::Destination => continue,
        };
        let first = window::origin(cell, cell_at(cell, p));
        w.entries[(SLOTS + win) as usize] = Entry {
            brick: first,
            present: 1,
            ticket: 0,
        };
        for s in 0..ENTRIES {
            w.entries[(win * ENTRIES + s) as usize].brick = wanted(cell, first, s);
        }
    }
    assign_slots(w, false);
}

/// Every brick held present, as if each had landed, its cells as they
/// came back, all zero unless a test held its own (`hold`): the windows
/// fold over them wherever they are laid again (`assign_slots`).
fn land_all(w: &mut Windows) {
    PLANES.get(|| Planes {
        cells: vec![0; CELLS_WORDS].leak(),
    });
    for b in w.slots.iter_mut().filter(|b| !b.is_free()) {
        b.present = true;
    }
    for i in 0..SLOTS {
        show(w, i);
    }
}

/// A leaf cell's middle, in units.
fn leaf(c: [f64; 2]) -> [f64; 2] {
    c.map(|v| (v + 0.5) * units(LEAF_CELL))
}

/// The slots holding a brick at `cell`.
fn held_at(w: &Windows, cell: Zoom) -> usize {
    w.slots.iter().filter(|b| b.cell == cell).count()
}

/// The brick window `win` wants in entry `s`.
fn want(w: &Windows, win: u32, s: u32) -> [i32; 2] {
    w.entries[(win * ENTRIES + s) as usize].brick
}

const CENTER: [f64; 2] = [37.0 * 4096.0 + 100.0, 11.0 * 4096.0 + 200.0];

#[test]
fn shared_brick_held_once() {
    let _g = sys::serial();
    WINDOWS_STATE.set(fresh());
    let w = windows();
    lay(w, leaf(CENTER), leaf(CENTER));
    assert_eq!(
        w.slots.iter().filter(|b| !b.is_free()).count(),
        (EYE_WINDOWS * ENTRIES) as usize,
        "the pivot's windows take no slot"
    );
    for pivot in EYE_WINDOWS..EYE_WINDOWS + 2 {
        let cell = layout(pivot).1;
        let eye = cell - FIRST_CELL;
        for s in 0..ENTRIES {
            assert_eq!(want(w, pivot, s), want(w, eye, s));
            assert_eq!(
                slot_of(w, cell, want(w, pivot, s)),
                Some(eye * ENTRIES + s),
                "in the eye's own slot"
            );
        }
    }
    land_all(w);
    for pivot in EYE_WINDOWS..EYE_WINDOWS + 2 {
        for s in 0..ENTRIES {
            let row = w.entries[(pivot * ENTRIES + s) as usize];
            assert!(
                row.holds(row.brick) && row.slot() == (layout(pivot).1 - FIRST_CELL) * ENTRIES + s,
                "the pivot's rows name the eye's slots"
            );
        }
    }
}

#[test]
fn left_brick_kept() {
    let _g = sys::serial();
    WINDOWS_STATE.set(fresh());
    let w = windows();
    let (eye, pivot) = (LEAF_CELL - FIRST_CELL, EYE_WINDOWS + 1);
    lay(w, leaf(CENTER), leaf(CENTER));
    land_all(w);
    // The eye a brick on at the leaves: its window there leaves a column
    // the pivot's still wants, and wants a column the pivot's does not.
    let on = leaf([CENTER[0] + f64::from(BRICK), CENTER[1]]);
    lay(w, on, leaf(CENTER));
    assert_eq!(held_at(w, LEAF_CELL), (ENTRIES + window::ACROSS) as usize);
    for s in 0..ENTRIES {
        let q = want(w, pivot, s);
        assert!(
            Windowed.present(LEAF_CELL, q).1,
            "the pivot's bricks stay present"
        );
        assert!(w.entries[(pivot * ENTRIES + s) as usize].holds(q));
    }
    let new: Vec<u32> = (0..ENTRIES)
        .filter(|&s| !w.entries[(eye * ENTRIES + s) as usize].holds(want(w, eye, s)))
        .collect();
    assert_eq!(
        new.len(),
        window::ACROSS as usize,
        "the eye's new column alone is absent"
    );
    for &s in &new {
        let b = slot_of(w, LEAF_CELL, want(w, eye, s)).unwrap();
        assert!(
            w.slots[b as usize].is_absent() && b != eye * ENTRIES + s,
            "in a free slot, the eye's own still holding the pivot's brick"
        );
    }
    // Back where it was, the eye finds the column it left present.
    land_all(w);
    lay(w, leaf(CENTER), leaf(CENTER));
    for s in 0..ENTRIES {
        assert!(
            w.entries[(eye * ENTRIES + s) as usize].holds(want(w, eye, s)),
            "not filled again"
        );
    }
    assert_eq!(
        held_at(w, LEAF_CELL),
        ENTRIES as usize,
        "the column no window wants is given up"
    );
}

#[test]
fn apart_windows_own_slots() {
    let _g = sys::serial();
    WINDOWS_STATE.set(fresh());
    let w = windows();
    lay(
        w,
        leaf(CENTER),
        leaf([CENTER[0] + 100.0 * f64::from(BRICK), CENTER[1]]),
    );
    for win in 0..EYE_WINDOWS + 2 {
        for s in 0..ENTRIES {
            assert_eq!(
                slot_of(w, layout(win).1, want(w, win, s)),
                Some(win * ENTRIES + s)
            );
        }
    }
}

#[test]
fn landing_shows_everywhere() {
    let _g = sys::serial();
    // The fold after the landing reads the bricks' cells.
    PLANES.set(Planes {
        cells: vec![0; CELLS_WORDS].leak(),
    });
    WINDOWS_STATE.set(fresh());
    let w = windows();
    let (eye, pivot) = (LEAF_CELL - FIRST_CELL, EYE_WINDOWS + 1);
    lay(w, leaf(CENTER), leaf(CENTER));
    let q = want(w, eye, 0);
    let b = slot_of(w, LEAF_CELL, q).unwrap();
    w.slots[b as usize].ticket = 7;
    for i in of_entry(0) {
        show(w, i);
    }
    on_landed(b, 6, Landing::Refused);
    assert_eq!(
        w.slots[b as usize].ticket, 7,
        "another fill's landing lays nothing"
    );
    on_landed(b, 7, Landing::Built(&[0]));
    refold();
    for win in [eye, pivot] {
        let row = w.entries[(win * ENTRIES) as usize];
        assert!(row.holds(q) && row.slot() == b);
    }
    // Refused while a window wants it, then given up with the last.
    w.slots[b as usize] = Slot {
        present: false,
        ticket: REFUSED,
        ..w.slots[b as usize]
    };
    let far = leaf([CENTER[0] + 100.0 * f64::from(BRICK), CENTER[1]]);
    lay(w, leaf(CENTER), far);
    assert_eq!(
        w.slots[b as usize].ticket, REFUSED,
        "the eye still wants it"
    );
    assert!(
        !slot_wants(w, tile_of(LEAF_CELL, q).0, b),
        "nor is its tile kept for it"
    );
    lay(w, far, far);
    assert_eq!(slot_of(w, LEAF_CELL, q), None);
}

#[test]
fn heap_wait_rehanded() {
    use crate::sys::boot::set_heap_stat;
    use core::sync::atomic::{AtomicU32, Ordering};
    use xpute_guest::mem::heap::HeapStat;
    static RELEASES: AtomicU32 = AtomicU32::new(0);
    let _g = sys::serial();
    set_heap_stat(|| HeapStat {
        releases: RELEASES.load(Ordering::Relaxed),
        ..HeapStat::default()
    });
    WINDOWS_STATE.set(fresh());
    let w = windows();
    lay(w, leaf(CENTER), leaf(CENTER));
    let (eye, pivot) = (LEAF_CELL - FIRST_CELL, EYE_WINDOWS + 1);
    let q = want(w, eye, 0);
    let b = slot_of(w, LEAF_CELL, q).unwrap();
    // As `on_landed` leaves it where the heap refused the mirror its chunk.
    (w.slots[b as usize].ticket, w.heap) = (WAITING, Some(Refused::now()));
    assert!(!wake(w), "no page given back: it waits");
    lay(w, leaf(CENTER), leaf(CENTER));
    assert_eq!(
        w.slots[b as usize].ticket, WAITING,
        "unhanded while it waits"
    );
    assert!(
        !slot_wants(w, tile_of(LEAF_CELL, q).0, b),
        "nor its tile kept for it"
    );
    RELEASES.fetch_add(1, Ordering::Relaxed);
    assert!(wake(w) && w.heap.is_none());
    assign_slots(w, false);
    assert!(
        w.slots[b as usize].is_absent() && slot_wants(w, tile_of(LEAF_CELL, q).0, b),
        "absent, to be handed again"
    );
    for win in [eye, pivot] {
        assert_eq!(
            w.entries[(win * ENTRIES) as usize].ticket,
            0,
            "its rows show it"
        );
    }
    set_heap_stat(HeapStat::default);
}
