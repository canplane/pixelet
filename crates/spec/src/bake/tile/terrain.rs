// pxl-spec/bake/tile/terrain.rs

//! A baked terrain tile and its wire: `[samples, ground, occupancy, separation]`.

use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

use std::borrow::Cow;

use super::{occupancy_of, Occupancy};
use crate::bake::constant::ELE_PADDED_SPAN;
use crate::bake::malformed;
use crate::bake::separation::{self, Separation};
use crate::procedural::Frame;
use crate::space::coord::adapter::epsg3857::u_per_m;
use crate::space::coord::zoom::Zoom;
use crate::space::layer::lattice::TILE_CELLS;
use crate::space::layer::tile::Tile;

/// A tile's lane lengths: a cell each, four looks a cell, and its padded
/// samples; a tile is these and no other size.
pub const GROUND_LANE: usize = (TILE_CELLS * TILE_CELLS) as usize;
pub const LOOK_LANE: usize = 4 * GROUND_LANE;
pub const ELE_LANE: usize = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
/// Bins a side a tile's ways are cut into, each `1 << WAYS_SHIFT` cells a
/// side, as a window's square lays its lines by them (`square_lines`).
pub use pxl_portable::window::{WAYS_ACROSS, WAYS_SHIFT};

/// The most entries a window's square holds of the ways (pxl-portable
/// `window::WAYS_SQUARE_SEGMENTS`); a tile past it fails its bake
/// (`ways_fit`).
pub const WAYS_SQUARE_SEGMENTS: usize = pxl_portable::window::WAYS_SQUARE_SEGMENTS as usize;

/// What a ground cell holds: the id of a cube of the ground's palette, a
/// Carto look, which fits a byte (`block/cube.json`).
pub type GroundId = u8;

/// A tile's ground: in the bake the four looks a cell folds, `dx + 2·dy`, as
/// a GPU holds them, a leaf's its own four; on the wire one a cell where
/// every cell's four are one; on the CPU each cell's own look (`decode_into`).
#[derive(Clone, Debug, PartialEq)]
pub struct Ground {
    pub cells: Cow<'static, [GroundId]>,
}

impl Ground {
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

/// A tile: one lane a fact the client cannot make. What it draws by rule —
/// a slab, a bore, a pier — is not here. The bake holds what goes on the
/// wire; a client what it reads of a tile as a whole, its lines. Its cells
/// a client holds in its windows, drawn by a GPU off the samples, and its
/// ways there off the lines (pxl-portable `window`).
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainTile {
    pub ground: Ground,           // a class a cell: the source rasterized, or its fold; a client's void tile's alone
    pub separation: Separation,   // ways off the ground: source lines, tops solved offline
    pub ele: Cow<'static, [i16]>, // the bake's: padded samples, a leaf's source or the fold's; a client hands them to a GPU off the packet
    pub occupancy: Occupancy,     // which children are tiles: none is asked for that is not
}

/// Whether every square of `square` cells a side a tile is cut into holds
/// its ways' entries within `WAYS_SQUARE_SEGMENTS`; the first that does not,
/// its place in squares and its count, where one does not.
pub fn ways_fit(separation: &Separation, square: u32) -> Result<(), ([u32; 2], usize)> {
    let mut entries = [0usize; (WAYS_ACROSS * WAYS_ACROSS) as usize];
    separation.reaches(f64::from(TILE_CELLS), WAYS_ACROSS, |k, _, _, _| entries[k] += 1);
    let (bins, squares) = (square >> WAYS_SHIFT, TILE_CELLS / square);
    for qy in 0..squares {
        for qx in 0..squares {
            let n: usize = (0..bins * bins).map(|b| entries[((qy * bins + b / bins) * WAYS_ACROSS + qx * bins + b % bins) as usize]).sum();
            if n > WAYS_SQUARE_SEGMENTS {
                return Err(([qx, qy], n));
            }
        }
    }
    Ok(())
}

/// The lines of the window's square at `place` (in squares) of `tile`, as
/// the GPU asks them (pxl-portable window.rs `LINES_*`), into `out`, a
/// square's `LINES_WORDS`: the tile's header, each bin's entries, and the
/// segments a bin reaches, each once, in their lines' order — the reach
/// `ways_fit` counts, so none holds more than `WAYS_SQUARE_SEGMENTS`. The
/// words written, the rest left as they were; `scratch` the room it works
/// in, `WAYS_SQUARE_SEGMENTS` pairs, kept between squares.
pub fn square_lines(tile: Tile, separation: &Separation, place: [u32; 2], scratch: &mut [(u32, u32)], out: &mut [u32]) -> usize {
    use pxl_portable::ways::{header, HEADER_WORDS, SEGMENT_WORDS};
    use pxl_portable::window::{LINES_ENTRIES_AT, LINES_SEGMENTS_AT, LINES_WORDS, SQUARE, WAYS_BINS};
    xpute_kit::ensure!(out.len() == LINES_WORDS as usize, ENOTRECOVERABLE);
    xpute_kit::ensure!(scratch.len() == WAYS_SQUARE_SEGMENTS, ENOTRECOVERABLE);
    let bins = SQUARE >> WAYS_SHIFT;
    // Each bin of the square a segment's footprint reaches, and the point it starts at.
    let mut count = 0;
    separation.reaches(f64::from(TILE_CELLS), WAYS_ACROSS, |k, d, _, s| {
        let [x, y] = [k as u32 % WAYS_ACROSS, k as u32 / WAYS_ACROSS];
        if [x / bins, y / bins] == place {
            if let Some(e) = scratch.get_mut(count) {
                *e = ((y % bins) * bins + x % bins, separation.starts[d] + s as u32);
            }
            count += 1;
        }
    });
    xpute_kit::ensure!(count <= WAYS_SQUARE_SEGMENTS, ENOTRECOVERABLE, count);
    let scratch = &mut scratch[..count];
    let frame = Frame::of(tile);
    out[..HEADER_WORDS as usize].copy_from_slice(&header(u_per_m(frame.corner[1]) as f32, &frame.stations()));
    // The segments, each once in their points' order, each entry then its
    // segment's place among them.
    scratch.sort_unstable_by_key(|&(b, p)| (p, b));
    let (mut n, mut last) = (0, None);
    for e in scratch.iter_mut() {
        let p = e.1;
        if last != Some(p) {
            let at = LINES_SEGMENTS_AT as usize + n * SEGMENT_WORDS;
            out[at..at + SEGMENT_WORDS].copy_from_slice(&separation.segment_words(p as usize, last == Some(p.wrapping_sub(1))));
            (n, last) = (n + 1, Some(p));
        }
        e.1 = (n - 1) as u32;
    }
    // Each bin's entries in its segments' order, its first and its count.
    scratch.sort_unstable();
    let mut first = 0;
    for b in 0..WAYS_BINS {
        let count = scratch[first..].iter().take_while(|e| e.0 == b).count();
        out[(HEADER_WORDS + b) as usize] = first as u32 | (count as u32) << 16;
        first += count;
    }
    for (k, pair) in scratch.chunks(2).enumerate() {
        let at = |j: usize| pair.get(j).map_or(0, |e| e.1);
        out[LINES_ENTRIES_AT as usize + k] = at(0) | at(1) << 16;
    }
    LINES_SEGMENTS_AT as usize + n * SEGMENT_WORDS
}

/// A window's square as its fill reads it (pxl-portable `window::Square`):
/// its cells' zoom and place in its tile, its lines as `square_lines` lays
/// them, and cell `i`'s four looks as a word (`looks`) and its height
/// (`height`), as its lanes hold them.
pub struct Laid<'a, L, H> {
    pub cell: Zoom,
    pub place: [u32; 2],
    pub lines: &'a [u32],
    pub looks: L,
    pub height: H,
}

impl<L: Fn(u32) -> u32, H: Fn(u32) -> i32> pxl_portable::window::Square for Laid<'_, L, H> {
    fn cell(&self) -> Zoom {
        self.cell
    }
    fn place(&self) -> [u32; 2] {
        self.place
    }
    fn line(&self, k: u32) -> u32 {
        self.lines[k as usize]
    }
    fn looks(&self, i: u32) -> u32 {
        (self.looks)(i)
    }
    fn height(&self, i: u32) -> i32 {
        (self.height)(i)
    }
}

/// Cell `i` of the square at `place` (in squares), in its tile's cells.
fn square_cell(place: [u32; 2], i: u32) -> [u32; 2] {
    use pxl_portable::window::SQUARE;
    [place[0] * SQUARE + i % SQUARE, place[1] * SQUARE + i / SQUARE]
}

/// A square's ways as its fill writes them on a GPU (pxl-portable
/// `ways_cs`), on the CPU: each column's spans off its lines
/// (`ways::column`) read along z as runs (`ways::rises`, `ways::run`), into
/// `out`, a square's runs lane (`window::RUNS_WORDS`), none past
/// `RUNS_SQUARE`; the runs it reads as, and the most spans a column makes.
pub fn fill_runs(sq: &impl pxl_portable::window::Square, out: &mut [u32]) -> [u32; 2] {
    use pxl_portable::window::{RUNS_RUN_AT, RUNS_SQUARE, SQUARE, SQUARE_CELLS};
    let cell = sq.cell();
    let (mut runs, mut most) = (0, 0);
    for i in 0..SQUARE_CELLS {
        out[i as usize] = runs;
        let (spans, fits) = pxl_portable::ways::column(sq, [(i % SQUARE) as i32, (i / SQUARE) as i32]);
        most = most.max(if fits { spans.n } else { u32::MAX });
        let ([mut z, to], any) = pxl_portable::ways::rises(&spans, cell);
        while any && z <= to {
            let (value, len) = pxl_portable::ways::run(&spans, cell, z, to);
            if value != pxl_portable::ways::NOTHING {
                if runs < RUNS_SQUARE {
                    out[(RUNS_RUN_AT + runs) as usize] = pxl_portable::ways::run_word(z, len, value);
                }
                runs += 1;
            }
            z += len as i32;
        }
    }
    out[SQUARE_CELLS as usize] = runs;
    [runs, most]
}

/// What the window's square at `place` (in squares) of `tile` reads as at
/// fill (`fill_runs`), off its lines laid into `lines`, a square's
/// `LINES_WORDS`, with `scratch` (`square_lines`), its looks four a cell as
/// a client hands them (`handed`) and its heights off the tile's samples as
/// `heights_cs` draws them; its runs into `out`.
pub fn square_runs(tile: Tile, held: &TerrainTile, place: [u32; 2], scratch: &mut [(u32, u32)], lines: &mut [u32], out: &mut [u32]) -> [u32; 2] {
    let n = square_lines(tile, &held.separation, place, scratch, lines);
    let cells = &held.ground.cells;
    let looks = |i: u32| {
        let at = square_cell(place, i);
        let c = (at[1] * TILE_CELLS + at[0]) as usize;
        pxl_portable::ground::word(four_looks(cells, c).map(u32::from))
    };
    let height = |i: u32| pxl_portable::ground::height(|j| i32::from(held.ele[j]), square_cell(place, i));
    let cell = crate::space::layer::lattice::GROUND.cell_zoom(tile.zoom);
    fill_runs(
        &Laid {
            cell,
            place,
            lines: &lines[..n],
            looks,
            height,
        },
        out,
    )
}

/// Whether each square of `tile`'s windows holds its ways' runs
/// (`square_runs`): none more than `window::RUNS_SQUARE` runs, no column
/// more than `ways::SPANS` spans; the first that does not, and what it
/// reads as.
pub fn runs_fit(tile: Tile, held: &TerrainTile) -> Result<(), ([u32; 2], [u32; 2])> {
    use pxl_portable::window::{LINES_WORDS, RUNS_SQUARE, RUNS_WORDS, SQUARE};
    if held.separation.is_empty() {
        return Ok(());
    }
    let (mut scratch, mut lines, mut out) = (vec![(0, 0); WAYS_SQUARE_SEGMENTS], vec![0; LINES_WORDS as usize], vec![0; RUNS_WORDS as usize]);
    for qy in 0..TILE_CELLS / SQUARE {
        for qx in 0..TILE_CELLS / SQUARE {
            let [runs, spans] = square_runs(tile, held, [qx, qy], &mut scratch, &mut lines, &mut out);
            if runs > RUNS_SQUARE || spans > pxl_portable::ways::SPANS as u32 {
                return Err(([qx, qy], [runs, spans]));
            }
        }
    }
    Ok(())
}

// Made on first use: a constant array of zeros would ship in the module's data.
static VOID_CELLS: std::sync::LazyLock<Vec<GroundId>> = std::sync::LazyLock::new(|| vec![crate::block::cube::sentinel::void; GROUND_LANE]);

impl TerrainTile {
    /// Where the source has nothing at all, as a client holds it: void
    /// cells at sea level, which give no surface, so nothing is drawn while
    /// the ground is still known and stays asked for, and no child, so none
    /// is asked for under it. Its windows' squares are filled from it as a
    /// packet's are (`ground`, and samples all zero).
    pub fn void() -> TerrainTile {
        TerrainTile {
            ground: Ground {
                cells: Cow::Borrowed(&VOID_CELLS[..]),
            },
            separation: Separation::default(),
            ele: Cow::Borrowed(&[]),
            occupancy: Occupancy::ZERO,
        }
    }

    /// A leaf as the bake makes it: its padded samples, a look a cell, each
    /// its own four, and its separation.
    pub fn leaf(ele: Vec<i16>, ground: Ground, separation: Separation) -> TerrainTile {
        let ground = Ground { cells: expand(&ground.cells).into() };
        TerrainTile::of(ele, ground, separation, Occupancy::LEAF)
    }

    /// A tile as the bake holds it: these lanes, nothing a client looks up.
    pub fn of(ele: Vec<i16>, ground: Ground, separation: Separation, occupancy: Occupancy) -> TerrainTile {
        TerrainTile {
            ground,
            separation,
            ele: ele.into(),
            occupancy,
        }
    }

    /// A tile as a client holds it: its lines, its cells none of its own (a
    /// client's windows hold them).
    pub fn held(separation: Separation, occupancy: Occupancy) -> TerrainTile {
        TerrainTile {
            ground: Ground { cells: Cow::Borrowed(&[]) },
            separation,
            ele: Cow::Borrowed(&[]),
            occupancy,
        }
    }
}

/// Cell `c`'s four looks, however a tile holds them: four a cell
/// (`LOOK_LANE`), one a cell, each its own four, or one for the whole tile.
fn four_looks(cells: &[GroundId], c: usize) -> [GroundId; 4] {
    match cells.len() {
        LOOK_LANE => [cells[4 * c], cells[4 * c + 1], cells[4 * c + 2], cells[4 * c + 3]],
        1 => [cells[0]; 4],
        _ => [cells[c]; 4],
    }
}

/// The tile's four looks a cell, however it holds them (`four_looks`).
fn expand(cells: &[GroundId]) -> Vec<GroundId> {
    (0..GROUND_LANE).flat_map(|c| four_looks(cells, c)).collect()
}

// A tile whose every cell's four looks are one, a leaf's, goes out a look a
// cell, expanded on decode.
pub fn encode(tile: &TerrainTile) -> Result<Vec<u8>, MarshalError> {
    let four = &tile.ground.cells;
    let one: Option<Vec<GroundId>> = four.chunks(4).all(|q| q.iter().all(|&l| l == q[0])).then(|| four.chunks(4).map(|q| q[0]).collect());
    let looks: &[GroundId] = one.as_deref().unwrap_or(four);
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.i16_array(Some(&tile.ele)).u8_array(Some(looks)).u8(Some(tile.occupancy.bits()));
        separation::write_branch(&tile.separation, b);
    }));
    encoder().encode(&t, TreeEncoderOptions::default())
}

struct Lanes<'a> {
    ele: &'a [i16],
    ground: &'a [GroundId],
    occupancy: Occupancy,
    separation: separation::Lanes<'a>,
}

/// Refused unless every lane is a tile's length.
fn read_tile(pkt: &[u8]) -> Result<Lanes<'_>, MarshalError> {
    let root = TreeReader::new(pkt)?.read_branch()?;
    let ele = root.at(0)?.get_array::<i16>()?;
    let ground = root.at(1)?.get_array::<GroundId>()?;
    let occupancy = occupancy_of(root.at(2)?.get_number()?)?;
    let separation = separation::read_branch(root.at_branch(3)?)?;
    if ele.len() != ELE_LANE || (ground.len() != GROUND_LANE && ground.len() != LOOK_LANE) {
        return Err(malformed());
    }
    Ok(Lanes { ele, ground, occupancy, separation })
}

/// What a GPU is handed off a packet: its looks, four a cell, a lane of
/// one look a cell expanded into `room`, each its own four; and its padded
/// samples.
pub fn handed<'a>(pkt: &'a [u8], room: &'a mut [GroundId]) -> Result<(&'a [GroundId], &'a [i16]), MarshalError> {
    let Lanes { ele, ground, .. } = read_tile(pkt)?;
    if ground.len() == LOOK_LANE {
        return Ok((ground, ele));
    }
    for (c, q) in room[..LOOK_LANE].chunks_mut(4).enumerate() {
        q.copy_from_slice(&four_looks(ground, c));
    }
    Ok((&room[..LOOK_LANE], ele))
}

/// Decode a tile straight into its resident room in `.fixed.terrain`: its
/// lines; its cells go to its windows (`handed`).
pub fn decode_into(pkt: &[u8], room: separation::Room) -> Result<TerrainTile, MarshalError> {
    let Lanes { occupancy, separation, .. } = read_tile(pkt)?;
    Ok(TerrainTile {
        ground: Ground { cells: Cow::Borrowed(&[]) },
        separation: separation.into_room(room),
        ele: Cow::Borrowed(&[]),
        occupancy,
    })
}

// Copied out as the bake holds it, for a tile that outlives its packet.
pub fn decode(pkt: &[u8]) -> Result<TerrainTile, MarshalError> {
    let Lanes { ele, ground, occupancy, separation } = read_tile(pkt)?;
    let cells = expand(ground);
    Ok(TerrainTile::of(ele.to_vec(), Ground { cells: cells.into() }, separation.to_separation(), occupancy))
}

#[cfg(test)]
#[path = "terrain.test.rs"]
mod test;
