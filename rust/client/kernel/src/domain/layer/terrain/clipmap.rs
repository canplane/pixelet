// pxl-kernel/domain/layer/terrain/clipmap.rs

//! The ground's cells as the kernel holds them: a clipmap level a zoom about each
//! center a reader declares — the eye, the pivot where walking and editing
//! happen, a destination being gone to — each as deep as its reader reads
//! (pxl-engine `clipmap`). A brick at a zoom is held once, in a slot,
//! however many clipmap levels want it (`Slot`). It is filled here, from its
//! tile: its four palette indices cut off the packet in the turn it lands, which
//! then goes (`cut`); its cells and its ways' tree off those and its tile's
//! samples and lines, past the frame, as the turn's tasks' share leaves room
//! (`fill_share`), into `.fixed.terrain` and the ways' pool (pool.rs). The
//! next frame hands it to the GPU, and it is present from then on (`upload`).
//! Nothing of a tile is kept for a brick still to come: one that enters asks
//! its tile again (`TileCache::again`).

use pxl_engine::area::{Area, Shard};
use pxl_engine::clipmap::{
    self, Bricks, Entry, Presence, BRICK, BRICK_CELLS, BRICK_LEVELS, BRICK_SHIFT, BRICK_WORDS,
    CLIP_LEVELS, ENTRIES, EYE_CLIP_LEVELS, FIRST_CELL, LINES_WORDS, ROOT_AT, SLOTS, SPANS_ACROSS,
    SPANS_WORDS, WAYS_BRICK_SEGMENTS,
};
use pxl_engine::fill;
use pxl_engine::ground::word;
use pxl_engine::way::tree;
use pxl_engine::wrap_idx;
use pxl_engine::LEAF_CELL;
use pxl_render::gpu::api;
use pxl_render::gpu::ray::{BrickWrite, TerrainWrite};
use pxl_spec::bake::tile::terrain::{
    brick_lines, palette_cells, GroundId, TerrainTile, ELE_COLUMN, PALETTE_COLUMN,
};
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::layer::lattice::{GROUND, TILE_CELLS};
use pxl_spec::space::layer::quadrant::{Quadrant, LEAF_ZOOM};
use xpute_core::global::Global;
use xpute_kit::collection::list::List;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::domain::layer::terrain::fixed::{
    cells_off, fill_off, palette_off, BUILD_WORDS, CELLS_ARR_BYTES, PALETTE_ARR_BYTES, RANGE,
};
use crate::domain::layer::terrain::pool::{way_pool, Run, Unlaid, WayPool};
use crate::domain::layer::terrain::tile;
use crate::domain::layer::Base;
use crate::sys::io::room::Refusal;
use crate::sys::io::source::ServiceArea;
use crate::sys::sched::edge::{request_world_tick, world_edge};
use xpute_core::mem::range::arr;

/// Who a clip level is about.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Center {
    Eye,
    Pivot,
    Destination,
}

/// The tiles the clip levels apart from the eye's want at most: a clip level
/// spans two tiles a side.
pub const PINNED_TILES: u32 = (CLIP_LEVELS - EYE_CLIP_LEVELS) * 4;
const _: () = assert!(
    clipmap::CLIP_SIZE <= TILE_CELLS + 1,
    "a clip level spans two tiles a side"
);

/// Who clip level `w` is about, and its cells' zoom as laid out; a
/// destination's is its line's.
const fn layout(w: u32) -> (Center, Zoom) {
    match w {
        w if w < EYE_CLIP_LEVELS => (Center::Eye, clipmap::eye_cell(w)),
        w if w < EYE_CLIP_LEVELS + 2 => (Center::Pivot, LEAF_CELL - 1 + (w - EYE_CLIP_LEVELS)),
        _ => (Center::Destination, LEAF_CELL),
    }
}

/// A clip level: its cells' zoom. Its first brick is its row past the entries
/// (`first_of`).
#[derive(Clone, Copy, PartialEq, Eq)]
struct ClipLevel {
    cell: Zoom,
}

/// Where a brick of the ground is held: its array of cells in `.fixed.terrain`
/// (`cells`) and its row of the GPU's bricks (pxl-render gpu/ray/ground.rs
/// `GROUND_BYTES`), one a brick at a zoom however many
/// clip levels want it. Clip levels at one zoom want a brick in the same entry
/// (`clipmap::entry`), so it is held in one of that entry's slots, `CLIP_LEVELS`
/// of them (`of_entry`); no more bricks want an entry than there are
/// clip levels, so a brick wanted always finds one free (`assign_slots`), and
/// clip levels that overlap take no slot more than the bricks they want. A
/// clip level takes its own where it is free: clip levels that never overlapped
/// hold their bricks as if alone, but once two have overlapped and
/// parted, one may hold its bricks in the other's.
#[derive(Clone, Copy)]
struct Slot {
    /// Its brick's cells' zoom, 0 where it holds none (`is_free`), no
    /// clip level's.
    cell: Zoom,
    brick: [i32; 2],
    present: bool,
    fill: Fill,
    /// Its ways' tree in the pool, given back with the slot.
    tree: Run,
}

/// Where a slot's brick stands with its fill.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fill {
    /// No fill going on or to come, nor refused or waiting.
    Idle,
    /// Its four palette indices cut off its tile's packet (`cut`): its fill is
    /// due (`due`).
    Cut,
    /// Its cells whole here and its tree in the pool, to be handed to the GPU
    /// (`upload`), not yet present.
    Filled,
    /// Its ways' tree refused, the brick past its own most or the pool at its
    /// cap, neither of which asking again changes: not absent, so the brick is
    /// not filled again every turn, nor its tile asked for it, while a clip
    /// level wants it (`assign_slots`). A reader reads the coarser clip
    /// level's, as for any brick absent.
    Refused,
    /// Its child groups refused the pool's room by the heap
    /// (`pool::Unlaid::Heap`): not absent, as `Refused`, until the heap
    /// gives a page back (`Clipmap::heap`), when it is absent again and
    /// filled again (`step`).
    Waiting,
}

const _: () = assert!(FIRST_CELL > 0, "a free slot's zoom is no clip level's");

impl Slot {
    const FREE: Slot = Slot {
        cell: 0,
        brick: [i32::MIN; 2],
        present: false,
        fill: Fill::Idle,
        tree: [0; 2],
    };

    fn is_free(&self) -> bool {
        self.cell == 0
    }

    /// Whether it holds brick `q` at `cell`, present or not.
    fn holds(&self, cell: Zoom, q: [i32; 2]) -> bool {
        self.cell == cell && self.brick == q
    }

    /// Wanted, nothing of its tile held for it: holding a brick, neither
    /// present nor on its way nor refused.
    fn is_absent(&self) -> bool {
        !self.is_free() && !self.present && self.fill == Fill::Idle
    }
}

/// The slots of entry `s`, and every clip level's entry `s`: `k × ENTRIES + s`,
/// `k` to `CLIP_LEVELS`.
fn of_entry(s: u32) -> impl Iterator<Item = u32> {
    (0..CLIP_LEVELS).map(move |k| k * ENTRIES + s)
}

/// The clip levels' table, rows of four words end to end, as the GPU binds it
/// (pxl-render clip.wgsl).
#[repr(C)]
struct Table {
    /// Every clip level's entries, each its brick and its slot's state, then
    /// each clip level's first brick, a row shaped as an entry's, its brick the
    /// first and present while it is laid out, none while its center is not
    /// (pxl-engine `clipmap::ORIGIN_AT`).
    entries: [Entry; (SLOTS + CLIP_LEVELS) as usize],
    /// The eye's clip levels' levels over their bricks, folded over the
    /// bricks as they are held here (`fold`), the zero entry, none, before
    /// any is (`clipmap::JOINED_AT`).
    joined: [[u32; clipmap::JOINED_WORDS as usize];
        (EYE_CLIP_LEVELS * clipmap::JOINED_ENTRIES) as usize],
}

struct Clipmap {
    clip_levels: [ClipLevel; CLIP_LEVELS as usize],
    /// What the GPU reads of them, as it is (`table`).
    table: Table,
    /// The bricks the clip levels want, each in one slot (`Slot`).
    slots: [Slot; SLOTS as usize],
    /// The device the entries' bricks were handed to (`device_generation`):
    /// one that went took them all.
    on: u32,
    /// What the entries were last stepped over: the tile cache's revision
    /// (`TileCache::revision`), and whether a brick went absent again since,
    /// which moves what is asked again.
    read: u32,
    filled: bool,
    /// The tiles the clip levels apart from the eye's want (`pinned_tiles`).
    pinned: List<Quadrant, { PINNED_TILES as usize }>,
    /// The slots to fill, the next last (`due`), and whether they stand for
    /// the slots as they are.
    due: List<u32, { SLOTS as usize }>,
    due_stale: bool,
    /// The brick being filled, whose fill goes on over the turns it takes.
    filling: Option<Filling>,
    /// The heap as it was when it first refused a brick's child groups the
    /// pool's room, while one waits on it (`Fill::Waiting`).
    heap: Option<Refusal>,
}

static CLIPMAP: Global<Clipmap> = Global::new();

fn clipmap() -> &'static mut Clipmap {
    CLIPMAP.get(fresh)
}

/// Clip level `clip_level`'s first brick, none while its center is not.
fn first_of(w: &Clipmap, clip_level: u32) -> Option<[i32; 2]> {
    let (first, laid) = w.table.entries[(SLOTS + clip_level) as usize].brick();
    laid.then_some(first)
}

/// The clip levels before any center stands: every entry absent.
fn fresh() -> Clipmap {
    Clipmap {
        clip_levels: core::array::from_fn(|w| ClipLevel {
            cell: layout(w as u32).1,
        }),
        table: Table {
            entries: [Entry::absent([i32::MIN; 2]); (SLOTS + CLIP_LEVELS) as usize],
            joined: [[0; clipmap::JOINED_WORDS as usize];
                (EYE_CLIP_LEVELS * clipmap::JOINED_ENTRIES) as usize],
        },
        slots: [Slot::FREE; SLOTS as usize],
        on: 0,
        read: 0,
        filled: true,
        pinned: List::new(),
        due: List::new(),
        due_stale: true,
        filling: None,
        heap: None,
    }
}

/// The cell at `cell` a point `p`, in units, lies in, X wrapped.
fn cell_at(cell: Zoom, p: [f64; 2]) -> [i32; 2] {
    let side = units(cell);
    [
        wrap_idx(cell, (p[0] / side).floor() as i32),
        (p[1] / side).floor() as i32,
    ]
}

/// The brick clip level `first` holds in entry `s`.
fn entry_brick(cell: Zoom, first: [i32; 2], s: u32) -> [i32; 2] {
    let m = (clipmap::ACROSS - 1) as i32;
    let at = |o: i32, k: i32| o + ((k - (o & m)) & m);
    let q = [
        at(first[0], (s % clipmap::ACROSS) as i32),
        at(first[1], (s / clipmap::ACROSS) as i32),
    ];
    [wrap_idx(cell - BRICK_SHIFT, q[0]), q[1]]
}

/// The tile brick `q` at `cell` lies in, and where in it, in bricks.
pub(crate) fn tile_of(cell: Zoom, q: [i32; 2]) -> (Quadrant, [u32; 2]) {
    let per = (GROUND.tile_cells() >> BRICK_SHIFT) as i32;
    let zoom = cell - GROUND.tile_cell_shamt();
    let t = Quadrant::new(zoom, [q[0].div_euclid(per), q[1].div_euclid(per)]);
    (
        t,
        [q[0].rem_euclid(per) as u32, q[1].rem_euclid(per) as u32],
    )
}

/// Every brick's planes on the CPU, in `.fixed.terrain` (fixed.rs); a test's
/// own. Its ways' trees' child groups are the heap's (pool.rs).
pub(crate) struct Planes {
    /// Each brick's words whole (`clipmap::BRICK_WORDS`), `CELLS_ARR_BYTES`
    /// apart.
    pub cells: &'static mut [u32],
    /// Each brick's four palette indices a cell, a word each, in its cells'
    /// order, `PALETTE_ARR_BYTES` apart.
    pub palette: &'static mut [u32],
}

pub(crate) static PLANES: Global<Planes> = Global::new();

fn planes() -> &'static mut Planes {
    PLANES.get(|| {
        let r = RANGE.get();
        // SAFETY: the planes are the clip levels' alone, plain numbers, zeroed
        // at boot.
        unsafe {
            Planes {
                cells: arr(r, cells_off(0), (SLOTS * CELLS_ARR_BYTES / 4) as usize),
                palette: arr(r, palette_off(0), (SLOTS * PALETTE_ARR_BYTES / 4) as usize),
            }
        }
    })
}

/// The eye's clip levels' levels over their bricks folded again, a level at a
/// time (pxl-engine `clipmap::fold`), over the bricks as they are held here:
/// the table carries them to the GPU (`table`). The writers of a brick
/// a clip level wants or its presence are `assign_slots`, a turn's step's,
/// which folds before it returns, and the bricks a frame hands (`upload`),
/// folded once after all of them: no read of a bound (`CpuClipmap`) comes
/// between a write and its fold, so none finds one of other bricks than its
/// read of the cells does. Read through the clip levels it is handed
/// (`Holding`), which its writer holds while it runs.
fn fold(w: &mut Clipmap) {
    for l in 1..=clipmap::JOINED_LEVELS {
        let blocks = (clipmap::ACROSS >> l) * (clipmap::ACROSS >> l);
        for clip_level in 0..EYE_CLIP_LEVELS {
            for b in 0..blocks {
                let e = clipmap::fold(&Holding(w), FIRST_CELL + clip_level, l, b);
                let at = clip_level * clipmap::JOINED_ENTRIES + clipmap::joined_idx(l, [0, 0]) + b;
                w.table.joined[at as usize] = e;
            }
        }
    }
}

/// The ways' pool, every held brick's tree's child groups, which a cursor reads
/// as the cast reads the GPU's copy (pxl-engine `way::tree`).
pub(in crate::domain) fn pool() -> &'static WayPool {
    way_pool()
}

/// The child groups the pool spans, which the GPU's copy of it holds.
pub fn pool_room() -> u32 {
    way_pool().top()
}

/// Slot `b`'s words: a brick's words whole, from its first, which pxl-engine
/// reads a brick's words by (`fill::column_of`).
fn cells(b: u32) -> &'static [u32] {
    let at = (b * CELLS_ARR_BYTES / 4) as usize;
    &planes().cells[at..at + BRICK_WORDS as usize]
}

/// Slot `b`'s words to write, a fill's alone while it holds it.
fn cells_mut(b: u32) -> &'static mut [u32] {
    let at = (b * CELLS_ARR_BYTES / 4) as usize;
    &mut planes().cells[at..at + BRICK_WORDS as usize]
}

/// Slot `b`'s four palette indices a cell.
fn palette(b: u32) -> &'static [u32] {
    let at = (b * PALETTE_ARR_BYTES / 4) as usize;
    &planes().palette[at..at + BRICK_CELLS as usize]
}

/// Whether the runtime holds what slot `b` was last handed from: nothing writes
/// it until it gives it back (pxl-render gpu/api.rs `in_flight`).
fn in_flight(b: u32) -> bool {
    api::in_flight(cells(b)) || api::in_flight(palette(b))
}

/// The turn's clip levels: each center where it now stands (`at`, in units, X
/// then Y, by `Centre`; none where it is not), a brick no clip level holds any
/// more given up (`assign_slots`); the tiles a clip level now wants but holds
/// nothing of are asked again. The entries are stepped only where a clip level
/// moved a brick or its zoom, the device went, the tile cache moved
/// (`TileCache::revision`) or a brick went absent again: of those alone are
/// they made. Before the turn's arrivals and its stage. True where the tiles
/// the clip levels apart from the eye's want changed.
pub fn step(at: [Option<[f64; 2]>; 3], on: u32) -> bool {
    let w = clipmap();
    let terrain = TerrainTile::tile_cache();
    let lost = core::mem::replace(&mut w.on, on) != on;
    // Per clip level, whether its zoom moved, and whether it moved at all.
    let (mut moved, mut changed) = ([false; CLIP_LEVELS as usize], [lost; CLIP_LEVELS as usize]);
    let released = wake(w);
    let mut stale =
        lost || released || core::mem::take(&mut w.filled) || terrain.revision() != w.read;
    for clip_level in 0..CLIP_LEVELS {
        let (center, laid) = layout(clip_level);
        let p = at[center as usize];
        let cell = match (center, p) {
            // Where its line says the ground: open sea's leaf is the tile over it.
            (Center::Destination, Some(p)) => {
                let leaf = Quadrant::new(LEAF_ZOOM, cell_at(LEAF_ZOOM, p));
                terrain.src(leaf).map_or(laid, |t| GROUND.cell_zoom(t.zoom))
            }
            _ => laid,
        };
        let first = p.map(|p| clipmap::origin(cell, cell_at(cell, p)));
        let was = (
            core::mem::replace(&mut w.clip_levels[clip_level as usize], ClipLevel { cell }).cell,
            first_of(w, clip_level),
        );
        w.table.entries[(SLOTS + clip_level) as usize] = match first {
            Some(f) => Entry::at(f, 1),
            None => Entry::absent([i32::MIN; 2]),
        };
        moved[clip_level as usize] = was.0 != cell;
        changed[clip_level as usize] |= was != (cell, first);
        stale |= was != (cell, first);
    }
    if !stale {
        return false;
    }
    // A clip level that did not move wants its bricks where they are.
    for clip_level in 0..CLIP_LEVELS {
        let (cell, first) = (
            w.clip_levels[clip_level as usize].cell,
            first_of(w, clip_level),
        );
        let Some(f) = first.filter(|_| changed[clip_level as usize]) else {
            continue;
        };
        for s in 0..ENTRIES {
            let entry = &mut w.table.entries[(clip_level * ENTRIES + s) as usize];
            let want = entry_brick(cell, f, s);
            if moved[clip_level as usize] || lost || entry.brick != want {
                entry.brick = want;
                terrain.moved_at(tile_of(cell, want).0);
            }
        }
    }
    // The slots follow the entries' bricks and the device: where neither
    // moved nor the heap woke a brick, every brick wanted holds its slot and
    // each entry shows it as its handing left it.
    if released || changed.contains(&true) {
        assign_slots(w, lost);
    }
    // Tiles are asked again only as far as the slots a packet comes in spare
    // them, in slot order; the rest once room is given back. Only an absent
    // brick's tile is asked of again.
    let mut room = tile::again_room();
    for b in 0..SLOTS {
        let held = w.slots[b as usize];
        if held.is_absent() {
            let t = tile_of(held.cell, held.brick).0;
            if room > 0 && !terrain.is_again_held(t) {
                terrain.again(t);
                room -= u32::from(terrain.is_again_held(t));
            }
        }
    }
    w.due_stale = true;
    w.read = terrain.revision();
    // What the clip levels apart from the eye's want moves only with them.
    if !changed[EYE_CLIP_LEVELS as usize..].contains(&true) {
        return false;
    }
    let was = core::mem::take(&mut w.pinned);
    for clip_level in EYE_CLIP_LEVELS..CLIP_LEVELS {
        let cell = w.clip_levels[clip_level as usize].cell;
        if first_of(w, clip_level).is_none() {
            continue;
        }
        for s in 0..ENTRIES {
            let t = tile_of(
                cell,
                w.table.entries[(clip_level * ENTRIES + s) as usize].brick,
            )
            .0;
            if !w.pinned.contains(&t) {
                w.pinned.push(t);
            }
        }
    }
    was[..] != w.pinned[..]
}

/// Each brick waiting on the heap absent again, to be filled again, where
/// the heap gave a page back since it refused one the pool's room
/// (`Fill::Waiting`): true where it did.
fn wake(w: &mut Clipmap) -> bool {
    if !w.heap.is_some_and(|h| h.over()) {
        return false;
    }
    w.heap = None;
    for held in w.slots.iter_mut().filter(|b| b.fill == Fill::Waiting) {
        held.fill = Fill::Idle;
    }
    true
}

/// Each brick a laid clip level wants held in one slot: the device that went
/// took the GPU's copy of every slot's cells, each present handed again; a slot
/// no clip level wants any more is given up, its tree with it, a fill going on
/// in it ending (`fill_stage`); and a brick wanted that no slot holds takes a
/// free one of its entry's, its clip level's own first, absent. Then every
/// entry's row shows its slot (`show`), and the clip levels are folded over
/// them (`fold`).
fn assign_slots(w: &mut Clipmap, lost: bool) {
    for b in 0..SLOTS as usize {
        let held = w.slots[b];
        if held.is_free() {
            continue;
        }
        let s = clipmap::entry(held.brick);
        let wants = |clip_level: u32| {
            w.clip_levels[clip_level as usize].cell == held.cell
                && first_of(w, clip_level).is_some()
                && w.table.entries[(clip_level * ENTRIES + s) as usize].brick == held.brick
        };
        if !(0..CLIP_LEVELS).any(wants) {
            way_pool().give(held.tree);
            w.slots[b] = Slot::FREE;
        } else if lost && held.present {
            (w.slots[b].present, w.slots[b].fill) = (false, Fill::Filled);
        }
    }
    for clip_level in 0..CLIP_LEVELS {
        let cell = w.clip_levels[clip_level as usize].cell;
        if first_of(w, clip_level).is_none() {
            continue;
        }
        for s in 0..ENTRIES {
            let (own, q) = (
                clip_level * ENTRIES + s,
                w.table.entries[(clip_level * ENTRIES + s) as usize].brick,
            );
            if slot_of(w, cell, q).is_some() {
                continue;
            }
            let free = |b: &u32| w.slots[*b as usize].is_free();
            let b = Some(own).filter(free).or_else(|| of_entry(s).find(free));
            // No more bricks want an entry than it has slots.
            let b = b.or_bug(Errno::ENOTRECOVERABLE);
            w.slots[b as usize] = Slot {
                cell,
                brick: q,
                ..Slot::FREE
            };
        }
    }
    for i in 0..SLOTS {
        show(w, i);
    }
    fold(w);
}

/// The slot holding brick `q` at `cell`, present or not.
fn slot_of(w: &Clipmap, cell: Zoom, q: [i32; 2]) -> Option<u32> {
    of_entry(clipmap::entry(q)).find(|&b| w.slots[b as usize].holds(cell, q))
}

/// Entry `i`'s row of the table: its brick, and the slot it is present in
/// (`Entry::present`, the slot plus one); absent while its clip level is not
/// laid out.
fn show(w: &mut Clipmap, i: u32) {
    let (clip_level, brick) = (i / ENTRIES, w.table.entries[i as usize].brick);
    let held = first_of(w, clip_level)
        .and_then(|_| slot_of(w, w.clip_levels[clip_level as usize].cell, brick));
    w.table.entries[i as usize] = match held {
        Some(b) if w.slots[b as usize].present => Entry::at(brick, b as i32 + 1),
        _ => Entry::absent(brick),
    };
}

/// The bricks filled, each to `write` while it takes them, its words, its four
/// palette indices a cell and its tree as they lie: present from here on, on the
/// CPU and the GPU alike, the frame that hands it the first to read it, and the
/// clip levels folded over them once after all of them (`fold`). A frame's
/// stage, before its cast reads the table; what `write` does not take asks the
/// next frame.
pub fn upload(write: &mut TerrainWrite<'_>) {
    let w = clipmap();
    let mut landed = false;
    for b in 0..SLOTS {
        let held = w.slots[b as usize];
        if held.fill != Fill::Filled {
            continue;
        }
        let pieces = way_pool().pieces(held.tree);
        let taken = write(BrickWrite {
            slot: b,
            cell: held.cell,
            brick: held.brick,
            words: cells(b),
            palette_indices: palette(b),
            tree: &pieces,
        });
        if !taken {
            request_world_tick();
            break;
        }
        (w.slots[b as usize].present, w.slots[b as usize].fill) = (true, Fill::Idle);
        TerrainTile::tile_cache().moved_at(tile_of(held.cell, held.brick).0);
        for i in of_entry(b % ENTRIES) {
            show(w, i);
        }
        landed = true;
    }
    if landed {
        fold(w);
    }
}

/// The slots to fill, the next last: those cut, and those of void tiles, which
/// have no packet to cut, in the order the eye reads them: the eye's clip
/// levels coarsest first, since a reader of a brick absent reads the coarser
/// clip level's (pxl-engine `clipmap::step`), then the pivot's and a
/// destination's; in a clip level by how far its brick lies from the center. A
/// brick another clip level listed first is not listed again.
fn due(w: &mut Clipmap) {
    let terrain = TerrainTile::tile_cache();
    let void = |t: Quadrant| terrain.get(t).is_some() && terrain.slot(t).is_none();
    let mut listed = [false; SLOTS as usize];
    w.due.clear();
    for clip_level in 0..CLIP_LEVELS {
        let (cell, Some(first)) = (
            w.clip_levels[clip_level as usize].cell,
            first_of(w, clip_level),
        ) else {
            continue;
        };
        let mut near = List::<(i64, u32), { ENTRIES as usize }>::new();
        for i in clip_level * ENTRIES..(clip_level + 1) * ENTRIES {
            let q = w.table.entries[i as usize].brick;
            let Some(b) = slot_of(w, cell, q) else {
                continue;
            };
            let held = w.slots[b as usize];
            let ready = held.fill == Fill::Cut || held.is_absent() && void(tile_of(cell, q).0);
            if ready && !core::mem::replace(&mut listed[b as usize], true) {
                near.push((far(cell, first, q), b));
            }
        }
        near.sort_unstable();
        for &(_, b) in near.iter() {
            w.due.push(b);
        }
    }
    w.due.reverse();
}

/// How far brick `q` at `cell` lies from the center of the clip level whose
/// first brick is `first`, squared, in cells, X the short way round: the
/// center as the clip level places it, the middle of the brick's worth of
/// cells its first holds it over (`clipmap::origin`).
fn far(cell: Zoom, first: [i32; 2], q: [i32; 2]) -> i64 {
    let mid = |v: i32| i64::from(v) * i64::from(BRICK) + i64::from(BRICK / 2);
    let center = |v: i32| mid(v) + i64::from(clipmap::RADIUS);
    let round = 1i64 << cell;
    let dx = (mid(q[0]) - center(first[0])).rem_euclid(round);
    let dx = dx.min(round - dx);
    let dy = mid(q[1]) - center(first[1]);
    dx * dx + dy * dy
}

/// The tiles the pivot's and a destination's clip levels want, which the eye's
/// demand need not name: held hard, as a destination's line is
/// (pxl-kernel domain/view/demand.rs `observe`).
pub fn pinned_tiles() -> &'static [Quadrant] {
    &clipmap().pinned
}

/// The eye's clip level at `cell`'s entries as it holds them, each brick
/// present or still wanted (`Entry`); none while its center is not. A debug
/// reader's (domain/scene/gizmo/coverage_gizmo.rs): a copy, moving nothing.
pub fn eye_entries(cell: Zoom) -> Option<[Entry; ENTRIES as usize]> {
    let (w, clip_level) = (
        clipmap(),
        cell.checked_sub(FIRST_CELL)
            .filter(|&w| w < EYE_CLIP_LEVELS)?,
    );
    first_of(w, clip_level)?;
    Some(core::array::from_fn(|s| {
        w.table.entries[(clip_level * ENTRIES) as usize + s]
    }))
}

/// The clip levels' table, as the GPU's cast reads it (pxl-render clip.wgsl
/// `step_to`, `held_of`): a row an entry (pxl-engine `clipmap::Entry`), a row a
/// clip level (`clipmap::ORIGIN_AT`), then the levels folded over the eye's
/// bricks (`clipmap::JOINED_AT`).
pub fn table() -> &'static [i32] {
    let table = &clipmap().table;
    // SAFETY: the table is rows of four plain words end to end (`Table`), so
    // it is their words.
    unsafe {
        core::slice::from_raw_parts(
            core::ptr::from_ref(table).cast::<i32>(),
            size_of::<Table>() / 4,
        )
    }
}

const _: () = assert!(
    size_of::<Table>() == clipmap::TABLE_WORDS as usize * 4,
    "the table is the rows the GPU binds"
);

/// The room a brick's fill works in, arrays of `.fixed.terrain` (fixed.rs
/// `FILL_ARRAY_BYTES`): a tile's palette indices four a cell as a packet
/// gives them, its samples, its lines and the pairs they are laid with, its
/// spans, its tree as it is built, and the walk that lays it in the pool.
struct FillScratch {
    room: &'static mut [GroundId],
    ele: &'static mut [i16],
    lines: &'static mut [u32],
    pairs: &'static mut [(u32, u32)],
    spans: &'static mut [u32],
    build: &'static mut [u32],
    order: &'static mut [u32],
}

static FILL: Global<FillScratch> = Global::new();

fn fill_scratch() -> &'static mut FillScratch {
    FILL.get(|| {
        let r = RANGE.get();
        // SAFETY: the fill's arrays are its alone, plain numbers, zeroed at boot.
        unsafe {
            FillScratch {
                room: arr(r, fill_off(0), PALETTE_COLUMN),
                ele: arr(r, fill_off(1), ELE_COLUMN),
                lines: arr(r, fill_off(2), LINES_WORDS as usize),
                pairs: arr(r, fill_off(3), WAYS_BRICK_SEGMENTS as usize),
                spans: arr(r, fill_off(4), SPANS_WORDS as usize),
                build: arr(r, fill_off(5), BUILD_WORDS),
                order: arr(r, fill_off(6), tree::BRICK_CHILD_GROUPS as usize + 1),
            }
        }
    })
}

/// Whether slot `b` holds a brick of `tile` a clip level wants, absent.
fn slot_wants(w: &Clipmap, tile: Quadrant, b: u32) -> bool {
    let held = w.slots[b as usize];
    held.is_absent()
        && held.cell == GROUND.cell_zoom(tile.zoom)
        && tile_of(held.cell, held.brick).0 == tile
}

/// Each brick of `tile` a clip level wants, absent, its four palette indices cut
/// from `pkt`, the tile's, in the turn it landed: what of a packet its fill reads
/// past what the tile keeps (`TerrainTile::separation`, `TerrainTile::ele`), so
/// the packet goes. One the runtime holds a slot of is left absent, its tile
/// asked again.
pub fn cut(tile: Quadrant, pkt: &[u8]) {
    let w = clipmap();
    if !(0..SLOTS).any(|b| slot_wants(w, tile, b)) {
        return;
    }
    let f = fill_scratch();
    let Ok(indices) = palette_cells(pkt, f.room) else {
        w.filled = true;
        return;
    };
    let four = |c: usize| {
        let at = 4 * c;
        u32::from_le_bytes([
            indices[at],
            indices[at + 1],
            indices[at + 2],
            indices[at + 3],
        ])
    };
    for b in 0..SLOTS {
        if !slot_wants(w, tile, b) {
            continue;
        }
        if in_flight(b) {
            w.filled = true;
            continue;
        }
        let held = w.slots[b as usize];
        cut_into(b, tile_of(held.cell, held.brick).1, &four);
        w.slots[b as usize].fill = Fill::Cut;
    }
    w.due_stale = true;
}

/// Slot `b`'s four palette indices a cell, off `four` a cell's of its tile's, the
/// brick lying at `place` in it, each at its index in the brick
/// (`clipmap::idx_of`).
fn cut_into(b: u32, place: [u32; 2], four: &impl Fn(usize) -> u32) {
    let at = (b * PALETTE_ARR_BYTES / 4) as usize;
    let side = GROUND.tile_cells() as usize;
    let (x0, y0) = ((place[0] * BRICK) as usize, (place[1] * BRICK) as usize);
    for i in 0..BRICK_CELLS {
        let [x, y] = clipmap::of_idx(i);
        planes().palette[at + i as usize] = four((y0 + y as usize) * side + x0 + x as usize);
    }
}

/// A brick's fill as it goes on: its slot, and what it was begun for, the brick
/// and where it lies; how far its lines run in the room (`FillScratch::lines`);
/// its tree's child groups held and its root as built (`fill::tree`); and what
/// comes next.
#[derive(Clone, Copy)]
struct Filling {
    slot: u32,
    cell: Zoom,
    brick: [i32; 2],
    place: [u32; 2],
    shard: Shard,
    lines: usize,
    built: [u32; 2],
    stage: Stage,
}

/// A part of a brick's fill, as a turn takes it: its columns; a row of its spans,
/// the ring's rows too; its levels; its tree; its tree laid in the pool.
#[derive(Clone, Copy)]
enum Stage {
    Columns,
    Spans(u32),
    Levels,
    Tree,
    Lay,
}

/// What became of a part of a fill.
enum Fills {
    /// A part ran, and more may.
    Ran,
    /// The runtime holds what it writes: a later turn's.
    Held,
    /// No brick is due.
    Done,
}

/// The bricks due filled, a part at a time (`fill_stage`), past the frame: as
/// far as the turn's tasks' share holds (sys/sched/edge.rs `tasks_after_frame`),
/// which is the frame's share however far the frame overran, and at least one
/// part, so a turn that spent its share before it still moves the fill. What
/// is left, and a brick filled to be handed, ask the next turn.
pub fn fill_share() {
    let w = clipmap();
    if core::mem::take(&mut w.due_stale) {
        due(w);
    }
    let mut ran = false;
    loop {
        match fill_stage(w) {
            Fills::Done => break,
            Fills::Held => {
                ran = true;
                break;
            }
            Fills::Ran => ran = true,
        }
        if world_edge().is_spent() {
            break;
        }
    }
    if ran {
        request_world_tick();
    }
}

/// The next part of the fill going on, or the next brick due begun. A fill whose
/// slot let its brick go ends.
fn fill_stage(w: &mut Clipmap) -> Fills {
    let Some(mut filling) = w.filling else {
        return begin(w);
    };
    let (b, held) = (filling.slot, w.slots[filling.slot as usize]);
    if !held.holds(filling.cell, filling.brick) || held.fill != Fill::Cut {
        w.filling = None;
        return Fills::Ran;
    }
    let f = fill_scratch();
    let site = fill::Site {
        cell: filling.cell,
        place: filling.place,
        first: filling.brick.map(|v| v << BRICK_SHIFT),
        shard: filling.shard,
        ele: f.ele,
        lines: &f.lines[..filling.lines],
    };
    filling.stage = match filling.stage {
        Stage::Columns => {
            let four = palette(b);
            fill::columns(cells_mut(b), |i| four[i as usize], &site);
            Stage::Spans(0)
        }
        Stage::Spans(row) => {
            for i in row * SPANS_ACROSS..(row + 1) * SPANS_ACROSS {
                fill::spans(cells(b), &site, f.spans, i);
            }
            match row + 1 < SPANS_ACROSS {
                true => Stage::Spans(row + 1),
                false => Stage::Levels,
            }
        }
        Stage::Levels => {
            fill::levels(cells_mut(b), f.spans, filling.cell);
            Stage::Tree
        }
        Stage::Tree => {
            let [taken, cnt, root] = fill::tree(cells(b), f.spans, filling.cell, f.build, 1);
            if taken == u32::MAX {
                return end(w, b, Fill::Refused);
            }
            filling.built = [cnt, root];
            Stage::Lay
        }
        Stage::Lay => return lay(w, filling, f),
    };
    w.filling = Some(filling);
    Fills::Ran
}

/// The next brick due begun: what a fill reads of its tile, its samples and its
/// lines, copied to the room, as the tile may go before it ends; a void tile's
/// brick cut here, having no packet. One whose tile went is absent, its tile
/// asked again.
fn begin(w: &mut Clipmap) -> Fills {
    let terrain = TerrainTile::tile_cache();
    while let Some(&b) = w.due.last() {
        let held = w.slots[b as usize];
        if !held.is_free() && in_flight(b) {
            return Fills::Held;
        }
        w.due.remove(w.due.len() - 1);
        if held.is_free() || held.fill != Fill::Cut && !held.is_absent() {
            continue;
        }
        let (t, place) = tile_of(held.cell, held.brick);
        let Some(tile) = terrain.get(t) else {
            w.slots[b as usize].fill = Fill::Idle;
            w.filled = true;
            continue;
        };
        let void = terrain.slot(t).is_none();
        if held.fill != Fill::Cut && !void {
            continue;
        }
        let f = fill_scratch();
        if void {
            let cells = &tile.ground.cells;
            cut_into(b, place, &|c: usize| word([u32::from(cells[c]); 4]));
            w.slots[b as usize].fill = Fill::Cut;
            f.ele.fill(0);
        } else {
            f.ele.copy_from_slice(&tile.ele);
        }
        let lines = brick_lines(t, &tile.separation, place, f.pairs, f.lines);
        w.filling = Some(Filling {
            slot: b,
            cell: held.cell,
            brick: held.brick,
            place,
            shard: Shard::UNKNOWN.at(&ServiceArea, (clipmap::bricks_zoom(held.cell), held.brick)),
            lines,
            built: [0; 2],
            stage: Stage::Columns,
        });
        return Fills::Ran;
    }
    Fills::Done
}

/// A brick's tree laid in the pool as a run of its own, its root word naming it,
/// and its fill over: refused past the pool's cap, waiting where the heap
/// refuses it room; held for a later turn while the runtime holds the run.
fn lay(w: &mut Clipmap, filling: Filling, f: &mut FillScratch) -> Fills {
    let (b, [cnt, root]) = (filling.slot, filling.built);
    if cnt == 0 {
        cells_mut(b)[ROOT_AT as usize] = 0;
        return end(w, b, Fill::Filled);
    }
    let pool = way_pool();
    let run = match pool.take(cnt) {
        Ok(run) => run,
        Err(Unlaid::Cap) => return end(w, b, Fill::Refused),
        Err(Unlaid::Heap) => {
            w.heap.get_or_insert_with(Refusal::now);
            return end(w, b, Fill::Waiting);
        }
    };
    if pool.in_flight(run) {
        pool.give(run);
        return Fills::Held;
    }
    let built = tree::Read(f.build);
    cells_mut(b)[ROOT_AT as usize] = tree::lay(&built, root, pool, run[0], f.order);
    w.slots[b as usize].tree = run;
    end(w, b, Fill::Filled)
}

/// Slot `b`'s fill over, as `fill` leaves it.
fn end(w: &mut Clipmap, b: u32, fill: Fill) -> Fills {
    w.slots[b as usize].fill = fill;
    w.filling = None;
    Fills::Ran
}

/// The clip levels as the CPU holds them, for pxl-engine's one reading of a
/// cell (`clipmap::step`, `clipmap::bound_over`): each brick's columns, levels
/// and ways' tree's root (its tree's child groups in the pool, `pool`), the
/// eye's clip levels' levels over
/// them folded here (`fold`); and the service area they end at (pxl-engine
/// `area::Area`).
#[derive(Clone, Copy)]
pub(in crate::domain) struct CpuClipmap;

impl Area for CpuClipmap {
    fn ends(&self) -> bool {
        ServiceArea.ends()
    }
    fn nine(&self, key: u32) -> u32 {
        ServiceArea.nine(key)
    }
}

/// The clip levels as a caller holds them, read as `CpuClipmap` reads the
/// kernel's (`fold`, which runs while its writer holds them).
struct Holding<'a>(&'a Clipmap);

impl Presence for Holding<'_> {
    /// Present: handed to the GPU, in the slot holding it, whichever clip
    /// levels want it.
    fn present(&self, cell: Zoom, q: [i32; 2]) -> (u32, bool) {
        let w = self.0;
        let held = slot_of(w, cell, q).filter(|&b| w.slots[b as usize].present);
        (held.unwrap_or(0), held.is_some())
    }
}

impl Bricks for Holding<'_> {
    fn brick(&self, cell: Zoom, entry: u32) -> ([i32; 2], bool) {
        self.0.table.entries[((cell - FIRST_CELL) * ENTRIES + entry) as usize].brick()
    }

    /// The CPU reads a cell's own palette index alone, as laid: its four are it four
    /// times.
    fn palette_word(&self, cell: Zoom, b: u32, i: u32) -> u32 {
        word([self.laid(cell, b, i); 4])
    }

    /// As this side folded it over its bricks (`fold`).
    fn joined(&self, cell: Zoom, e: u32) -> [u32; clipmap::JOINED_WORDS as usize] {
        self.0.table.joined[((cell - FIRST_CELL) * clipmap::JOINED_ENTRIES + e) as usize]
    }

    fn origin(&self, cell: Zoom) -> ([i32; 2], bool) {
        let first = first_of(self.0, cell - FIRST_CELL);
        (first.unwrap_or([0; 2]), first.is_some())
    }

    fn laid(&self, _: Zoom, b: u32, i: u32) -> u32 {
        fill::laid_of(cells(b), 0, i)
    }

    fn column(&self, _: Zoom, b: u32, i: u32) -> u32 {
        fill::column_of(cells(b), 0, i)
    }

    fn bound_level(&self, _: Zoom, b: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
        fill::level_of(cells(b), 0, j, at)
    }

    fn root(&self, _: Zoom, b: u32) -> u32 {
        fill::root_of(cells(b), 0)
    }
}

/// The kernel's clip levels as they are held (`Holding`).
impl Presence for CpuClipmap {
    fn present(&self, cell: Zoom, q: [i32; 2]) -> (u32, bool) {
        Holding(clipmap()).present(cell, q)
    }
}

impl Bricks for CpuClipmap {
    fn brick(&self, cell: Zoom, entry: u32) -> ([i32; 2], bool) {
        Holding(clipmap()).brick(cell, entry)
    }
    fn palette_word(&self, cell: Zoom, b: u32, i: u32) -> u32 {
        Holding(clipmap()).palette_word(cell, b, i)
    }
    fn joined(&self, cell: Zoom, e: u32) -> [u32; clipmap::JOINED_WORDS as usize] {
        Holding(clipmap()).joined(cell, e)
    }
    fn origin(&self, cell: Zoom) -> ([i32; 2], bool) {
        Holding(clipmap()).origin(cell)
    }
    fn laid(&self, cell: Zoom, b: u32, i: u32) -> u32 {
        Holding(clipmap()).laid(cell, b, i)
    }
    fn column(&self, cell: Zoom, b: u32, i: u32) -> u32 {
        Holding(clipmap()).column(cell, b, i)
    }
    fn bound_level(&self, cell: Zoom, b: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
        Holding(clipmap()).bound_level(cell, b, j, at)
    }
    fn root(&self, cell: Zoom, b: u32) -> u32 {
        Holding(clipmap()).root(cell, b)
    }
}

const _: () = assert!(BRICK_LEVELS <= BRICK_SHIFT);

#[cfg(test)]
#[path = "clipmap.test.rs"]
pub(crate) mod test;
