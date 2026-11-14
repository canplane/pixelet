// pxl-core/domain/layer/terrain/clipmap.test.rs
//
// Tests of the clip levels, and the helpers other tests use to hold clip
// levels around a center with every brick of the given tiles present at
// once, its cells as a fill leaves them.

use super::*;
use crate::domain::layer::terrain::pool::{exhaust, set_way_pool, WayPool};
use pxl_engine::fill;
use pxl_engine::way::way_cell;
use pxl_spec::bake::separation::{self, Separation};
use pxl_spec::bake::tile::terrain;
use pxl_spec::brick;
use pxl_spec::ground::word;
use pxl_spec::ground::GroundId;
use pxl_spec::value::TRANSPORT;
use pxl_spec::way::BRIDGE;

/// A tile as `hold` takes it: for each cell, row by row from the first, its
/// own asset id and its elevation; then the tile's lines.
pub(crate) type TileData = (Quadrant, Vec<GroundId>, Vec<i16>, Separation);

/// What `hold` leaves held: the bricks' words up to the last nonzero one,
/// the pool, the entries and the slots.
struct Snapshot {
    cells: Vec<u32>,
    pool: WayPool,
    entries: Vec<Entry>,
    slots: Vec<Slot>,
}

/// What clip levels are built over: a center and the tiles held.
type Over = ([f64; 2], Vec<TileData>);

/// The clip levels built in this run, keyed by what they were built over.
/// Each set is built once, and every later `hold` of it takes a copy.
static BUILT: std::sync::Mutex<Vec<(Over, Snapshot)>> = std::sync::Mutex::new(Vec::new());

const CELLS_WORDS: usize = (SLOT_CNT * CELLS_ARR_BYTES / 4) as usize;
const FOLDS_WORDS: usize = (SLOT_CNT * FOLDS_ARR_BYTES / 4) as usize;

/// The test's own planes, made once in a run and written over by each hold
/// and fresh start, so the heap holds one set of them.
fn own_planes() -> &'static mut BrickMemory {
    PLANES.get(|| BrickMemory {
        cells: vec![0; CELLS_WORDS].leak(),
        folds: vec![0; FOLDS_WORDS].leak(),
    })
}

/// The test's own planes, zeroed.
fn zeroed_planes() -> &'static mut BrickMemory {
    let p = own_planes();
    p.cells.fill(0);
    p.folds.fill(0);
    p
}

/// Holds the eye's and the pivot's clip levels around `center`, in units, X
/// then Y, with every brick the tiles in `tiles` cover present in its slot.
/// The pivot's clip levels repeat the eye's finest two. Each brick holds its
/// columns, tops and ways' spans as its fill keeps them (`fill_columns`),
/// and its mip levels (pxl-engine `fill::mips`). The clip levels for a center
/// and its tiles are built once per run (`BUILT`), and each hold gets its own
/// copy, whatever a test does to it.
pub(crate) fn hold(center: [f64; 2], tiles: &[TileData]) {
    let mut built = BUILT.lock().unwrap_or_else(|e| e.into_inner());
    let k = match built
        .iter()
        .position(|((c, h), _)| *c == center && h.as_slice() == tiles)
    {
        Some(k) => k,
        None => {
            build(center, tiles);
            let upto = |words: &[u32]| {
                words[..words.iter().rposition(|&w| w != 0).map_or(0, |i| i + 1)].to_vec()
            };
            let b = Snapshot {
                cells: upto(planes().cells),
                pool: way_pool().clone(),
                entries: clipmap().table.entries.to_vec(),
                slots: clipmap().slots.to_vec(),
            };
            built.push(((center, tiles.to_vec()), b));
            built.len() - 1
        }
    };
    let b = &built[k].1;
    zeroed_planes().cells[..b.cells.len()].copy_from_slice(&b.cells);
    set_way_pool(b.pool.clone());
    CLIPMAP.set(fresh());
    clipmap().table.entries.copy_from_slice(&b.entries);
    clipmap().slots.copy_from_slice(&b.slots);
    fold(clipmap());
}

/// Marks the brick at `cell_zoom` that holds cell `g` as filled and not yet
/// handed over, and folds the clip levels without it, as an entry's step leaves
/// them before a frame hands the brick over.
pub(crate) fn unland(cell_zoom: Zoom, g: [i32; 2]) {
    let w = clipmap();
    let b = slot_of(w, cell_zoom, clipmap::brick_of(g).0).expect("held");
    (w.slots[b as usize].present, w.slots[b as usize].fill) = (false, Fill::Filled);
    for i in of_entry(b % ENTRY_CNT) {
        show(w, i);
    }
    fold(w);
}

/// Hands every filled brick over, as a frame with room for all of them does.
pub(crate) fn hand() {
    upload(&mut |_| true);
}

/// Loses the device, as a frame's step finds it lost: the GPU's copy of
/// every brick is gone, and each present brick must be handed over again
/// (`assign_slots`).
pub(crate) fn lose() {
    assign_slots(clipmap(), true);
}

/// Builds `hold`'s clip levels, laying each brick's lines into the store as a
/// fill lays them.
fn build(center: [f64; 2], tiles: &[TileData]) {
    zeroed_planes();
    set_way_pool(WayPool::default());
    CLIPMAP.set(fresh());
    let w = clipmap();
    for clip_level in 0..CLIP_LEVEL_CNT {
        let (center_of, cell_zoom) = layout(clip_level);
        if center_of == Center::Destination {
            continue;
        }
        let first = clipmap::origin(cell_zoom, cell_at(cell_zoom, center));
        w.table.entries[(SLOT_CNT + clip_level) as usize] = Entry::at(first, 1);
        for s in 0..ENTRY_CNT {
            w.table.entries[(clip_level * ENTRY_CNT + s) as usize].brick =
                entry_brick(cell_zoom, first, s);
        }
    }
    assign_slots(w, false);
    for b in 0..SLOT_CNT {
        let Slot {
            cell_zoom, brick, ..
        } = w.slots[b as usize];
        if w.slots[b as usize].is_free() {
            continue;
        }
        let (t, place) = tile_of(cell_zoom, brick);
        let Some((_, cells, ele, separation)) = tiles.iter().find(|(p, _, _, _)| *p == t) else {
            continue;
        };
        let side = GROUND.tile_cells();
        let at = |k: u32| {
            let [x, y] = brick::of_idx(k);
            ((place[1] * BRICK + y) * side + place[0] * BRICK + x) as usize
        };
        let mut pairs = Vec::new();
        let segments = brick_pairs(separation, place, &mut pairs).unwrap();
        let cnt = pairs.len();
        let mut lines = vec![0; brick::lines_words(cnt as u32, segments) as usize];
        if cnt != 0 {
            brick_lines(t, separation, &mut pairs, &mut lines);
        }
        let laid = terrain::TileBrick {
            cell_zoom,
            place,
            first: terrain::first_cell(t, cell_zoom, place),
            lines: &lines,
            fold_at: |k: u32| word([u32::from(cells[at(k)]); 4]),
            ele_at: |c: [i32; 2]| {
                i32::from(
                    ele[at(brick::idx_of([
                        c[0].clamp(0, BRICK as i32 - 1) as u32,
                        c[1].clamp(0, BRICK as i32 - 1) as u32,
                    ]))],
                )
            },
        };
        let (mut back, mut scratch) = (
            vec![0u32; BRICK_WORDS as usize],
            vec![0u32; SPANS_WORDS as usize],
        );
        fill_columns(&laid, |k| i32::from(ele[at(k)]), &mut back, &mut scratch);
        let n = clipmap::OWN_CELL_CNT;
        for k in 0..clipmap::OWN_WORDS {
            back[(clipmap::OWN_AT + k) as usize] = (0..n)
                .map(|j| u32::from(cells[at(n * k + j)]) << (GroundId::BITS * j))
                .sum();
        }
        fill::mips(&mut back, &scratch, cell_zoom);
        let words = fill::spans_run_words(&scratch);
        if words != 0 {
            let run = way_pool().take(words).expect("a brick's run past the pool");
            fill::lay_spans(&scratch, &mut |k, v| way_pool().set(run[0] + k, v));
            back[ROOT_AT as usize] = run[0];
            w.slots[b as usize].run = run;
        }
        let at = (b * CELLS_ARR_BYTES / 4) as usize;
        planes().cells[at..at + BRICK_WORDS as usize].copy_from_slice(&back);
        w.slots[b as usize].present = true;
    }
    for i in 0..SLOT_CNT {
        show(w, i);
    }
}

/// Writes a brick's columns as a fill does (pxl-engine `fill::columns`,
/// `fill::spans`), but over cells whose tops `top` gives instead of
/// computing them from samples. Each column's word, its top, goes into
/// `back`, and the spans of its columns and of its ring, from its lines
/// (`fill::keep`), go into `scratch`.
fn fill_columns(
    sq: &impl brick::Brick,
    top: impl Fn(u32) -> i32,
    back: &mut [u32],
    scratch: &mut [u32],
) {
    for i in 0..BRICK_CELL_CNT {
        back[(clipmap::COLUMNS_AT + i) as usize] = top(i) as u32 & 0xffff;
    }
    for i in 0..clipmap::SPANS_COLUMN_CNT {
        fill::keep(sq, scratch, i);
    }
}

/// Lays out the eye's clip levels around `eye` and the pivot's around
/// `pivot`, in units, X then Y, as `step` lays out a clip level that moved,
/// with no destination. Their bricks are given slots (`assign_slots`), and
/// what was held before is kept.
fn lay(w: &mut Clipmap, eye: [f64; 2], pivot: [f64; 2]) {
    for clip_level in 0..CLIP_LEVEL_CNT {
        let (center_of, cell_zoom) = layout(clip_level);
        let p = match center_of {
            Center::Eye => eye,
            Center::Pivot => pivot,
            Center::Destination => continue,
        };
        let first = clipmap::origin(cell_zoom, cell_at(cell_zoom, p));
        w.table.entries[(SLOT_CNT + clip_level) as usize] = Entry::at(first, 1);
        for s in 0..ENTRY_CNT {
            w.table.entries[(clip_level * ENTRY_CNT + s) as usize].brick =
                entry_brick(cell_zoom, first, s);
        }
    }
    assign_slots(w, false);
}

/// Marks every held brick present, as if each had been handed over. Its
/// cells are all zero unless a test held its own (`hold`). The clip levels
/// fold over them wherever they are laid out again (`assign_slots`).
fn land_all(w: &mut Clipmap) {
    own_planes();
    for b in w.slots.iter_mut().filter(|b| !b.is_free()) {
        b.present = true;
    }
    for i in 0..SLOT_CNT {
        show(w, i);
    }
}

/// A leaf cell's middle, in units.
fn leaf(c: [f64; 2]) -> [f64; 2] {
    c.map(|v| (v + 0.5) * units(LEAF_CELL_ZOOM))
}

fn slots_at(w: &Clipmap, cell_zoom: Zoom) -> usize {
    w.slots.iter().filter(|b| b.cell_zoom == cell_zoom).count()
}

/// The brick clip level `clip_level` wants in entry `s`.
fn want(w: &Clipmap, clip_level: u32, s: u32) -> [i32; 2] {
    w.table.entries[(clip_level * ENTRY_CNT + s) as usize].brick
}

const CENTER: [f64; 2] = [37.0 * 4096.0 + 100.0, 11.0 * 4096.0 + 200.0];

/// The leaf tile `CENTER` lies in.
const CENTER_TILE: [i32; 2] = [296, 88];

/// The clip levels around `CENTER` with nothing held, and everything a fill
/// works in owned by the test.
fn fill_room() -> &'static mut Clipmap {
    zeroed_planes();
    set_way_pool(WayPool::default());
    FILL.set(FillScratch {
        ele: vec![0; ELE_COLUMN].leak(),
        spans: vec![0; SPANS_WORDS as usize].leak(),
        lines: Vec::new(),
    });
    CLIPMAP.set(fresh());
    let w = clipmap();
    lay(w, leaf(CENTER), leaf(CENTER));
    w
}

/// The tile at `CENTER_TILE` and its packet: rolling ground with a bridge
/// crossing it from west to east, high enough to stand clear of the ground.
fn hilly_tile() -> (Quadrant, TerrainTile, Vec<u8>) {
    let ele: Vec<i16> = (0..ELE_COLUMN as i32)
        .map(|i| ((i * 37) % 11 + (i / 36) % 5) as i16)
        .collect();
    let mut builder = separation::Builder::default();
    let point = |x: f64| ([x, 180.0], 40, 0);
    builder.push(
        [point(16.0), point(160.0), point(320.0), point(480.0)],
        24,
        TRANSPORT[0] as u8,
        separation::Form::of(BRIDGE as u8),
        0,
    );
    let cells = (0..terrain::GROUND_COLUMN)
        .map(|c| 1 + (c % 7) as GroundId)
        .collect::<Vec<_>>();
    let t = Quadrant::new(LEAF_ZOOM, CENTER_TILE);
    let tile = TerrainTile::leaf(ele, cells.into(), builder.finish());
    let pkt = terrain::encode(&tile).unwrap();
    (t, tile, pkt)
}

/// `t` held in the tile cache as `tile`, the only tile asked for.
fn cache_tile(t: Quadrant, tile: TerrainTile) {
    let cache = TerrainTile::tile_cache();
    *cache = Default::default();
    cache.admit(t, |_| None, |_| Ok::<_, ()>(tile)).unwrap();
    let asks: fnv::FnvHashMap<Quadrant, ()> = [(t, ())].into_iter().collect();
    cache.ask(&asks);
}

#[test]
fn brick_filled_in_parts_reads_as_one_filled_whole() {
    let w = fill_room();
    let (t, tile, pkt) = hilly_tile();
    cache_tile(t, tile.clone());
    cut(t, &pkt);
    let wanted: Vec<u32> = (0..SLOT_CNT)
        .filter(|&b| {
            let slot = w.slots[b as usize];
            !slot.is_free() && tile_of(slot.cell_zoom, slot.brick).0 == t
        })
        .collect();
    assert!(!wanted.is_empty(), "the clip levels want of the tile");
    assert!(
        wanted
            .iter()
            .all(|&b| w.slots[b as usize].fill == Fill::Cut),
        "each cut in the entry the packet landed"
    );
    // Drop the packet, so the fill can read only the tile and the slots.
    drop(pkt);
    due(w);
    while !matches!(fill_stage(w), Poll::Done) {}
    let (mut ways, mut filled) = (0, Vec::new());
    for &b in &wanted {
        let slot = w.slots[b as usize];
        assert!(slot.fill == Fill::Filled, "filled, not yet handed");
        assert!(!slot.present, "not present before a frame hands it");
        let (cell_zoom, place) = (slot.cell_zoom, tile_of(slot.cell_zoom, slot.brick).1);
        let mut pairs = Vec::new();
        let segments = brick_pairs(&tile.separation, place, &mut pairs).unwrap();
        let cnt = pairs.len();
        let mut lines = vec![0; brick::lines_words(cnt as u32, segments) as usize];
        brick_lines(t, &tile.separation, &mut pairs, &mut lines);
        let site = fill::Site {
            cell_zoom,
            place,
            first: slot.brick.map(|v| v << BRICK_SHIFT),
            shard: Shard::UNKNOWN.at(&ServiceArea, (brick::bricks_zoom(cell_zoom), slot.brick)),
            ele: &tile.ele,
            lines: &lines,
        };
        let four = |i: u32| {
            let [x, y] = brick::of_idx(i);
            let c = ((place[1] * BRICK + y) * GROUND.tile_cells() + place[0] * BRICK + x) as usize;
            let at = 4 * c;
            word([0, 1, 2, 3].map(|k| u32::from(tile.cells[at + k])))
        };
        let (mut lane, mut spans) = (vec![0; BRICK_WORDS as usize], vec![0; SPANS_WORDS as usize]);
        fill::brick(&mut lane, &mut spans, four, &site);
        let got = cells(b);
        assert_eq!(
            got[..ROOT_AT as usize],
            lane[..ROOT_AT as usize],
            "its words as a fill at once writes them"
        );
        assert_eq!(
            slot.run[1],
            fill::spans_run_words(&spans),
            "its run, sized by its spans"
        );
        assert_eq!(got[ROOT_AT as usize], slot.run[0]);
        assert!((0..BRICK_CELL_CNT).all(|i| folds(b)[i as usize] == four(i)));
        ways += u32::from(slot.run[1] != 0);
        filled.push((b, slot.cell_zoom, slot.brick, spans));
    }
    assert!(ways > 0, "the bridge is some brick's");
    let mut handed = Vec::new();
    upload(&mut |q| {
        handed.push(q.slot);
        assert_eq!(q.words, cells(q.slot), "handed where it lies");
        true
    });
    handed.sort_unstable();
    assert_eq!(handed, wanted, "each handed once");
    assert!(wanted.iter().all(|&b| w.slots[b as usize].present));
    // The cells as the query reads them from the brick's lines, its ring's elevations included, are
    // what the rule answers over the spans the fill kept: the tree's leaves, had one been built.
    for (b, cell_zoom, q, spans) in filled {
        let column_spans = fill::Scratch {
            words: &spans,
            at: 0,
        };
        for i in 0..BRICK_CELL_CNT {
            let [x, y] = brick::of_idx(i);
            let g = [
                (q[0] << BRICK_SHIFT) + x as i32,
                (q[1] << BRICK_SHIFT) + y as i32,
            ];
            let (place, _) = clipmap::step(&clip_read(), Lookup::NONE, cell_zoom, g);
            assert!(place.here() && place.slot() == b);
            for z in -64..256 {
                let got = clipmap::way_at(
                    &clip_read(),
                    way_pool(),
                    place,
                    [i32::MIN / 2, i32::MAX / 2],
                    (cell_zoom, [g[0], g[1], z]),
                );
                let rule = way_cell(&column_spans, [x as i32, y as i32], cell_zoom, z);
                assert_eq!(got, rule, "cell {x},{y},{z}");
            }
        }
    }
}

#[test]
fn void_tile_bricks_filled_with_no_packet() {
    let w = fill_room();
    let t = Quadrant::new(LEAF_ZOOM, CENTER_TILE);
    let cache = TerrainTile::tile_cache();
    *cache = Default::default();
    let asks: fnv::FnvHashMap<Quadrant, ()> = [(t, ())].into_iter().collect();
    cache.ask(&asks);
    cache.set(t, TerrainTile::default());
    due(w);
    assert!(!w.due.is_empty(), "a void tile's bricks are due");
    while !matches!(fill_stage(w), Poll::Done) {}
    assert!(
        w.slots.iter().any(|s| s.fill == Fill::Filled),
        "filled off the void's asset id, over samples all zero"
    );
}

#[test]
fn brick_let_go_ends_its_fill() {
    let w = fill_room();
    let (t, tile, pkt) = hilly_tile();
    cache_tile(t, tile);
    cut(t, &pkt);
    due(w);
    fill_stage(w);
    let b = w.filling.unwrap().slot;
    let far = leaf([CENTER[0] + 100.0 * f64::from(BRICK), CENTER[1]]);
    lay(w, far, far);
    assert!(w.slots[b as usize].fill != Fill::Cut, "its slot let it go");
    while !matches!(fill_stage(w), Poll::Done) {}
    assert!(w.filling.is_none(), "and nothing lands in the slot it left");
}

/// A brick with a column past pxl-spec `way::SPAN_CAP` is refused and no give-back wakes it.
#[test]
fn column_past_its_spans_refuses_the_brick_for_good() {
    let w = fill_room();
    let (t, tile, pkt) = hilly_tile();
    cache_tile(t, tile);
    cut(t, &pkt);
    due(w);
    fill_stage(w);
    let filling = w.filling.as_mut().unwrap();
    let b = filling.slot;
    filling.past = true;
    loop {
        fill_stage(w);
        if w.filling.is_none() {
            break;
        }
    }
    let s = w.slots[b as usize];
    assert!(s.fill == Fill::Refused && s.run[1] == 0 && s.want == NEVER);
    let lent = way_pool().take(16).unwrap();
    way_pool().give(lent);
    wake(w);
    assert!(
        w.slots[b as usize].fill == Fill::Refused,
        "a give-back changes nothing of it"
    );
}

/// A brick is not handed before its run of kept spans has landed.
#[test]
fn brick_is_handed_only_after_its_run_landed() {
    let w = fill_room();
    let (t, tile, pkt) = hilly_tile();
    cache_tile(t, tile);
    cut(t, &pkt);
    due(w);
    let (mut laid, mut pieced) = (false, 0);
    while !matches!(fill_stage(w), Poll::Done) {
        if let Some(f) = w.filling {
            laid |= matches!(f.stage, Stage::Lay);
            if matches!(f.stage, Stage::Lay) {
                let mut handed = false;
                upload(&mut |q| {
                    handed |= q.slot == f.slot;
                    pieced += q.run.len();
                    true
                });
                assert!(!handed, "its slot is not offered at its last part");
            }
        }
    }
    assert!(laid, "a fill goes through laying its run");
    upload(&mut |q| {
        pieced += q.run.len();
        true
    });
    assert!(pieced > 0, "handed with its run");
}

/// A device lost hands each present brick's run over again.
#[test]
fn device_loss_rehands_the_run() {
    let w = fill_room();
    let (t, tile, pkt) = hilly_tile();
    cache_tile(t, tile);
    cut(t, &pkt);
    due(w);
    while !matches!(fill_stage(w), Poll::Done) {}
    hand();
    let mut ran: Vec<u32> = (0..SLOT_CNT)
        .filter(|&b| w.slots[b as usize].present && w.slots[b as usize].run[1] != 0)
        .collect();
    ran.sort_unstable();
    assert!(!ran.is_empty());
    assign_slots(w, true);
    let mut again = Vec::new();
    upload(&mut |q| {
        if !q.run.is_empty() {
            again.push(q.slot);
        }
        true
    });
    again.sort_unstable();
    assert_eq!(again, ran, "every run handed over again, as it lies");
}

/// A brick filled after a refusal is forgotten like one that lands late: its row in the table
/// moves, which the cast reads as a brick that moved (pxl-render ray/cast.rs
/// `entry(..).present_changed_from`), and the tile cache hears of it (`changed_at`).
#[test]
fn brick_filled_after_a_refusal_moves_its_row() {
    let w = fill_room();
    let (t, tile, pkt) = hilly_tile();
    cache_tile(t, tile);
    let lent = way_pool().take(1 << 13).expect("a lent run");
    exhaust(way_pool());
    cut(t, &pkt);
    due(w);
    while !matches!(fill_stage(w), Poll::Done) {}
    let b = (0..SLOT_CNT)
        .find(|&b| w.slots[b as usize].fill == Fill::Refused)
        .expect("a refused brick");
    let (zoom, q) = (w.slots[b as usize].cell_zoom, w.slots[b as usize].brick);
    let row = (0..SLOT_CNT)
        .find(|&i| {
            let e = w.table.entries[i as usize];
            e.brick == q && i / ENTRY_CNT == zoom - FIRST_CELL_ZOOM
        })
        .expect("the eye's row of it");
    let was = w.table.entries[row as usize];
    assert_eq!(was.present, 0, "refused, a reader reads the coarser level");
    way_pool().give(lent);
    assert!(wake(w));
    cut(t, &pkt);
    due(w);
    while !matches!(fill_stage(w), Poll::Done) {}
    let mut changed = Vec::new();
    TerrainTile::tile_cache().take_changed(&mut changed);
    upload(&mut |_| true);
    TerrainTile::tile_cache().take_changed(&mut changed);
    let now = w.table.entries[row as usize];
    assert!(
        now.present != 0 && now.present_changed_from(was),
        "the row the cast reads moved"
    );
    assert!(
        changed.contains(&brick_quad(zoom, q)),
        "and the cache heard"
    );
}

/// A slot's run reused by another brick follows a change of the row a cached answer read: the
/// node cache forgets it for the same reason it forgot the tree.
#[test]
fn run_reused_only_after_the_row_moved() {
    let w = fill_room();
    let (t, tile, pkt) = hilly_tile();
    cache_tile(t, tile);
    cut(t, &pkt);
    due(w);
    while !matches!(fill_stage(w), Poll::Done) {}
    hand();
    let b = (0..SLOT_CNT)
        .find(|&b| w.slots[b as usize].run[1] != 0)
        .expect("a brick with a run");
    let (zoom, q) = (w.slots[b as usize].cell_zoom, w.slots[b as usize].brick);
    let row = (0..SLOT_CNT)
        .find(|&i| w.table.entries[i as usize].holds(q) && i / ENTRY_CNT == zoom - FIRST_CELL_ZOOM)
        .expect("its row");
    let was = w.table.entries[row as usize];
    let far = leaf([CENTER[0] + 100.0 * f64::from(BRICK), CENTER[1]]);
    lay(w, far, far);
    assert!(
        w.table.entries[row as usize].changed_from(was),
        "the brick left its row, which the cast forgets over"
    );
}

/// A brick refused at the pool's cap is filled again once after a give-back, and only then.
#[test]
fn brick_past_the_cap_refilled_once_after_a_give_back() {
    let w = fill_room();
    let (t, tile, pkt) = hilly_tile();
    cache_tile(t, tile);
    let lent = way_pool().take(1 << 13).expect("a lent run");
    exhaust(way_pool());
    cut(t, &pkt);
    due(w);
    while !matches!(fill_stage(w), Poll::Done) {}
    let refused = w.slots.iter().filter(|s| s.fill == Fill::Refused).count();
    assert!(refused > 0, "the bridge's bricks past the cap");
    assert!(w.slots.iter().all(|s| s.run[1] == 0), "no run taken");
    assert!(!wake(w), "no give-back yet");
    way_pool().give(lent);
    assert!(wake(w), "the give-back wakes them");
    assert!(
        w.slots.iter().filter(|s| s.fill == Fill::Refused).count() == 0,
        "all absent"
    );
    assert!(!wake(w), "once for that give-back");
    cut(t, &pkt);
    due(w);
    while !matches!(fill_stage(w), Poll::Done) {}
    assert!(
        w.slots.iter().all(|s| s.fill != Fill::Refused),
        "filled again from the given run, none refused"
    );
    assert!(
        w.slots
            .iter()
            .any(|s| s.fill == Fill::Filled && s.run[1] != 0),
        "with their lines"
    );
}

#[test]
fn slot_given_up_gives_its_run_back() {
    let w = fill_room();
    let (t, tile, pkt) = hilly_tile();
    cache_tile(t, tile);
    cut(t, &pkt);
    due(w);
    while !matches!(fill_stage(w), Poll::Done) {}
    let took: u32 = w.slots.iter().map(|s| s.run[1]).sum();
    assert!(took > 0, "the bridge's lines");
    let far = leaf([CENTER[0] + 100.0 * f64::from(BRICK), CENTER[1]]);
    lay(w, far, far);
    assert_eq!(
        way_pool().take(took),
        Ok([1, took]),
        "their runs joined and taken again"
    );
}

#[test]
fn shared_brick_held_once() {
    CLIPMAP.set(fresh());
    let w = clipmap();
    lay(w, leaf(CENTER), leaf(CENTER));
    assert_eq!(
        w.slots.iter().filter(|b| !b.is_free()).count(),
        (EYE_CLIP_LEVEL_CNT * ENTRY_CNT) as usize,
        "the pivot's clip levels take no slot"
    );
    for pivot in EYE_CLIP_LEVEL_CNT..EYE_CLIP_LEVEL_CNT + 2 {
        let cell_zoom = layout(pivot).1;
        let eye = cell_zoom - FIRST_CELL_ZOOM;
        for s in 0..ENTRY_CNT {
            assert_eq!(want(w, pivot, s), want(w, eye, s));
            assert_eq!(
                slot_of(w, cell_zoom, want(w, pivot, s)),
                Some(eye * ENTRY_CNT + s),
                "in the eye's own slot"
            );
        }
    }
    land_all(w);
    for pivot in EYE_CLIP_LEVEL_CNT..EYE_CLIP_LEVEL_CNT + 2 {
        for s in 0..ENTRY_CNT {
            let row = w.table.entries[(pivot * ENTRY_CNT + s) as usize];
            assert!(
                row.holds(row.brick)
                    && row.slot() == (layout(pivot).1 - FIRST_CELL_ZOOM) * ENTRY_CNT + s,
                "the pivot's rows name the eye's slots"
            );
        }
    }
}

#[test]
fn left_brick_part() {
    CLIPMAP.set(fresh());
    let w = clipmap();
    let (eye, pivot) = (LEAF_CELL_ZOOM - FIRST_CELL_ZOOM, EYE_CLIP_LEVEL_CNT + 1);
    lay(w, leaf(CENTER), leaf(CENTER));
    land_all(w);
    // Move the eye one brick along at the leaves. Its clip level there
    // leaves a column of bricks the pivot's still wants, and wants a column
    // the pivot's does not.
    let on = leaf([CENTER[0] + f64::from(BRICK), CENTER[1]]);
    lay(w, on, leaf(CENTER));
    assert_eq!(
        slots_at(w, LEAF_CELL_ZOOM),
        (ENTRY_CNT + clipmap::ACROSS) as usize
    );
    for s in 0..ENTRY_CNT {
        let q = want(w, pivot, s);
        assert!(
            clip_read().present(LEAF_CELL_ZOOM, q).1,
            "the pivot's bricks stay present"
        );
        assert!(w.table.entries[(pivot * ENTRY_CNT + s) as usize].holds(q));
    }
    let new: Vec<u32> = (0..ENTRY_CNT)
        .filter(|&s| !w.table.entries[(eye * ENTRY_CNT + s) as usize].holds(want(w, eye, s)))
        .collect();
    assert_eq!(
        new.len(),
        clipmap::ACROSS as usize,
        "the eye's new column alone is absent"
    );
    for &s in &new {
        let b = slot_of(w, LEAF_CELL_ZOOM, want(w, eye, s)).unwrap();
        assert!(
            w.slots[b as usize].is_absent() && b != eye * ENTRY_CNT + s,
            "in a free slot, the eye's own still holding the pivot's brick"
        );
    }
    // Moved back, the eye finds the column it left still present.
    land_all(w);
    lay(w, leaf(CENTER), leaf(CENTER));
    for s in 0..ENTRY_CNT {
        assert!(
            w.table.entries[(eye * ENTRY_CNT + s) as usize].holds(want(w, eye, s)),
            "not filled again"
        );
    }
    assert_eq!(
        slots_at(w, LEAF_CELL_ZOOM),
        ENTRY_CNT as usize,
        "the column no clip level wants is given up"
    );
}

#[test]
fn apart_clip_levels_own_slots() {
    CLIPMAP.set(fresh());
    let w = clipmap();
    lay(
        w,
        leaf(CENTER),
        leaf([CENTER[0] + 100.0 * f64::from(BRICK), CENTER[1]]),
    );
    for clip_level in 0..EYE_CLIP_LEVEL_CNT + 2 {
        for s in 0..ENTRY_CNT {
            assert_eq!(
                slot_of(w, layout(clip_level).1, want(w, clip_level, s)),
                Some(clip_level * ENTRY_CNT + s)
            );
        }
    }
}

#[test]
fn handing_shows_everywhere() {
    // The fold after the hand-off reads the bricks' cells, so the test needs
    // planes of its own.
    zeroed_planes();
    CLIPMAP.set(fresh());
    let w = clipmap();
    let (eye, pivot) = (LEAF_CELL_ZOOM - FIRST_CELL_ZOOM, EYE_CLIP_LEVEL_CNT + 1);
    lay(w, leaf(CENTER), leaf(CENTER));
    let q = want(w, eye, 0);
    let b = slot_of(w, LEAF_CELL_ZOOM, q).unwrap();
    w.slots[b as usize].fill = Fill::Filled;
    for i in of_entry(0) {
        show(w, i);
    }
    upload(&mut |_| false);
    assert!(
        w.slots[b as usize].fill == Fill::Filled && !w.slots[b as usize].present,
        "not taken, not present"
    );
    hand();
    for clip_level in [eye, pivot] {
        let row = w.table.entries[(clip_level * ENTRY_CNT) as usize];
        assert!(row.holds(q) && row.slot() == b);
    }
    // A refused brick stays while a clip level wants it, and is given up
    // with the last one.
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
        !slot_wants(w, tile_of(LEAF_CELL_ZOOM, q).0, b),
        "nor is its tile kept for it"
    );
    lay(w, far, far);
    assert_eq!(slot_of(w, LEAF_CELL_ZOOM, q), None);
}
