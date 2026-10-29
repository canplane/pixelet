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
use crate::space::coord::zoom::Zoom;
use crate::space::layer::lattice::TILE_CELLS;
use crate::space::layer::tile::Tile;

/// A tile's lane lengths: a cell each, four looks a cell, and its padded
/// samples; a tile is these and no other size.
pub const GROUND_LANE: usize = (TILE_CELLS * TILE_CELLS) as usize;
pub const LOOK_LANE: usize = 4 * GROUND_LANE;
pub const ELE_LANE: usize = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
use crate::asset::cube::sentinel;
use crate::space::layer::lattice::GROUND;
use pxl_spec_core::fill;
use pxl_spec_core::ground;
use pxl_spec_core::lattice;
use pxl_spec_core::way;
use pxl_spec_core::window;
use pxl_spec_core::window::Brick;
/// Bins a side a tile's ways are cut into, each `1 << WAYS_SHIFT` cells a
/// side, as a window's brick lays its lines by them (`brick_lines`).
pub use pxl_spec_core::window::{WAYS_ACROSS, WAYS_SHIFT};

/// The most a packet takes: every lane at its longest, a look a cell four
/// and the separation at a record's room (`separation::MAX_LINES`,
/// `MAX_POINTS`), as `encode` lays them.
pub const MAX_PKT_BYTES: usize = {
    use super::pkt::{branch, seq, HEADER, SCALAR};
    use crate::bake::separation::{MAX_LINES, MAX_POINTS};
    let separation = branch(8)
        + seq((MAX_LINES + 1) * 4)
        + seq(MAX_POINTS * 4)
        + seq(MAX_POINTS * 2)
        + 4 * seq(MAX_LINES)
        + seq(MAX_POINTS);
    HEADER + branch(4) + seq(ELE_LANE * 2) + seq(LOOK_LANE) + SCALAR + separation
};

/// The most entries a window's brick holds of the ways (pxl-spec-core
/// `window::WAYS_BRICK_SEGMENTS`); a tile past it fails its bake
/// (`ways_fit`).
pub const WAYS_BRICK_SEGMENTS: usize = window::WAYS_BRICK_SEGMENTS as usize;

/// What a ground cell holds: the id of a cube of the ground's palette, a
/// Carto look, which fits a byte (`asset/cube.json`).
pub type GroundId = u8;

/// A tile's ground: in the bake the four looks a cell folds, `dx + 2·dy`, as
/// a GPU holds them, a leaf's its own four; on the wire one a cell where
/// every cell's four are one; on the CPU each cell's own look (`decode_into`).
#[derive(Clone, Debug, PartialEq)]
pub struct Ground {
    pub cells: Cow<'static, [GroundId]>,
}

/// A tile: one lane a fact the client cannot make. What it draws by rule —
/// a slab, a bore, a pier — is not here. The bake holds what goes on the
/// wire; a client what it reads of a tile as a whole, its lines. Its cells
/// a client holds in its windows, drawn by a GPU off the samples, and its
/// ways there off the lines (pxl-spec-core `window`).
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainTile {
    pub ground: Ground, // a class a cell: the source rasterized, or its fold; a client's void tile's alone
    pub separation: Separation, // ways off the ground: source lines, tops solved offline
    pub ele: Cow<'static, [i16]>, // the bake's: padded samples, a leaf's source or the fold's; a client hands them to a GPU off the packet
    pub occupancy: Occupancy,     // which children are tiles: none is asked for that is not
}

/// Whether every brick of `brick` cells a side a tile is cut into holds
/// its ways' entries within `WAYS_BRICK_SEGMENTS`; the first that does not,
/// its place in bricks and its count, where one does not.
pub fn ways_fit(separation: &Separation, brick: u32) -> Result<(), ([u32; 2], usize)> {
    let mut entries = [0usize; (WAYS_ACROSS * WAYS_ACROSS) as usize];
    separation.reaches(f64::from(TILE_CELLS), WAYS_ACROSS, |k, _, _, _| {
        entries[k] += 1
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

/// The lines of the window's brick at `place` (in bricks) of `tile`, as
/// the GPU asks them (pxl-spec-core window.rs `LINES_*`), into `out`, a
/// brick's `LINES_WORDS`: the tile's header, each bin's entries, and the
/// segments a bin reaches, each once, in their lines' order — the reach
/// `ways_fit` counts, so none holds more than `WAYS_BRICK_SEGMENTS`. The
/// words written, the rest left as they were; `scratch` the room it works
/// in, `WAYS_BRICK_SEGMENTS` pairs, kept between bricks.
pub fn brick_lines(
    tile: Tile,
    separation: &Separation,
    place: [u32; 2],
    scratch: &mut [(u32, u32)],
    out: &mut [u32],
) -> usize {
    use pxl_spec_core::way::{header, HEADER_WORDS, SEGMENT_WORDS};
    use pxl_spec_core::window::{
        BRICK, LINES_ENTRIES_AT, LINES_SEGMENTS_AT, LINES_WORDS, WAYS_BINS,
    };
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

/// A window's brick as its fill reads it (pxl-spec-core `window::Brick`):
/// its cells' zoom and place in its tile, its lines as `brick_lines` lays
/// them, cell `i` as laid off its four looks (`looks`, its own look, none
/// about it gone), and column `c`'s height,
/// counted from its first cell, its ring included (`height`).
pub struct Laid<'a, L, H> {
    pub cell: Zoom,
    pub place: [u32; 2],
    pub lines: &'a [u32],
    pub looks: L,
    pub height: H,
}

impl<L: Fn(u32) -> u32, H: Fn([i32; 2]) -> i32> Brick for Laid<'_, L, H> {
    fn cell(&self) -> Zoom {
        self.cell
    }
    fn place(&self) -> [u32; 2] {
        self.place
    }
    fn line(&self, k: u32) -> u32 {
        self.lines[k as usize]
    }
    /// Inside the area, none about it past: a bake sizes the most runs a
    /// client's fill writes, and no wall cuts its ways.
    fn laid(&self, i: u32) -> u32 {
        window::laid_half(ground::own((self.looks)(i), self.cell), 0)
    }
    fn height(&self, c: [i32; 2]) -> i32 {
        (self.height)(c)
    }
}

/// Cell `i` of the brick at `place` (in bricks), in its tile's cells.
fn brick_cell(place: [u32; 2], i: u32) -> [u32; 2] {
    use pxl_spec_core::window::{of_idx, BRICK};
    let [x, y] = of_idx(i);
    [place[0] * BRICK + x, place[1] * BRICK + y]
}

/// The window's brick at `place` (in bricks) of `tile` as its fill reads
/// it, its lines `lines` (`brick_lines`): its looks four a cell as a client
/// hands them (`handed`) and its heights off the tile's samples `ele` as
/// `height_cs` draws them (`ground::height`), its ring's too.
fn laid<'a>(
    tile: Tile,
    ground: &'a Ground,
    ele: &'a [i16],
    place: [u32; 2],
    lines: &'a [u32],
) -> Laid<'a, impl Fn(u32) -> u32 + 'a, impl Fn([i32; 2]) -> i32 + 'a> {
    let looks = move |i: u32| {
        let at = brick_cell(place, i);
        let c = (at[1] * TILE_CELLS + at[0]) as usize;
        ground::word(four_looks(&ground.cells, c).map(u32::from))
    };
    let height = move |c: [i32; 2]| {
        let at =
            [0, 1].map(|a| (place[a] * window::BRICK) as f32 + c[a] as f32 + lattice::CELL_CENTER);
        ground::bicubic(|j| i32::from(ele[j]), at[0], at[1])
    };
    Laid {
        cell: GROUND.cell_zoom(tile.zoom),
        place,
        lines,
        looks,
        height,
    }
}

/// The window's brick at `place` (in bricks) of `tile` handed into
/// `record` as a client hands it to a GPU's fill (pxl-spec-core window.rs
/// `HANDED_WORDS`): its looks four a cell (`handed`), its tile's padded
/// samples, and its lines (`brick_lines`, with `scratch`); the rest zero.
fn record(
    tile: Tile,
    held: &TerrainTile,
    place: [u32; 2],
    scratch: &mut [(u32, u32)],
    record: &mut [u32],
) {
    use pxl_spec_core::window::{BRICK_CELLS, ELE_AT, LINES_AT, LINES_WORDS, LOOKS_AT};
    record.fill(0);
    for i in 0..BRICK_CELLS {
        let at = brick_cell(place, i);
        record[(LOOKS_AT + i) as usize] = ground::word(
            four_looks(&held.ground.cells, (at[1] * TILE_CELLS + at[0]) as usize).map(u32::from),
        );
    }
    for (k, pair) in held.ele.chunks(2).enumerate() {
        record[ELE_AT as usize + k] =
            u32::from(pair[0] as u16) | u32::from(pair.get(1).map_or(0, |&v| v as u16)) << 16;
    }
    brick_lines(
        tile,
        &held.separation,
        place,
        scratch,
        &mut record[LINES_AT as usize..(LINES_AT + LINES_WORDS) as usize],
    );
}

/// The window's brick at `place` (in bricks) of `tile`, handed into
/// `handed` (`record`) and filled into `lane`, its spans into `kept`, its
/// scratch (`way::tree::SCRATCH_WORDS`), as a GPU fills it (pxl-spec-core
/// `fill::brick`): the most spans a column makes.
pub fn brick_spans(
    tile: Tile,
    held: &TerrainTile,
    place: [u32; 2],
    scratch: &mut [(u32, u32)],
    [handed, lane, kept]: [&mut [u32]; 3],
) -> u32 {
    use pxl_spec_core::window::BRICK;
    record(tile, held, place, scratch, handed);
    let cell = GROUND.cell_zoom(tile.zoom);
    let first = [0, 1].map(|k| tile.at[k] * TILE_CELLS as i32 + (place[k] * BRICK) as i32);
    let first = [pxl_spec_core::wrap_idx(cell, first[0]), first[1]];
    fill::brick(handed, lane, kept, place, cell, first)
}

/// The families the windows' trees take wherever the eye is, of the pool
/// (pxl-spec-core `way::tree::POOL_FAMILIES`): each eye's window's most at
/// its cells (`worst`, the most any placement of it holds), the others'
/// each the most of any, and the bricks handed a frame each the most a
/// brick takes before its fold gives families back (`taken`). A brick
/// without ways takes none.
pub fn pool_needs(worst: impl Fn(Zoom) -> u64, taken: u64) -> u64 {
    use pxl_spec_core::window::{EYE_WINDOWS, FIRST_CELL, HANDED, WINDOWS};
    let cells = FIRST_CELL..FIRST_CELL + EYE_WINDOWS;
    let most = cells.clone().map(&worst).max().unwrap_or(0);
    cells.map(worst).sum::<u64>()
        + u64::from(WINDOWS - EYE_WINDOWS) * most
        + u64::from(HANDED) * taken
}

/// `held` with only the lines it draws at `tile`'s zoom: each read alone,
/// its piers standing as none blocks them, by the rule its bricks' columns
/// are read by, their rings included (`way::column`) — so a line every span
/// of which fills no cell there (`way::fills_any`) is none at that zoom,
/// and is not stored.
pub fn drawn(tile: Tile, held: TerrainTile) -> TerrainTile {
    use pxl_spec_core::window::{BRICK, LINES_WORDS};
    let lines = &held.separation;
    let bricks = TILE_CELLS / BRICK;
    let (mut scratch, mut words) = (
        vec![(0, 0); WAYS_BRICK_SEGMENTS],
        vec![0; LINES_WORDS as usize],
    );
    let mut kept = separation::Builder::default();
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
        let reach = line.footprint();
        let span = |k: usize| {
            let first = ((lo[k] - reach) / f64::from(BRICK)).floor().max(0.0) as u32;
            let last = ((hi[k] + reach) / f64::from(BRICK))
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
            kept.push(
                (0..line.points.len()).map(|i| (line.point(i), line.tops[i], line.coarsest[i])),
                line.width,
                line.kind,
                line.form,
                line.clear,
            );
        }
    }
    TerrainTile {
        separation: kept.finish(),
        ..held
    }
}

/// What a tile's windows' bricks take of a client's ways (`fill_fit`):
/// each brick with ways, by its index in the tile row by row, and the
/// families its tree holds once folded; and the most a brick takes before
/// its fold gives families back. Empty for a tile without ways.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fit {
    pub held: Vec<(u32, u32)>,
    pub taken: u32,
}

/// Why a brick does not fill as a client fills it (`fill_fit`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unfit {
    /// A column making more than `way::SPANS` spans, how many.
    Spans(u32),
    /// Its ways meeting more than `way::tree::ROOT_NODES` of its own nodes
    /// along z, its tree's root's: how many.
    Nodes(u32),
    /// Its tree taking more than `way::tree::BRICK_FAMILIES` families.
    Families,
}

/// A brick as `fill_fit` filled and built it: its words and its kept
/// spans (pxl-spec-core `fill::brick`), its cells' zoom, and its ways' tree
/// in a pool of its own, its root word (`way::tree::root_word`) and the
/// families its build took and holds.
pub struct Built<'a> {
    pub lane: &'a [u32],
    pub kept: &'a [u32],
    pub cell: Zoom,
    pub pool: &'a [u32],
    pub root: u32,
    pub taken: u32,
    pub held: u32,
}

/// Each brick of `tile`'s windows filled (`brick_spans`) and its ways'
/// tree built (pxl-spec-core `fill::tree`) as a client fills and builds
/// them, row by row of its bricks, once, each handed to `each` as built
/// with its index in the tile, which a proof reads (pxl-geo-bake
/// bin/fill_digest.rs): the first brick that does not fit and why, else
/// what the tile takes of the client's ways (`Fit`).
pub fn fill_fit(
    tile: Tile,
    held: &TerrainTile,
    mut each: impl FnMut(u32, &Built<'_>),
) -> Result<Fit, ([u32; 2], Unfit)> {
    use pxl_spec_core::way::tree::{BRICK_FAMILIES, ROOT_NODES, SCRATCH_WORDS};
    use pxl_spec_core::window::{BRICK, BRICK_WORDS, HANDED_WORDS};
    let mut fit = Fit::default();
    if held.separation.is_empty() {
        return Ok(fit);
    }
    let mut scratch = vec![(0, 0); WAYS_BRICK_SEGMENTS];
    let (mut handed, mut lane, mut kept) = (
        vec![0; HANDED_WORDS as usize],
        vec![0; BRICK_WORDS as usize],
        vec![0; SCRATCH_WORDS as usize],
    );
    // A brick's room: the most a build takes, past the family none holds.
    let mut pool = vec![0; 16 * (BRICK_FAMILIES as usize + 1)];
    let side = TILE_CELLS / BRICK;
    for k in 0..side * side {
        let place = [k % side, k / side];
        let spans = brick_spans(
            tile,
            held,
            place,
            &mut scratch,
            [&mut handed, &mut lane, &mut kept],
        );
        if spans > way::SPANS as u32 {
            return Err((place, Unfit::Spans(spans)));
        }
        let cell = fill::cell_of(&handed, 0);
        // Asked before the build, which refuses past either alike.
        let (_, nodes) = fill::root_span(&lane, 0, cell);
        if nodes > ROOT_NODES {
            return Err((place, Unfit::Nodes(nodes)));
        }
        let [taken, families, root] = fill::tree(&lane, &kept, cell, &mut pool, 1);
        if taken == u32::MAX {
            return Err((place, Unfit::Families));
        }
        each(
            k,
            &Built {
                lane: &lane,
                kept: &kept,
                cell,
                pool: &pool,
                root,
                taken,
                held: families,
            },
        );
        if families > 0 {
            fit.held.push((k, families));
        }
        fit.taken = fit.taken.max(taken);
    }
    Ok(fit)
}

// Made on first use: a constant array of zeros would ship in the module's data.
static VOID_CELLS: std::sync::LazyLock<Vec<GroundId>> =
    std::sync::LazyLock::new(|| vec![sentinel::void; GROUND_LANE]);

impl TerrainTile {
    /// Where the source has nothing at all, as a client holds it: void
    /// cells at sea level, which give no surface, so nothing is drawn while
    /// the ground is still known and stays asked for, and no child, so none
    /// is asked for under it. Its windows' bricks are filled from it as a
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
        let ground = Ground {
            cells: expand(&ground.cells).into(),
        };
        TerrainTile::of(ele, ground, separation, Occupancy::LEAF)
    }

    /// A tile as the bake holds it: these lanes, nothing a client looks up.
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

/// Cell `c`'s four looks, however a tile holds them: four a cell
/// (`LOOK_LANE`), one a cell, each its own four, or one for the whole tile.
fn four_looks(cells: &[GroundId], c: usize) -> [GroundId; 4] {
    match cells.len() {
        LOOK_LANE => [
            cells[4 * c],
            cells[4 * c + 1],
            cells[4 * c + 2],
            cells[4 * c + 3],
        ],
        1 => [cells[0]; 4],
        _ => [cells[c]; 4],
    }
}

/// The tile's four looks a cell, however it holds them (`four_looks`).
fn expand(cells: &[GroundId]) -> Vec<GroundId> {
    (0..GROUND_LANE)
        .flat_map(|c| four_looks(cells, c))
        .collect()
}

// A tile whose every cell's four looks are one, a leaf's, goes out a look a
// cell, expanded on decode.
pub fn encode(tile: &TerrainTile) -> Result<Vec<u8>, MarshalError> {
    let four = &tile.ground.cells;
    let one: Option<Vec<GroundId>> = four
        .chunks(4)
        .all(|q| q.iter().all(|&l| l == q[0]))
        .then(|| four.chunks(4).map(|q| q[0]).collect());
    let looks: &[GroundId] = one.as_deref().unwrap_or(four);
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.i16_array(Some(&tile.ele))
            .u8_array(Some(looks))
            .u8(Some(tile.occupancy.bits()));
        separation::write_branch(&tile.separation, b);
    }));
    encoder().encode(
        &t,
        TreeEncoderOptions {
            init_cap: super::ENCODE_FROM,
            max_cap: None,
        },
    )
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
    Ok(Lanes {
        ele,
        ground,
        occupancy,
        separation,
    })
}

/// What a GPU is handed off a packet: its looks, four a cell, a lane of
/// one look a cell expanded into `room`, each its own four; and its padded
/// samples.
pub fn handed<'a>(
    pkt: &'a [u8],
    room: &'a mut [GroundId],
) -> Result<(&'a [GroundId], &'a [i16]), MarshalError> {
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
    let Lanes {
        occupancy,
        separation,
        ..
    } = read_tile(pkt)?;
    Ok(TerrainTile {
        ground: Ground {
            cells: Cow::Borrowed(&[]),
        },
        separation: separation.into_room(room),
        ele: Cow::Borrowed(&[]),
        occupancy,
    })
}

// Copied out as the bake holds it, for a tile that outlives its packet.
pub fn decode(pkt: &[u8]) -> Result<TerrainTile, MarshalError> {
    let Lanes {
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
