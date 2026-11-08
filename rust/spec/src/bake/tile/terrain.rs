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
use pxl_engine::fill;
use pxl_engine::ground;
use pxl_engine::way;

/// The most a packet takes: every column at its longest, four palette indices a cell, and the
/// separation at a record's room (`separation::MAX_LINES`, `MAX_POINTS`), as `encode` lays
/// them.
pub(crate) const MAX_PKT_BYTES: usize = {
    use super::pkt::{branch, seq, HEADER, SCALAR};
    use crate::bake::separation::{MAX_LINES, MAX_POINTS};
    let separation = branch(8)
        + seq((MAX_LINES + 1) * 4)
        + seq(MAX_POINTS * 4)
        + seq(MAX_POINTS * 2)
        + 4 * seq(MAX_LINES)
        + seq(MAX_POINTS);
    HEADER + branch(4) + seq(ELE_COLUMN * 2) + seq(PALETTE_COLUMN) + SCALAR + separation
};

/// The most entries a clip level's brick holds of the ways (pxl-engine
/// `clipmap::WAYS_BRICK_SEGMENTS`); a tile past it fails its bake
/// (`ways_fit`).
pub const WAYS_BRICK_SEGMENTS: usize = clipmap::WAYS_BRICK_SEGMENTS as usize;

/// What a ground cell holds: the id of a cube of the ground's palette, a
/// Carto palette index, which fits a byte (`asset/cube.json`).
pub type GroundId = u8;

/// The samples, the palette indices, the occupancy, the separation.
type Packet = (Vec<i16>, Vec<GroundId>, u8, separation::Packet);

/// A tile's ground. In the bake it holds the four palette indices each cell folds,
/// `dx + 2·dy`, as a GPU holds them, a leaf's cell holding its own four; on the wire, one a
/// cell where every cell's four are one; on the CPU, each cell's own palette index
/// (`decode_into`).
#[derive(Clone, Debug, PartialEq)]
pub struct Ground {
    pub cells: Cow<'static, [GroundId]>,
}

/// A tile: one column for each fact the client cannot make. What the client draws by rule,
/// such as a slab, a bore or a pier, is not here. The bake holds what goes on the wire; a
/// client holds what it reads of a tile as a whole, its lines. A client holds the tile's cells
/// in its clip levels, filled from the samples, and its ways there from the lines (pxl-engine
/// `clipmap`, `fill`).
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainTile {
    pub ground: Ground, // a class a cell: the source rasterized, or its fold; a client's void tile's alone
    pub separation: Separation, // ways off the ground: source lines, tops solved offline
    pub ele: Cow<'static, [i16]>, // padded samples, a leaf's source or the fold's; a client's in its room
    pub occupancy: Occupancy,     // which children are tiles: none is asked for that is not
}

/// Whether every brick, `brick` cells a side, that a tile is cut into holds its ways' entries
/// within `WAYS_BRICK_SEGMENTS`. Where one does not, the first such brick's place in bricks
/// and its count.
pub fn ways_fit(separation: &Separation, brick: u32) -> Result<(), ([u32; 2], usize)> {
    let mut entries = [0usize; (WAYS_ACROSS * WAYS_ACROSS) as usize];
    separation.reaches(f64::from(TILE_CELLS), WAYS_ACROSS, |k, _, _, _| {
        entries[k] += 1;
    });
    let (bins, bricks) = (brick >> WAYS_SHIFT, TILE_CELLS / brick);
    for qy in 0..bricks {
        for qx in 0..bricks {
            let n: usize = (0..bins * bins)
                .map(|b| {
                    entries[((qy * bins + b / bins) * WAYS_ACROSS + qx * bins + b % bins) as usize]
                })
                .sum();
            if n > WAYS_BRICK_SEGMENTS {
                return Err(([qx, qy], n));
            }
        }
    }
    Ok(())
}

/// Writes the lines of the clip level's brick at `place` (in bricks) of `tile`, as its fill
/// asks them (pxl-engine clipmap.rs `LINES_*`), into `out`, a brick's `LINES_WORDS`: the
/// tile's header, each bin's entries, and the segments the bins reach, each once, in their
/// lines' order. That is the reach `ways_fit` counts, so no brick holds more than
/// `WAYS_BRICK_SEGMENTS`. It returns the words written and leaves the rest as they were;
/// `scratch` is the room it works in, `WAYS_BRICK_SEGMENTS` pairs, kept between bricks.
pub fn brick_lines(
    tile: Quadrant,
    separation: &Separation,
    place: [u32; 2],
    scratch: &mut [(u32, u32)],
    out: &mut [u32],
) -> usize {
    use pxl_engine::clipmap::{BRICK, LINES_ENTRIES_AT, LINES_SEGMENTS_AT, LINES_WORDS, WAYS_BINS};
    use pxl_engine::way::{header, HEADER_WORDS, SEGMENT_WORDS};
    xpute_kit::ensure!(out.len() == LINES_WORDS as usize, ENOTRECOVERABLE);
    xpute_kit::ensure!(scratch.len() == WAYS_BRICK_SEGMENTS, ENOTRECOVERABLE);
    let bins = BRICK >> WAYS_SHIFT;
    // Each bin of the brick a segment's footprint reaches, and the point it starts at.
    let mut cnt = 0;
    separation.reaches(f64::from(TILE_CELLS), WAYS_ACROSS, |k, d, _, s| {
        let [x, y] = [k as u32 % WAYS_ACROSS, k as u32 / WAYS_ACROSS];
        if [x / bins, y / bins] == place {
            if let Some(e) = scratch.get_mut(cnt) {
                *e = (
                    (y % bins) * bins + x % bins,
                    separation.starts[d] + s as u32,
                );
            }
            cnt += 1;
        }
    });
    xpute_kit::ensure!(cnt <= WAYS_BRICK_SEGMENTS, ENOTRECOVERABLE, cnt);
    let scratch = &mut scratch[..cnt];
    let frame = Frame::of(tile);
    out[..HEADER_WORDS as usize].copy_from_slice(&header(frame.per_m(), &frame.stations()));
    // The segments, each once in their points' order, each entry then its
    // segment's place among them.
    scratch.sort_unstable_by_key(|&(b, p)| (p, b));
    let (mut n, mut last) = (0, None);
    for e in scratch.iter_mut() {
        let p = e.1;
        if last != Some(p) {
            let at = LINES_SEGMENTS_AT as usize + n * SEGMENT_WORDS;
            out[at..at + SEGMENT_WORDS].copy_from_slice(
                &separation.segment_words(p as usize, last == Some(p.wrapping_sub(1))),
            );
            (n, last) = (n + 1, Some(p));
        }
        e.1 = (n - 1) as u32;
    }
    // Each bin's entries in its segments' order, its first and its count.
    scratch.sort_unstable();
    let mut first = 0;
    for b in 0..WAYS_BINS {
        let n = scratch[first..].iter().take_while(|e| e.0 == b).count();
        out[(HEADER_WORDS + b) as usize] = first as u32 | (n as u32) << 16;
        first += n;
    }
    for (k, pair) in scratch.chunks(2).enumerate() {
        let at = |j: usize| pair.get(j).map_or(0, |e| e.1);
        out[LINES_ENTRIES_AT as usize + k] = at(0) | at(1) << 16;
    }
    LINES_SEGMENTS_AT as usize + n * SEGMENT_WORDS
}

/// A clip level's brick as its fill reads it (pxl-engine `clipmap::Brick`): its cells' zoom,
/// its place in its tile and its first cell (`first_cell`), inside the area; its lines as
/// `brick_lines` lays them; cell `i`'s own palette index from its four (`palette_indices`);
/// and column `c`'s height, counted from its first cell, its ring included (`height`).
pub struct TileBrick<'a, L, H> {
    pub cell_zoom: Zoom,
    pub place: [u32; 2],
    pub first: [i32; 2],
    pub lines: &'a [u32],
    pub palette_indices: L,
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
    /// Inside the area, with nothing around it past the area: a bake sizes the most runs a
    /// client's fill writes, and no wall cuts its ways.
    fn shard(&self) -> Shard {
        Shard::INSIDE
    }
    fn line(&self, k: u32) -> u32 {
        self.lines[k as usize]
    }
    fn own(&self, i: u32) -> u32 {
        ground::own((self.palette_indices)(i), self.cell_zoom)
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
    ground: &'a Ground,
    ele: &'a [i16],
    place: [u32; 2],
    lines: &'a [u32],
) -> TileBrick<'a, impl Fn(u32) -> u32 + 'a, impl Fn([i32; 2]) -> i32 + 'a> {
    let palette_indices = move |i: u32| {
        let at = brick_cell(place, i);
        let c = (at[1] * TILE_CELLS + at[0]) as usize;
        ground::word(four_palette_indices(&ground.cells, c).map(u32::from))
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
        palette_indices,
        height,
    }
}

/// The clip level's brick at `place` (in bricks) of `tile`, filled into `column`, with its
/// lines in `lines` (`brick_lines`, using `scratch`) and its spans in `spans`
/// (`clipmap::SPANS_WORDS`), as a client fills it (pxl-engine `fill::brick`). It returns the
/// most spans a column makes.
pub(crate) fn brick_spans(
    tile: Quadrant,
    held: &TerrainTile,
    place: [u32; 2],
    scratch: &mut [(u32, u32)],
    [lines, column, spans]: [&mut [u32]; 3],
) -> u32 {
    let n = brick_lines(tile, &held.separation, place, scratch, lines);
    let cell_zoom = GROUND.cell_zoom(tile.zoom);
    let site = fill::Site {
        cell_zoom,
        place,
        first: first_cell(tile, cell_zoom, place),
        shard: Shard::INSIDE,
        ele: &held.ele,
        lines: &lines[..n],
    };
    let four = |i: u32| {
        let at = brick_cell(place, i);
        ground::word(
            four_palette_indices(&held.ground.cells, (at[1] * TILE_CELLS + at[0]) as usize)
                .map(u32::from),
        )
    };
    fill::brick(column, spans, four, &site)
}

/// The first cell at `cell_zoom`, its cells' zoom, of the clip level's brick at
/// `place` (in bricks) of `tile`, as the world names it.
pub fn first_cell(tile: Quadrant, cell_zoom: Zoom, place: [u32; 2]) -> [i32; 2] {
    use pxl_engine::clipmap::BRICK;
    let first = [0, 1].map(|k| tile.at[k] * TILE_CELLS as i32 + (place[k] * BRICK) as i32);
    [pxl_engine::wrap_idx(cell_zoom, first[0]), first[1]]
}

/// The child groups of the pool (pxl-engine `way::tree::POOL_CHILD_GROUPS`) that the clip
/// levels' trees take wherever the eye is: each eye clip level's most at its cells (`worst`,
/// the most any placement of it holds), and for each of the others the most of any. A brick
/// without ways takes none.
pub fn pool_needs(worst: impl Fn(Zoom) -> u64) -> u64 {
    use pxl_engine::clipmap::{CLIP_LEVELS, EYE_CLIP_LEVELS, FIRST_CELL_ZOOM};
    let cells = FIRST_CELL_ZOOM..FIRST_CELL_ZOOM + EYE_CLIP_LEVELS;
    let most = cells.clone().map(&worst).max().unwrap_or(0);
    cells.map(worst).sum::<u64>() + u64::from(CLIP_LEVELS - EYE_CLIP_LEVELS) * most
}

/// `held` with only the lines it draws at `tile`'s zoom. Each line is read alone, its piers
/// standing as if nothing blocks them, by the rule its bricks' columns are read by, their
/// rings included (`way::column`), so a line no span of which fills a cell there
/// (`way::fills_any`) is none at that zoom and is not stored.
pub fn drawn(tile: Quadrant, held: TerrainTile) -> TerrainTile {
    use pxl_engine::clipmap::{BRICK, LINES_WORDS};
    let lines = &held.separation;
    let bricks = TILE_CELLS / BRICK;
    let (mut scratch, mut words) = (
        vec![(0, 0); WAYS_BRICK_SEGMENTS],
        vec![0; LINES_WORDS as usize],
    );
    let mut separation = separation::Builder::default();
    for line in lines.iter() {
        let mut one = separation::Builder::default();
        one.push(
            (0..line.points.len()).map(|i| (line.point(i), line.tops[i], line.coarsest[i])),
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
                let n = brick_lines(tile, &one, [qx, qy], &mut scratch, &mut words);
                let sq = laid(tile, &held.ground, &held.ele, [qx, qy], &words[..n]);
                let ring = -1..=BRICK as i32;
                ring.clone()
                    .any(|y| ring.clone().any(|x| way::column(&sq, [x, y]).0.n > 0))
            })
        });
        if seen {
            separation.push(
                (0..line.points.len()).map(|i| (line.point(i), line.tops[i], line.coarsest[i])),
                line.width,
                line.kind,
                line.form,
                line.clear,
            );
        }
    }
    TerrainTile {
        separation: separation.finish(),
        ..held
    }
}

/// What a tile's clip levels' bricks take of a client's ways (`fill_fit`):
/// each brick with ways, by its index in the tile row by row, and the
/// child groups its tree holds once folded; and the most a brick takes before
/// its fold gives child groups back. Empty for a tile without ways.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fit {
    pub bricks: Vec<(u32, u32)>,
    pub peak_group_cnt: u32,
}

/// Why a brick does not fill as a client fills it (`fill_fit`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unfit {
    /// A column making more than `way::SPANS` spans, how many.
    Spans(u32),
    /// Its ways meeting more than `way::tree::ROOT_NODES` of its own nodes
    /// along z, its tree's root's: how many.
    Nodes(u32),
    /// Its tree taking more than `way::tree::BRICK_CHILD_GROUPS` child groups.
    ChildGroups,
}

/// A brick as `fill_fit` filled and built it: its words and its
/// spans (pxl-engine `fill::brick`), its cells' zoom, and its ways' tree
/// in a pool of its own, its root word (`way::tree::root_word`) and the
/// child groups its build took and holds.
pub struct BrickWords<'a> {
    pub column: &'a [u32],
    pub spans: &'a [u32],
    pub cell_zoom: Zoom,
    pub pool: &'a [u32],
    pub root: u32,
    pub peak_group_cnt: u32,
    pub group_cnt: u32,
}

/// Each brick of `tile`'s clip levels filled (`brick_spans`) and its ways'
/// tree built (pxl-engine `fill::tree`) as a client fills and builds
/// them, row by row of its bricks, once, each handed to `each` as built
/// with its index in the tile, which a proof reads (pxl-geo-bake
/// bin/fill_digest.rs): the first brick that does not fit and why, else
/// what the tile takes of the client's ways (`Fit`).
pub fn fill_fit(
    tile: Quadrant,
    held: &TerrainTile,
    mut each: impl FnMut(u32, &BrickWords<'_>),
) -> Result<Fit, ([u32; 2], Unfit)> {
    use pxl_engine::clipmap::{BRICK, BRICK_WORDS, LINES_WORDS, SPANS_WORDS};
    use pxl_engine::way::tree::{BRICK_CHILD_GROUPS, ROOT_NODES};
    let mut fit = Fit::default();
    if held.separation.is_empty() {
        return Ok(fit);
    }
    let mut scratch = vec![(0, 0); WAYS_BRICK_SEGMENTS];
    let (mut lines, mut column, mut span_scratch) = (
        vec![0; LINES_WORDS as usize],
        vec![0; BRICK_WORDS as usize],
        vec![0; SPANS_WORDS as usize],
    );
    // A brick's room: the most a build takes, past child group 0, which none holds.
    let mut pool = vec![0; 16 * (BRICK_CHILD_GROUPS as usize + 1)];
    let side = TILE_CELLS / BRICK;
    for k in 0..side * side {
        let place = [k % side, k / side];
        let spans = brick_spans(
            tile,
            held,
            place,
            &mut scratch,
            [&mut lines, &mut column, &mut span_scratch],
        );
        if spans > way::SPANS as u32 {
            return Err((place, Unfit::Spans(spans)));
        }
        let cell_zoom = GROUND.cell_zoom(tile.zoom);
        // Asked before the build, which refuses past either alike.
        let (_, nodes) = fill::root_span(&column, 0, cell_zoom);
        if nodes > ROOT_NODES {
            return Err((place, Unfit::Nodes(nodes)));
        }
        let [used, child_groups, root] =
            fill::tree(&column, &span_scratch, cell_zoom, &mut pool, 1);
        if used == u32::MAX {
            return Err((place, Unfit::ChildGroups));
        }
        each(
            k,
            &BrickWords {
                column: &column,
                spans: &span_scratch,
                cell_zoom,
                pool: &pool,
                root,
                peak_group_cnt: used,
                group_cnt: child_groups,
            },
        );
        if child_groups > 0 {
            fit.bricks.push((k, child_groups));
        }
        fit.peak_group_cnt = fit.peak_group_cnt.max(used);
    }
    Ok(fit)
}

// Made on first use: a constant array of zeros would ship in the module's data.
static VOID_CELLS: std::sync::LazyLock<Vec<GroundId>> =
    std::sync::LazyLock::new(|| vec![sentinel::void; GROUND_COLUMN]);

impl TerrainTile {
    /// Where the source has nothing at all, as a client holds it: void
    /// cells at sea level, which give no surface, so nothing is drawn while
    /// the ground is still known and stays asked for, and no child, so none
    /// is asked for under it. Its clip levels' bricks are filled from it as a
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

    /// A leaf as the bake makes it: its padded samples, a palette index a cell, each
    /// its own four, and its separation.
    pub fn leaf(ele: Vec<i16>, ground: Ground, separation: Separation) -> TerrainTile {
        let ground = Ground {
            cells: expand(&ground.cells).into(),
        };
        TerrainTile::of(ele, ground, separation, Occupancy::LEAF)
    }

    /// A tile as the bake holds it: these columns, nothing a client looks up.
    pub fn of(
        ele: Vec<i16>,
        ground: Ground,
        separation: Separation,
        occupancy: Occupancy,
    ) -> TerrainTile {
        TerrainTile {
            ground,
            separation,
            ele: ele.into(),
            occupancy,
        }
    }
}

/// Cell `c`'s four palette indices, however a tile holds them: four a cell
/// (`PALETTE_COLUMN`), one a cell, each its own four, or one for the whole tile.
fn four_palette_indices(cells: &[GroundId], c: usize) -> [GroundId; 4] {
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

/// The tile's four palette indices a cell, however it holds them (`four_palette_indices`).
fn expand(cells: &[GroundId]) -> Vec<GroundId> {
    (0..GROUND_COLUMN)
        .flat_map(|c| four_palette_indices(cells, c))
        .collect()
}

// A tile whose every cell's four palette indices are one, as a leaf's are, goes out with one
// palette index a cell, expanded on decode.
pub fn encode(tile: &TerrainTile) -> Result<Vec<u8>, MarshalError> {
    let four = &tile.ground.cells;
    let one: Option<Vec<GroundId>> = four
        .chunks(4)
        .all(|q| q.iter().all(|&l| l == q[0]))
        .then(|| four.chunks(4).map(|q| q[0]).collect());
    let palette_indices: &[GroundId] = one.as_deref().unwrap_or(four);
    shape::encode::<Packet>(
        (
            &tile.ele,
            palette_indices,
            tile.occupancy.bits(),
            tile.separation.pkt(),
        ),
        MAX_PKT_SZ,
    )
}

struct Columns<'a> {
    ele: &'a [i16],
    ground: &'a [GroundId],
    occupancy: Occupancy,
    separation: separation::Columns<'a>,
}

/// Refused unless every column is a tile's length.
fn read_tile(pkt: &[u8]) -> Result<Columns<'_>, MarshalError> {
    let (ele, ground, occupancy, separation) = shape::get_deep::<Packet>(pkt)?;
    let occupancy = occupancy_of(occupancy)?;
    let separation = separation::Columns::of(separation)?;
    if ele.len() != ELE_COLUMN || (ground.len() != GROUND_COLUMN && ground.len() != PALETTE_COLUMN)
    {
        return Err(malformed());
    }
    Ok(Columns {
        ele,
        ground,
        occupancy,
        separation,
    })
}

/// A packet's palette indices as it holds them, four a cell (`PALETTE_COLUMN`), one a
/// cell, each its own four, or one for the whole tile: which a client's clip levels cut
/// their bricks' from.
pub fn palette_cells(pkt: &[u8]) -> Result<&[GroundId], MarshalError> {
    Ok(read_tile(pkt)?.ground)
}

/// Decode a tile straight into its resident room in `.fixed.terrain`: its
/// lines and, into `ele`, its padded samples; its cells go to its clip levels
/// (`palette_cells`).
pub fn decode_into(
    pkt: &[u8],
    room: separation::Room,
    ele: &'static mut [i16],
) -> Result<TerrainTile, MarshalError> {
    let Columns {
        ele: samples,
        occupancy,
        separation,
        ..
    } = read_tile(pkt)?;
    ele.copy_from_slice(samples);
    Ok(TerrainTile {
        ground: Ground {
            cells: Cow::Borrowed(&[]),
        },
        separation: separation.into_room(room),
        ele: Cow::Borrowed(ele),
        occupancy,
    })
}

// Copied out as the bake holds it, for a tile that outlives its packet.
pub fn decode(pkt: &[u8]) -> Result<TerrainTile, MarshalError> {
    let Columns {
        ele,
        ground,
        occupancy,
        separation,
    } = read_tile(pkt)?;
    let cells = expand(ground);
    Ok(TerrainTile::of(
        ele.to_vec(),
        Ground {
            cells: cells.into(),
        },
        separation.to_separation(),
        occupancy,
    ))
}

#[cfg(test)]
#[path = "terrain.test.rs"]
mod test;
