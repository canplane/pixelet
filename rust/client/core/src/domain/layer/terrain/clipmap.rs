// pxl-core/domain/layer/terrain/clipmap.rs

//! The ground's cells as the core holds them (pxl-engine `clipmap`). Each
//! center a reader declares has one clip level per zoom, as deep as that
//! reader reads. The centers are the eye, the pivot where walking and
//! editing happen, and a destination being travelled to. A brick at a given
//! zoom is held once, in one slot, however many clip levels want it (`Slot`).
//!
//! A brick is filled here from its tile in two stages. Its palette, four
//! indices per cell, is cut from the tile's packet in the turn the packet
//! lands, and the packet is then freed (`cut`). Later, after the frame and
//! as far as the turn's share for tasks leaves room (`fill_share`), its cells
//! and its ways' tree are built from that palette and the tile's samples and
//! lines, into `.fixed.terrain` and the ways' pool (pool.rs). The next frame
//! hands the brick to the GPU, and it is present from then on (`upload`).
//! Nothing of a tile is kept for a brick still to come: a brick that enters
//! later asks for its tile again (`TileCache::again`).

use pxl_engine::area::{Area, Shard};
use pxl_engine::clipmap::{
    self, Bricks, Entry, Presence, BRICK, BRICK_CELLS, BRICK_MIPS, BRICK_SHIFT, BRICK_WORDS,
    CLIP_LEVELS, ENTRIES, EYE_CLIP_LEVELS, FIRST_CELL_ZOOM, LINES_WORDS, ROOT_AT, SLOTS,
    SPANS_ACROSS, SPANS_WORDS, WAYS_BRICK_SEGMENTS,
};
use pxl_engine::fill;
use pxl_engine::ground::word;
use pxl_engine::way::tree;
use pxl_engine::wrap_idx;
use pxl_engine::LEAF_CELL_ZOOM;
use pxl_render::gpu::api;
use pxl_render::gpu::ray::{BrickWrite, TerrainWrite};
use pxl_spec::bake::tile::terrain::{
    brick_lines, palette_cells, TerrainTile, ELE_COLUMN, PALETTE_COLUMN,
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
use crate::domain::layer::terrain::pool::{way_pool, PoolRefusal, Run, WayPool};
use crate::domain::layer::terrain::tile;
use crate::domain::layer::Base;
use crate::sys::io::room::Refusal;
use crate::sys::io::source::ServiceArea;
use crate::sys::sched::edge::{request_world_tick, world_edge};
use xpute_core::mem::range::arr;

/// The center a clip level is laid around.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Center {
    Eye,
    Pivot,
    Destination,
}

/// The most tiles the clip levels other than the eye's want: each such clip
/// level spans at most two tiles a side, so four tiles.
pub const PINNED_TILES: u32 = (CLIP_LEVELS - EYE_CLIP_LEVELS) * 4;
const _: () = assert!(
    clipmap::CLIP_SIZE <= TILE_CELLS + 1,
    "a clip level spans two tiles a side"
);

/// The center of clip level `w` and the zoom of its cells as laid out. A
/// destination's clip level is laid out at the leaf cells; `step` moves it to
/// the zoom where the destination's line gives the ground.
const fn layout(w: u32) -> (Center, Zoom) {
    match w {
        w if w < EYE_CLIP_LEVELS => (Center::Eye, clipmap::eye_cell_zoom(w)),
        w if w < EYE_CLIP_LEVELS + 2 => (Center::Pivot, LEAF_CELL_ZOOM - 1 + (w - EYE_CLIP_LEVELS)),
        _ => (Center::Destination, LEAF_CELL_ZOOM),
    }
}

/// A clip level, known by the zoom of its cells. Its first brick is kept in
/// its row past the table's entries (`first_of`).
#[derive(Clone, Copy, PartialEq, Eq)]
struct ClipLevel {
    cell_zoom: Zoom,
}

/// Where a brick of the ground is held: its array of cells in
/// `.fixed.terrain` (`cells`) and its row of the GPU's bricks (pxl-render
/// gpu/ray/ground.rs `GROUND_BYTES`). A brick at a zoom has one slot, however
/// many clip levels want it.
///
/// Clip levels at one zoom want a given brick in the same entry
/// (`clipmap::entry`), so the brick is held in one of that entry's
/// `CLIP_LEVELS` slots (`of_entry`). No more bricks want an entry than there
/// are clip levels, so a wanted brick always finds a free slot
/// (`assign_slots`), and overlapping clip levels take no more slots than the
/// bricks they want. A clip level takes its own slot where that is free.
/// Clip levels that never overlapped hold their bricks as if each were
/// alone; once two have overlapped and parted, one may hold its bricks in
/// the other's slots.
#[derive(Clone, Copy)]
struct Slot {
    /// The zoom of its brick's cells, or 0 where it holds none (`is_free`);
    /// no clip level is at zoom 0.
    cell_zoom: Zoom,
    brick: [i32; 2],
    present: bool,
    fill: Fill,
    /// Its ways' tree in the pool, given back with the slot.
    tree: Run,
}

/// How far a slot's brick has come in its fill.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fill {
    /// No fill is going on or due, and none was refused or is waiting.
    Idle,
    /// Its palette was cut from its tile's packet (`cut`), so its fill is due
    /// (`due`).
    Cut,
    /// Its cells are whole here and its tree is in the pool. It waits to be
    /// handed to the GPU (`upload`) and is not present yet.
    Filled,
    /// Its ways' tree was refused, because the brick needs more than a
    /// brick's most or the pool is at its cap, and asking again changes
    /// neither. The slot does not count as absent, so the brick is not filled
    /// again every turn, and its tile is not asked for again, while a clip
    /// level wants it (`assign_slots`). A reader reads the coarser clip level
    /// instead, as for any absent brick.
    Refused,
    /// The heap refused pool room for its child groups
    /// (`pool::PoolRefusal::Heap`). Like `Refused`, the slot does not count
    /// as absent, until the heap gives a page back (`Clipmap::heap`); the
    /// brick is then absent again and is filled again (`step`).
    Waiting,
}

const _: () = assert!(FIRST_CELL_ZOOM > 0, "a free slot's zoom is no clip level's");

impl Slot {
    const FREE: Slot = Slot {
        cell_zoom: 0,
        brick: [i32::MIN; 2],
        present: false,
        fill: Fill::Idle,
        tree: [0; 2],
    };

    fn is_free(&self) -> bool {
        self.cell_zoom == 0
    }

    /// Whether it holds brick `q` at `cell_zoom`, present or not.
    fn holds(&self, cell_zoom: Zoom, q: [i32; 2]) -> bool {
        self.cell_zoom == cell_zoom && self.brick == q
    }

    /// Whether the slot holds a wanted brick for which nothing of its tile is
    /// held: not present, not on its way, and not refused.
    fn is_absent(&self) -> bool {
        !self.is_free() && !self.present && self.fill == Fill::Idle
    }
}

/// The slots of entry `s`, which are entry `s` of every clip level:
/// `k × ENTRIES + s` for each `k` below `CLIP_LEVELS`.
fn of_entry(s: u32) -> impl Iterator<Item = u32> {
    (0..CLIP_LEVELS).map(move |k| k * ENTRIES + s)
}

/// The clip levels' table as the GPU binds it (pxl-render clip.wgsl): rows
/// of four words, end to end.
#[repr(C)]
struct Table {
    /// Every clip level's entries, each with its brick and its slot's state.
    /// After them comes one row per clip level, shaped like an entry, that
    /// holds the clip level's first brick: marked present while the clip
    /// level is laid out, and empty while its center is not (pxl-engine
    /// `clipmap::ORIGIN_AT`).
    entries: [Entry; (SLOTS + CLIP_LEVELS) as usize],
    /// For each of the eye's clip levels, the mip levels over its bricks:
    /// each entry joins the bounds of an aligned block of bricks, folded here
    /// from the bricks as they are held (`fold`). An entry no fold has
    /// written is the zero entry, which names no brick (`clipmap::JOINED_AT`).
    joined: [[u32; clipmap::JOINED_WORDS as usize];
        (EYE_CLIP_LEVELS * clipmap::JOINED_ENTRIES) as usize],
}

struct Clipmap {
    clip_levels: [ClipLevel; CLIP_LEVELS as usize],
    /// The table, kept here exactly as the GPU will read it (`table`).
    table: Table,
    /// The bricks the clip levels want, each in one slot (`Slot`).
    slots: [Slot; SLOTS as usize],
    /// The device generation the entries' bricks were handed to
    /// (`device_generation`). A device that is lost took all of them with
    /// it.
    on: u32,
    /// What the entries were last stepped over: in `read`, the tile cache's
    /// revision (`TileCache::revision`); in `filled`, whether a brick went
    /// absent again since, which changes what is asked for again.
    read: u32,
    filled: bool,
    /// The tiles the clip levels other than the eye's want (`pinned_tiles`).
    pinned: List<Quadrant, { PINNED_TILES as usize }>,
    /// The slots to fill, the next one last (`due`); `due_stale` says the
    /// list no longer matches the slots as they are.
    due: List<u32, { SLOTS as usize }>,
    due_stale: bool,
    /// The brick being filled, whose fill goes on over the turns it takes.
    filling: Option<BrickFill>,
    /// The heap's state when it first refused pool room for a brick's child
    /// groups, kept while a brick waits on it (`Fill::Waiting`).
    heap: Option<Refusal>,
}

static CLIPMAP: Global<Clipmap> = Global::new();

fn clipmap() -> &'static mut Clipmap {
    CLIPMAP.get(fresh)
}

/// Clip level `clip_level`'s first brick, or none while its center is not
/// laid out.
fn first_of(w: &Clipmap, clip_level: u32) -> Option<[i32; 2]> {
    let (first, laid) = w.table.entries[(SLOTS + clip_level) as usize].brick();
    laid.then_some(first)
}

/// The clip levels before any center is set: every entry absent.
fn fresh() -> Clipmap {
    Clipmap {
        clip_levels: core::array::from_fn(|w| ClipLevel {
            cell_zoom: layout(w as u32).1,
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

/// The cell at `cell_zoom` that point `p`, in units, lies in, with X wrapped
/// around the world.
fn cell_at(cell_zoom: Zoom, p: [f64; 2]) -> [i32; 2] {
    let side = units(cell_zoom);
    [
        wrap_idx(cell_zoom, (p[0] / side).floor() as i32),
        (p[1] / side).floor() as i32,
    ]
}

/// The brick that the clip level whose first brick is `first` holds in
/// entry `s`.
fn entry_brick(cell_zoom: Zoom, first: [i32; 2], s: u32) -> [i32; 2] {
    let m = (clipmap::ACROSS - 1) as i32;
    let at = |o: i32, k: i32| o + ((k - (o & m)) & m);
    let q = [
        at(first[0], (s % clipmap::ACROSS) as i32),
        at(first[1], (s / clipmap::ACROSS) as i32),
    ];
    [wrap_idx(cell_zoom - BRICK_SHIFT, q[0]), q[1]]
}

/// The tile brick `q` at `cell_zoom` lies in, and where in it, in bricks.
pub(crate) fn tile_of(cell_zoom: Zoom, q: [i32; 2]) -> (Quadrant, [u32; 2]) {
    let per = (GROUND.tile_cells() >> BRICK_SHIFT) as i32;
    let zoom = cell_zoom - GROUND.tile_cell_shamt();
    let t = Quadrant::new(zoom, [q[0].div_euclid(per), q[1].div_euclid(per)]);
    (
        t,
        [q[0].rem_euclid(per) as u32, q[1].rem_euclid(per) as u32],
    )
}

/// Every brick's planes on the CPU, in `.fixed.terrain` (fixed.rs); a test
/// may set its own. The child groups of the bricks' ways' trees live on the
/// heap instead (pool.rs).
pub(crate) struct Planes {
    /// Each brick's whole words (`clipmap::BRICK_WORDS`), `CELLS_ARR_BYTES`
    /// apart.
    pub cells: &'static mut [u32],
    /// Each brick's palette, one word of four palette indices per cell in
    /// its cells' order, `PALETTE_ARR_BYTES` apart.
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

/// Folds the mip levels over the eye's clip levels' bricks again, one level
/// at a time (pxl-engine `clipmap::fold`), from the bricks as they are held
/// here; the table carries them to the GPU (`table`).
///
/// Two places write which brick a clip level wants or whether it is
/// present: `assign_slots`, in a turn's step, which folds before it returns,
/// and the bricks a frame hands over (`upload`), folded once after all of
/// them. No read of a bound (`CpuClipmap`) comes between a write and its
/// fold, so a bound read never sees other bricks than a read of the cells
/// does. The fold reads through the clip levels it is handed (`ClipRead`),
/// which its writer holds while it runs.
fn fold(w: &mut Clipmap) {
    for l in 1..=clipmap::JOINED_MIPS {
        let blocks = (clipmap::ACROSS >> l) * (clipmap::ACROSS >> l);
        for clip_level in 0..EYE_CLIP_LEVELS {
            for b in 0..blocks {
                let e = clipmap::fold(&ClipRead(w), FIRST_CELL_ZOOM + clip_level, l, b);
                let at = clip_level * clipmap::JOINED_ENTRIES + clipmap::joined_idx(l, [0, 0]) + b;
                w.table.joined[at as usize] = e;
            }
        }
    }
}

/// The ways' pool, holding the child groups of every held brick's tree. A
/// cursor reads it here as the GPU's cast reads its copy (pxl-engine
/// `way::tree`).
pub(in crate::domain) fn pool() -> &'static WayPool {
    way_pool()
}

/// The child groups the pool spans, which the GPU's copy of it holds.
pub fn pool_top() -> u32 {
    way_pool().top()
}

/// Slot `b`'s words: the brick's whole words from its first, the way
/// pxl-engine reads a brick's words (`fill::column_of`).
fn cells(b: u32) -> &'static [u32] {
    let at = (b * CELLS_ARR_BYTES / 4) as usize;
    &planes().cells[at..at + BRICK_WORDS as usize]
}

/// Slot `b`'s words to write. Only the fill that holds the slot writes them.
fn cells_mut(b: u32) -> &'static mut [u32] {
    let at = (b * CELLS_ARR_BYTES / 4) as usize;
    &mut planes().cells[at..at + BRICK_WORDS as usize]
}

/// Slot `b`'s palette, four palette indices per cell.
fn palette(b: u32) -> &'static [u32] {
    let at = (b * PALETTE_ARR_BYTES / 4) as usize;
    &planes().palette[at..at + BRICK_CELLS as usize]
}

/// Whether the runtime holds the words slot `b` was last handed from
/// (pxl-render gpu/api.rs `in_flight`). Nothing may write them until the
/// runtime gives them back.
fn in_flight(b: u32) -> bool {
    api::in_flight(cells(b)) || api::in_flight(palette(b))
}

/// Lays out the turn's clip levels. `at` gives where each center now
/// stands, in units, X then Y, indexed by `Center`, or none where a center is
/// not set. A brick no clip level wants any more gives up its slot
/// (`assign_slots`), and the tiles a clip level now wants but holds nothing
/// of are asked for again.
///
/// The entries depend only on whether a clip level moved a brick or changed
/// zoom, the device was lost, the tile cache changed (`TileCache::revision`),
/// or a brick went absent again, so they are stepped only then. It runs
/// before the turn's arrivals and its stage. Returns true where the tiles
/// the clip levels other than the eye's want changed.
pub fn step(at: [Option<[f64; 2]>; 3], on: u32) -> bool {
    let w = clipmap();
    let terrain = TerrainTile::tile_cache();
    let lost = core::mem::replace(&mut w.on, on) != on;
    // For each clip level, whether its zoom changed, and whether anything of
    // it changed.
    let (mut zoom_changed, mut changed) =
        ([false; CLIP_LEVELS as usize], [lost; CLIP_LEVELS as usize]);
    let released = wake(w);
    let mut stale =
        lost || released || core::mem::take(&mut w.filled) || terrain.revision() != w.read;
    for clip_level in 0..CLIP_LEVELS {
        let (center, laid) = layout(clip_level);
        let p = at[center as usize];
        let cell_zoom = match (center, p) {
            // A destination's clip level sits at the zoom where its line gives
            // the ground. Over open sea, the leaf's ground is given by the
            // tile over it.
            (Center::Destination, Some(p)) => {
                let leaf = Quadrant::new(LEAF_ZOOM, cell_at(LEAF_ZOOM, p));
                terrain.src(leaf).map_or(laid, |t| GROUND.cell_zoom(t.zoom))
            }
            _ => laid,
        };
        let first = p.map(|p| clipmap::origin(cell_zoom, cell_at(cell_zoom, p)));
        let was = (
            core::mem::replace(
                &mut w.clip_levels[clip_level as usize],
                ClipLevel { cell_zoom },
            )
            .cell_zoom,
            first_of(w, clip_level),
        );
        w.table.entries[(SLOTS + clip_level) as usize] = match first {
            Some(f) => Entry::at(f, 1),
            None => Entry::absent([i32::MIN; 2]),
        };
        zoom_changed[clip_level as usize] = was.0 != cell_zoom;
        changed[clip_level as usize] |= was != (cell_zoom, first);
        stale |= was != (cell_zoom, first);
    }
    if !stale {
        return false;
    }
    // A clip level that did not change wants its bricks where they already
    // are, so only changed clip levels have their entries rewritten.
    for clip_level in 0..CLIP_LEVELS {
        let (cell_zoom, first) = (
            w.clip_levels[clip_level as usize].cell_zoom,
            first_of(w, clip_level),
        );
        let Some(f) = first.filter(|_| changed[clip_level as usize]) else {
            continue;
        };
        for s in 0..ENTRIES {
            let entry = &mut w.table.entries[(clip_level * ENTRIES + s) as usize];
            let want = entry_brick(cell_zoom, f, s);
            if zoom_changed[clip_level as usize] || lost || entry.brick != want {
                entry.brick = want;
                terrain.changed_at(tile_of(cell_zoom, want).0);
            }
        }
    }
    // The slots follow the entries' bricks and the device. Where neither
    // changed and the heap woke no brick, every wanted brick keeps its slot,
    // and each entry shows it as its last hand-off left it.
    if released || changed.contains(&true) {
        assign_slots(w, lost);
    }
    // Tiles are asked for again only as far as the spare body slots a packet
    // arrives in allow, in slot order; the rest wait until room is given
    // back. Only an absent brick's tile is asked for again.
    let mut room = tile::again_room();
    for b in 0..SLOTS {
        let held = w.slots[b as usize];
        if held.is_absent() {
            let t = tile_of(held.cell_zoom, held.brick).0;
            if room > 0 && !terrain.is_again_held(t) {
                terrain.again(t);
                room -= u32::from(terrain.is_again_held(t));
            }
        }
    }
    w.due_stale = true;
    w.read = terrain.revision();
    // What the clip levels other than the eye's want changes only when they
    // change.
    if !changed[EYE_CLIP_LEVELS as usize..].contains(&true) {
        return false;
    }
    let was = core::mem::take(&mut w.pinned);
    for clip_level in EYE_CLIP_LEVELS..CLIP_LEVELS {
        let cell_zoom = w.clip_levels[clip_level as usize].cell_zoom;
        if first_of(w, clip_level).is_none() {
            continue;
        }
        for s in 0..ENTRIES {
            let t = tile_of(
                cell_zoom,
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

/// Makes every brick waiting on the heap (`Fill::Waiting`) absent again, to
/// be filled again, once the heap has given a page back since it refused pool
/// room. Returns whether it did.
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

/// Holds each brick a laid-out clip level wants in exactly one slot. Where
/// the device was lost (`lost`), it took the GPU's copy of every slot's
/// cells, so each present brick is handed over again. A slot no clip level
/// wants any more is given up with its tree, which ends a fill going on in
/// it (`fill_stage`). A wanted brick that no slot holds takes a free slot of
/// its entry, its own clip level's first, and starts absent. Then every
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
            w.clip_levels[clip_level as usize].cell_zoom == held.cell_zoom
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
        let cell_zoom = w.clip_levels[clip_level as usize].cell_zoom;
        if first_of(w, clip_level).is_none() {
            continue;
        }
        for s in 0..ENTRIES {
            let (own, q) = (
                clip_level * ENTRIES + s,
                w.table.entries[(clip_level * ENTRIES + s) as usize].brick,
            );
            if slot_of(w, cell_zoom, q).is_some() {
                continue;
            }
            let free = |b: &u32| w.slots[*b as usize].is_free();
            let b = Some(own).filter(free).or_else(|| of_entry(s).find(free));
            // No more bricks want an entry than it has slots.
            let b = b.or_bug(Errno::ENOTRECOVERABLE);
            w.slots[b as usize] = Slot {
                cell_zoom,
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

/// The slot holding brick `q` at `cell_zoom`, present or not.
fn slot_of(w: &Clipmap, cell_zoom: Zoom, q: [i32; 2]) -> Option<u32> {
    of_entry(clipmap::entry(q)).find(|&b| w.slots[b as usize].holds(cell_zoom, q))
}

/// Writes entry `i`'s row of the table: its brick and the slot it is present
/// in, stored as the slot plus one (`Entry::present`). The row is absent
/// while its clip level is not laid out or the brick is not present.
fn show(w: &mut Clipmap, i: u32) {
    let (clip_level, brick) = (i / ENTRIES, w.table.entries[i as usize].brick);
    let held = first_of(w, clip_level)
        .and_then(|_| slot_of(w, w.clip_levels[clip_level as usize].cell_zoom, brick));
    w.table.entries[i as usize] = match held {
        Some(b) if w.slots[b as usize].present => Entry::at(brick, b as i32 + 1),
        _ => Entry::absent(brick),
    };
}

/// Hands each filled brick to `write`, while it takes them: its words, its
/// palette and its tree, in place. A handed brick is present from then on,
/// on the CPU and the GPU alike, and the frame that hands it is the first to
/// read it. The clip levels are folded over the handed bricks once, after all
/// of them (`fold`). It runs in a frame's stage, before the frame's cast
/// reads the table; what `write` does not take asks for the next frame.
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
            cell_zoom: held.cell_zoom,
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
        TerrainTile::tile_cache().changed_at(tile_of(held.cell_zoom, held.brick).0);
        for i in of_entry(b % ENTRIES) {
            show(w, i);
        }
        landed = true;
    }
    if landed {
        fold(w);
    }
}

/// Lists the slots to fill, the next one last: those whose palette was cut,
/// and those of void tiles, which have no packet to cut. They come in the
/// order the eye reads them. The eye's clip levels come first, coarsest
/// first, since a reader of an absent brick reads the coarser clip level
/// (pxl-engine `clipmap::step`); then come the pivot's and a destination's.
/// Within a clip level, slots go by how far the brick lies from the center.
/// A brick another clip level listed first is not listed again.
fn due(w: &mut Clipmap) {
    let terrain = TerrainTile::tile_cache();
    let void = |t: Quadrant| terrain.get(t).is_some() && terrain.slot(t).is_none();
    let mut listed = [false; SLOTS as usize];
    w.due.clear();
    for clip_level in 0..CLIP_LEVELS {
        let (cell_zoom, Some(first)) = (
            w.clip_levels[clip_level as usize].cell_zoom,
            first_of(w, clip_level),
        ) else {
            continue;
        };
        let mut near = List::<(i64, u32), { ENTRIES as usize }>::new();
        for i in clip_level * ENTRIES..(clip_level + 1) * ENTRIES {
            let q = w.table.entries[i as usize].brick;
            let Some(b) = slot_of(w, cell_zoom, q) else {
                continue;
            };
            let held = w.slots[b as usize];
            let ready = held.fill == Fill::Cut || held.is_absent() && void(tile_of(cell_zoom, q).0);
            if ready && !core::mem::replace(&mut listed[b as usize], true) {
                near.push((far(cell_zoom, first, q), b));
            }
        }
        near.sort_unstable();
        for &(_, b) in near.iter() {
            w.due.push(b);
        }
    }
    w.due.reverse();
}

/// The squared distance, in cells, from the middle of brick `q` at
/// `cell_zoom` to the center of the clip level whose first brick is `first`,
/// with X measured the short way around the world. The center is where the
/// clip level places it (`clipmap::origin`): `RADIUS` cells past the middle
/// of its first brick.
fn far(cell_zoom: Zoom, first: [i32; 2], q: [i32; 2]) -> i64 {
    let mid = |v: i32| i64::from(v) * i64::from(BRICK) + i64::from(BRICK / 2);
    let center = |v: i32| mid(v) + i64::from(clipmap::RADIUS);
    let round = 1i64 << cell_zoom;
    let dx = (mid(q[0]) - center(first[0])).rem_euclid(round);
    let dx = dx.min(round - dx);
    let dy = mid(q[1]) - center(first[1]);
    dx * dx + dy * dy
}

/// The tiles the pivot's and a destination's clip levels want, which the
/// eye's demand need not name. They are held hard, as a destination's line
/// is (domain/view/demand.rs `observe`).
pub fn pinned_tiles() -> &'static [Quadrant] {
    &clipmap().pinned
}

/// A copy of the entries of the eye's clip level at `cell_zoom`, each brick
/// present or still wanted (`Entry`), or none while its center is not laid
/// out. It serves a debug reader (domain/scene/gizmo/coverage_gizmo.rs) and
/// changes nothing.
pub fn eye_entries(cell_zoom: Zoom) -> Option<[Entry; ENTRIES as usize]> {
    let (w, clip_level) = (
        clipmap(),
        cell_zoom
            .checked_sub(FIRST_CELL_ZOOM)
            .filter(|&w| w < EYE_CLIP_LEVELS)?,
    );
    first_of(w, clip_level)?;
    Some(core::array::from_fn(|s| {
        w.table.entries[(clip_level * ENTRIES) as usize + s]
    }))
}

/// The clip levels' table as the GPU's cast reads it (pxl-render clip.wgsl
/// `step_to`, `held_of`): one row per entry (pxl-engine `clipmap::Entry`),
/// one row per clip level (`clipmap::ORIGIN_AT`), then the mip levels folded
/// over the eye's bricks (`clipmap::JOINED_AT`).
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

/// The room a brick's fill works in, arrays in `.fixed.terrain` (fixed.rs
/// `FILL_ARRAY_BYTES`): a tile's samples, its lines and the pairs they are
/// laid with, its spans, its tree as it is built, and the walk that lays the
/// tree into the pool.
struct FillScratch {
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
                ele: arr(r, fill_off(0), ELE_COLUMN),
                lines: arr(r, fill_off(1), LINES_WORDS as usize),
                pairs: arr(r, fill_off(2), WAYS_BRICK_SEGMENTS as usize),
                spans: arr(r, fill_off(3), SPANS_WORDS as usize),
                build: arr(r, fill_off(4), BUILD_WORDS),
                order: arr(r, fill_off(5), tree::BRICK_CHILD_GROUPS as usize + 1),
            }
        }
    })
}

/// Whether slot `b` holds an absent brick of `tile` that a clip level wants.
fn slot_wants(w: &Clipmap, tile: Quadrant, b: u32) -> bool {
    let held = w.slots[b as usize];
    held.is_absent()
        && held.cell_zoom == GROUND.cell_zoom(tile.zoom)
        && tile_of(held.cell_zoom, held.brick).0 == tile
}

/// Cuts from `pkt`, the tile's packet, in the turn it landed, the palette of
/// every absent brick of `tile` that a clip level wants. The palette is all
/// that a brick's fill reads of a packet beyond what the tile keeps
/// (`TerrainTile::separation`, `TerrainTile::ele`), so the packet can be
/// freed afterwards. A brick whose slot the runtime holds is left absent, and
/// its tile is asked for again.
pub fn cut(tile: Quadrant, pkt: &[u8]) {
    let w = clipmap();
    if !(0..SLOTS).any(|b| slot_wants(w, tile, b)) {
        return;
    }
    let Ok(cells) = palette_cells(pkt) else {
        w.filled = true;
        return;
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
        let place = tile_of(held.cell_zoom, held.brick).1;
        // Each layout a packet may hold the palette in has its own loop, so
        // the layout is not tested once per cell.
        match cells.len() {
            PALETTE_COLUMN => cut_into(b, place, &|c: usize| {
                u32::from_le_bytes([
                    cells[4 * c],
                    cells[4 * c + 1],
                    cells[4 * c + 2],
                    cells[4 * c + 3],
                ])
            }),
            1 => cut_into(b, place, &|_| word([u32::from(cells[0]); 4])),
            _ => cut_into(b, place, &|c: usize| word([u32::from(cells[c]); 4])),
        }
        w.slots[b as usize].fill = Fill::Cut;
    }
    w.due_stale = true;
}

/// Fills slot `b`'s palette from `four`, which gives the palette word of
/// each cell of the tile, for the brick lying at `place` in the tile. Each
/// cell goes to its index in the brick (`clipmap::idx_of`).
fn cut_into(b: u32, place: [u32; 2], four: &impl Fn(usize) -> u32) {
    let at = (b * PALETTE_ARR_BYTES / 4) as usize;
    let side = GROUND.tile_cells() as usize;
    let (x0, y0) = ((place[0] * BRICK) as usize, (place[1] * BRICK) as usize);
    let palette = &mut planes().palette[at..at + BRICK_CELLS as usize];
    let across: [u32; BRICK as usize] = core::array::from_fn(|x| clipmap::idx_of([x as u32, 0]));
    for y in 0..BRICK {
        let (row, down) = ((y0 + y as usize) * side + x0, clipmap::idx_of([0, y]));
        for (x, &a) in across.iter().enumerate() {
            palette[(a | down) as usize] = four(row + x);
        }
    }
}

/// A brick's fill while it goes on: its slot, and the brick it was begun
/// for with where that lies; how many of the room's line words it uses
/// (`FillScratch::lines`); the number of its tree's child groups and its
/// root as built (`fill::tree`); and the stage that comes next.
#[derive(Clone, Copy)]
struct BrickFill {
    slot: u32,
    cell_zoom: Zoom,
    brick: [i32; 2],
    place: [u32; 2],
    shard: Shard,
    lines: usize,
    built: [u32; 2],
    stage: Stage,
}

/// A part of a brick's fill, the unit a turn takes: its columns; one row of
/// its spans, the ring's rows included; its mip levels; its tree; and laying
/// its tree into the pool.
#[derive(Clone, Copy)]
enum Stage {
    Columns,
    Spans(u32),
    Mips,
    Tree,
    Lay,
}

/// What became of a part of a fill.
enum Fills {
    /// A part ran, and more may.
    Ran,
    /// The runtime holds what the part writes, so a later turn runs it.
    Held,
    /// No brick is due.
    Done,
}

/// Fills the due bricks one part at a time (`fill_stage`), after the frame.
/// It runs as far as the turn's share for tasks holds (sys/sched/edge.rs
/// `tasks_after_frame`), which is the frame's share however far the frame
/// overran, and always at least one part, so a turn that spent its share
/// before this point still moves the fill. What is left, and a filled brick
/// waiting to be handed over, ask for the next turn.
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

/// Runs the next part of the fill going on, or begins the next due brick. A
/// fill whose slot let its brick go ends.
fn fill_stage(w: &mut Clipmap) -> Fills {
    let Some(mut filling) = w.filling else {
        return begin(w);
    };
    let (b, held) = (filling.slot, w.slots[filling.slot as usize]);
    if !held.holds(filling.cell_zoom, filling.brick) || held.fill != Fill::Cut {
        w.filling = None;
        return Fills::Ran;
    }
    let f = fill_scratch();
    let site = fill::Site {
        cell_zoom: filling.cell_zoom,
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
                false => Stage::Mips,
            }
        }
        Stage::Mips => {
            fill::mips(cells_mut(b), f.spans, filling.cell_zoom);
            Stage::Tree
        }
        Stage::Tree => {
            let [used, cnt, root] = fill::tree(cells(b), f.spans, filling.cell_zoom, f.build, 1);
            if used == u32::MAX {
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

/// Begins the next due brick. What a fill reads of its tile, its samples and
/// its lines, is copied into the room, since the tile may be let go before
/// the fill ends. A void tile has no packet, so its brick's palette is cut
/// here. A brick whose tile was let go becomes absent, and its tile is asked
/// for again.
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
        let (t, place) = tile_of(held.cell_zoom, held.brick);
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
        w.filling = Some(BrickFill {
            slot: b,
            cell_zoom: held.cell_zoom,
            brick: held.brick,
            place,
            shard: Shard::UNKNOWN.at(
                &ServiceArea,
                (clipmap::bricks_zoom(held.cell_zoom), held.brick),
            ),
            lines,
            built: [0; 2],
            stage: Stage::Columns,
        });
        return Fills::Ran;
    }
    Fills::Done
}

/// Lays a brick's tree into the pool as a run of its own, named by the
/// brick's root word, and ends the fill. Past the pool's cap the brick is
/// refused; where the heap refuses room, it waits; and while the runtime
/// holds the run, the part is held for a later turn.
fn lay(w: &mut Clipmap, filling: BrickFill, f: &mut FillScratch) -> Fills {
    let (b, [cnt, root]) = (filling.slot, filling.built);
    if cnt == 0 {
        cells_mut(b)[ROOT_AT as usize] = 0;
        return end(w, b, Fill::Filled);
    }
    let pool = way_pool();
    let run = match pool.take(cnt) {
        Ok(run) => run,
        Err(PoolRefusal::Cap) => return end(w, b, Fill::Refused),
        Err(PoolRefusal::Heap) => {
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

/// Ends slot `b`'s fill, leaving the slot in state `fill`.
fn end(w: &mut Clipmap, b: u32, fill: Fill) -> Fills {
    w.slots[b as usize].fill = fill;
    w.filling = None;
    Fills::Ran
}

/// The clip levels as the CPU holds them, for the one way pxl-engine reads a
/// cell on the CPU and the GPU alike (`clipmap::step`,
/// `clipmap::bound_over`). It gives each brick's columns, mip levels and the
/// root of its ways' tree, whose child groups are in the pool (`pool`); the
/// mip levels folded here over the eye's bricks (`fold`); and the service
/// area the clip levels end at (pxl-engine `area::Area`).
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

/// Clip levels that a caller already holds, read the way `CpuClipmap` reads
/// the core's. `fold` reads through it, since it runs while its writer holds
/// the clip levels.
struct ClipRead<'a>(&'a Clipmap);

impl Presence for ClipRead<'_> {
    /// A brick is present once it was handed to the GPU, in the slot that
    /// holds it, whichever clip levels want it.
    fn present(&self, cell_zoom: Zoom, q: [i32; 2]) -> (u32, bool) {
        let w = self.0;
        let held = slot_of(w, cell_zoom, q).filter(|&b| w.slots[b as usize].present);
        (held.unwrap_or(0), held.is_some())
    }
}

impl Bricks for ClipRead<'_> {
    fn brick(&self, cell_zoom: Zoom, entry: u32) -> ([i32; 2], bool) {
        self.0.table.entries[((cell_zoom - FIRST_CELL_ZOOM) * ENTRIES + entry) as usize].brick()
    }

    /// The CPU reads only a cell's own palette index, as laid, so the word
    /// repeats it four times.
    fn palette_word(&self, cell_zoom: Zoom, b: u32, i: u32) -> u32 {
        word([self.own(cell_zoom, b, i); 4])
    }

    /// As the CPU folded it over its bricks (`fold`).
    fn joined(&self, cell_zoom: Zoom, e: u32) -> [u32; clipmap::JOINED_WORDS as usize] {
        self.0.table.joined[((cell_zoom - FIRST_CELL_ZOOM) * clipmap::JOINED_ENTRIES + e) as usize]
    }

    fn origin(&self, cell_zoom: Zoom) -> ([i32; 2], bool) {
        let first = first_of(self.0, cell_zoom - FIRST_CELL_ZOOM);
        (first.unwrap_or([0; 2]), first.is_some())
    }

    fn own(&self, _: Zoom, b: u32, i: u32) -> u32 {
        fill::own_of(cells(b), 0, i)
    }

    fn column(&self, _: Zoom, b: u32, i: u32) -> u32 {
        fill::column_of(cells(b), 0, i)
    }

    fn bound_mip(&self, _: Zoom, b: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
        fill::mip_of(cells(b), 0, j, at)
    }

    fn root(&self, _: Zoom, b: u32) -> u32 {
        fill::root_of(cells(b), 0)
    }
}

/// The core's clip levels, read in place through `ClipRead`.
impl Presence for CpuClipmap {
    fn present(&self, cell_zoom: Zoom, q: [i32; 2]) -> (u32, bool) {
        ClipRead(clipmap()).present(cell_zoom, q)
    }
}

impl Bricks for CpuClipmap {
    fn brick(&self, cell_zoom: Zoom, entry: u32) -> ([i32; 2], bool) {
        ClipRead(clipmap()).brick(cell_zoom, entry)
    }
    fn palette_word(&self, cell_zoom: Zoom, b: u32, i: u32) -> u32 {
        ClipRead(clipmap()).palette_word(cell_zoom, b, i)
    }
    fn joined(&self, cell_zoom: Zoom, e: u32) -> [u32; clipmap::JOINED_WORDS as usize] {
        ClipRead(clipmap()).joined(cell_zoom, e)
    }
    fn origin(&self, cell_zoom: Zoom) -> ([i32; 2], bool) {
        ClipRead(clipmap()).origin(cell_zoom)
    }
    fn own(&self, cell_zoom: Zoom, b: u32, i: u32) -> u32 {
        ClipRead(clipmap()).own(cell_zoom, b, i)
    }
    fn column(&self, cell_zoom: Zoom, b: u32, i: u32) -> u32 {
        ClipRead(clipmap()).column(cell_zoom, b, i)
    }
    fn bound_mip(&self, cell_zoom: Zoom, b: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
        ClipRead(clipmap()).bound_mip(cell_zoom, b, j, at)
    }
    fn root(&self, cell_zoom: Zoom, b: u32) -> u32 {
        ClipRead(clipmap()).root(cell_zoom, b)
    }
}

const _: () = assert!(BRICK_MIPS <= BRICK_SHIFT);

#[cfg(test)]
#[path = "clipmap.test.rs"]
pub(crate) mod test;
