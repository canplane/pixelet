// pxl-core/domain/layer/terrain/clipmap.test.rs
//
// Tests of the clip levels, and the helpers other tests use to hold clip
// levels around a center with every brick of the given tiles present at
// once, its cells as a fill leaves them.

use super::*;
use crate::domain::layer::terrain::pool::set_way_pool;
use crate::sys;
use pxl_engine::fill;
use pxl_engine::ground::word;
use pxl_engine::value::TRANSPORTS;
use pxl_engine::way::BRIDGE;
use pxl_engine::way_tree::Read;
use pxl_spec::bake::separation::{self, Separation};
use pxl_spec::bake::tile::terrain::{self, GroundId};

/// A tile as `hold` takes it: for each cell, row by row from the first, its
/// own palette index and its height; then the tile's lines.
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

const CELLS_WORDS: usize = (SLOTS * CELLS_ARR_BYTES / 4) as usize;
const PALETTE_WORDS: usize = (SLOTS * PALETTE_ARR_BYTES / 4) as usize;

/// A test's own planes, with `cells` as the cells and a zeroed palette.
fn planes_of(cells: &'static mut [u32]) -> BrickMemory {
    BrickMemory {
        cells,
        palette: vec![0; PALETTE_WORDS].leak(),
    }
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
    let of = |words: &[u32], len: usize| {
        let mut arr = vec![0; len];
        arr[..words.len()].copy_from_slice(words);
        arr.leak()
    };
    PLANES.set(planes_of(of(&b.cells, CELLS_WORDS)));
    set_way_pool(b.pool.clone());
    CLIPMAP.set(fresh());
    clipmap().table.entries.copy_from_slice(&b.entries);
    clipmap().slots.copy_from_slice(&b.slots);
    fold(clipmap());
}

/// Marks the brick at `cell_zoom` that holds cell `g` as filled and not yet
/// handed over, and folds the clip levels without it, as a turn's step leaves
/// them before a frame hands the brick over.
pub(crate) fn unland(cell_zoom: Zoom, g: [i32; 2]) {
    let w = clipmap();
    let b = slot_of(w, cell_zoom, clipmap::brick_of(g).0).expect("held");
    (w.slots[b as usize].present, w.slots[b as usize].fill) = (false, Fill::Filled);
    for i in of_entry(b % ENTRIES) {
        show(w, i);
    }
    fold(w);
}

/// Hands every filled brick over, as a frame with room for all of them does.
pub(crate) fn hand() {
    upload(&mut |_| true);
}

/// Loses the device, as a turn's step finds it lost: the GPU's copy of
/// every brick is gone, and each present brick must be handed over again
/// (`assign_slots`).
pub(crate) fn lose() {
    assign_slots(clipmap(), true);
}

/// Builds `hold`'s clip levels, laying each tree into the pool as a fill
/// lays it.
fn build(center: [f64; 2], tiles: &[TileData]) {
    PLANES.set(planes_of(vec![0; CELLS_WORDS].leak()));
    set_way_pool(WayPool::default());
    let mut pool = vec![0; 16 * (way_tree::BRICK_CHILD_GROUPS as usize + 1)];
    let mut order = vec![0; way_tree::BRICK_CHILD_GROUPS as usize + 1];
    CLIPMAP.set(fresh());
    let w = clipmap();
    for clip_level in 0..CLIP_LEVELS {
        let (center_of, cell_zoom) = layout(clip_level);
        if center_of == Center::Destination {
            continue;
        }
        let first = clipmap::origin(cell_zoom, cell_at(cell_zoom, center));
        w.table.entries[(SLOTS + clip_level) as usize] = Entry::at(first, 1);
        for s in 0..ENTRIES {
            w.table.entries[(clip_level * ENTRIES + s) as usize].brick =
                entry_brick(cell_zoom, first, s);
        }
    }
    assign_slots(w, false);
    for b in 0..SLOTS {
        let Slot {
            cell_zoom, brick, ..
        } = w.slots[b as usize];
        if w.slots[b as usize].is_free() {
            continue;
        }
        let (t, place) = tile_of(cell_zoom, brick);
        let Some((_, palette_of, heights, separation)) = tiles.iter().find(|(p, _, _, _)| *p == t)
        else {
            continue;
        };
        let side = GROUND.tile_cells();
        let at = |k: u32| {
            let [x, y] = clipmap::of_idx(k);
            ((place[1] * BRICK + y) * side + place[0] * BRICK + x) as usize
        };
        let (mut segments, mut lines) = (
            vec![(0, 0); WAYS_BRICK_SEGMENTS as usize],
            vec![0; LINES_WORDS as usize],
        );
        let n = brick_lines(t, separation, place, &mut segments, &mut lines).unwrap();
        let laid = terrain::TileBrick {
            cell_zoom,
            place,
            first: terrain::first_cell(t, cell_zoom, place),
            lines: &lines[..n],
            palette_idxs: |k: u32| word([u32::from(palette_of[at(k)]); 4]),
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
            vec![0u32; BRICK_WORDS as usize],
            vec![0u32; SPANS_WORDS as usize],
        );
        fill_columns(
            &laid,
            |k| i32::from(heights[at(k)]),
            &mut back,
            &mut scratch,
        );
        let n = clipmap::OWN_CELLS;
        for k in 0..clipmap::OWN_WORDS {
            back[(clipmap::OWN_AT + k) as usize] = (0..n)
                .map(|j| u32::from(palette_of[at(n * k + j)]) << (8 * j))
                .sum();
        }
        fill::mips(&mut back, &scratch, cell_zoom);
        let [used, cnt, root] = fill::tree(&back, &scratch, cell_zoom, &mut pool, 1);
        assert_ne!(used, u32::MAX, "a brick's tree past the pool");
        if cnt != 0 {
            let run = way_pool().take(cnt).expect("a brick's tree past the pool");
            let built = Read(&pool);
            back[ROOT_AT as usize] = way_tree::lay(&built, root, way_pool(), run[0], &mut order);
            w.slots[b as usize].tree = run;
        }
        let at = (b * CELLS_ARR_BYTES / 4) as usize;
        planes().cells[at..at + BRICK_WORDS as usize].copy_from_slice(&back);
        w.slots[b as usize].present = true;
    }
    for i in 0..SLOTS {
        show(w, i);
    }
}

/// Writes a brick's columns as a fill does (pxl-engine `fill::columns`,
/// `fill::spans`), but over cells whose tops `top` gives instead of
/// computing them from samples. Each column's word, its top, goes into
/// `back`, and the spans of its columns and of its ring, from its lines
/// (`fill::keep`), go into `scratch`.
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
        fill::keep(sq, scratch, i);
    }
}

/// Lays out the eye's clip levels around `eye` and the pivot's around
/// `pivot`, in units, X then Y, as `step` lays out a clip level that moved,
/// with no destination. Their bricks are given slots (`assign_slots`), and
/// what was held before is kept.
fn lay(w: &mut Clipmap, eye: [f64; 2], pivot: [f64; 2]) {
    for clip_level in 0..CLIP_LEVELS {
        let (center_of, cell_zoom) = layout(clip_level);
        let p = match center_of {
            Center::Eye => eye,
            Center::Pivot => pivot,
            Center::Destination => continue,
        };
        let first = clipmap::origin(cell_zoom, cell_at(cell_zoom, p));
        w.table.entries[(SLOTS + clip_level) as usize] = Entry::at(first, 1);
        for s in 0..ENTRIES {
            w.table.entries[(clip_level * ENTRIES + s) as usize].brick =
                entry_brick(cell_zoom, first, s);
        }
    }
    assign_slots(w, false);
}

/// Marks every held brick present, as if each had been handed over. Its
/// cells are all zero unless a test held its own (`hold`). The clip levels
/// fold over them wherever they are laid out again (`assign_slots`).
fn land_all(w: &mut Clipmap) {
    PLANES.get(|| planes_of(vec![0; CELLS_WORDS].leak()));
    for b in w.slots.iter_mut().filter(|b| !b.is_free()) {
        b.present = true;
    }
    for i in 0..SLOTS {
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
    w.table.entries[(clip_level * ENTRIES + s) as usize].brick
}

const CENTER: [f64; 2] = [37.0 * 4096.0 + 100.0, 11.0 * 4096.0 + 200.0];

/// The leaf tile `CENTER` lies in.
const CENTER_TILE: [i32; 2] = [296, 88];

/// The clip levels around `CENTER` with nothing held, and everything a fill
/// works in owned by the test.
fn fill_room() -> &'static mut Clipmap {
    PLANES.set(planes_of(vec![0; CELLS_WORDS].leak()));
    set_way_pool(WayPool::default());
    FILL.set(FillScratch {
        ele: vec![0; ELE_COLUMN].leak(),
        lines: vec![0; LINES_WORDS as usize].leak(),
        pairs: vec![(0, 0); WAYS_BRICK_SEGMENTS as usize].leak(),
        spans: vec![0; SPANS_WORDS as usize].leak(),
        build: vec![0; BUILD_WORDS].leak(),
        order: vec![0; way_tree::BRICK_CHILD_GROUPS as usize + 1].leak(),
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
        TRANSPORTS[0] as u8,
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
    use pxl_engine::object::{self, Cut};
    let _g = sys::serial();
    let w = fill_room();
    let (t, tile, pkt) = hilly_tile();
    cache_tile(t, tile.clone());
    cut(t, &pkt);
    let wanted: Vec<u32> = (0..SLOTS)
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
        "each cut in the turn the packet landed"
    );
    // Drop the packet, so the fill can read only the tile and the slots.
    drop(pkt);
    due(w);
    while !matches!(fill_stage(w), Poll::Done) {}
    let mut ways = 0;
    for &b in &wanted {
        let slot = w.slots[b as usize];
        assert!(slot.fill == Fill::Filled, "filled, not yet handed");
        assert!(!slot.present, "not present before a frame hands it");
        let (cell_zoom, place) = (slot.cell_zoom, tile_of(slot.cell_zoom, slot.brick).1);
        let mut pairs = vec![(0, 0); WAYS_BRICK_SEGMENTS as usize];
        let mut lines = vec![0; LINES_WORDS as usize];
        let n = brick_lines(t, &tile.separation, place, &mut pairs, &mut lines).unwrap();
        let site = fill::Site {
            cell_zoom,
            place,
            first: slot.brick.map(|v| v << BRICK_SHIFT),
            shard: Shard::UNKNOWN.at(&ServiceArea, (clipmap::bricks_zoom(cell_zoom), slot.brick)),
            ele: &tile.ele,
            lines: &lines[..n],
        };
        let four = |i: u32| {
            let [x, y] = clipmap::of_idx(i);
            let c = ((place[1] * BRICK + y) * GROUND.tile_cells() + place[0] * BRICK + x) as usize;
            let at = 4 * c;
            word([0, 1, 2, 3].map(|k| u32::from(tile.cells[at + k])))
        };
        let (mut lane, mut spans) = (vec![0; BRICK_WORDS as usize], vec![0; SPANS_WORDS as usize]);
        fill::brick(&mut lane, &mut spans, four, &site);
        let mut build = vec![0; BUILD_WORDS];
        let [used, cnt, root] = fill::tree(&lane, &spans, cell_zoom, &mut build, 1);
        assert_ne!(used, u32::MAX);
        let got = cells(b);
        assert_eq!(
            got[..ROOT_AT as usize],
            lane[..ROOT_AT as usize],
            "its words as a fill at once writes them"
        );
        assert_eq!(
            slot.tree[1], cnt,
            "its tree's run, the child groups it holds"
        );
        assert!((0..BRICK_CELLS).all(|i| palette(b)[i as usize] == four(i)));
        ways += u32::from(cnt != 0);
        let built = Read(&build);
        let (pool, laid) = (way_pool(), got[ROOT_AT as usize]);
        for i in 0..BRICK_CELLS {
            let [x, y] = clipmap::of_idx(i);
            for z in 20..60 {
                let at = [x as i32, y as i32, z];
                let was = way_tree::cursor(&built, root, cell_zoom, cell_zoom, at);
                let now = way_tree::cursor(pool, laid, cell_zoom, cell_zoom, at);
                assert_eq!(
                    object::val(&built, was, Cut::NONE, cell_zoom, at),
                    object::val(pool, now, Cut::NONE, cell_zoom, at),
                    "its tree read as built"
                );
            }
        }
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
}

#[test]
fn void_tile_bricks_filled_with_no_packet() {
    let _g = sys::serial();
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
        "filled off the void's palette index, over samples all zero"
    );
}

#[test]
fn spent_turn_runs_one_part_and_asks_the_next() {
    use crate::sys::sched::edge::world_fall;
    let _g = sys::serial();
    let w = fill_room();
    let (t, tile, pkt) = hilly_tile();
    cache_tile(t, tile);
    cut(t, &pkt);
    world_edge().rise(0.0);
    world_fall();
    fill_share();
    assert!(w.filling.is_some(), "begun, and no further");
    assert_eq!(world_fall(), 0.0, "the next turn asked");
    let stage = |w: &Clipmap| w.filling.map(|f| f.stage);
    fill_share();
    assert!(
        matches!(stage(w), Some(Stage::Spans(0))),
        "a part a spent turn, so a turn past its share still moves the fill"
    );
    // A turn with room takes as much as it has.
    world_edge().rise(1e9);
    fill_share();
    assert!(w.due.is_empty() && w.filling.is_none());
    assert!(w.slots.iter().any(|s| s.fill == Fill::Filled));
    assert_eq!(
        world_fall(),
        0.0,
        "what is filled asks the frame that hands it"
    );
}

#[test]
fn brick_let_go_ends_its_fill() {
    let _g = sys::serial();
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
    assert_eq!(way_pool().top(), 1, "no tree of it in the pool");
}

#[test]
fn slot_given_up_gives_its_tree_back() {
    let _g = sys::serial();
    let w = fill_room();
    let (t, tile, pkt) = hilly_tile();
    cache_tile(t, tile);
    cut(t, &pkt);
    due(w);
    while !matches!(fill_stage(w), Poll::Done) {}
    let took: u32 = w.slots.iter().map(|s| s.tree[1]).sum();
    assert!(took > 0, "the bridge's trees");
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
        let cell_zoom = layout(pivot).1;
        let eye = cell_zoom - FIRST_CELL_ZOOM;
        for s in 0..ENTRIES {
            assert_eq!(want(w, pivot, s), want(w, eye, s));
            assert_eq!(
                slot_of(w, cell_zoom, want(w, pivot, s)),
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
                row.holds(row.brick)
                    && row.slot() == (layout(pivot).1 - FIRST_CELL_ZOOM) * ENTRIES + s,
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
    let (eye, pivot) = (LEAF_CELL_ZOOM - FIRST_CELL_ZOOM, EYE_CLIP_LEVELS + 1);
    lay(w, leaf(CENTER), leaf(CENTER));
    land_all(w);
    // Move the eye one brick along at the leaves. Its clip level there
    // leaves a column of bricks the pivot's still wants, and wants a column
    // the pivot's does not.
    let on = leaf([CENTER[0] + f64::from(BRICK), CENTER[1]]);
    lay(w, on, leaf(CENTER));
    assert_eq!(
        slots_at(w, LEAF_CELL_ZOOM),
        (ENTRIES + clipmap::ACROSS) as usize
    );
    for s in 0..ENTRIES {
        let q = want(w, pivot, s);
        assert!(
            CpuClipmap.present(LEAF_CELL_ZOOM, q).1,
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
        let b = slot_of(w, LEAF_CELL_ZOOM, want(w, eye, s)).unwrap();
        assert!(
            w.slots[b as usize].is_absent() && b != eye * ENTRIES + s,
            "in a free slot, the eye's own still holding the pivot's brick"
        );
    }
    // Moved back, the eye finds the column it left still present.
    land_all(w);
    lay(w, leaf(CENTER), leaf(CENTER));
    for s in 0..ENTRIES {
        assert!(
            w.table.entries[(eye * ENTRIES + s) as usize].holds(want(w, eye, s)),
            "not filled again"
        );
    }
    assert_eq!(
        slots_at(w, LEAF_CELL_ZOOM),
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
fn handing_shows_everywhere() {
    let _g = sys::serial();
    // The fold after the hand-off reads the bricks' cells, so the test needs
    // planes of its own.
    PLANES.set(planes_of(vec![0; CELLS_WORDS].leak()));
    CLIPMAP.set(fresh());
    let w = clipmap();
    let (eye, pivot) = (LEAF_CELL_ZOOM - FIRST_CELL_ZOOM, EYE_CLIP_LEVELS + 1);
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
        let row = w.table.entries[(clip_level * ENTRIES) as usize];
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

#[test]
fn heap_wait_rehanded() {
    use crate::sys::boot::test::set_heap_stat;
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
    let eye = LEAF_CELL_ZOOM - FIRST_CELL_ZOOM;
    let q = want(w, eye, 0);
    let b = slot_of(w, LEAF_CELL_ZOOM, q).unwrap();
    // The state a fill leaves when the heap refused the pool a chunk.
    (w.slots[b as usize].fill, w.heap) = (Fill::Waiting, Some(Refusal::now()));
    assert!(!wake(w), "no page given back: it waits");
    lay(w, leaf(CENTER), leaf(CENTER));
    assert!(
        w.slots[b as usize].fill == Fill::Waiting,
        "unhanded while it waits"
    );
    assert!(
        !slot_wants(w, tile_of(LEAF_CELL_ZOOM, q).0, b),
        "nor its tile kept for it"
    );
    RELEASES.fetch_add(1, Ordering::Relaxed);
    assert!(wake(w) && w.heap.is_none());
    assign_slots(w, false);
    assert!(
        w.slots[b as usize].is_absent() && slot_wants(w, tile_of(LEAF_CELL_ZOOM, q).0, b),
        "absent, to be handed again"
    );
    set_heap_stat(HeapStat::default);
}
