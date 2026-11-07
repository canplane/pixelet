// pxl-kernel/domain/layer/terrain/clipmap.test.rs
//
// The clip levels as a test holds them: about a center, each brick of the
// tiles given present at once, its cells as a GPU would hand them back.

use super::*;
use crate::sys;
use pxl_engine::fill;
use pxl_engine::ground::word;
use pxl_engine::overwrite::Columns;
use pxl_engine::way::tree;
use pxl_engine::way::tree::Read;
use pxl_spec::bake::separation::Separation;
use pxl_spec::bake::tile::terrain;

/// A tile as `hold` takes it: a cell's own palette index and its height, row by row
/// from its first, and its lines.
pub(crate) type HeldTile = (Quadrant, Vec<GroundId>, Vec<i16>, Separation);

/// What `hold` leaves held: the bricks' words up to their last that is
/// not zero, the pool's child groups taken as records (pxl-engine
/// `way::tree::lay`), the entries and the slots.
struct Levels {
    cells: Vec<u32>,
    records: Vec<u32>,
    entries: Vec<Entry>,
    slots: Vec<Slot>,
}

/// What clip levels are built over: a center and the tiles held.
type Over = ([f64; 2], Vec<HeldTile>);

/// The clip levels built this run, by what they were built over: the same are
/// built once, and each `hold` after takes a copy.
static BUILT: std::sync::Mutex<Vec<(Over, Levels)>> = std::sync::Mutex::new(Vec::new());

const CELLS_WORDS: usize = (SLOTS * CELLS_ARR_BYTES / 4) as usize;
const POOL_WORDS: usize = 16 * tree::POOL_CHILD_GROUPS as usize;

/// The pool's mirror holding `records`, a brick's most at a time.
fn mirrored(records: &[u32]) {
    use pxl_engine::way::tree::{BRICK_CHILD_GROUPS, RECORD_WORDS};
    let m = mirror();
    m.clear();
    for child_groups in records.chunks((BRICK_CHILD_GROUPS * RECORD_WORDS) as usize) {
        let mut laid = vec![(child_groups.len() / RECORD_WORDS as usize) as u32];
        laid.extend_from_slice(child_groups);
        assert_eq!(
            m.lay(&laid),
            Ok(()),
            "the test's pool past the mirror's cap"
        );
    }
}

/// The eye's and the pivot's clip levels about `centre`, in units, X then Y,
/// each brick the tiles `held` cover present in its slot, the pivot's
/// clip levels the eye's finest two again: per tile, a cell's own palette index
/// and its height, row by row from its first, and its lines; each brick's
/// columns, its tops and its ways' spans as its fill keeps them
/// (`fill_columns`), and its levels as a GPU folds them (pxl-engine
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
            let b = Levels {
                cells: upto(planes().cells),
                records,
                entries: clipmap().table.entries.to_vec(),
                slots: clipmap().slots.to_vec(),
            };
            built.push(((center, held.to_vec()), b));
            built.len() - 1
        }
    };
    let b = &built[k].1;
    let of = |words: &[u32], len: usize| {
        let mut arr = vec![0; len];
        arr[..words.len()].copy_from_slice(words);
        arr.leak()
    };
    PLANES.set(Planes {
        cells: of(&b.cells, CELLS_WORDS),
    });
    mirrored(&b.records);
    CLIPMAP.set(fresh());
    clipmap().table.entries.copy_from_slice(&b.entries);
    clipmap().slots.copy_from_slice(&b.slots);
    fold(clipmap());
}

/// The brick at `cell` holding cell `g` absent, a fill on its way to it,
/// and the clip levels folded without it, as a turn's step leaves them before
/// it lands: its slot and that fill's ticket, which `on_filled` takes.
pub(crate) fn unland(cell: Zoom, g: [i32; 2]) -> (u32, u32) {
    let w = clipmap();
    let b = slot_of(w, cell, clipmap::brick_of(g).0).expect("held");
    (w.slots[b as usize].present, w.slots[b as usize].fill) = (false, Fill::Pending(7));
    for i in of_entry(b % ENTRIES) {
        show(w, i);
    }
    fold(w);
    (b, 7)
}

/// The device gone, as a turn's step finds it: every brick's cells given
/// up (`assign_slots`).
pub(crate) fn lose() {
    assign_slots(clipmap(), true);
}

/// `hold`'s clip levels, built: the child groups their trees took, as records.
fn build(center: [f64; 2], held: &[HeldTile]) -> Vec<u32> {
    PLANES.set(Planes {
        cells: vec![0; CELLS_WORDS].leak(),
    });
    // The trees built into a pool in one array, as the GPU's.
    let mut pool = vec![0; POOL_WORDS];
    // The pool's child groups, each brick's tree taken after the one before's, as
    // the GPU's stack hands them, child group 0 none's.
    let mut next = 1;
    CLIPMAP.set(fresh());
    let w = clipmap();
    for clip_level in 0..CLIP_LEVELS {
        let (center_of, cell) = layout(clip_level);
        if center_of == Center::Destination {
            continue;
        }
        let first = clipmap::origin(cell, cell_at(cell, center));
        w.table.entries[(SLOTS + clip_level) as usize] = Entry {
            brick: first,
            present: 1,
            ticket: 0,
        };
        for s in 0..ENTRIES {
            w.table.entries[(clip_level * ENTRIES + s) as usize].brick =
                entry_brick(cell, first, s);
        }
    }
    assign_slots(w, false);
    for b in 0..SLOTS {
        let Slot { cell, brick, .. } = w.slots[b as usize];
        if w.slots[b as usize].is_free() {
            continue;
        }
        let (t, place) = tile_of(cell, brick);
        let Some((_, palette_of, heights, separation)) = held.iter().find(|(p, _, _, _)| *p == t)
        else {
            continue;
        };
        let side = GROUND.tile_cells();
        let at = |k: u32| {
            let [x, y] = clipmap::of_idx(k);
            ((place[1] * BRICK + y) * side + place[0] * BRICK + x) as usize
        };
        let (mut segments, mut lines) = (
            vec![(0, 0); clipmap::WAYS_BRICK_SEGMENTS as usize],
            vec![0; clipmap::LINES_WORDS as usize],
        );
        let n = brick_lines(t, separation, place, &mut segments, &mut lines);
        let laid = terrain::TileBrick {
            cell,
            place,
            first: terrain::first_cell(t, cell, place),
            lines: &lines[..n],
            palette_indices: |k: u32| word([u32::from(palette_of[at(k)]); 4]),
            height: |c: [i32; 2]| {
                i32::from(
                    heights[at(clipmap::idx_of([
                        c[0].clamp(0, BRICK as i32 - 1) as u32,
                        c[1].clamp(0, BRICK as i32 - 1) as u32,
                    ]))],
                )
            },
        };
        let (mut back, mut scratch) = (
            vec![0u32; clipmap::BRICK_WORDS as usize],
            vec![0u32; tree::SCRATCH_WORDS as usize],
        );
        fill_columns(
            &laid,
            |k| i32::from(heights[at(k)]),
            &mut back,
            &mut scratch,
        );
        let n = clipmap::LAID_CELLS;
        for k in 0..clipmap::LAID_WORDS {
            back[(clipmap::LAID_AT + k) as usize] = (0..n)
                .map(|j| u32::from(palette_of[at(n * k + j)]) << (8 * j))
                .sum();
        }
        for j in 0..=BRICK_LEVELS {
            for y in 0..BRICK >> j {
                for x in 0..BRICK >> j {
                    fill::level(
                        &mut back,
                        0,
                        &pxl_engine::fill::Scratch {
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
        let [taken, _, root] = fill::tree(&back, &scratch, cell, &mut pool, next);
        assert_ne!(taken, u32::MAX, "a brick's tree past the pool");
        back[clipmap::ROOT_AT as usize] = root;
        next += taken;
        let at = (b * CELLS_ARR_BYTES / 4) as usize;
        planes().cells[at..at + clipmap::BACK_WORDS as usize].copy_from_slice(&back);
        w.slots[b as usize].present = true;
    }
    for i in 0..SLOTS {
        show(w, i);
    }
    // Every child group taken, a record each: its id, its child pointers, its data.
    let p = Read(&pool);
    let mut records = Vec::new();
    for b in 1..next {
        records.push(b);
        records.extend((0..8).map(|k| p.first_child(8 * b + k)));
        records.extend((0..8).map(|k| p.data(8 * b + k)));
    }
    records
}

/// A brick's columns as a GPU's fill writes them (pxl-engine
/// `fill::heights`, `fill::spans`), over cells whose tops are given
/// (`top`) rather than drawn off samples: each column's word, its top, into
/// `back`; and the spans of its columns and its ring off its lines
/// (`fill::keep`) into its `scratch`.
fn fill_columns(
    sq: &impl clipmap::Brick,
    top: impl Fn(u32) -> i32,
    back: &mut [u32],
    scratch: &mut [u32],
) {
    for i in 0..BRICK_CELLS {
        back[(clipmap::COLUMNS_AT + i) as usize] = top(i) as u32 & 0xffff;
    }
    for i in 0..clipmap::SPANS_COLUMNS {
        fill::keep(sq, scratch, 0, i);
    }
}

/// The eye's clip levels laid out about `eye` and the pivot's about `pivot`, in
/// units, X then Y, as `step` lays a clip level that moved, no destination's;
/// their bricks slotted (`assign_slots`), what was held before kept.
fn lay(w: &mut Clipmap, eye: [f64; 2], pivot: [f64; 2]) {
    for clip_level in 0..CLIP_LEVELS {
        let (center_of, cell) = layout(clip_level);
        let p = match center_of {
            Center::Eye => eye,
            Center::Pivot => pivot,
            Center::Destination => continue,
        };
        let first = clipmap::origin(cell, cell_at(cell, p));
        w.table.entries[(SLOTS + clip_level) as usize] = Entry {
            brick: first,
            present: 1,
            ticket: 0,
        };
        for s in 0..ENTRIES {
            w.table.entries[(clip_level * ENTRIES + s) as usize].brick =
                entry_brick(cell, first, s);
        }
    }
    assign_slots(w, false);
}

/// Every brick held present, as if each had landed, its cells as they
/// came back, all zero unless a test held its own (`hold`): the clip levels
/// fold over them wherever they are laid again (`assign_slots`).
fn land_all(w: &mut Clipmap) {
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
fn held_at(w: &Clipmap, cell: Zoom) -> usize {
    w.slots.iter().filter(|b| b.cell == cell).count()
}

/// The brick clip level `clip_level` wants in entry `s`.
fn want(w: &Clipmap, clip_level: u32, s: u32) -> [i32; 2] {
    w.table.entries[(clip_level * ENTRIES + s) as usize].brick
}

const CENTER: [f64; 2] = [37.0 * 4096.0 + 100.0, 11.0 * 4096.0 + 200.0];

#[test]
fn shared_brick_held_once() {
    let _g = sys::serial();
    CLIPMAP.set(fresh());
    let w = clipmap();
    lay(w, leaf(CENTER), leaf(CENTER));
    assert_eq!(
        w.slots.iter().filter(|b| !b.is_free()).count(),
        (EYE_CLIP_LEVELS * ENTRIES) as usize,
        "the pivot's clip levels take no slot"
    );
    for pivot in EYE_CLIP_LEVELS..EYE_CLIP_LEVELS + 2 {
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
    for pivot in EYE_CLIP_LEVELS..EYE_CLIP_LEVELS + 2 {
        for s in 0..ENTRIES {
            let row = w.table.entries[(pivot * ENTRIES + s) as usize];
            assert!(
                row.holds(row.brick) && row.slot() == (layout(pivot).1 - FIRST_CELL) * ENTRIES + s,
                "the pivot's rows name the eye's slots"
            );
        }
    }
}

#[test]
fn left_brick_part() {
    let _g = sys::serial();
    CLIPMAP.set(fresh());
    let w = clipmap();
    let (eye, pivot) = (LEAF_CELL - FIRST_CELL, EYE_CLIP_LEVELS + 1);
    lay(w, leaf(CENTER), leaf(CENTER));
    land_all(w);
    // The eye a brick on at the leaves: its clip level there leaves a column
    // the pivot's still wants, and wants a column the pivot's does not.
    let on = leaf([CENTER[0] + f64::from(BRICK), CENTER[1]]);
    lay(w, on, leaf(CENTER));
    assert_eq!(held_at(w, LEAF_CELL), (ENTRIES + clipmap::ACROSS) as usize);
    for s in 0..ENTRIES {
        let q = want(w, pivot, s);
        assert!(
            CpuClipmap.present(LEAF_CELL, q).1,
            "the pivot's bricks stay present"
        );
        assert!(w.table.entries[(pivot * ENTRIES + s) as usize].holds(q));
    }
    let new: Vec<u32> = (0..ENTRIES)
        .filter(|&s| !w.table.entries[(eye * ENTRIES + s) as usize].holds(want(w, eye, s)))
        .collect();
    assert_eq!(
        new.len(),
        clipmap::ACROSS as usize,
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
            w.table.entries[(eye * ENTRIES + s) as usize].holds(want(w, eye, s)),
            "not filled again"
        );
    }
    assert_eq!(
        held_at(w, LEAF_CELL),
        ENTRIES as usize,
        "the column no clip level wants is given up"
    );
}

#[test]
fn apart_clip_levels_own_slots() {
    let _g = sys::serial();
    CLIPMAP.set(fresh());
    let w = clipmap();
    lay(
        w,
        leaf(CENTER),
        leaf([CENTER[0] + 100.0 * f64::from(BRICK), CENTER[1]]),
    );
    for clip_level in 0..EYE_CLIP_LEVELS + 2 {
        for s in 0..ENTRIES {
            assert_eq!(
                slot_of(w, layout(clip_level).1, want(w, clip_level, s)),
                Some(clip_level * ENTRIES + s)
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
    CLIPMAP.set(fresh());
    let w = clipmap();
    let (eye, pivot) = (LEAF_CELL - FIRST_CELL, EYE_CLIP_LEVELS + 1);
    lay(w, leaf(CENTER), leaf(CENTER));
    let q = want(w, eye, 0);
    let b = slot_of(w, LEAF_CELL, q).unwrap();
    w.slots[b as usize].fill = Fill::Pending(7);
    for i in of_entry(0) {
        show(w, i);
    }
    on_filled(b, 6, Filled::Refused);
    assert!(
        w.slots[b as usize].fill == Fill::Pending(7),
        "another fill's landing lays nothing"
    );
    on_filled(b, 7, Filled::Built(&[0]));
    refold();
    for clip_level in [eye, pivot] {
        let row = w.table.entries[(clip_level * ENTRIES) as usize];
        assert!(row.holds(q) && row.slot() == b);
    }
    // Refused while a clip level wants it, then given up with the last.
    w.slots[b as usize] = Slot {
        present: false,
        fill: Fill::Refused,
        ..w.slots[b as usize]
    };
    let far = leaf([CENTER[0] + 100.0 * f64::from(BRICK), CENTER[1]]);
    lay(w, leaf(CENTER), far);
    assert!(
        w.slots[b as usize].fill == Fill::Refused,
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
    use xpute_core::mem::heap::HeapStat;
    static RELEASES: AtomicU32 = AtomicU32::new(0);
    let _g = sys::serial();
    set_heap_stat(|| HeapStat {
        releases: RELEASES.load(Ordering::Relaxed),
        ..HeapStat::default()
    });
    CLIPMAP.set(fresh());
    let w = clipmap();
    lay(w, leaf(CENTER), leaf(CENTER));
    let (eye, pivot) = (LEAF_CELL - FIRST_CELL, EYE_CLIP_LEVELS + 1);
    let q = want(w, eye, 0);
    let b = slot_of(w, LEAF_CELL, q).unwrap();
    // As `on_filled` leaves it where the heap refused the mirror its chunk.
    (w.slots[b as usize].fill, w.heap) = (Fill::Waiting, Some(Refusal::now()));
    assert!(!wake(w), "no page given back: it waits");
    lay(w, leaf(CENTER), leaf(CENTER));
    assert!(
        w.slots[b as usize].fill == Fill::Waiting,
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
    for clip_level in [eye, pivot] {
        assert_eq!(
            w.table.entries[(clip_level * ENTRIES) as usize].ticket,
            0,
            "its rows show it"
        );
    }
    set_heap_stat(HeapStat::default);
}
