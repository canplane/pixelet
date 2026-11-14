// pxl-engine/fill.rs

//! A brick's fill: from its tile's samples, its lines and the four palette indices of each
//! cell (`Site`), it writes the brick's words (`clipmap::BRICK_WORDS`). It writes each
//! column's ground top and its cell as laid (`columns`); the ways' spans, worked out once a
//! column from its lines into the brick's scratch (`spans`, `clipmap::SPANS_AT`); from them
//! the brick's levels, level 0 holding each column's own entry (`mip`), and the spans the query
//! reads, kept in a run of the ways' pool (`spans_run_words`, `lay_spans`). The core runs
//! these a part at a time, as its entries have room (pxl-core domain/layer/terrain/clipmap.rs
//! `fill_share`).

use crate::answer;
use crate::clipmap;
use crate::clipmap::{
    column_top, BrickSource, BOUNDS_AT, BOUND_ENTRY_WORDS, COLUMNS_AT, OWN_AT, OWN_CELL_CNT,
    ROOT_AT, RUN_BITS_AT, RUN_OFFS_AT, RUN_RANK_AT, SPANS_ACROSS, SPANS_AT, SPANS_COLUMN_CNT,
    SPANS_COLUMN_WORDS,
};
use crate::{at, ground, way};
use pxl_spec::asset::cube::Sentinel;
use pxl_spec::brick;
use pxl_spec::brick::{BRICK, BRICK_CELL_CNT};
use pxl_spec::ground::{ele_at, own, word};
use pxl_spec::space::coord::world::SEA_LEVEL;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::area::Shard;
use pxl_spec::way::{column, SpanList, AIR_OF};

/// Sets word `i` of `s` to `v`. Like `at`, it is total: a store past the slice does nothing,
/// so no store aborts.
pub fn put(s: &mut [u32], i: u32, v: u32) {
    if (i as usize) < s.len() {
        // SAFETY: just bounded by the slice's own length.
        unsafe { *s.get_unchecked_mut(i as usize) = v };
    }
}

/// Column word `i` of the brick at `base` (`clipmap::COLUMN_WORDS`).
pub fn column_of(ground: &[u32], base: u32, i: u32) -> u32 {
    at(ground, base + COLUMNS_AT + i)
}

/// Entry `[x, y]` of level `j` of the brick at `base`, as its two bound words
/// (`clipmap::BOUND_ENTRY_WORDS`).
pub fn mip_of(ground: &[u32], base: u32, j: u32, [x, y]: [u32; 2]) -> [i32; 2] {
    let e = base + BOUNDS_AT + BOUND_ENTRY_WORDS * clipmap::mip_idx(j, [x, y]);
    [at(ground, e) as i32, at(ground, e + 1) as i32]
}

/// Cell `i`'s own palette index as laid, in the brick at `base` (`clipmap::OWN_WORDS`).
pub fn own_of(ground: &[u32], base: u32, i: u32) -> u32 {
    clipmap::own_at(at(ground, base + OWN_AT + i / OWN_CELL_CNT), i)
}

/// The first word of the run of kept spans of the brick at `base`, 0 where it has none
/// (`clipmap::ROOT_AT`).
pub fn root_of(ground: &[u32], base: u32) -> u32 {
    at(ground, base + ROOT_AT)
}

/// A brick being filled, read on its own, as `mip` folds it.
pub struct LoneBrick<'a> {
    pub ground: &'a [u32],
    pub base: u32,
}

impl BrickSource for LoneBrick<'_> {
    fn present(&self, _: u32, _: [i32; 2]) -> (u32, bool) {
        (0, true)
    }
    /// A brick on its own belongs to no clip level.
    fn brick(&self, _: u32, _: u32) -> ([i32; 2], bool) {
        ([0; 2], false)
    }
    fn joined(&self, _: u32, _: u32) -> [u32; clipmap::JOINED_WORDS as usize] {
        [0; clipmap::JOINED_WORDS as usize]
    }
    fn origin(&self, _: u32) -> ([i32; 2], bool) {
        ([0; 2], false)
    }
    /// Its own palette index four times; what `mip` folds reads no palette index.
    fn fold_at(&self, _: u32, _: u32, i: u32) -> u32 {
        word([own_of(self.ground, self.base, i); 4])
    }
    fn column(&self, _: u32, _: u32, i: u32) -> u32 {
        column_of(self.ground, self.base, i)
    }
    fn bound_mip(&self, _: u32, _: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
        mip_of(self.ground, self.base, j, at)
    }
    fn own(&self, _: u32, _: u32, i: u32) -> u32 {
        own_of(self.ground, self.base, i)
    }
    fn root(&self, _: u32, _: u32) -> u32 {
        at(self.ground, self.base + ROOT_AT)
    }
}

/// A brick's spans in its scratch, starting at `at` (`clipmap::SPANS_AT`), as its fill kept
/// them (`spans`). Its levels read its cells from them.
pub struct Scratch<'a> {
    pub words: &'a [u32],
    pub at: u32,
}

impl Scratch<'_> {
    /// Where column `c`'s spans begin, with `c` in the brick's cells and the ring one cell past
    /// each side.
    fn column(&self, c: [i32; 2]) -> u32 {
        let i = (c[1] + 1) as u32 * SPANS_ACROSS + (c[0] + 1) as u32;
        self.at + SPANS_AT + i * SPANS_COLUMN_WORDS
    }
}

impl way::SpanTable for Scratch<'_> {
    fn hdr(&self, c: [i32; 2]) -> (i32, u32) {
        clipmap::head_of(at(self.words, self.column(c)))
    }
    fn span(&self, c: [i32; 2], j: u32) -> ([i32; 2], u32) {
        let w = self.column(c) + 1 + 2 * j;
        (
            answer::bound_of(at(self.words, w) as i32),
            at(self.words, w + 1),
        )
    }
}

/// The head word of a column's spans (`clipmap::SPANS_COLUMN_WORDS`).
fn hdr_word(s: &SpanList) -> u32 {
    let face = if s.face == i32::MIN {
        i16::MIN as i32
    } else {
        s.face.clamp(i16::MIN as i32 + 1, i16::MAX as i32)
    };
    s.n | (face as u32) << 16
}

/// Where a brick being filled lies, and what it is filled from besides the four palette
/// indices of each cell (`columns`): its cells' zoom, its place in its tile, its first cell as
/// the world names it, what is known of the area there, its tile's padded samples
/// (pxl-spec `ground::ele_idx`) and its lines (`brick::LINES_ENTRIES_AT`).
#[derive(Clone, Copy)]
pub struct Site<'a> {
    pub cell_zoom: Zoom,
    pub place: [u32; 2],
    pub first: [i32; 2],
    pub shard: Shard,
    pub ele: &'a [i16],
    pub lines: &'a [u32],
}

impl Site<'_> {
    /// The elevation of cell `[x, y]`, counted from the brick's first cell, ring cells included
    /// (pxl-spec `ground::ele_at`).
    fn ele_at(&self, [x, y]: [i32; 2]) -> i32 {
        let first = self.place.map(|v| (v * BRICK) as i32);
        ele_at(|i| i32::from(self.ele[i]), [first[0] + x, first[1] + y])
    }
}

/// A brick being filled, as the ways' rule reads it (`brick::Brick`): its
/// words in `lane`, and its site.
struct LaneBrick<'a> {
    lane: &'a [u32],
    site: &'a Site<'a>,
}

impl brick::Brick for LaneBrick<'_> {
    fn cell_zoom(&self) -> u32 {
        self.site.cell_zoom
    }
    fn place(&self) -> [u32; 2] {
        self.site.place
    }
    fn first(&self) -> [i32; 2] {
        self.site.first
    }
    fn shard(&self) -> Shard {
        self.site.shard
    }
    fn line(&self, k: u32) -> u32 {
        at(self.site.lines, k)
    }
    fn own(&self, i: u32) -> u32 {
        own_of(self.lane, 0, i)
    }
    /// A column of the brick's own is read as `columns` wrote it (`column_top`), not drawn
    /// again. A ring column, which no column word holds, is drawn from the tile's samples as
    /// `columns` draws a cell.
    fn ele_at(&self, c: [i32; 2]) -> i32 {
        let side = BRICK as i32;
        if c[0] >= 0 && c[1] >= 0 && c[0] < side && c[1] < side {
            return column_top(column_of(
                self.lane,
                0,
                brick::idx_of([c[0] as u32, c[1] as u32]),
            ));
        }
        self.site.ele_at(c)
    }
}

/// Writes a brick's columns and its cells as laid into `lane`, where `four(i)` gives cell `i`'s
/// four palette indices (pxl-spec `ground::asset_ids`). Each column gets its ground top
/// (pxl-spec `ground::ele_at`); a gone cell, whose own palette index is `Sentinel::Unknown`
/// past the area, gets the Earth's top at sea level. Each cell gets its own palette index
/// (pxl-spec `ground::own` of its four at the brick's zoom), four to a word
/// (`clipmap::OWN_WORDS`).
pub fn columns(lane: &mut [u32], four: impl Fn(u32) -> u32, site: &Site) {
    let mut i = 0;
    while i < BRICK_CELL_CNT {
        let own = own(four(i), site.cell_zoom);
        let [x, y] = brick::of_idx(i);
        let top = if own == Sentinel::Unknown as u32 {
            SEA_LEVEL
        } else {
            site.ele_at([x as i32, y as i32])
        };
        put(lane, COLUMNS_AT + i, top as u32 & 0xffff);
        let w = OWN_AT + i / OWN_CELL_CNT;
        let k = 8 * (i % OWN_CELL_CNT);
        put(lane, w, (at(lane, w) & !(0xff << k)) | own << k);
        i += 1;
    }
}

/// Writes the spans of column `i` of the brick in `lane`, ring included
/// (`clipmap::SPANS_COLUMN_CNT`), into its scratch (`keep`). Each column's spans are worked out
/// once, before the brick's levels read them. It says whether the column made more than
/// pxl-spec `way::SPAN_CAP` (`keep`).
pub fn spans(lane: &[u32], site: &Site, scratch: &mut [u32], i: u32) -> bool {
    keep(&LaneBrick { lane, site }, scratch, i)
}

/// Writes the spans of column `i` of brick `sq`, counting row by row from the ring's first column
/// (`clipmap::SPANS_COLUMN_CNT`), from its lines and its face (pxl-spec `way::column`) into the
/// scratch (`clipmap::SPANS_AT`). A corner of the ring lies beside no column of the brick and gets
/// none. It says whether the column made more spans than pxl-spec `way::SPAN_CAP`, which keeps the
/// brick from being held whole.
pub fn keep(sq: &impl brick::Brick, scratch: &mut [u32], i: u32) -> bool {
    if i >= SPANS_COLUMN_CNT {
        return false;
    }
    let c = [(i % SPANS_ACROSS) as i32 - 1, (i / SPANS_ACROSS) as i32 - 1];
    let edge = |v: i32| v == -1 || v == BRICK as i32;
    let s = if edge(c[0]) && edge(c[1]) {
        SpanList::NONE
    } else {
        column(sq, c)
    };
    let at = SPANS_AT + i * SPANS_COLUMN_WORDS;
    put(scratch, at, hdr_word(&s));
    let mut j = 0;
    while j < s.n as usize {
        let w = at + 1 + 2 * j as u32;
        let clamp16 = |v: i32| v.clamp(i16::MIN as i32, i16::MAX as i32);
        let run = [clamp16(s.run[j][0]), clamp16(s.run[j][1])];
        put(scratch, w, answer::bound_word(run) as u32);
        put(scratch, w + 1, s.val[j]);
        j += 1;
    }
    s.past
}

/// Entry `[x, y]` of level `j` of the brick at `base`, whose cells are at `cell_zoom`. At
/// level 0 it is column `[x, y]`'s own entry: its ground's top, and the leaf cells the ways
/// write in it, from the spans its fill kept (`way::lined_hull`; `ground::Extent`). At a
/// higher level it is joined over the level under it, which is written first
/// (`clipmap::join`). A bore's air counts only as far up as the column's top, as the ways
/// write it: above the top a cell is air whatever writes it. A bound reaching up there, past a
/// tunnel's mouth where the ground falls away under the tunnel's clear height, would split
/// every node above it.
pub fn mip(
    ground: &mut [u32],
    base: u32,
    spans: &impl way::SpanTable,
    cell_zoom: Zoom,
    j: u32,
    at: [u32; 2],
) {
    let w = if j == 0 {
        let top = column_top(column_of(ground, base, brick::idx_of(at)));
        let lines = way::lined_hull(spans, [at[0] as i32, at[1] as i32], cell_zoom, top);
        ground::extent_word([[top, top], lines])
    } else {
        let under: &[u32] = ground;
        clipmap::join(
            &LoneBrick {
                ground: under,
                base,
            },
            0,
            0,
            j,
            at,
        )
    };
    let e = base + BOUNDS_AT + BOUND_ENTRY_WORDS * clipmap::mip_idx(j, at);
    put(ground, e, w[0] as u32);
    put(ground, e + 1, w[1] as u32);
}

/// Writes every level of the brick in `lane`, whose cells are at `cell_zoom`, from the spans
/// in `scratch`, each level over the one under it (`mip`).
pub fn mips(lane: &mut [u32], scratch: &[u32], cell_zoom: Zoom) {
    let column_spans = Scratch {
        words: scratch,
        at: 0,
    };
    for j in 0..=clipmap::BRICK_MIP_LAST {
        let side = BRICK >> j;
        for k in 0..side * side {
            mip(lane, 0, &column_spans, cell_zoom, j, [k % side, k / side]);
        }
    }
}

/// Fills a brick whole into `lane`, with `scratch` for its spans (`clipmap::SPANS_WORDS`)
/// and `four` giving each cell's four palette indices (`columns`): its columns, its spans and
/// its levels. It says whether a column made more spans than pxl-spec `way::SPAN_CAP` (`keep`).
pub fn brick(
    lane: &mut [u32],
    scratch: &mut [u32],
    four: impl Fn(u32) -> u32,
    site: &Site,
) -> bool {
    columns(lane, four, site);
    let mut past = false;
    for i in 0..SPANS_COLUMN_CNT {
        past |= spans(lane, site, scratch, i);
    }
    mips(lane, scratch, site.cell_zoom);
    past
}

/// Whether column `i` is kept in the brick's run (`clipmap::RUN_BITS_AT`): it has spans, or it
/// is the brick's own and has a face, and a bore's air beside it may line it (`way::way_cell`).
fn run_keeps(spans: &Scratch, i: u32) -> bool {
    use way::SpanTable;
    let c = [(i % SPANS_ACROSS) as i32 - 1, (i / SPANS_ACROSS) as i32 - 1];
    let (face, n) = spans.hdr(c);
    let own = |v: i32| (0..BRICK as i32).contains(&v);
    if n > 0 {
        return true;
    }
    if !own(c[0]) || !own(c[1]) || face == i32::MIN {
        return false;
    }
    [[-1, 0], [1, 0], [0, -1], [0, 1]]
        .iter()
        .any(|d: &[i32; 2]| {
            let b = [c[0] + d[0], c[1] + d[1]];
            (0..spans.hdr(b).1).any(|j| spans.span(b, j).1 & AIR_OF != 0)
        })
}

/// How many columns the brick's run keeps.
fn kept_cnt(spans: &Scratch) -> u32 {
    let (mut kept, mut i) = (0, 0);
    while i < SPANS_COLUMN_CNT {
        kept += u32::from(run_keeps(spans, i));
        i += 1;
    }
    kept
}

/// The words of the brick's run of kept spans: its index and, for each column kept, a word in the
/// index and its head and spans (`clipmap::RUN_BITS_AT`). 0 where no column has spans.
pub fn spans_run_words(scratch: &[u32]) -> u32 {
    let spans = Scratch {
        words: scratch,
        at: 0,
    };
    let (mut kept, mut spanned, mut i) = (0, 0, 0);
    while i < SPANS_COLUMN_CNT {
        if run_keeps(&spans, i) {
            kept += 1;
            spanned += at(scratch, SPANS_AT + i * SPANS_COLUMN_WORDS) & clipmap::SPANS_CNT;
        }
        i += 1;
    }
    if spanned == 0 {
        return 0;
    }
    RUN_OFFS_AT + kept + kept + 2 * spanned
}

/// Writes the brick's run of kept spans from the scratch its fill made it from, a word at a time
/// by its place in the run (`put`, which writes across the pool's chunks). Its size is
/// `spans_run_words`.
pub fn lay_spans(scratch: &[u32], put: &mut impl FnMut(u32, u32)) {
    let spans = Scratch {
        words: scratch,
        at: 0,
    };
    let (mut bits, mut rank, mut r) = (0u32, 0u32, 0u32);
    let mut payload = RUN_OFFS_AT + kept_cnt(&spans);
    let mut i = 0;
    while i < SPANS_COLUMN_CNT {
        if i % 64 == 0 {
            // A count of the columns before each pair of bitmap words, two to a word.
            let block = i / 64;
            if block % 2 == 1 {
                put(RUN_RANK_AT + block / 2, rank | r << 16);
            }
            rank = r;
        }
        if run_keeps(&spans, i) {
            bits |= 1 << (i % 32);
            put(RUN_OFFS_AT + r, payload);
            let from = SPANS_AT + i * SPANS_COLUMN_WORDS;
            let words = 1 + 2 * (at(scratch, from) & clipmap::SPANS_CNT);
            let mut w = 0;
            while w < words {
                put(payload + w, at(scratch, from + w));
                w += 1;
            }
            payload += words;
            r += 1;
        }
        i += 1;
        if i % 32 == 0 || i == SPANS_COLUMN_CNT {
            put(RUN_BITS_AT + (i - 1) / 32, bits);
            bits = 0;
        }
    }
    let last = (SPANS_COLUMN_CNT - 1) / 64;
    if last.is_multiple_of(2) {
        put(RUN_RANK_AT + last / 2, rank);
    }
}
