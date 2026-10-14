// pxl-spec/bake/ways.rs

//! A tile's ways region as the bake writes it and every reader takes it
//! (pxl-portable ways.rs): its bridges' and tunnels' segments and the piers
//! it stood, each listed in the bins of the tile's cells it reaches.

use pxl_portable::ways::{self as region, BINS, BIN_CELLS, ENTRIES, FIRST, LAST, PIERS, PIER_WORDS, SEGMENTS, SEGMENT_WORDS, SLAB, TABLE};

use super::separation::Form;
use crate::space::layer::lattice::TILE_CELLS;

/// A segment of a bridge or a tunnel, as the region holds it: its points in
/// eighths of the tile's cells, its surface at each in leaf cells, and its
/// line's width (eighths of a cell), kind, form and bore's clear height, and
/// whether it is the line's first or last.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Segment {
    pub a: [i16; 2],
    pub b: [i16; 2],
    pub tops: [i16; 2],
    pub width: u8,
    pub kind: u8,
    pub form: Form,
    pub clear: u8,
    pub first: bool,
    pub last: bool,
}

/// A pier the bake stood: its cell and its span in leaf cells, low to high.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pier {
    pub cell: [u16; 2],
    pub span: [i16; 2],
}

impl Segment {
    /// Its footprint in the tile's cells, x0 y0 x1 y1: as far as it is drawn
    /// past its line (`LINE_PAST`), what `may` reads.
    fn footprint(&self) -> [f32; 4] {
        let p = |v: i16| f32::from(v) / region::POINT_SCALE;
        let half = f32::from(self.width) / region::POINT_SCALE * 0.5 + region::LINE_PAST;
        [
            p(self.a[0]).min(p(self.b[0])) - half,
            p(self.a[1]).min(p(self.b[1])) - half,
            p(self.a[0]).max(p(self.b[0])) + half,
            p(self.a[1]).max(p(self.b[1])) + half,
        ]
    }

    fn words(&self) -> [u32; 4] {
        let pair = |v: [i16; 2]| u32::from(v[0] as u16) | u32::from(v[1] as u16) << 16;
        let line = u32::from(self.width) << region::WIDTH
            | u32::from(self.kind) << region::KIND
            | u32::from(self.form.byte()) << region::FORM
            | if self.first { FIRST } else { 0 }
            | if self.last { LAST } else { 0 }
            | u32::from(self.clear) << region::CLEAR;
        [pair(self.a), pair(self.b), pair(self.tops), line]
    }
}

/// The region of `segments` and `piers` under a slab `slab` leaf cells deep:
/// each in the bins its footprint (a pier, its cell) reaches inside the tile,
/// and one reaching none left out. A region names at most 2^16 segments and
/// piers in all, which a tile's ways are far from; past it the bake refuses
/// the tile rather than drop one.
pub fn region(segments: &[Segment], piers: &[Pier], slab: i32) -> Vec<u32> {
    let n = TILE_CELLS as f32;
    let bins = BINS as usize;
    let reaches = |f: [f32; 4]| f[2] >= 0.0 && f[0] <= n && f[3] >= 0.0 && f[1] <= n;
    let kept: Vec<&Segment> = segments.iter().filter(|s| s.form != Form::Approach && reaches(s.footprint())).collect();
    xpute_kit::ensure!(kept.len() + piers.len() <= 1 << 16, E2BIG, kept.len() + piers.len());
    let mut lists: Vec<Vec<u16>> = vec![Vec::new(); bins * bins];
    for (i, s) in kept.iter().enumerate() {
        let f = s.footprint();
        for y in region::bin(f[1])..=region::bin(f[3]) {
            for x in region::bin(f[0])..=region::bin(f[2]) {
                lists[(y * BINS + x) as usize].push(i as u16);
            }
        }
    }
    for (j, p) in piers.iter().enumerate() {
        let b = (u32::from(p.cell[1]) / BIN_CELLS) * BINS + u32::from(p.cell[0]) / BIN_CELLS;
        lists[b as usize].push((kept.len() + j) as u16);
    }
    let entries: usize = lists.iter().map(Vec::len).sum();
    let words = ENTRIES as usize + entries.div_ceil(2) + SEGMENT_WORDS as usize * kept.len() + PIER_WORDS as usize * piers.len();
    let mut out = vec![0u32; words];
    out[SEGMENTS as usize] = kept.len() as u32;
    out[PIERS as usize] = piers.len() as u32;
    out[SLAB as usize] = slab as u32;
    let mut e = 0usize;
    for (b, list) in lists.iter().enumerate() {
        out[TABLE as usize + b] = e as u32;
        for &id in list {
            out[ENTRIES as usize + e / 2] |= u32::from(id) << (16 * (e % 2));
            e += 1;
        }
    }
    out[TABLE as usize + bins * bins] = e as u32;
    let mut at = ENTRIES as usize + e.div_ceil(2);
    for s in &kept {
        out[at..at + SEGMENT_WORDS as usize].copy_from_slice(&s.words());
        at += SEGMENT_WORDS as usize;
    }
    for p in piers {
        out[at] = u32::from(p.cell[0]) | u32::from(p.cell[1]) << 16;
        out[at + 1] = u32::from(p.span[0] as u16) | u32::from(p.span[1] as u16) << 16;
        at += PIER_WORDS as usize;
    }
    out
}
