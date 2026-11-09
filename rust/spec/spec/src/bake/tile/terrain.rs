// pxl-spec/bake/tile/terrain.rs

//! A baked terrain tile and its wire: `[samples, ground, occupancy, separation]`.

use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::shape;
use xpute_kit::wire::xtp::MAX_PKT_SZ;

use std::borrow::Cow;

use super::{occupancy_of, Occupancy};
use crate::bake::constant::ELE_PADDED_SPAN;
use crate::bake::malformed;
use crate::bake::separation::{self, Separation};
use crate::procedural::Frame;
use crate::space::coord::zoom::Zoom;
use crate::space::layer::lattice::TILE_CELLS;
use crate::space::layer::quadrant::Quadrant;

/// A tile's column lengths: one a cell, four palette indices a cell, and its padded samples.
/// A tile is these sizes and no other.
pub const GROUND_COLUMN: usize = (TILE_CELLS * TILE_CELLS) as usize;
pub const PALETTE_COLUMN: usize = 4 * GROUND_COLUMN;
pub const ELE_COLUMN: usize = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
use crate::asset::cube::sentinel;
use crate::space::layer::lattice::GROUND;
use pxl_engine::area::Shard;
use pxl_engine::clipmap;
use pxl_engine::clipmap::Brick;
/// Bins a side a tile's ways are cut into, each `1 << WAYS_SHIFT` cells a
/// side, as a clip level's brick lays its lines by them (`brick_lines`).
pub use pxl_engine::clipmap::{WAYS_ACROSS, WAYS_SHIFT};
use pxl_engine::ground;
use pxl_engine::way;

/// The most a packet takes: what a client holds one in and refuses one
/// past, and where a bake refuses a tile.
pub(crate) const MAX_PKT_BYTES: usize = 2 << 20;

/// What a ground cell holds: the id of a cube of the ground's palette, a
/// Carto palette index, which fits a byte (`asset/cube.json`).
pub type GroundId = u8;

/// The samples, the palette indices, the occupancy, the separation.
type Packet = (Vec<i16>, Vec<GroundId>, u8, separation::Packet);

/// A tile's ground. In the bake it holds the four palette indices each cell folds,
/// `dx + 2·dy`, as a GPU holds them, a leaf's cell holding its own four; on the wire, one a
/// cell where every cell's four are one; on the CPU, each cell's own palette index
/// (`decode_into`).
pub type GroundCells = Cow<'static, [GroundId]>;

/// A tile: one column for each fact the client cannot make. What the client draws by rule,
/// such as a slab, a bore or a pier, is not here. The bake holds what goes on the wire; a
/// client holds what it reads of a tile as a whole, its lines. A client holds the tile's cells
/// in its clip levels, filled from the samples, and its ways there from the lines (pxl-engine
/// `clipmap`, `fill`).
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainTile {
    pub cells: GroundCells, // One class a cell, rasterized from the source or folded; a client holds it only for a void tile.
    pub separation: Separation, // The ways off the ground: the source's lines, with tops solved offline.
    pub ele: Cow<'static, [i16]>, // The padded samples, a leaf's from the source or folded; a client holds its own in its room.
    pub occupancy: Occupancy, // Which children are tiles; a child that is not one is never asked for.
}

/// How many entries of the brick at `place` (in bricks) a tile's ways make: one for each
/// segment and each bin its footprint reaches (`Separation::reaches`).
pub fn entry_cnt(separation: &Separation, place: [u32; 2]) -> usize {
    let bins = clipmap::BRICK >> WAYS_SHIFT;
    let mut cnt = 0;
    separation.reaches(f64::from(TILE_CELLS), WAYS_ACROSS, |k, _, _, _| {
        let k = k as u32;
        cnt += usize::from([k % WAYS_ACROSS / bins, k / WAYS_ACROSS / bins] == place);
    });
    cnt
}

/// Lays the lines of the clip level's brick at `place` (in bricks) of `tile` into `out`, as its
/// fill asks them (pxl-engine clipmap.rs `LINES_*`): the tile's header, each bin's first entry,
/// the entries, and the segments the bins reach, each once, in their lines' order. `pairs` holds
/// each entry while they are worked out. It returns the words laid, none where `pairs` holds
/// fewer than its entries or `out` fewer than its words.
pub fn brick_lines(
    tile: Quadrant,
    separation: &Separation,
    place: [u32; 2],
    pairs: &mut [(u32, u32)],
    out: &mut [u32],
) -> Option<usize> {
    use pxl_engine::clipmap::{lines_words, BRICK, LINES_BINS_AT, LINES_ENTRIES_AT, WAYS_BINS};
    use pxl_engine::way::{header, HEADER_WORDS, SEGMENT_WORDS};
    let cnt = entry_cnt(separation, place);
    if cnt > pairs.len() {
        return None;
    }
    let pairs = &mut pairs[..cnt];
    let bins = BRICK >> WAYS_SHIFT;
    // Each bin of the brick a segment's footprint reaches, and the point it starts at.
    let mut at = 0;
    separation.reaches(f64::from(TILE_CELLS), WAYS_ACROSS, |k, d, _, s| {
        let [x, y] = [k as u32 % WAYS_ACROSS, k as u32 / WAYS_ACROSS];
        if [x / bins, y / bins] == place {
            pairs[at] = (
                (y % bins) * bins + x % bins,
                separation.starts[d] + s as u32,
            );
            at += 1;
        }
    });
    // The segments, each once in their points' order, each entry then its
    // segment's place among them.
    pairs.sort_unstable_by_key(|&(b, p)| (p, b));
    let segments = pairs.chunk_by(|a, b| a.1 == b.1).count() as u32;
    let words = lines_words(cnt as u32, segments) as usize;
    let out = out.get_mut(..words)?;
    let frame = Frame::of(tile);
    out[..HEADER_WORDS as usize].copy_from_slice(&header(frame.per_m(), &frame.stations()));
    let (mut n, mut last) = (0, None);
    for e in pairs.iter_mut() {
        let p = e.1;
        if last != Some(p) {
            let at = LINES_ENTRIES_AT as usize + cnt + n * SEGMENT_WORDS;
            out[at..at + SEGMENT_WORDS].copy_from_slice(
                &separation.segment_words(p as usize, last == Some(p.wrapping_sub(1))),
            );
            (n, last) = (n + 1, Some(p));
        }
        e.1 = (n - 1) as u32;
    }
    // Each bin's entries in its segments' order: its first, and after the last
    // bin's, how many there are.
    pairs.sort_unstable();
    let mut first = 0;
    for b in 0..WAYS_BINS {
        out[(LINES_BINS_AT + b) as usize] = first as u32;
        first += pairs[first..].iter().take_while(|e| e.0 == b).count();
    }
    out[(LINES_BINS_AT + WAYS_BINS) as usize] = first as u32;
    for (k, e) in pairs.iter().enumerate() {
        out[LINES_ENTRIES_AT as usize + k] = e.1;
    }
    Some(words)
}

/// A clip level's brick as its fill reads it (pxl-engine `clipmap::Brick`): its cells' zoom,
/// its place in its tile and its first cell (`first_cell`), inside the area; its lines as
/// `brick_lines` lays them; cell `i`'s own palette index from its four (`palette_idxs`);
/// and column `c`'s height, counted from its first cell, its ring included (`height`).
pub struct TileBrick<'a, L, H> {
    pub cell_zoom: Zoom,
    pub place: [u32; 2],
    pub first: [i32; 2],
    pub lines: &'a [u32],
    pub palette_idxs: L,
    pub height: H,
}

impl<L: Fn(u32) -> u32, H: Fn([i32; 2]) -> i32> Brick for TileBrick<'_, L, H> {
    fn cell_zoom(&self) -> Zoom {
        self.cell_zoom
    }
    fn place(&self) -> [u32; 2] {
        self.place
    }
    fn first(&self) -> [i32; 2] {
        self.first
    }
    /// Inside the area, with nothing around it past the area: a bake asks which of a tile's
    /// lines any span fills, and no wall cuts its ways.
    fn shard(&self) -> Shard {
        Shard::INSIDE
    }
    fn line(&self, k: u32) -> u32 {
        self.lines[k as usize]
    }
    fn own(&self, i: u32) -> u32 {
        ground::own((self.palette_idxs)(i), self.cell_zoom)
    }
    fn height(&self, c: [i32; 2]) -> i32 {
        (self.height)(c)
    }
}

/// Cell `i` of the brick at `place` (in bricks), in its tile's cells.
fn brick_cell(place: [u32; 2], i: u32) -> [u32; 2] {
    use pxl_engine::clipmap::{of_idx, BRICK};
    let [x, y] = of_idx(i);
    [place[0] * BRICK + x, place[1] * BRICK + y]
}

/// The clip level's brick at `place` (in bricks) of `tile` as its fill reads it, with its
/// lines `lines` (`brick_lines`): its palette indices, four a cell, as a client cuts them
/// (`palette_cells`), and its heights from the tile's samples `ele` (`ground::height`), ring
/// included.
fn laid<'a>(
    tile: Quadrant,
    cells: &'a GroundCells,
    ele: &'a [i16],
    place: [u32; 2],
    lines: &'a [u32],
) -> TileBrick<'a, impl Fn(u32) -> u32 + 'a, impl Fn([i32; 2]) -> i32 + 'a> {
    let palette_idxs = move |i: u32| {
        let at = brick_cell(place, i);
        let c = (at[1] * TILE_CELLS + at[0]) as usize;
        ground::word(four_palette_idxs(cells, c).map(u32::from))
    };
    let height = move |c: [i32; 2]| {
        let at = [0, 1].map(|a| (place[a] * clipmap::BRICK) as i32 + c[a]);
        ground::height(|j| i32::from(ele[j]), at)
    };
    let cell_zoom = GROUND.cell_zoom(tile.zoom);
    TileBrick {
        cell_zoom,
        place,
        first: first_cell(tile, cell_zoom, place),
        lines,
        palette_idxs,
        height,
    }
}

/// The first cell at `cell_zoom`, its cells' zoom, of the clip level's brick at
/// `place` (in bricks) of `tile`, as the world names it.
pub fn first_cell(tile: Quadrant, cell_zoom: Zoom, place: [u32; 2]) -> [i32; 2] {
    use pxl_engine::clipmap::BRICK;
    let first = [0, 1].map(|k| tile.at[k] * TILE_CELLS as i32 + (place[k] * BRICK) as i32);
    [pxl_engine::wrap_idx(cell_zoom, first[0]), first[1]]
}

/// `terrain` with only the lines it draws at `tile`'s zoom. Each line is read alone, its piers
/// standing as if nothing blocks them, by the rule its bricks' columns are read by, their
/// rings included (`way::column`), so a line no span of which fills a cell there
/// (`way::fills_any`) is none at that zoom and is not stored.
pub fn drawn(tile: Quadrant, terrain: TerrainTile) -> TerrainTile {
    use pxl_engine::clipmap::{lines_words, BRICK};
    let lines = &terrain.separation;
    let bricks = TILE_CELLS / BRICK;
    let (mut pairs, mut words) = (Vec::new(), Vec::new());
    let mut separation = separation::Builder::default();
    for line in lines.iter() {
        let mut one = separation::Builder::default();
        one.push(
            (0..line.points.len()).map(|i| (line.point(i), line.tops[i], line.first_zooms[i])),
            line.width,
            line.kind,
            line.form,
            line.clear,
        );
        let one = one.finish();
        let [lo, hi] = line.bounds();
        let footprint = line.footprint();
        let span = |k: usize| {
            let first = ((lo[k] - footprint) / f64::from(BRICK)).floor().max(0.0) as u32;
            let last = ((hi[k] + footprint) / f64::from(BRICK))
                .floor()
                .min(f64::from(bricks - 1)) as u32;
            first..=last
        };
        let seen = span(1).any(|qy| {
            span(0).any(|qx| {
                let cnt = entry_cnt(&one, [qx, qy]);
                pairs.resize(cnt, (0, 0));
                words.resize(lines_words(cnt as u32, cnt as u32) as usize, 0);
                let n = brick_lines(tile, &one, [qx, qy], &mut pairs, &mut words)
                    .expect("sized by its entries");
                let sq = laid(tile, &terrain.cells, &terrain.ele, [qx, qy], &words[..n]);
                let ring = -1..=BRICK as i32;
                ring.clone()
                    .any(|y| ring.clone().any(|x| way::column(&sq, [x, y]).0.n > 0))
            })
        });
        if seen {
            separation.push(
                (0..line.points.len()).map(|i| (line.point(i), line.tops[i], line.first_zooms[i])),
                line.width,
                line.kind,
                line.form,
                line.clear,
            );
        }
    }
    TerrainTile {
        separation: separation.finish(),
        ..terrain
    }
}

// Made on first use: a constant array of zeros would ship in the module's data.
static VOID_CELLS: std::sync::LazyLock<Vec<GroundId>> =
    std::sync::LazyLock::new(|| vec![sentinel::void; GROUND_COLUMN]);

/// Where the source has nothing at all, as a client holds it: void
/// cells at sea level, which give no surface, so nothing is drawn while
/// the ground is still known and stays asked for, and no child, so none
/// is asked for under it. Its clip levels' bricks are filled from it as a
/// packet's are (`cells`, and samples all zero).
impl Default for TerrainTile {
    fn default() -> TerrainTile {
        TerrainTile {
            cells: Cow::Borrowed(&VOID_CELLS[..]),
            separation: Separation::default(),
            ele: Cow::Borrowed(&[]),
            occupancy: Occupancy::ZERO,
        }
    }
}

impl TerrainTile {
    /// A leaf as the bake makes it: its padded samples, a palette index a cell, each
    /// its own four, and its separation.
    pub fn leaf(ele: Vec<i16>, cells: GroundCells, separation: Separation) -> TerrainTile {
        TerrainTile::of(ele, expand(&cells).into(), separation, Occupancy::LEAF)
    }

    /// A tile as the bake holds it: these columns, nothing a client looks up.
    pub fn of(
        ele: Vec<i16>,
        cells: GroundCells,
        separation: Separation,
        occupancy: Occupancy,
    ) -> TerrainTile {
        TerrainTile {
            cells,
            separation,
            ele: ele.into(),
            occupancy,
        }
    }
}

/// Cell `c`'s four palette indices, however a tile holds them: four a cell
/// (`PALETTE_COLUMN`), one a cell, each its own four, or one for the whole tile.
fn four_palette_idxs(cells: &[GroundId], c: usize) -> [GroundId; 4] {
    match cells.len() {
        PALETTE_COLUMN => [
            cells[4 * c],
            cells[4 * c + 1],
            cells[4 * c + 2],
            cells[4 * c + 3],
        ],
        1 => [cells[0]; 4],
        _ => [cells[c]; 4],
    }
}

/// The tile's four palette indices a cell, however it holds them (`four_palette_idxs`).
fn expand(cells: &[GroundId]) -> Vec<GroundId> {
    (0..GROUND_COLUMN)
        .flat_map(|c| four_palette_idxs(cells, c))
        .collect()
}

// A tile whose every cell's four palette indices are one, as a leaf's are, goes out with one
// palette index a cell, expanded on decode.
pub fn encode(tile: &TerrainTile) -> Result<Vec<u8>, MarshalError> {
    let four = &tile.cells;
    let one: Option<Vec<GroundId>> = four
        .chunks(4)
        .all(|q| q.iter().all(|&l| l == q[0]))
        .then(|| four.chunks(4).map(|q| q[0]).collect());
    let palette_idxs: &[GroundId] = one.as_deref().unwrap_or(four);
    shape::encode::<Packet>(
        (
            &tile.ele,
            palette_idxs,
            tile.occupancy.bits(),
            tile.separation.pkt(),
        ),
        MAX_PKT_SZ,
    )
}

struct Table<'a> {
    ele: &'a [i16],
    cells: &'a [GroundId],
    occupancy: Occupancy,
    separation: separation::Table<'a>,
}

/// Refused unless every column is a tile's length.
fn read_tile(pkt: &[u8]) -> Result<Table<'_>, MarshalError> {
    let (ele, cells, occupancy, separation) = shape::get_deep::<Packet>(pkt)?;
    let occupancy = occupancy_of(occupancy)?;
    let separation = separation::Table::of(separation)?;
    if ele.len() != ELE_COLUMN || (cells.len() != GROUND_COLUMN && cells.len() != PALETTE_COLUMN) {
        return Err(malformed());
    }
    Ok(Table {
        ele,
        cells,
        occupancy,
        separation,
    })
}

/// A packet's palette indices as it holds them, four a cell (`PALETTE_COLUMN`), one a
/// cell, each its own four, or one for the whole tile: which a client's clip levels cut
/// their bricks' from.
pub fn palette_cells(pkt: &[u8]) -> Result<&[GroundId], MarshalError> {
    Ok(read_tile(pkt)?.cells)
}

/// Decode a tile straight into its resident room in `.fixed.terrain`: its
/// lines and, into `ele`, its padded samples; its cells go to its clip levels
/// (`palette_cells`). Refused where its lines are past `room`.
pub fn decode_into(
    pkt: &[u8],
    room: separation::Room,
    ele: &'static mut [i16],
) -> Result<TerrainTile, MarshalError> {
    let Table {
        ele: samples,
        occupancy,
        separation,
        ..
    } = read_tile(pkt)?;
    ele.copy_from_slice(samples);
    Ok(TerrainTile {
        cells: Cow::Borrowed(&[]),
        separation: separation.into_room(room)?,
        ele: Cow::Borrowed(ele),
        occupancy,
    })
}

// Copied out as the bake holds it, for a tile that outlives its packet.
pub fn decode(pkt: &[u8]) -> Result<TerrainTile, MarshalError> {
    let Table {
        ele,
        cells,
        occupancy,
        separation,
    } = read_tile(pkt)?;
    let cells = expand(cells);
    Ok(TerrainTile::of(
        ele.to_vec(),
        cells.into(),
        separation.to_separation(),
        occupancy,
    ))
}

#[cfg(test)]
#[path = "terrain.test.rs"]
mod test;
