// pxl-kernel/domain/layer/terrain/window.rs

//! The ground's cells as the kernel holds them: a window a zoom about each
//! centre a reader declares — the eye, the focus where walking and editing
//! happen, a destination being gone to — each as deep as its reader reads
//! (pxl-spec-core `window`). A square at a zoom is held once, in a berth,
//! however many windows want it (`Berth`). It is filled from its tile as it
//! lands: its four looks and its tile's samples handed to the GPU, which
//! draws and lays its cells; they come back into `.fixed.terrain`, and the
//! square is present from then on (`landed`). Nothing of a tile is kept for
//! a square still to come: one that enters asks its tile again
//! (`Residency::again`), which the browser's cache answers.

use pxl_spec::bake::tile::terrain::{
    handed, square_lines, GroundId, TerrainTile, ELE_LANE, LOOK_LANE,
};
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::layer::lattice::{GROUND, TILE_CELLS};
use pxl_spec::space::layer::tile::{tile_at, Tile, LEAF_ZOOM};
use pxl_spec_core::area::{Area, Shard};
use pxl_spec_core::fill;
use pxl_spec_core::ground::word;
use pxl_spec_core::window::{
    self, Slot, Squares, EYE_WINDOWS, FIRST_CELL, HANDED, LEVELS, SLOTS, SQUARE, SQUARES,
    SQUARE_CELLS, SQUARE_SHIFT, WINDOWS,
};
use pxl_spec_core::wrap_index;
use pxl_spec_core::LEAF_CELL;
use xpute_guest::global::Global;
use xpute_guest::mem::off_of_slice;
use xpute_kit::collection::list::List;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::domain::layer::kept::Offered;
use crate::domain::layer::terrain::fixed::{
    cells_off, fill_off, joined_off, CELLS_LANE_BYTES, RANGE,
};
use crate::domain::layer::terrain::mirror::{mirror, Mirror, Unlaid};
use crate::domain::layer::Base;
use crate::sys::io::room::Refused;
use crate::sys::io::source::Declared;
use crate::sys::mem::fixed::lane;
use pxl_render::gpu::api::DONE;
use pxl_render::gpu::renderer::{CellsBack, FreshSquare, Landing};

/// Who a window is about.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Centre {
    Eye,
    Focus,
    Destination,
}

/// The tiles the windows apart from the eye's want at most: a window spans
/// two tiles a side.
pub const PINNED_TILES: u32 = (WINDOWS - EYE_WINDOWS) * 4;
const _: () = assert!(
    window::WINDOW <= TILE_CELLS + 1,
    "a window spans two tiles a side"
);

/// Who window `w` is about, and its cells' zoom as laid out; a
/// destination's is its line's.
const fn layout(w: u32) -> (Centre, Zoom) {
    match w {
        w if w < EYE_WINDOWS => (Centre::Eye, window::eye_cell(w)),
        w if w < EYE_WINDOWS + 2 => (Centre::Focus, LEAF_CELL - 1 + (w - EYE_WINDOWS)),
        _ => (Centre::Destination, LEAF_CELL),
    }
}

/// A window: its cells' zoom. Its first square is its row past the slots
/// (`first_of`).
#[derive(Clone, Copy, PartialEq, Eq)]
struct Window {
    cell: Zoom,
}

/// Where a square of the ground is held: its row of the GPU's squares
/// (pxl-render gpu/ray/ground.rs `GROUND_BYTES`) and its lane of cells back
/// in `.fixed.terrain` (`cells`), one a square at a zoom however many
/// windows want it. Windows at one zoom want a square in the same slot
/// (`window::slot`), so it is held in one of that slot's berths, `WINDOWS`
/// of them (`of_slot`); no more squares want a slot than there are
/// windows, so a square wanted always finds one free (`berth`), and
/// windows that overlap take no berth more than the squares they want. A
/// window takes its own where it is free: windows that never overlapped
/// hold their squares as if alone, but once two have overlapped and
/// parted, one may hold its squares in the other's.
#[derive(Clone, Copy)]
struct Berth {
    /// Its square's cells' zoom, 0 where it holds none (`is_free`), no
    /// window's.
    cell: Zoom,
    square: [i32; 2],
    present: bool,
    /// The fill its cells are on their way from, 0 where none is, or
    /// `REFUSED`, or `WAITING`.
    ticket: u32,
}

const _: () = assert!(FIRST_CELL > 0, "a free berth's zoom is no window's");

impl Berth {
    const FREE: Berth = Berth {
        cell: 0,
        square: [i32::MIN; 2],
        present: false,
        ticket: 0,
    };

    fn is_free(&self) -> bool {
        self.cell == 0
    }

    /// Whether it holds square `q` at `cell`, present or not.
    fn holds(&self, cell: Zoom, q: [i32; 2]) -> bool {
        self.cell == cell && self.square == q
    }

    /// Wanted, its tile not yet handed it: holding a square, neither present
    /// nor on its way nor refused.
    fn is_absent(&self) -> bool {
        !self.is_free() && !self.present && self.ticket == 0
    }
}

/// The berths of slot `s`, and every window's slot `s`: `k × SLOTS + s`,
/// `k` to `WINDOWS`.
fn of_slot(s: u32) -> impl Iterator<Item = u32> {
    (0..WINDOWS).map(move |k| k * SLOTS + s)
}

struct Windows {
    windows: [Window; WINDOWS as usize],
    /// Every window's slots, each its square and its berth's state, then
    /// each window's first square, a row shaped as a slot's, its square the
    /// first and present while it is laid out, none while its centre is not
    /// (pxl-spec-core `window::ORIGIN_AT`): the GPU's table as it is
    /// (`table`).
    slots: [Slot; (SQUARES + WINDOWS) as usize],
    /// The squares the windows want, each in one berth (`Berth`).
    berths: [Berth; SQUARES as usize],
    /// The device the slots' squares were handed to (`device_generation`):
    /// one that went took them all.
    on: u32,
    ticket: u32,
    /// What the slots were last stepped over: the residency's revision
    /// (`Residency::revision`), and whether a fill handed a square or let a
    /// packet go since, which moves what is due and what is asked again.
    read: u32,
    filled: bool,
    /// The packets kept when the slots were last stepped over: one given
    /// back may leave room to ask again for a tile whose packet was not
    /// kept (`tile::again_room`).
    kept: u32,
    /// The tiles the windows apart from the eye's want (`pinned`).
    pinned: List<Tile, { PINNED_TILES as usize }>,
    /// The berths this turn hands the fill (`due`).
    due: List<u32, { HANDED as usize }>,
    /// The heap as it was when it first refused a square's families the
    /// mirror's room, while one waits on it (`WAITING`).
    heap: Option<Refused>,
}

static WINDOWS_STATE: Global<Windows> = Global::new();

fn windows() -> &'static mut Windows {
    WINDOWS_STATE.get(fresh)
}

/// Window `win`'s first square, none while its centre is not.
fn first_of(w: &Windows, win: u32) -> Option<[i32; 2]> {
    let (first, laid) = w.slots[(SQUARES + win) as usize].held();
    laid.then_some(first)
}

/// The windows before any centre stands: every slot absent.
fn fresh() -> Windows {
    Windows {
        windows: core::array::from_fn(|w| Window {
            cell: layout(w as u32).1,
        }),
        slots: [Slot::absent([i32::MIN; 2]); (SQUARES + WINDOWS) as usize],
        berths: [Berth::FREE; SQUARES as usize],
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
        wrap_index(cell, (p[0] / side).floor() as i32),
        (p[1] / side).floor() as i32,
    ]
}

/// The square window `first` holds in slot `s`.
fn wanted(cell: Zoom, first: [i32; 2], s: u32) -> [i32; 2] {
    let m = (window::ACROSS - 1) as i32;
    let at = |o: i32, k: i32| o + ((k - (o & m)) & m);
    let q = [
        at(first[0], (s % window::ACROSS) as i32),
        at(first[1], (s / window::ACROSS) as i32),
    ];
    [wrap_index(cell - SQUARE_SHIFT, q[0]), q[1]]
}

/// The tile square `q` at `cell` lies in, and where in it, in squares.
pub(crate) fn tile_of(cell: Zoom, q: [i32; 2]) -> (Tile, [u32; 2]) {
    let per = (GROUND.tile_cells() >> SQUARE_SHIFT) as i32;
    let zoom = cell - GROUND.tile_cell_shamt();
    let t = tile_at(zoom, [q[0].div_euclid(per), q[1].div_euclid(per)]);
    (
        t,
        [q[0].rem_euclid(per) as u32, q[1].rem_euclid(per) as u32],
    )
}

/// Every square's planes on the CPU, in `.fixed.terrain` (fixed.rs): its
/// cells as they come back, and the eye's windows' fold; a test's own. Its
/// ways' trees' families are the heap's (mirror.rs).
pub(crate) struct Planes {
    /// Each square's words as they came back, whole (`window::BACK_WORDS`),
    /// `CELLS_LANE_BYTES` apart.
    pub cells: &'static mut [u32],
    /// The eye's windows' levels over their squares, as the GPU folded them
    /// and the renderer reads them back (pxl-spec-core `window::fold`).
    pub joined: &'static mut [u32],
}

pub(crate) static PLANES: Global<Planes> = Global::new();

fn planes() -> &'static mut Planes {
    PLANES.get(|| {
        let r = RANGE.get();
        // SAFETY: the planes are the windows' alone, plain numbers, zeroed at boot.
        unsafe {
            Planes {
                cells: lane(r, cells_off(0), (SQUARES * CELLS_LANE_BYTES / 4) as usize),
                joined: lane(r, joined_off(), window::EYE_JOINED_WORDS as usize),
            }
        }
    })
}

/// Where the eye's windows' levels over their squares come back to in the
/// module's memory (pxl-render `GroundSquares::joined`).
pub fn joined_lane() -> u32 {
    let joined: &[u32] = planes().joined;
    off_of_slice(joined) as u32
}

/// The GPU's ways' pool as the CPU mirrors it, the landed squares' families
/// at the GPU's slots, which a cursor reads as the cast reads the GPU's
/// (pxl-spec-core `way::tree`).
pub fn pool() -> &'static Mirror {
    mirror()
}

/// Berth `b`'s words as they came back: a square's words whole, from its
/// first, which pxl-spec-core reads a square's words by (`fill::column_of`).
fn cells(b: u32) -> &'static [u32] {
    let at = (b * CELLS_LANE_BYTES / 4) as usize;
    &planes().cells[at..at + window::BACK_WORDS as usize]
}

const _: () = assert!(
    window::COLUMNS_AT == 0 && window::BACK_WORDS == window::SQUARE_WORDS,
    "a square comes back whole"
);

/// The turn's windows: each centre where it now stands (`at`, in units, X
/// then Y, by `Centre`; none where it is not), a square no window holds any
/// more given up (`berth`); the tiles a window now wants but holds no
/// packet for are asked again, and the squares due found (`due`). The
/// slots are stepped only where a window moved a square or its zoom, the
/// device went, the residency moved (`Residency::revision`) or a fill
/// handed: of those alone are they made. Before the turn's arrivals and its
/// stage. True where the tiles the windows apart from the eye's want
/// changed.
pub fn step(at: [Option<[f64; 2]>; 3], on: u32) -> bool {
    let w = windows();
    let terrain = TerrainTile::residency();
    let lost = core::mem::replace(&mut w.on, on) != on;
    // The device that went took its pool: no square holds a tree of it.
    if lost {
        mirror().clear();
    }
    // Per window, whether its zoom moved, and whether it moved at all.
    let (mut moved, mut changed) = ([false; WINDOWS as usize], [lost; WINDOWS as usize]);
    let kept = crate::domain::layer::terrain::tile::kept().count();
    let released = wake(w);
    let mut stale = lost
        || released
        || core::mem::take(&mut w.filled)
        || terrain.revision() != w.read
        || kept != w.kept;
    for win in 0..WINDOWS {
        let (centre, laid) = layout(win);
        let p = at[centre as usize];
        let cell = match (centre, p) {
            // Where its line says the ground: open sea's leaf is the tile over it.
            (Centre::Destination, Some(p)) => {
                let leaf = tile_at(LEAF_ZOOM, cell_at(LEAF_ZOOM, p));
                terrain
                    .source(leaf)
                    .map_or(laid, |t| GROUND.cell_zoom(t.zoom))
            }
            _ => laid,
        };
        let first = p.map(|p| window::origin(cell, cell_at(cell, p)));
        let was = (
            core::mem::replace(&mut w.windows[win as usize], Window { cell }).cell,
            first_of(w, win),
        );
        w.slots[(SQUARES + win) as usize] = match first {
            Some(f) => Slot {
                square: f,
                present: 1,
                ticket: 0,
            },
            None => Slot::absent([i32::MIN; 2]),
        };
        moved[win as usize] = was.0 != cell;
        changed[win as usize] |= was != (cell, first);
        stale |= was != (cell, first);
    }
    if !stale {
        return false;
    }
    w.kept = kept;
    // A window that did not move wants its squares where they are.
    for win in 0..WINDOWS {
        let (cell, first) = (w.windows[win as usize].cell, first_of(w, win));
        let Some(f) = first.filter(|_| changed[win as usize]) else {
            continue;
        };
        for s in 0..SLOTS {
            let slot = &mut w.slots[(win * SLOTS + s) as usize];
            let want = wanted(cell, f, s);
            if moved[win as usize] || lost || slot.square != want {
                slot.square = want;
                terrain.moved_at(tile_of(cell, want).0);
            }
        }
    }
    berth(w, lost);
    // Tiles are asked again only as far as their packets could be kept,
    // in berth order, or they would come back only to be freed: while no
    // frame is drawn, so none gives a packet back, the rest are asked once
    // room is given back instead. Only an absent square's tile is asked of
    // again.
    let mut room = crate::domain::layer::terrain::tile::again_room();
    for b in 0..SQUARES {
        let held = w.berths[b as usize];
        if held.is_absent() {
            let t = tile_of(held.cell, held.square).0;
            if room > 0
                && !crate::domain::layer::terrain::tile::kept_for(t)
                && !terrain.again_held(t)
            {
                terrain.again(t);
                room -= u32::from(terrain.again_held(t));
            }
        }
    }
    due(w);
    w.read = terrain.revision();
    // What the windows apart from the eye's want moves only with them.
    if !changed[EYE_WINDOWS as usize..].contains(&true) {
        return false;
    }
    let was = core::mem::take(&mut w.pinned);
    for win in EYE_WINDOWS..WINDOWS {
        let cell = w.windows[win as usize].cell;
        if first_of(w, win).is_none() {
            continue;
        }
        for s in 0..SLOTS {
            let t = tile_of(cell, w.slots[(win * SLOTS + s) as usize].square).0;
            if !w.pinned.contains(&t) {
                w.pinned.push(t);
            }
        }
    }
    was[..] != w.pinned[..]
}

/// Each square waiting on the heap absent again, to be handed again, where
/// the heap gave a page back since it refused one the mirror's room
/// (`WAITING`): true where it did.
fn wake(w: &mut Windows) -> bool {
    if !w.heap.is_some_and(|h| h.over()) {
        return false;
    }
    w.heap = None;
    for held in w.berths.iter_mut().filter(|b| b.ticket == WAITING) {
        held.ticket = 0;
    }
    true
}

/// Each square a laid window wants held in one berth: the device that went
/// took every berth's cells; a berth no window wants any more is given up,
/// a fill on its way to it laying nothing (`landed`); and a square wanted
/// that no berth holds takes a free one of its slot's, its window's own
/// first, absent. Then every slot's row shows its berth (`show`).
fn berth(w: &mut Windows, lost: bool) {
    for b in 0..SQUARES as usize {
        let held = w.berths[b];
        if held.is_free() {
            continue;
        }
        let s = window::slot(held.square);
        let wants = |win: u32| {
            w.windows[win as usize].cell == held.cell
                && first_of(w, win).is_some()
                && w.slots[(win * SLOTS + s) as usize].square == held.square
        };
        if !(0..WINDOWS).any(wants) {
            w.berths[b] = Berth::FREE;
        } else if lost {
            (w.berths[b].present, w.berths[b].ticket) = (false, 0);
        }
    }
    for win in 0..WINDOWS {
        let cell = w.windows[win as usize].cell;
        if first_of(w, win).is_none() {
            continue;
        }
        for s in 0..SLOTS {
            let (own, q) = (win * SLOTS + s, w.slots[(win * SLOTS + s) as usize].square);
            if berth_of(w, cell, q).is_some() {
                continue;
            }
            let free = |b: &u32| w.berths[*b as usize].is_free();
            let b = Some(own).filter(free).or_else(|| of_slot(s).find(free));
            // No more squares want a slot than it has berths.
            let b = b.or_bug(Errno::ENOTRECOVERABLE);
            w.berths[b as usize] = Berth {
                cell,
                square: q,
                present: false,
                ticket: 0,
            };
        }
    }
    for i in 0..SQUARES {
        show(w, i);
    }
}

/// The berth holding square `q` at `cell`, present or not.
fn berth_of(w: &Windows, cell: Zoom, q: [i32; 2]) -> Option<u32> {
    of_slot(window::slot(q)).find(|&b| w.berths[b as usize].holds(cell, q))
}

/// Slot `i`'s row of the table: its square, and its berth's state, present
/// there (`Slot::present`, the berth plus one), on its way, refused or
/// absent; absent while its window is not laid out.
fn show(w: &mut Windows, i: u32) {
    let (win, square) = (i / SLOTS, w.slots[i as usize].square);
    let held = first_of(w, win).and_then(|_| berth_of(w, w.windows[win as usize].cell, square));
    w.slots[i as usize] = match held {
        Some(b) => Slot {
            square,
            present: if w.berths[b as usize].present {
                b as i32 + 1
            } else {
                0
            },
            ticket: w.berths[b as usize].ticket,
        },
        None => Slot::absent(square),
    };
}

/// A berth's ticket while its square's ways' tree is refused, the pool or
/// the scratch at its cap (pxl-render gpu/ray/ground.rs `Short`), the square
/// past its own most or its families past the mirror's cap (`landed`), none
/// of which asking again changes: never a fill's (`fill`), and not absent,
/// so the square is not handed again every turn, nor its tile asked or kept
/// for it, while a window wants it (`berth`). A reader reads the coarser
/// window's, as for any square absent.
const REFUSED: u32 = u32::MAX;
/// A berth's ticket while the heap refused its square's families the
/// mirror's room (`mirror::Unlaid::Heap`): not absent, as `REFUSED`, until
/// the heap gives a page back (`Windows::heap`), when it is absent again
/// and handed again (`step`).
const WAITING: u32 = u32::MAX - 1;
const _: () = assert!(WAITING > DONE - 1, "no fill's ticket waits");

/// Berth `b`'s cells back from its fill `ticket` (pxl-render
/// `read_back`), and how its ways' tree landed, where the berth still waits
/// on that fill: built, its families laid into the pool's mirror at their
/// slots, it is present and what is read over its tile moved; to be filled
/// again (`Landing::Again`), it is absent, handed again; refused, or its
/// families past the mirror's cap, it is absent and `REFUSED` while a window
/// wants it; its families refused the mirror's room by the heap, it is absent
/// and `WAITING` on the heap. A fill no berth waits on lays
/// nothing: its families may be another's by now. Every window's slot
/// holding the square shows it (`show`).
pub fn landed(b: u32, ticket: u32, landing: Landing) {
    let w = windows();
    let held = &mut w.berths[b as usize];
    if held.ticket != ticket {
        return;
    }
    held.ticket = match landing {
        Landing::Built(records) => match mirror().lay(records) {
            Ok(()) => {
                held.present = true;
                TerrainTile::residency().moved_at(tile_of(held.cell, held.square).0);
                0
            }
            Err(Unlaid::Heap) => {
                w.heap.get_or_insert_with(Refused::now);
                WAITING
            }
            Err(Unlaid::Cap) => REFUSED,
        },
        Landing::Again => 0,
        Landing::Refused => REFUSED,
    };
    // Absent again: what is due moves.
    w.filled |= !held.present;
    for i in of_slot(b % SLOTS) {
        show(w, i);
    }
}

/// The berths this turn hands, nearest first, as many as a frame's fill
/// takes: the eye's windows coarsest first, since a reader of a square
/// absent reads the coarser window's (pxl-spec-core `window::step`), then the
/// focus's and a destination's; in a window by how far its square lies from
/// the centre. A square another window listed first is not listed again.
/// Only a square whose tile can hand it now: one waiting on its tile does
/// not hold a place another could take.
fn due(w: &mut Windows) {
    let terrain = TerrainTile::residency();
    let handable = |t: Tile| {
        crate::domain::layer::terrain::tile::kept_for(t)
            || terrain.get(t).is_some() && terrain.frame(t).is_none()
    };
    w.due.clear();
    for win in 0..WINDOWS {
        let (cell, Some(first)) = (w.windows[win as usize].cell, first_of(w, win)) else {
            continue;
        };
        let mut near = List::<(i64, u32), { SLOTS as usize }>::new();
        for i in win * SLOTS..(win + 1) * SLOTS {
            let q = w.slots[i as usize].square;
            let Some(b) = berth_of(w, cell, q) else {
                continue;
            };
            if w.berths[b as usize].is_absent()
                && handable(tile_of(cell, q).0)
                && !w.due.contains(&b)
            {
                near.push((far(cell, first, q), b));
            }
        }
        near.sort_unstable();
        for &(_, i) in near.iter().take(HANDED as usize - w.due.len()) {
            w.due.push(i);
        }
        if w.due.len() == HANDED as usize {
            return;
        }
    }
}

/// The tiles of the squares this turn hands (`due`) held with no frame,
/// each once: void tiles, whose squares are filled off their looks alone
/// (`Handed::Void`).
pub fn voids() -> List<Tile, { HANDED as usize }> {
    let (w, terrain) = (windows(), TerrainTile::residency());
    let mut voids = List::new();
    for &b in w.due.iter() {
        let held = w.berths[b as usize];
        let t = tile_of(held.cell, held.square).0;
        if terrain.get(t).is_some() && terrain.frame(t).is_none() && !voids.contains(&t) {
            voids.push(t);
        }
    }
    voids
}

/// How far square `q` at `cell` lies from the centre of the window whose
/// first square is `first`, squared, in cells, X the short way round: the
/// centre as the window places it, the middle of the square's worth of
/// cells its first holds it over (`window::origin`).
fn far(cell: Zoom, first: [i32; 2], q: [i32; 2]) -> i64 {
    let mid = |v: i32| i64::from(v) * i64::from(SQUARE) + i64::from(SQUARE / 2);
    let centre = |v: i32| mid(v) + i64::from(window::RADIUS);
    let round = 1i64 << cell;
    let dx = (mid(q[0]) - centre(first[0])).rem_euclid(round);
    let dx = dx.min(round - dx);
    let dy = mid(q[1]) - centre(first[1]);
    dx * dx + dy * dy
}

/// The tiles the focus's and a destination's windows want, which the eye's
/// demand need not name: held hard, as a destination's line is
/// (pxl-kernel domain/view/demand.rs `observe`).
pub fn pinned() -> &'static [Tile] {
    &windows().pinned
}

/// The eye's window at `cell`'s slots as it holds them, each square
/// present or still wanted (`Slot`); none while its centre is not. A debug
/// reader's (domain/scene/indicator/coverage_gizmo.rs): a copy, moving nothing.
pub fn eye_slots(cell: Zoom) -> Option<[Slot; SLOTS as usize]> {
    let (w, win) = (
        windows(),
        cell.checked_sub(FIRST_CELL).filter(|&w| w < EYE_WINDOWS)?,
    );
    first_of(w, win)?;
    Some(core::array::from_fn(|s| {
        w.slots[(win * SLOTS) as usize + s]
    }))
}

/// The windows' slots and their first squares, as the GPU's cast reads
/// them (pxl-shader `World::present`, `World::origin`): a row a slot
/// (pxl-spec-core `window::Slot`), then a row a window (`window::ORIGIN_AT`).
pub fn table() -> &'static [i32] {
    let rows = &windows().slots[..];
    // SAFETY: a slot is four plain words in order (`Slot`), so the slots are
    // their rows' words.
    unsafe { core::slice::from_raw_parts(rows.as_ptr().cast::<i32>(), 4 * rows.len()) }
}

/// What a tile hands its squares: its packet and the tile as it is held, its
/// lines; or a void tile's own
/// looks over samples all zero.
pub enum Handed<'a> {
    Packet(&'a [u8], &'a TerrainTile),
    Void,
}

/// A square's four looks a cell, cut from its tile's, the tile's own
/// expanded where it holds one a cell, and its lines and the room they are
/// laid in: lanes of `.fixed.terrain` (fixed.rs `FILL_LANE_BYTES`).
struct Fill {
    room: &'static mut [GroundId],
    cut: &'static mut [GroundId],
    lines: &'static mut [u32],
    scratch: &'static mut [(u32, u32)],
}

static FILL: Global<Fill> = Global::new();

/// A void tile's samples.
static ZEROS: [i16; ELE_LANE] = [0; ELE_LANE];

/// Whether berth `b` holds a square of `tile` a window wants, absent.
fn berth_wants(w: &Windows, tile: Tile, b: u32) -> bool {
    let held = w.berths[b as usize];
    held.is_absent()
        && held.cell == GROUND.cell_zoom(tile.zoom)
        && tile_of(held.cell, held.square).0 == tile
}

/// Whether a window wants one of `tile`'s squares, absent: its packet is
/// kept for it while one does.
pub fn tile_wanted(tile: Tile) -> bool {
    let w = windows();
    (0..SQUARES).any(|b| berth_wants(w, tile, b))
}

/// Each square the turn hands (`due`) in `tile`, filled from `from` while
/// `write` takes them: its own looks here, its four looks and the tile's
/// samples to the GPU. Taken where no window wants another of its squares;
/// kept where one still does, for a later turn; full where the stream ran
/// out first.
pub fn fill(tile: Tile, from: Handed, write: &mut dyn FnMut(FreshSquare<'_>) -> bool) -> Offered {
    let cell = GROUND.cell_zoom(tile.zoom);
    let w = windows();
    let wants = |w: &Windows, b: u32| berth_wants(w, tile, b);
    let rest = |w: &Windows| match (0..SQUARES).any(|i| wants(w, i)) {
        true => Offered::Kept,
        false => Offered::Taken,
    };
    if !w.due.iter().any(|&i| wants(w, i)) {
        return rest(w);
    }
    let f = FILL.get(|| {
        let r = RANGE.get();
        // SAFETY: the fill's lanes are its alone, plain numbers, zeroed at boot.
        unsafe {
            Fill {
                room: lane(r, fill_off(0), LOOK_LANE),
                cut: lane(r, fill_off(1), 4 * SQUARE_CELLS as usize),
                lines: lane(r, fill_off(2), window::LINES_WORDS as usize),
                scratch: lane(r, fill_off(3), window::WAYS_SQUARE_SEGMENTS as usize),
            }
        }
    });
    let (room, cut, lines, scratch) = (&mut *f.room, &mut *f.cut, &mut *f.lines, &mut *f.scratch);
    let void = TerrainTile::void();
    let held: &TerrainTile = match from {
        Handed::Packet(_, held) => held,
        Handed::Void => &void,
    };
    let (lane, ele): (&[GroundId], &[i16]) = match from {
        Handed::Packet(pkt, _) => match handed(pkt, room) {
            Ok(h) => h,
            Err(_) => {
                w.filled = true;
                return Offered::Taken;
            }
        },
        Handed::Void => {
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
        let key = w.berths[b as usize].square;
        let place = tile_of(cell, key).1;
        // The square's cells out of the tile's, four looks a cell, each at
        // its index in the square (`window::index_of`).
        let side = GROUND.tile_cells() as usize;
        let (x0, y0) = ((place[0] * SQUARE) as usize, (place[1] * SQUARE) as usize);
        for i in 0..SQUARE_CELLS {
            let [x, y] = window::of_index(i);
            let from = 4 * ((y0 + y as usize) * side + x0 + x as usize);
            cut[4 * i as usize..4 * i as usize + 4].copy_from_slice(&lane[from..from + 4]);
        }
        let written = square_lines(tile, &held.separation, place, scratch, lines);
        // Under `DONE`, which a read's word holds once it lands, so no
        // ticket reads as landed before it is: 1 to `DONE` − 1, never
        // `REFUSED`, which is past them.
        w.ticket = w.ticket % (DONE - 1) + 1;
        let back = CellsBack {
            cells: off_of_slice(cells(b)) as u32,
            ticket: w.ticket,
        };
        let shard = Shard::UNKNOWN.at(&Declared, (window::squares_zoom(cell), key));
        let taken = write(FreshSquare {
            fresh: window::fresh(
                b,
                window::place_word(place),
                cell,
                [key[0] << SQUARE_SHIFT, key[1] << SQUARE_SHIFT],
                shard,
            ),
            looks: cut,
            samples: ele,
            lines: &lines[..written],
            back,
        });
        if !taken {
            return Offered::Full;
        }
        w.berths[b as usize].ticket = back.ticket;
        for i in of_slot(b % SLOTS) {
            show(w, i);
        }
        w.filled = true;
    }
    rest(w)
}

/// The windows as the CPU holds them, for pxl-spec-core's one reading of a
/// cell (`window::step`, `window::bound_over`): each square's columns,
/// levels and ways' tree's root as the GPU drew and folded them, read back
/// (its tree's families in the pool's mirror, `pool`); and
/// the service area they end at (pxl-spec-core `area::Area`).
#[derive(Clone, Copy)]
pub struct Windowed;

impl Area for Windowed {
    fn ends(&self) -> bool {
        Declared.ends()
    }
    fn nine(&self, key: u32) -> u32 {
        Declared.nine(key)
    }
}

impl Squares for Windowed {
    /// Present as the GPU holds it: its cells back, in the berth holding
    /// it, whichever windows want it.
    fn present(&self, cell: Zoom, q: [i32; 2]) -> (u32, bool) {
        let w = windows();
        let held = berth_of(w, cell, q).filter(|&b| w.berths[b as usize].present);
        (held.unwrap_or(0), held.is_some())
    }

    fn square(&self, cell: Zoom, slot: u32) -> ([i32; 2], bool) {
        windows().slots[((cell - FIRST_CELL) * SLOTS + slot) as usize].held()
    }

    /// The CPU reads a cell's own look alone, as laid: its four are it four
    /// times.
    fn look(&self, cell: Zoom, b: u32, i: u32) -> u32 {
        word([self.laid(cell, b, i) & 0xff; 4])
    }

    /// As the GPU folded it, read back a frame or more later: an entry for
    /// a cell no longer held names another, and reads none (pxl-spec-core
    /// `window::unfold`).
    fn joined(&self, cell: Zoom, e: u32) -> [u32; window::JOINED_WORDS as usize] {
        let at =
            (((cell - FIRST_CELL) * window::JOINED_ENTRIES + e) * window::JOINED_WORDS) as usize;
        core::array::from_fn(|k| planes().joined[at + k])
    }

    fn origin(&self, cell: Zoom) -> ([i32; 2], bool) {
        let first = first_of(windows(), cell - FIRST_CELL);
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

const _: () = assert!(LEVELS <= SQUARE_SHIFT);

#[cfg(test)]
#[path = "window.test.rs"]
pub(crate) mod test;
