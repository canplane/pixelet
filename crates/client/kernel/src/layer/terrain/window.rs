// pxl-kernel/layer/terrain/window.rs

//! The ground's cells as the kernel holds them: a window a zoom about each
//! centre a reader declares — the eye, the focus where walking and editing
//! happen, a destination being gone to — each as deep as its reader reads
//! (pxl-portable `window`). A square is filled from its tile as it lands:
//! its four looks and its tile's samples handed to the GPU, which draws and
//! lays its cells; they come back into `.fixed.terrain`, and the
//! square is present from then on (`landed`). Nothing of a tile is kept for a square
//! still to come: one that enters asks its tile again (`Residency::again`),
//! which the browser's cache answers.

use pxl_portable::area::{Area, Shard};
use pxl_portable::window::{self, CellsBack, FreshSquare, Squares, EYE_WINDOWS, FIRST_CELL, HANDED, LEVELS, SLOTS, SQUARE, SQUARES, SQUARE_CELLS, SQUARE_SHIFT, WINDOWS};
use pxl_portable::wrap_index;
use pxl_portable::LEAF_CELL;
use pxl_spec::bake::tile::terrain::{handed, square_lines, GroundId, TerrainTile, ELE_LANE, LOOK_LANE};
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::layer::lattice::{GROUND, TILE_CELLS};
use pxl_spec::space::layer::tile::{tile_at, Tile, LEAF_ZOOM};
use xpute_guest::global::Global;
use xpute_guest::mem::base::off_of_slice;
use xpute_kit::collection::list::List;

use crate::io::source::Declared;
use crate::layer::kept::Offered;
use crate::layer::terrain::fixed::{cells_off, fill_off, joined_off, CELLS_BYTES, CELLS_LANE_BYTES, RANGE};
use crate::layer::Base;
use crate::mem::fixed::lane;

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
const _: () = assert!(window::WINDOW <= TILE_CELLS + 1, "a window spans two tiles a side");

/// Who window `w` is about, and its cells' zoom as laid out; a
/// destination's is its line's.
const fn layout(w: u32) -> (Centre, Zoom) {
    match w {
        w if w < EYE_WINDOWS => (Centre::Eye, window::eye_cell(w)),
        w if w < EYE_WINDOWS + 2 => (Centre::Focus, LEAF_CELL - 1 + (w - EYE_WINDOWS)),
        _ => (Centre::Destination, LEAF_CELL),
    }
}

/// The windows whose cells may be at `cell`, from the layout: the eye's,
/// the focus's, and a destination's, whose zoom its line moves.
fn windows_at(cell: Zoom) -> [Option<u32>; 3] {
    let focus = LEAF_CELL - 1;
    [
        (FIRST_CELL..=LEAF_CELL).contains(&cell).then(|| cell - FIRST_CELL),
        (focus..=LEAF_CELL).contains(&cell).then(|| EYE_WINDOWS + cell - focus),
        Some(WINDOWS - 1),
    ]
}

/// What a slot holds, a row of the table the GPU's cast reads
/// (gpu-query-shader `World::present`): the square its window wants there,
/// whether its cells are present, and the fill they are on their way from
/// (0 where none is). Neither: wanted, its tile not yet handed it.
#[repr(C)]
#[derive(Clone, Copy)]
struct Slot {
    square: [i32; 2],
    present: i32,
    ticket: u32,
}

impl Slot {
    const fn absent(square: [i32; 2]) -> Slot {
        Slot { square, present: 0, ticket: 0 }
    }

    fn is_absent(&self) -> bool {
        self.present == 0 && self.ticket == 0
    }
}

const _: () = assert!(size_of::<Slot>() == 16, "a slot is a row of four words");

/// A window: its cells' zoom. Its first square is its row past the slots
/// (`first_of`).
#[derive(Clone, Copy, PartialEq, Eq)]
struct Window {
    cell: Zoom,
}

struct Windows {
    windows: [Window; WINDOWS as usize],
    /// Every window's slots, then each window's first square, a row shaped
    /// as a slot's, its square the first and present while it is laid out,
    /// none while its centre is not (pxl-portable `window::ORIGIN_AT`): the
    /// GPU's table as it is (`table`).
    slots: [Slot; (SQUARES + WINDOWS) as usize],
    /// The device the slots' squares were handed to (`device_generation`):
    /// one that went took them all.
    on: u32,
    ticket: u32,
    /// What the slots were last stepped over: the residency's revision
    /// (`Residency::revision`), and whether a fill handed a square or let a
    /// packet go since, which moves what is due and what is asked again.
    read: u32,
    filled: bool,
    /// The tiles the windows apart from the eye's want (`pinned`).
    pinned: List<Tile, { PINNED_TILES as usize }>,
    /// The squares this turn hands the fill (`due`).
    due: List<u32, { HANDED as usize }>,
}

static WINDOWS_STATE: Global<Windows> = Global::new();

fn windows() -> &'static mut Windows {
    WINDOWS_STATE.get(fresh)
}

/// Window `win`'s first square, none while its centre is not.
fn first_of(w: &Windows, win: u32) -> Option<[i32; 2]> {
    let row = w.slots[(SQUARES + win) as usize];
    (row.present != 0).then_some(row.square)
}

/// The windows before any centre stands: every slot absent.
fn fresh() -> Windows {
    Windows {
        windows: core::array::from_fn(|w| Window { cell: layout(w as u32).1 }),
        slots: [Slot::absent([i32::MIN; 2]); (SQUARES + WINDOWS) as usize],
        on: 0,
        ticket: 0,
        read: 0,
        filled: true,
        pinned: List::new(),
        due: List::new(),
    }
}

/// The cell at `cell` a point `p`, in units, lies in, X wrapped.
fn cell_at(cell: Zoom, p: [f64; 2]) -> [i32; 2] {
    let side = units(cell);
    [wrap_index(cell, (p[0] / side).floor() as i32), (p[1] / side).floor() as i32]
}

/// The square window `first` holds in slot `s`.
fn wanted(cell: Zoom, first: [i32; 2], s: u32) -> [i32; 2] {
    let m = (window::ACROSS - 1) as i32;
    let at = |o: i32, k: i32| o + ((k - (o & m)) & m);
    let q = [at(first[0], (s % window::ACROSS) as i32), at(first[1], (s / window::ACROSS) as i32)];
    [wrap_index(cell - SQUARE_SHIFT, q[0]), q[1]]
}

/// The tile square `q` at `cell` lies in, and where in it, in squares.
fn tile_of(cell: Zoom, q: [i32; 2]) -> (Tile, [u32; 2]) {
    let per = (GROUND.tile_cells() >> SQUARE_SHIFT) as i32;
    let zoom = cell - GROUND.tile_cell_shamt();
    let t = tile_at(zoom, [q[0].div_euclid(per), q[1].div_euclid(per)]);
    (t, [q[0].rem_euclid(per) as u32, q[1].rem_euclid(per) as u32])
}

/// Every square's planes on the CPU, in `.fixed.terrain` (fixed.rs): its
/// cells as they come back and its own looks; a test's own.
pub(crate) struct Planes {
    pub cells: &'static mut [u8],
    /// The eye's windows' levels over their squares, as the GPU folded them
    /// and the renderer reads them back (pxl-portable `window::fold`).
    pub joined: &'static mut [u32],
}

pub(crate) static PLANES: Global<Planes> = Global::new();

fn planes() -> &'static mut Planes {
    PLANES.get(|| {
        let r = RANGE.get();
        // SAFETY: the planes are the windows' alone, plain numbers, zeroed at boot.
        unsafe {
            Planes {
                cells: lane(r, cells_off(0), (SQUARES * CELLS_LANE_BYTES) as usize),
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

fn cells(s: u32) -> &'static [u8] {
    let at = (s * CELLS_LANE_BYTES) as usize;
    &planes().cells[at..at + CELLS_BYTES as usize]
}

/// The turn's windows: each centre where it now stands (`at`, in units, X
/// then Y, by `Centre`; none where it is not), a square whose window no
/// longer holds it given up; the tiles a window now wants but holds no
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
    // Per window, whether its zoom moved, and whether it moved at all.
    let (mut moved, mut changed) = ([false; WINDOWS as usize], [lost; WINDOWS as usize]);
    let mut stale = lost || core::mem::take(&mut w.filled) || terrain.revision() != w.read;
    for win in 0..WINDOWS {
        let (centre, laid) = layout(win);
        let p = at[centre as usize];
        let cell = match (centre, p) {
            // Where its line says the ground: open sea's leaf is the tile over it.
            (Centre::Destination, Some(p)) => {
                let leaf = tile_at(LEAF_ZOOM, cell_at(LEAF_ZOOM, p));
                terrain.source(leaf).map_or(laid, |t| GROUND.cell_zoom(t.zoom))
            }
            _ => laid,
        };
        let first = p.map(|p| window::origin(cell, cell_at(cell, p)));
        let was = (core::mem::replace(&mut w.windows[win as usize], Window { cell }).cell, first_of(w, win));
        w.slots[(SQUARES + win) as usize] = match first {
            Some(f) => Slot { square: f, present: 1, ticket: 0 },
            None => Slot::absent([i32::MIN; 2]),
        };
        moved[win as usize] = was.0 != cell;
        changed[win as usize] |= was != (cell, first);
        stale |= was != (cell, first);
    }
    if !stale {
        return false;
    }
    for win in 0..WINDOWS {
        let (cell, first) = (w.windows[win as usize].cell, first_of(w, win));
        for s in 0..SLOTS {
            let slot = &mut w.slots[(win * SLOTS + s) as usize];
            let Some(f) = first else {
                *slot = Slot::absent(slot.square);
                continue;
            };
            // A window that did not move wants its squares where they are:
            // only an absent one's tile is asked of again.
            if changed[win as usize] {
                let want = wanted(cell, f, s);
                if moved[win as usize] || lost || slot.square != want {
                    *slot = Slot::absent(want);
                    terrain.moved_at(tile_of(cell, want).0);
                }
            }
            if slot.is_absent() {
                let t = tile_of(cell, slot.square).0;
                if !crate::layer::terrain::tile::kept_for(t) {
                    terrain.again(t);
                }
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

/// Square `s`'s cells back from its fill `ticket` (pxl-render
/// `read_back`): present where its slot still waits on that fill, and what
/// is read over its tile moved.
pub fn landed(s: u32, ticket: u32) {
    let w = windows();
    let cell = w.windows[(s / SLOTS) as usize].cell;
    let slot = &mut w.slots[s as usize];
    if slot.ticket == ticket {
        (slot.present, slot.ticket) = (1, 0);
        TerrainTile::residency().moved_at(tile_of(cell, slot.square).0);
    }
}

/// The squares this turn hands, nearest first, as many as a frame's fill
/// takes: the eye's windows coarsest first, since a reader of a square
/// absent reads the coarser window's (pxl-portable `window::find`), then the
/// focus's and a destination's; in a window by how far its square lies from
/// the centre. Only a square whose tile can hand it now: one waiting on its
/// tile does not hold a place another could take.
fn due(w: &mut Windows) {
    let terrain = TerrainTile::residency();
    let handable = |t: Tile| crate::layer::terrain::tile::kept_for(t) || terrain.get(t).is_some() && terrain.frame(t).is_none();
    w.due.clear();
    for win in 0..WINDOWS {
        let (cell, Some(first)) = (w.windows[win as usize].cell, first_of(w, win)) else { continue };
        let mut near = List::<(i64, u32), { SLOTS as usize }>::new();
        for i in win * SLOTS..(win + 1) * SLOTS {
            let slot = w.slots[i as usize];
            if slot.is_absent() && handable(tile_of(cell, slot.square).0) {
                near.push((far(cell, first, slot.square), i));
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
    for &i in w.due.iter() {
        let t = tile_of(w.windows[(i / SLOTS) as usize].cell, w.slots[i as usize].square).0;
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
/// (pxl-kernel view/demand.rs `observe`).
pub fn pinned() -> &'static [Tile] {
    &windows().pinned
}

/// The windows' slots and their first squares, as the GPU's cast reads
/// them (gpu-query-shader `World::present`, `World::origin`): a row a slot
/// (`Slot`), then a row a window (pxl-portable `window::ORIGIN_AT`).
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

/// Each square the turn hands (`due`) in `tile`, filled from `from` while
/// `write` takes them: its own looks here, its four looks and the tile's
/// samples to the GPU. Taken where no window wants another of its squares;
/// kept where one still does, for a later turn; full where the stream ran
/// out first.
pub fn fill(tile: Tile, from: Handed, write: &mut dyn FnMut(FreshSquare<'_>) -> bool) -> Offered {
    let cell = GROUND.cell_zoom(tile.zoom);
    let w = windows();
    let wants = |w: &Windows, i: u32| {
        let (slot, win) = (w.slots[i as usize], w.windows[(i / SLOTS) as usize]);
        slot.is_absent() && win.cell == cell && first_of(w, i / SLOTS).is_some() && tile_of(cell, slot.square).0 == tile
    };
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
        let i = w.due[k];
        if !wants(w, i) {
            continue;
        }
        let place = tile_of(cell, w.slots[i as usize].square).1;
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
        w.ticket = w.ticket % u32::MAX + 1;
        let back = CellsBack {
            cells: off_of_slice(cells(i)) as u32,
            ticket: w.ticket,
        };
        let key = w.slots[i as usize].square;
        let shard = Shard::UNKNOWN.at(&Declared, (window::squares_zoom(cell), key));
        let taken = write(FreshSquare {
            fresh: window::fresh(i, window::place_word(place), cell, [key[0] << SQUARE_SHIFT, key[1] << SQUARE_SHIFT], shard),
            looks: cut,
            samples: ele,
            lines: &lines[..written],
            back,
        });
        if !taken {
            return Offered::Full;
        }
        w.slots[i as usize].ticket = back.ticket;
        w.filled = true;
    }
    rest(w)
}

/// The windows as the CPU holds them, for pxl-portable's one reading of a
/// cell (`window::find`, `window::bound_over`, `window::ran`): each square's
/// columns, levels and runs as the GPU drew and folded them, read back; and
/// the service area they end at (pxl-portable `area::Area`).
#[derive(Clone, Copy)]
pub struct Windowed;

impl Area for Windowed {
    fn ends(&self) -> bool {
        Declared.ends()
    }
    fn entries(&self) -> u32 {
        Declared.entries()
    }
    fn entry(&self, i: u32) -> u32 {
        Declared.entry(i)
    }
}

/// Word `k` of square `s`'s cells as they came back: its columns, its
/// levels, then its runs (`window::BACK_WORDS`).
fn back_at(s: u32, k: usize) -> u32 {
    let c = cells(s);
    u32::from_le_bytes([c[4 * k], c[4 * k + 1], c[4 * k + 2], c[4 * k + 3]])
}

impl Squares for Windowed {
    /// Present as the GPU holds it: its cells back, in a window at `cell`.
    fn present(&self, cell: Zoom, q: [i32; 2]) -> (u32, bool) {
        let w = windows();
        let held = windows_at(cell).into_iter().flatten().map(|win| win * SLOTS + window::slot(q)).find(|&i| {
            let slot = w.slots[i as usize];
            w.windows[(i / SLOTS) as usize].cell == cell && slot.present != 0 && slot.square == q
        });
        (held.unwrap_or(0), held.is_some())
    }

    fn square(&self, cell: Zoom, slot: u32) -> ([i32; 2], bool) {
        let s = windows().slots[((cell - FIRST_CELL) * SLOTS + slot) as usize];
        (s.square, s.present != 0)
    }

    /// As the GPU folded it, read back a frame or more later: an entry for
    /// a block no longer held names another, and reads none (pxl-portable
    /// `window::unfold`).
    fn joined(&self, cell: Zoom, e: u32) -> [u32; window::JOINED_WORDS as usize] {
        let at = (((cell - FIRST_CELL) * window::JOINED_ENTRIES + e) * window::JOINED_WORDS) as usize;
        core::array::from_fn(|k| planes().joined[at + k])
    }

    fn origin(&self, cell: Zoom) -> ([i32; 2], bool) {
        let first = first_of(windows(), cell - FIRST_CELL);
        (first.unwrap_or([0; 2]), first.is_some())
    }

    fn laid(&self, _: Zoom, s: u32, i: u32) -> u32 {
        window::laid_at(back_at(s, (window::LAID_AT - window::COLUMNS_AT + i / 2) as usize), i)
    }

    fn column(&self, _: Zoom, s: u32, i: u32) -> u32 {
        back_at(s, i as usize)
    }

    fn level(&self, _: Zoom, s: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
        let e = (window::LEVELS_AT - window::COLUMNS_AT + window::LEVEL_ENTRY_WORDS * window::level_index(j, at)) as usize;
        [back_at(s, e) as i32, back_at(s, e + 1) as i32]
    }
}

impl window::Ran for Windowed {
    fn ran(&self, _: Zoom, s: u32, r: u32) -> u32 {
        back_at(s, (window::RUNS_AT - window::COLUMNS_AT + r) as usize)
    }
}

const _: () = assert!(LEVELS <= SQUARE_SHIFT);

#[cfg(test)]
#[path = "window.test.rs"]
pub(crate) mod test;
