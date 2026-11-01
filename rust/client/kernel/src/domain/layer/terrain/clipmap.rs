// pxl-kernel/domain/layer/terrain/clipmap.rs

//! The ground's cells as the kernel holds them: a clipmap level a zoom about each
//! center a reader declares — the eye, the pivot where walking and editing
//! happen, a destination being gone to — each as deep as its reader reads
//! (pxl-spec-core `clipmap`). A brick at a zoom is held once, in a slot,
//! however many clipmap levels want it (`Slot`). It is filled from its tile as it
//! lands: its four palette indices and its tile's samples handed to the GPU, which
//! draws and lays its cells; they come back into `.fixed.terrain`, and the
//! brick is present from then on (`on_resident`). Nothing of a tile is kept for
//! a brick still to come: one that enters asks its tile again
//! (`TileCache::again`), which the browser's cache answers.

use pxl_spec::bake::tile::terrain::{
    brick_lines, gpu_cells, GroundId, TerrainTile, ELE_COLUMN, PALETTE_COLUMN,
};
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::layer::lattice::{GROUND, TILE_CELLS};
use pxl_spec::space::layer::tile::{tile_at, Tile, LEAF_ZOOM};
use pxl_spec_core::area::{Area, Shard};
use pxl_spec_core::clipmap::{
    self, Bricks, Entry, Presence, BRICK, BRICK_CELLS, BRICK_SHIFT, CLIP_LEVELS, ENTRIES,
    EYE_CLIP_LEVELS, FILL_CAP, FIRST_CELL, LEVELS, SLOTS,
};
use pxl_spec_core::fill;
use pxl_spec_core::ground::word;
use pxl_spec_core::wrap_idx;
use pxl_spec_core::LEAF_CELL;
use xpute_guest::global::Global;
use xpute_guest::mem::off_of_slice;
use xpute_kit::collection::list::List;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::domain::layer::terrain::fixed::{cells_off, fill_off, CELLS_ARR_BYTES, RANGE};
use crate::domain::layer::terrain::mirror::{mirror, Mirror, Unlaid};
use crate::domain::layer::terrain::tile;
use crate::domain::layer::upload::Uptake;
use crate::domain::layer::Base;
use crate::sys::io::room::Refusal;
use crate::sys::io::source::ServiceArea;
use crate::sys::mem::range::arr;
use pxl_render::gpu::api::DONE;
use pxl_render::gpu::renderer::{BrickFill, CellsBack, Resident};

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

/// Where a brick of the ground is held: its row of the GPU's bricks
/// (pxl-render gpu/ray/ground.rs `GROUND_BYTES`) and its array of cells back
/// in `.fixed.terrain` (`cells`), one a brick at a zoom however many
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
    /// The fill its cells are on their way from, 0 where none is, or
    /// `REFUSED`, or `WAITING`.
    ticket: u32,
}

const _: () = assert!(FIRST_CELL > 0, "a free slot's zoom is no clip level's");

impl Slot {
    const FREE: Slot = Slot {
        cell: 0,
        brick: [i32::MIN; 2],
        present: false,
        ticket: 0,
    };

    fn is_free(&self) -> bool {
        self.cell == 0
    }

    /// Whether it holds brick `q` at `cell`, present or not.
    fn holds(&self, cell: Zoom, q: [i32; 2]) -> bool {
        self.cell == cell && self.brick == q
    }

    /// Wanted, its tile not yet handed it: holding a brick, neither present
    /// nor on its way nor refused.
    fn is_absent(&self) -> bool {
        !self.is_free() && !self.present && self.ticket == 0
    }
}

/// The slots of entry `s`, and every clip level's entry `s`: `k × ENTRIES + s`,
/// `k` to `CLIP_LEVELS`.
fn of_entry(s: u32) -> impl Iterator<Item = u32> {
    (0..CLIP_LEVELS).map(move |k| k * ENTRIES + s)
}

struct Clipmap {
    clip_levels: [ClipLevel; CLIP_LEVELS as usize],
    /// Every clip level's entries, each its brick and its slot's state, then
    /// each clip level's first brick, a row shaped as an entry's, its brick the
    /// first and present while it is laid out, none while its center is not
    /// (pxl-spec-core `clipmap::ORIGIN_AT`): the GPU's table as it is
    /// (`table`).
    entries: [Entry; (SLOTS + CLIP_LEVELS) as usize],
    /// The bricks the clip levels want, each in one slot (`Slot`).
    slots: [Slot; SLOTS as usize],
    /// The eye's clip levels' levels over their bricks, folded over the
    /// bricks as they are held here (`fold`), the zero entry, none, before
    /// any is.
    joined: [[u32; clipmap::JOINED_WORDS as usize];
        (EYE_CLIP_LEVELS * clipmap::JOINED_ENTRIES) as usize],
    /// The device the entries' bricks were handed to (`device_generation`):
    /// one that went took them all.
    on: u32,
    ticket: u32,
    /// What the entries were last stepped over: the tile cache's revision
    /// (`TileCache::revision`), and whether a fill handed a brick or let a
    /// packet go since, which moves what is due and what is asked again.
    read: u32,
    filled: bool,
    /// The packets kept when the entries were last stepped over: one given
    /// back may leave room to ask again for a tile whose packet was not
    /// kept (`tile::again_room`).
    kept: u32,
    /// The tiles the clip levels apart from the eye's want (`pinned_tiles`).
    pinned: List<Tile, { PINNED_TILES as usize }>,
    /// The slots this turn hands the fill (`due`).
    due: List<u32, { FILL_CAP as usize }>,
    /// The heap as it was when it first refused a brick's child groups the
    /// mirror's room, while one waits on it (`WAITING`).
    heap: Option<Refusal>,
}

static CLIPMAP: Global<Clipmap> = Global::new();

fn clipmap() -> &'static mut Clipmap {
    CLIPMAP.get(fresh)
}

/// Clip level `clip_level`'s first brick, none while its center is not.
fn first_of(w: &Clipmap, clip_level: u32) -> Option<[i32; 2]> {
    let (first, laid) = w.entries[(SLOTS + clip_level) as usize].brick();
    laid.then_some(first)
}

/// The clip levels before any center stands: every entry absent.
fn fresh() -> Clipmap {
    Clipmap {
        clip_levels: core::array::from_fn(|w| ClipLevel {
            cell: layout(w as u32).1,
        }),
        entries: [Entry::absent([i32::MIN; 2]); (SLOTS + CLIP_LEVELS) as usize],
        slots: [Slot::FREE; SLOTS as usize],
        joined: [[0; clipmap::JOINED_WORDS as usize];
            (EYE_CLIP_LEVELS * clipmap::JOINED_ENTRIES) as usize],
        on: 0,
        ticket: 0,
        read: 0,
        filled: true,
        kept: 0,
        pinned: List::new(),
        due: List::new(),
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
pub(crate) fn tile_of(cell: Zoom, q: [i32; 2]) -> (Tile, [u32; 2]) {
    let per = (GROUND.tile_cells() >> BRICK_SHIFT) as i32;
    let zoom = cell - GROUND.tile_cell_shamt();
    let t = tile_at(zoom, [q[0].div_euclid(per), q[1].div_euclid(per)]);
    (
        t,
        [q[0].rem_euclid(per) as u32, q[1].rem_euclid(per) as u32],
    )
}

/// Every brick's planes on the CPU, in `.fixed.terrain` (fixed.rs): its
/// cells as they come back; a test's own. Its ways' trees' child groups are the
/// heap's (mirror.rs).
pub(crate) struct Planes {
    /// Each brick's words as they came back, whole (`clipmap::BACK_WORDS`),
    /// `CELLS_ARR_BYTES` apart.
    pub cells: &'static mut [u32],
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
            }
        }
    })
}

/// The eye's clip levels' levels over their bricks folded again, a level at a
/// time, as the GPU folds its own (pxl-spec-core `clipmap::fold`; pxl-shader
/// `fold_cs`), over the bricks as they are held here. The writers of a brick
/// a clip level wants or its presence are `assign_slots`, a turn's step's,
/// which folds before it returns, and the landings of a frame's read-back
/// (`on_resident`), folded once after all of them (`refold`): no read of a
/// bound (`CpuClipmap`) comes between a write and its fold, so none finds one
/// of other bricks than its read of the cells does. Read through the clip
/// levels it is handed (`Holding`), which its writer holds while it runs.
fn fold(w: &mut Clipmap) {
    for l in 1..=clipmap::JOINED_LEVELS {
        let blocks = (clipmap::ACROSS >> l) * (clipmap::ACROSS >> l);
        for clip_level in 0..EYE_CLIP_LEVELS {
            for b in 0..blocks {
                let e = clipmap::fold(&Holding(w), FIRST_CELL + clip_level, l, b);
                let at = clip_level * clipmap::JOINED_ENTRIES + clipmap::joined_idx(l, [0, 0]) + b;
                w.joined[at as usize] = e;
            }
        }
    }
}

/// The GPU's ways' pool as the CPU mirrors it, the landed bricks' child groups
/// at the GPU's entries, which a cursor reads as the cast reads the GPU's
/// (pxl-spec-core `way::tree`).
pub fn pool() -> &'static Mirror {
    mirror()
}

/// Slot `b`'s words as they came back: a brick's words whole, from its
/// first, which pxl-spec-core reads a brick's words by (`fill::column_of`).
fn cells(b: u32) -> &'static [u32] {
    let at = (b * CELLS_ARR_BYTES / 4) as usize;
    &planes().cells[at..at + clipmap::BACK_WORDS as usize]
}

const _: () = assert!(
    clipmap::COLUMNS_AT == 0 && clipmap::BACK_WORDS == clipmap::BRICK_WORDS,
    "a brick comes back whole"
);

/// The turn's clip levels: each center where it now stands (`at`, in units, X
/// then Y, by `Centre`; none where it is not), a brick no clip level holds any
/// more given up (`assign_slots`); the tiles a clip level now wants but holds
/// no packet for are asked again, and the bricks due found (`due`). The entries
/// are stepped only where a clip level moved a brick or its zoom, the device
/// went, the tile cache moved (`TileCache::revision`) or a fill handed: of
/// those alone are they made. Before the turn's arrivals and its stage. True
/// where the tiles the clip levels apart from the eye's want changed.
pub fn step(at: [Option<[f64; 2]>; 3], on: u32) -> bool {
    let w = clipmap();
    let terrain = TerrainTile::tile_cache();
    let lost = core::mem::replace(&mut w.on, on) != on;
    // The device that went took its pool: no brick holds a tree of it.
    if lost {
        mirror().clear();
    }
    // Per clip level, whether its zoom moved, and whether it moved at all.
    let (mut moved, mut changed) = ([false; CLIP_LEVELS as usize], [lost; CLIP_LEVELS as usize]);
    let kept = tile::uploads().cnt();
    let released = wake(w);
    let mut stale = lost
        || released
        || core::mem::take(&mut w.filled)
        || terrain.revision() != w.read
        || kept != w.kept;
    for clip_level in 0..CLIP_LEVELS {
        let (center, laid) = layout(clip_level);
        let p = at[center as usize];
        let cell = match (center, p) {
            // Where its line says the ground: open sea's leaf is the tile over it.
            (Center::Destination, Some(p)) => {
                let leaf = tile_at(LEAF_ZOOM, cell_at(LEAF_ZOOM, p));
                terrain.src(leaf).map_or(laid, |t| GROUND.cell_zoom(t.zoom))
            }
            _ => laid,
        };
        let first = p.map(|p| clipmap::origin(cell, cell_at(cell, p)));
        let was = (
            core::mem::replace(&mut w.clip_levels[clip_level as usize], ClipLevel { cell }).cell,
            first_of(w, clip_level),
        );
        w.entries[(SLOTS + clip_level) as usize] = match first {
            Some(f) => Entry {
                brick: f,
                present: 1,
                ticket: 0,
            },
            None => Entry::absent([i32::MIN; 2]),
        };
        moved[clip_level as usize] = was.0 != cell;
        changed[clip_level as usize] |= was != (cell, first);
        stale |= was != (cell, first);
    }
    if !stale {
        return false;
    }
    w.kept = kept;
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
            let entry = &mut w.entries[(clip_level * ENTRIES + s) as usize];
            let want = entry_brick(cell, f, s);
            if moved[clip_level as usize] || lost || entry.brick != want {
                entry.brick = want;
                terrain.moved_at(tile_of(cell, want).0);
            }
        }
    }
    // The slots follow the entries' bricks and the device: where neither
    // moved nor the heap woke a brick, every brick wanted holds its slot and
    // each entry shows it as its fill and landing left it.
    if released || changed.contains(&true) {
        assign_slots(w, lost);
    }
    // Tiles are asked again only as far as their packets could be kept,
    // in slot order, or they would come back only to be freed: while no
    // frame is drawn, so none gives a packet back, the rest are asked once
    // room is given back instead. Only an absent brick's tile is asked of
    // again.
    let mut room = tile::again_room();
    for b in 0..SLOTS {
        let held = w.slots[b as usize];
        if held.is_absent() {
            let t = tile_of(held.cell, held.brick).0;
            if room > 0 && !tile::pending_for(t) && !terrain.is_again_held(t) {
                terrain.again(t);
                room -= u32::from(terrain.is_again_held(t));
            }
        }
    }
    due(w);
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
            let t = tile_of(cell, w.entries[(clip_level * ENTRIES + s) as usize].brick).0;
            if !w.pinned.contains(&t) {
                w.pinned.push(t);
            }
        }
    }
    was[..] != w.pinned[..]
}

/// Each brick waiting on the heap absent again, to be handed again, where
/// the heap gave a page back since it refused one the mirror's room
/// (`WAITING`): true where it did.
fn wake(w: &mut Clipmap) -> bool {
    if !w.heap.is_some_and(|h| h.over()) {
        return false;
    }
    w.heap = None;
    for held in w.slots.iter_mut().filter(|b| b.ticket == WAITING) {
        held.ticket = 0;
    }
    true
}

/// Each brick a laid clip level wants held in one slot: the device that went
/// took every slot's cells; a slot no clip level wants any more is given up,
/// a fill on its way to it laying nothing (`on_resident`); and a brick wanted
/// that no slot holds takes a free one of its entry's, its clip level's own
/// first, absent. Then every entry's row shows its slot (`show`), and the
/// clip levels are folded over them (`fold`).
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
                && w.entries[(clip_level * ENTRIES + s) as usize].brick == held.brick
        };
        if !(0..CLIP_LEVELS).any(wants) {
            w.slots[b] = Slot::FREE;
        } else if lost {
            (w.slots[b].present, w.slots[b].ticket) = (false, 0);
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
                w.entries[(clip_level * ENTRIES + s) as usize].brick,
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
                present: false,
                ticket: 0,
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

/// Entry `i`'s row of the table: its brick, and its slot's state, present
/// there (`Entry::present`, the slot plus one), on its way, refused or
/// absent; absent while its clip level is not laid out.
fn show(w: &mut Clipmap, i: u32) {
    let (clip_level, brick) = (i / ENTRIES, w.entries[i as usize].brick);
    let held = first_of(w, clip_level)
        .and_then(|_| slot_of(w, w.clip_levels[clip_level as usize].cell, brick));
    w.entries[i as usize] = match held {
        Some(b) => Entry {
            brick,
            present: if w.slots[b as usize].present {
                b as i32 + 1
            } else {
                0
            },
            ticket: w.slots[b as usize].ticket,
        },
        None => Entry::absent(brick),
    };
}

/// A slot's ticket while its brick's ways' tree is refused, the pool or the
/// scratch at its cap (pxl-render gpu/ray/ground.rs `Short`), the brick past
/// its own most or its child groups past the mirror's cap (`on_resident`), none
/// of which asking again changes: never a fill's (`fill`), and not absent, so
/// the brick is not handed again every turn, nor its tile asked or kept for it,
/// while a clip level wants it (`assign_slots`). A reader reads the coarser
/// clip level's, as for any brick absent.
const REFUSED: u32 = u32::MAX;
/// A slot's ticket while the heap refused its brick's child groups the
/// mirror's room (`mirror::Unlaid::Heap`): not absent, as `REFUSED`, until
/// the heap gives a page back (`Clipmap::heap`), when it is absent again
/// and handed again (`step`).
const WAITING: u32 = u32::MAX - 1;
const _: () = assert!(WAITING > DONE - 1, "no fill's ticket waits");

/// Slot `b`'s cells back from its fill `ticket` (pxl-render `read_back`), and
/// how its ways' tree landed, where the slot still waits on that fill: built,
/// its child groups laid into the pool's mirror at their entries, it is present
/// and what is read over its tile moved; to be filled again
/// (`Resident::Again`), it is absent, handed again; refused, or its child
/// groups past the mirror's cap, it is absent and `REFUSED` while a clip level
/// wants it; its child groups refused the mirror's room by the heap, it is
/// absent and `WAITING` on the heap. A fill no slot waits on lays nothing: its
/// child groups may be another's by now. Every clip level's entry holding the
/// brick shows it (`show`). The clip levels are folded over it once the
/// read-back's landings are all in (`refold`).
pub fn on_resident(b: u32, ticket: u32, resident: Resident) {
    let w = clipmap();
    let held = &mut w.slots[b as usize];
    if held.ticket != ticket {
        return;
    }
    held.ticket = match resident {
        Resident::Built(records) => match mirror().lay(records) {
            Ok(()) => {
                held.present = true;
                TerrainTile::tile_cache().moved_at(tile_of(held.cell, held.brick).0);
                0
            }
            Err(Unlaid::Heap) => {
                w.heap.get_or_insert_with(Refusal::now);
                WAITING
            }
            Err(Unlaid::Cap) => REFUSED,
        },
        Resident::Again => 0,
        Resident::Refused => REFUSED,
    };
    // Absent again: what is due moves.
    w.filled |= !held.present;
    for i in of_entry(b % ENTRIES) {
        show(w, i);
    }
}

/// The clip levels folded again over the bricks a frame's read-back landed
/// (`on_resident`), once after all of them: the stage's, as soon as the frame it
/// renders returns (domain/scene/stage.rs `stage_render`), between whose
/// landings nothing reads a bound here, the renderer reading only the GPU's.
pub fn refold() {
    fold(clipmap());
}

/// The slots this turn hands, nearest first, as many as a frame's fill takes:
/// the eye's clip levels coarsest first, since a reader of a brick absent reads
/// the coarser clip level's (pxl-spec-core `clipmap::step`), then the pivot's
/// and a destination's; in a clip level by how far its brick lies from the
/// center. A brick another clip level listed first is not listed again. Only a
/// brick whose tile can hand it now: one waiting on its tile does not hold a
/// place another could take.
fn due(w: &mut Clipmap) {
    let terrain = TerrainTile::tile_cache();
    let handable =
        |t: Tile| tile::pending_for(t) || terrain.get(t).is_some() && terrain.slot(t).is_none();
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
            let q = w.entries[i as usize].brick;
            let Some(b) = slot_of(w, cell, q) else {
                continue;
            };
            if w.slots[b as usize].is_absent()
                && handable(tile_of(cell, q).0)
                && !w.due.contains(&b)
            {
                near.push((far(cell, first, q), b));
            }
        }
        near.sort_unstable();
        for &(_, i) in near.iter().take(FILL_CAP as usize - w.due.len()) {
            w.due.push(i);
        }
        if w.due.len() == FILL_CAP as usize {
            return;
        }
    }
}

/// The tiles of the bricks this turn hands (`due`) held with no frame,
/// each once: void tiles, whose bricks are filled off their palette indices alone
/// (`TileContent::Void`).
pub fn voids() -> List<Tile, { FILL_CAP as usize }> {
    let (w, terrain) = (clipmap(), TerrainTile::tile_cache());
    let mut voids = List::new();
    for &b in w.due.iter() {
        let held = w.slots[b as usize];
        let t = tile_of(held.cell, held.brick).0;
        if terrain.get(t).is_some() && terrain.slot(t).is_none() && !voids.contains(&t) {
            voids.push(t);
        }
    }
    voids
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
pub fn pinned_tiles() -> &'static [Tile] {
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
        w.entries[(clip_level * ENTRIES) as usize + s]
    }))
}

/// The clip levels' entries and their first bricks, as the GPU's cast reads
/// them (pxl-shader `World::present`, `World::origin`): a row an entry
/// (pxl-spec-core `clipmap::Entry`), then a row a clip level
/// (`clipmap::ORIGIN_AT`).
pub fn table() -> &'static [i32] {
    let rows = &clipmap().entries[..];
    // SAFETY: an entry is four plain words in order (`Entry`), so the entries are
    // their rows' words.
    unsafe { core::slice::from_raw_parts(rows.as_ptr().cast::<i32>(), 4 * rows.len()) }
}

/// What a tile hands its bricks: its packet and the tile as it is held, its
/// lines; or a void tile's own
/// palette indices over samples all zero.
pub enum TileContent<'a> {
    Packet(&'a [u8], &'a TerrainTile),
    Void,
}

/// A brick's four palette indices a cell, cut from its tile's, the tile's own
/// expanded where it holds one a cell, and its lines and the room they are
/// laid in: arrays of `.fixed.terrain` (fixed.rs `FILL_ARRAY_BYTES`).
struct Fill {
    room: &'static mut [GroundId],
    cut: &'static mut [GroundId],
    lines: &'static mut [u32],
    scratch: &'static mut [(u32, u32)],
}

static FILL: Global<Fill> = Global::new();

/// A void tile's samples.
static ZEROS: [i16; ELE_COLUMN] = [0; ELE_COLUMN];

/// Whether slot `b` holds a brick of `tile` a clip level wants, absent.
fn slot_wants(w: &Clipmap, tile: Tile, b: u32) -> bool {
    let held = w.slots[b as usize];
    held.is_absent()
        && held.cell == GROUND.cell_zoom(tile.zoom)
        && tile_of(held.cell, held.brick).0 == tile
}

/// Whether a clip level wants one of `tile`'s bricks, absent: its packet is
/// kept for it while one does.
pub fn is_tile_wanted(tile: Tile) -> bool {
    let w = clipmap();
    (0..SLOTS).any(|b| slot_wants(w, tile, b))
}

/// Each brick the turn hands (`due`) in `tile`, filled from `from` while
/// `write` takes them: its own palette indices here, its four palette indices and the tile's
/// samples to the GPU. Taken where no clip level wants another of its bricks;
/// kept where one still does, for a later turn; full where the stream ran
/// out first.
pub fn fill(tile: Tile, from: TileContent, write: &mut dyn FnMut(BrickFill<'_>) -> bool) -> Uptake {
    let cell = GROUND.cell_zoom(tile.zoom);
    let w = clipmap();
    let wants = |w: &Clipmap, b: u32| slot_wants(w, tile, b);
    let rest = |w: &Clipmap| match (0..SLOTS).any(|i| wants(w, i)) {
        true => Uptake::Kept,
        false => Uptake::Taken,
    };
    if !w.due.iter().any(|&i| wants(w, i)) {
        return rest(w);
    }
    let f = FILL.get(|| {
        let r = RANGE.get();
        // SAFETY: the fill's arrays are its alone, plain numbers, zeroed at boot.
        unsafe {
            Fill {
                room: arr(r, fill_off(0), PALETTE_COLUMN),
                cut: arr(r, fill_off(1), 4 * BRICK_CELLS as usize),
                lines: arr(r, fill_off(2), clipmap::LINES_WORDS as usize),
                scratch: arr(r, fill_off(3), clipmap::WAYS_BRICK_SEGMENTS as usize),
            }
        }
    });
    let (room, cut, lines, scratch) = (&mut *f.room, &mut *f.cut, &mut *f.lines, &mut *f.scratch);
    let void = TerrainTile::void();
    let held: &TerrainTile = match from {
        TileContent::Packet(_, held) => held,
        TileContent::Void => &void,
    };
    let (indices, ele): (&[GroundId], &[i16]) = match from {
        TileContent::Packet(pkt, _) => match gpu_cells(pkt, room) {
            Ok(h) => h,
            Err(_) => {
                w.filled = true;
                return Uptake::Taken;
            }
        },
        TileContent::Void => {
            for (q, &l) in room.chunks_mut(4).zip(void.ground.cells.iter()) {
                q.fill(l);
            }
            (&room[..], &ZEROS[..])
        }
    };
    for k in 0..w.due.len() {
        let b = w.due[k];
        if !wants(w, b) {
            continue;
        }
        let key = w.slots[b as usize].brick;
        let place = tile_of(cell, key).1;
        // The brick's cells out of the tile's, four palette indices a cell, each at
        // its index in the brick (`clipmap::idx_of`).
        let side = GROUND.tile_cells() as usize;
        let (x0, y0) = ((place[0] * BRICK) as usize, (place[1] * BRICK) as usize);
        for i in 0..BRICK_CELLS {
            let [x, y] = clipmap::of_idx(i);
            let from = 4 * ((y0 + y as usize) * side + x0 + x as usize);
            cut[4 * i as usize..4 * i as usize + 4].copy_from_slice(&indices[from..from + 4]);
        }
        let written = brick_lines(tile, &held.separation, place, scratch, lines);
        // Under `DONE`, which a read's word holds once it lands, so no
        // ticket reads as landed before it is: 1 to `DONE` − 1, never
        // `REFUSED`, which is past them.
        w.ticket = w.ticket % (DONE - 1) + 1;
        let back = CellsBack {
            cells: off_of_slice(cells(b)) as u32,
            ticket: w.ticket,
        };
        let shard = Shard::UNKNOWN.at(&ServiceArea, (clipmap::bricks_zoom(cell), key));
        let taken = write(BrickFill {
            words: clipmap::fill_words(
                b,
                clipmap::place_word(place),
                cell,
                [key[0] << BRICK_SHIFT, key[1] << BRICK_SHIFT],
                shard,
            ),
            palette_indices: cut,
            samples: ele,
            lines: &lines[..written],
            back,
        });
        if !taken {
            return Uptake::Full;
        }
        w.slots[b as usize].ticket = back.ticket;
        for i in of_entry(b % ENTRIES) {
            show(w, i);
        }
        w.filled = true;
    }
    rest(w)
}

/// The clip levels as the CPU holds them, for pxl-spec-core's one reading of a
/// cell (`clipmap::step`, `clipmap::bound_over`): each brick's columns, levels
/// and ways' tree's root as the GPU drew them, read back (its tree's child
/// groups in the pool's mirror, `pool`), the eye's clip levels' levels over
/// them folded here (`fold`); and the service area they end at (pxl-spec-core
/// `area::Area`).
#[derive(Clone, Copy)]
pub struct CpuClipmap;

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
    /// Present as the GPU holds it: its cells back, in the slot holding
    /// it, whichever clip levels want it.
    fn present(&self, cell: Zoom, q: [i32; 2]) -> (u32, bool) {
        let w = self.0;
        let held = slot_of(w, cell, q).filter(|&b| w.slots[b as usize].present);
        (held.unwrap_or(0), held.is_some())
    }
}

impl Bricks for Holding<'_> {
    fn brick(&self, cell: Zoom, entry: u32) -> ([i32; 2], bool) {
        self.0.entries[((cell - FIRST_CELL) * ENTRIES + entry) as usize].brick()
    }

    /// The CPU reads a cell's own palette index alone, as laid: its four are it four
    /// times.
    fn palette_word(&self, cell: Zoom, b: u32, i: u32) -> u32 {
        word([self.laid(cell, b, i) & 0xff; 4])
    }

    /// As this side folded it over its bricks (`fold`).
    fn joined(&self, cell: Zoom, e: u32) -> [u32; clipmap::JOINED_WORDS as usize] {
        self.0.joined[((cell - FIRST_CELL) * clipmap::JOINED_ENTRIES + e) as usize]
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

    fn level(&self, _: Zoom, b: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
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
    fn level(&self, cell: Zoom, b: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
        Holding(clipmap()).level(cell, b, j, at)
    }
    fn root(&self, cell: Zoom, b: u32) -> u32 {
        Holding(clipmap()).root(cell, b)
    }
}

const _: () = assert!(LEVELS <= BRICK_SHIFT);

#[cfg(test)]
#[path = "clipmap.test.rs"]
pub(crate) mod test;
