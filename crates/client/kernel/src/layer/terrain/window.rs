// pxl-kernel/layer/terrain/window.rs

//! The ground's cells as the kernel holds them: a window a zoom about each
//! centre a reader declares — the eye, the focus where walking and editing
//! happen, a destination being gone to — each as deep as its reader reads
//! (pxl-portable `window`). A square is filled from its tile as it lands:
//! its own looks here, its four looks and its tile's samples handed to the
//! GPU, which draws its cells; they come back into `.fixed.terrain`, and the
//! square is present from then on. Nothing of a tile is kept for a square
//! still to come: one that enters asks its tile again (`Residency::again`),
//! which the browser's cache answers.

use pxl_portable::ground::{own, word};
use pxl_portable::window::{self, Squares, LEVELS, SLOTS, SQUARE, SQUARE_CELLS, SQUARE_SHIFT};
use pxl_portable::wrap_index;
use pxl_render::gpu::api_spec::read;
use pxl_render::gpu::renderer::{CellsBack, Square, TerrainWrite};
use pxl_spec::bake::tile::terrain::{handed, square_lines, GroundId, TerrainTile, LOOK_LANE};
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::layer::lattice::{GROUND, TILE_CELLS};
use pxl_spec::space::layer::tile::{tile_at, Tile, LEAF_ZOOM};
use xpute_guest::global::Global;
use xpute_guest::mem::base::{off_of, off_of_slice};

use crate::layer::terrain::fixed::{back_off, cells_off, looks_off, CELLS_BYTES, CELLS_LANE_BYTES, LOOK_LANE_BYTES, RANGE};
use crate::layer::Base;
use crate::mem::fixed::lane;
use pxl_spec::block::sample::Cells;

/// The eye's coarsest window's cells' zoom: the top's.
pub const FIRST_CELL: Zoom = GROUND.cell_zoom(GROUND.top());
const LEAF_CELL: Zoom = GROUND.cell_zoom(LEAF_ZOOM);

/// Who a window is about.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Centre {
    Eye,
    Focus,
    Destination,
}

/// The eye's windows, every zoom the pyramid draws, coarsest first, which
/// the GPU's cast reads.
pub const EYE_WINDOWS: u32 = LEAF_CELL - FIRST_CELL + 1;
/// Every window: the eye's; the focus's, the leaves' and one over, what
/// walking and editing read; a destination's one, at the zoom its line says
/// its ground (`step`), found exactly.
pub const WINDOWS: u32 = EYE_WINDOWS + 3;
/// Squares every window holds, the eye's first.
pub const SQUARES: u32 = WINDOWS * SLOTS;
/// The tiles the windows apart from the eye's want at most: a window spans
/// two tiles a side.
pub const PINNED_TILES: u32 = (WINDOWS - EYE_WINDOWS) * 4;
const _: () = assert!(window::WINDOW <= TILE_CELLS + 1, "a window spans two tiles a side");

/// Who window `w` is about, and its cells' zoom as laid out; a
/// destination's is its line's.
const fn layout(w: u32) -> (Centre, Zoom) {
    match w {
        w if w < EYE_WINDOWS => (Centre::Eye, FIRST_CELL + w),
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    /// Wanted, its tile not yet handed it.
    Absent,
    /// Handed to the GPU, its cells on their way.
    Sent,
    Present,
}

/// What a slot holds: the square its window wants there, and how far it is.
#[derive(Clone, Copy)]
struct Slot {
    square: [i32; 2],
    state: State,
}

/// A window: its cells' zoom, and its first square; none while its centre
/// is not.
#[derive(Clone, Copy)]
struct Window {
    cell: Zoom,
    first: Option<[i32; 2]>,
}

struct Windows {
    windows: [Window; WINDOWS as usize],
    slots: [Slot; SQUARES as usize],
    ticket: u32,
    /// The slot table the GPU's cast reads: per eye window, a row a slot.
    table: Vec<i32>,
    /// The tiles the windows apart from the eye's want (`pinned`).
    pinned: Vec<Tile>,
}

static WINDOWS_STATE: Global<Windows> = Global::new();

fn windows() -> &'static mut Windows {
    WINDOWS_STATE.get(|| Windows {
        windows: core::array::from_fn(|w| Window {
            cell: layout(w as u32).1,
            first: None,
        }),
        slots: [Slot {
            square: [i32::MIN; 2],
            state: State::Absent,
        }; SQUARES as usize],
        ticket: 0,
        table: Vec::new(),
        pinned: Vec::new(),
    })
}

/// The cell at `cell` a point `p`, in units, lies in, X wrapped.
fn cell_at(cell: Zoom, p: [f64; 2]) -> [i32; 2] {
    let side = units(cell);
    [wrap_index(cell, (p[0] / side).floor() as i64 as i32), (p[1] / side).floor() as i32]
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
    let t = tile_at(zoom, [i64::from(q[0].div_euclid(per)), i64::from(q[1].div_euclid(per))]);
    (t, [q[0].rem_euclid(per) as u32, q[1].rem_euclid(per) as u32])
}

/// Every square's planes on the CPU, in `.fixed.terrain` (fixed.rs): its
/// cells as they come back, its own looks, and the word they come back
/// under; a test's own.
pub(crate) struct Planes {
    pub cells: &'static mut [u8],
    pub looks: &'static mut [GroundId],
    pub back: &'static mut [u32],
}

pub(crate) static PLANES: Global<Planes> = Global::new();

fn planes() -> &'static mut Planes {
    PLANES.get(|| {
        let r = RANGE.get();
        // SAFETY: the planes are the windows' alone, plain numbers, zeroed at boot.
        unsafe {
            Planes {
                cells: lane(r, cells_off(0), (SQUARES * CELLS_LANE_BYTES) as usize),
                looks: lane(r, looks_off(0), (SQUARES * LOOK_LANE_BYTES) as usize),
                back: lane(r, back_off(0), SQUARES as usize),
            }
        }
    })
}

/// The word square `s`'s cells come back under.
fn back_word(s: u32) -> &'static mut u32 {
    &mut planes().back[s as usize]
}

fn cells(s: u32) -> &'static [u8] {
    let at = (s * CELLS_LANE_BYTES) as usize;
    &planes().cells[at..at + CELLS_BYTES as usize]
}

fn looks(s: u32) -> &'static mut [GroundId] {
    let at = (s * LOOK_LANE_BYTES) as usize;
    &mut planes().looks[at..at + LOOK_LANE_BYTES as usize]
}

/// The turn's windows: each centre where it now stands (`at`, in units, X
/// then Y, by `Centre`; none where it is not), a square whose window no
/// longer holds it given up, and a square whose cells came back present;
/// the tiles a window now wants but holds no packet for are asked again.
/// Before the turn's arrivals and its stage. True where the tiles the
/// windows apart from the eye's want changed.
pub fn step(at: [Option<[f64; 2]>; 3]) -> bool {
    let w = windows();
    let terrain = TerrainTile::residency();
    for win in 0..WINDOWS {
        let (centre, laid) = layout(win);
        let p = at[centre as usize];
        let cell = match (centre, p) {
            // Where its line says the ground: open sea's leaf is the tile over it.
            (Centre::Destination, Some(p)) => {
                let leaf = tile_at(LEAF_ZOOM, cell_at(LEAF_ZOOM, p).map(i64::from));
                terrain.source(leaf).map_or(laid, |t| GROUND.cell_zoom(t.zoom))
            }
            _ => laid,
        };
        let moved = core::mem::replace(
            &mut w.windows[win as usize],
            Window {
                cell,
                first: p.map(|p| window::origin(cell, cell_at(cell, p))),
            },
        )
        .cell
            != cell;
        let first = w.windows[win as usize].first;
        for s in 0..SLOTS {
            let i = (win * SLOTS + s) as usize;
            let Some(f) = first else {
                w.slots[i].state = State::Absent;
                continue;
            };
            let want = wanted(cell, f, s);
            let t = tile_of(cell, want).0;
            let slot = &mut w.slots[i];
            if moved || slot.square != want {
                *slot = Slot { square: want, state: State::Absent };
                *back_word(i as u32) = 0;
                terrain.moved_at(t);
            }
            if slot.state == State::Sent && *back_word(i as u32) & read::DONE != 0 {
                slot.state = State::Present;
                terrain.moved_at(t);
            }
            if slot.state == State::Absent && !crate::layer::terrain::tile::kept_for(t) {
                terrain.again(t);
            }
        }
    }
    let was = core::mem::take(&mut w.pinned);
    for win in EYE_WINDOWS..WINDOWS {
        let Window { cell, first } = w.windows[win as usize];
        if first.is_none() {
            continue;
        }
        for s in 0..SLOTS {
            let t = tile_of(cell, w.slots[(win * SLOTS + s) as usize].square).0;
            if !w.pinned.contains(&t) {
                w.pinned.push(t);
            }
        }
    }
    was != w.pinned
}

/// The tiles the focus's and a destination's windows want, which the eye's
/// demand need not name: held hard, as a destination's line is
/// (pxl-kernel view/demand.rs `observe`).
pub fn pinned() -> &'static [Tile] {
    &windows().pinned
}

/// The eye's windows' slot table, as the GPU's cast reads it
/// (gpu-query-shader `World::present`): a row a slot, its square and
/// whether present.
pub fn table() -> &'static [i32] {
    let w = windows();
    w.table.clear();
    for i in 0..EYE_WINDOWS * SLOTS {
        let slot = w.slots[i as usize];
        w.table.extend_from_slice(&[slot.square[0], slot.square[1], i32::from(slot.state == State::Present), 0]);
    }
    &w.table
}

/// What a tile hands its squares: its packet and the tile as it is held, its
/// lines and its ways' bound made where it landed; or a void tile's own
/// looks over samples all zero.
pub enum Handed<'a> {
    Packet(&'a [u8], &'a TerrainTile),
    Void,
}

/// A square's four looks a cell, cut from its tile's, the tile's own
/// expanded where it holds one a cell, and its lines and the room they are
/// laid in, kept so a fill takes nothing more from the heap.
struct Fill {
    room: Vec<GroundId>,
    cut: Vec<GroundId>,
    zeros: Vec<i16>,
    lines: Vec<u32>,
    scratch: Vec<(u32, u32)>,
}

static FILL: Global<Fill> = Global::new();

/// Each square any window wants in `tile` and holds absent, filled from
/// `from` while `write` takes them: its own looks here, its four looks and
/// the tile's samples to the GPU. False where the stream ran out first, so
/// the tile hands the rest a turn later.
pub fn fill(tile: Tile, from: Handed, write: &mut TerrainWrite<'_>) -> bool {
    let cell = GROUND.cell_zoom(tile.zoom);
    let w = windows();
    let wants = |w: &Windows, i: u32| {
        let (slot, win) = (w.slots[i as usize], w.windows[(i / SLOTS) as usize]);
        slot.state == State::Absent && win.cell == cell && win.first.is_some() && tile_of(cell, slot.square).0 == tile
    };
    if !(0..SQUARES).any(|i| wants(w, i)) {
        return true;
    }
    let f = FILL.get(|| Fill {
        room: vec![0; LOOK_LANE],
        cut: vec![0; 4 * SQUARE_CELLS as usize],
        zeros: vec![0; pxl_spec::bake::tile::terrain::ELE_LANE],
        lines: vec![0; window::LINES_WORDS as usize],
        scratch: Vec::new(),
    });
    let (room, cut, zeros, lines, scratch) = (&mut f.room, &mut f.cut, &f.zeros, &mut f.lines, &mut f.scratch);
    let void = TerrainTile::void();
    let held: &TerrainTile = match from {
        Handed::Packet(_, held) => held,
        Handed::Void => &void,
    };
    let (lane, ele): (&[GroundId], &[i16]) = match from {
        Handed::Packet(pkt, _) => match handed(pkt, room) {
            Ok(h) => h,
            Err(_) => return true,
        },
        Handed::Void => {
            for (q, &l) in room.chunks_mut(4).zip(void.ground.cells.iter()) {
                q.fill(l);
            }
            (&room[..], &zeros[..])
        }
    };
    let mut ways = [0u32; window::WAYS_BOUND as usize];
    for i in 0..SQUARES {
        if !wants(w, i) {
            continue;
        }
        let place = tile_of(cell, w.slots[i as usize].square).1;
        // The square's rows out of the tile's, four looks a cell.
        let side = GROUND.tile_cells() as usize;
        let (x0, y0) = ((place[0] * SQUARE) as usize, (place[1] * SQUARE) as usize);
        for y in 0..SQUARE as usize {
            let from_row = 4 * ((y0 + y) * side + x0);
            cut[4 * y * SQUARE as usize..4 * (y + 1) * SQUARE as usize].copy_from_slice(&lane[from_row..from_row + 4 * SQUARE as usize]);
        }
        window::square_ways(|k| held.ways[k as usize].map(i32::from), place, &mut ways);
        let written = square_lines(tile, &held.separation, place, scratch, lines);
        w.ticket = (w.ticket % (read::DONE - 1)) + 1;
        let back = CellsBack {
            cells: off_of_slice(cells(i)) as u32,
            word: off_of(back_word(i)) as u32,
            ticket: w.ticket,
        };
        let taken = write(Square {
            square: i,
            place: place[0] | place[1] << 16,
            looks: cut,
            ways: &ways,
            samples: pxl_render::gpu::api::bytes(ele),
            lines: &lines[..written],
            back,
        });
        if !taken {
            return false;
        }
        for (l, q) in looks(i).iter_mut().zip(cut.as_chunks::<4>().0) {
            *l = own(u32::from_le_bytes(*q), cell) as GroundId;
        }
        *back_word(i) = back.ticket;
        w.slots[i as usize].state = State::Sent;
    }
    true
}

/// The windows as the CPU holds them, for pxl-portable's one reading of a
/// cell (`window::find`, `window::bounded`): each square's cells and levels
/// as the GPU drew and joined them, and its ways' bound, read back.
#[derive(Clone, Copy)]
pub struct Windowed;

impl Cells for Windowed {
    fn cell(&self, cell: Zoom, g: [i32; 2]) -> (u32, i32) {
        match window::find(self, cell, g, FIRST_CELL) {
            (from, slot, i, true) => (self.look(from, slot, i), self.height(from, slot, i)),
            _ => (word([u32::from(pxl_spec::block::cube::sentinel::void); 4]), 0),
        }
    }
}

/// Word `k` of square `s`'s cells as they came back: its heights two a
/// word, then its levels, then its ways' bound (`window::BACK_WORDS`).
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
            w.windows[(i / SLOTS) as usize].cell == cell && slot.state == State::Present && slot.square == q
        });
        (held.unwrap_or(0), held.is_some())
    }

    /// The CPU holds a cell's own look alone: its four are it four times.
    fn look(&self, _: Zoom, s: u32, i: u32) -> u32 {
        word([u32::from(looks(s)[i as usize]); 4])
    }

    fn height(&self, _: Zoom, s: u32, i: u32) -> i32 {
        (back_at(s, i as usize / 2) >> (16 * (i % 2)) << 16) as i32 >> 16
    }

    fn way(&self, _: Zoom, s: u32, k: u32) -> u32 {
        back_at(s, (window::HEIGHT_WORDS + window::LEVEL_WORDS + k) as usize)
    }

    fn level(&self, _: Zoom, s: u32, j: u32, at: [u32; 2]) -> i32 {
        back_at(s, (window::HEIGHT_WORDS + window::level_at(j) + at[1] * (SQUARE >> j) + at[0]) as usize) as i32
    }
}

const _: () = assert!(LEVELS <= SQUARE_SHIFT);

#[cfg(test)]
#[path = "window.test.rs"]
pub(crate) mod test;
