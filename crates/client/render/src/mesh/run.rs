// pxl-render/mesh/run.rs

//! A chunk's column runs, written one call each from the layers' own
//! sources, a column at a time, and packed into one block (`RunSet`).

use crate::mesh::word::{taps_of, unpack_material, SUB_CELL_NONE};
use pxl_spec::block::sample::{grow, try_push, Refused, COL_SPAN};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

const COLUMN_CNT: usize = (COL_SPAN * COL_SPAN) as usize;

#[derive(Clone, Copy)]
struct Run {
    z0: i32,
    z1: i32,
    layer_taps: u32,
    r: u8,
    g: u8,
    b: u8,
    side: u8,
}

pub struct RunWriter {
    runs: Vec<Run>,
    starts: Vec<u32>,
    open: bool,
    span: u32,
    packed: bool,
    next_col: Option<u32>,
}

impl Default for RunWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl RunWriter {
    pub fn new() -> Self {
        RunWriter {
            runs: Vec::with_capacity(1 << 13),
            starts: vec![0; COLUMN_CNT + 1],
            open: false,
            span: 0,
            packed: false,
            next_col: None,
        }
    }

    /// Columns in row-major order, `span` a row, each column's runs together.
    /// False when one is open.
    pub fn begin(&mut self, span: u32) -> bool {
        if self.open {
            return false;
        }
        xpute_kit::ensure!(span <= COL_SPAN, EINVAL, span);
        self.runs.clear();
        self.open = true;
        self.span = span;
        self.packed = false;
        self.next_col = Some(0);
        true
    }

    pub fn column(&mut self, col: u32) {
        let next = self.next_col.or_bug(Errno::ENOTRECOVERABLE);
        xpute_kit::ensure!(col < self.span * self.span, ENOSPC);
        for c in next..=col {
            self.starts[c as usize] = self.runs.len() as u32;
        }
        self.next_col = Some(col + 1);
    }

    pub fn run(&mut self, z0: i32, z1: i32, material: u32, layer_idx: u8) -> Result<(), Refused> {
        xpute_kit::ensure!(self.next_col.is_some_and(|n| n > 0), ENOTRECOVERABLE);
        try_push(&mut self.runs, run_of(z0, z1, material, layer_idx))
    }

    pub fn close(&mut self) -> u32 {
        let next = self.next_col.take().or_bug(Errno::ENOTRECOVERABLE);
        for c in next..=self.span * self.span {
            self.starts[c as usize] = self.runs.len() as u32;
        }
        self.set_bytes()
    }

    fn set_bytes(&self) -> u32 {
        run_set_bytes(self.runs.len() as u32, self.span)
    }

    /// # Safety
    /// `ptr` must address at least the bytes asked for, 4-aligned, and be
    /// the caller's to write.
    pub unsafe fn provide_set(&mut self, ptr: *mut u8, bytes: u32) {
        xpute_kit::ensure!(self.open && self.next_col.is_none() && !self.packed, ENOTRECOVERABLE);
        let need = self.set_bytes();
        xpute_kit::ensure!(bytes >= need, ENOTRECOVERABLE, bytes, need);
        let n = self.runs.len();
        let mut at = ptr;
        let mut words = |len: usize| {
            let p = at.cast::<u32>();
            at = at.add(len * 4);
            core::slice::from_raw_parts_mut(p, len)
        };
        let cols = (self.span * self.span) as usize;
        words(cols + 1).copy_from_slice(&self.starts[..=cols]);
        let z0 = words(n);
        let z1 = words(n);
        let taps = words(n);
        let mut lane = |len: usize| {
            let p = at;
            at = at.add(len);
            core::slice::from_raw_parts_mut(p, len)
        };
        let (r, g, b, side) = (lane(n), lane(n), lane(n), lane(n));
        let (ox, oy, oz, size) = (lane(n), lane(n), lane(n), lane(n));
        for (i, run) in self.runs.iter().enumerate() {
            z0[i] = run.z0 as u32;
            z1[i] = run.z1 as u32;
            taps[i] = run.layer_taps;
            r[i] = run.r;
            g[i] = run.g;
            b[i] = run.b;
            side[i] = run.side;
            // A written run fills its cells; only a summary shrinks one.
            (ox[i], oy[i], oz[i], size[i]) = (0, 0, 0, SUB_CELL_NONE);
        }
        self.packed = true;
    }

    pub fn finish(&mut self) -> u32 {
        xpute_kit::ensure!(self.open && self.packed, ENOTRECOVERABLE);
        self.open = false;
        self.runs.len() as u32
    }

    pub fn abort(&mut self) {
        self.open = false;
        self.runs.clear();
        self.next_col = None;
    }
}

fn run_of(z0: i32, z1: i32, material: u32, layer_idx: u8) -> Run {
    let (side, [r, g, b]) = unpack_material(material);
    Run {
        z0,
        z1,
        layer_taps: taps_of(layer_idx),
        r,
        g,
        b,
        side,
    }
}

pub struct RunSet<'a> {
    pub count: u32,
    /// Column `(cx, cy)` is `cy * span + cx`.
    pub span: u32,
    /// None for a coarse tier's set, which carries no neighbors of its own.
    pub apron: u32,
    pub start: &'a [u32],
    pub z0: &'a [i32],
    pub z1: &'a [i32],
    pub layer_taps: &'a [u32],
    pub r: &'a [u8],
    pub g: &'a [u8],
    pub b: &'a [u8],
    pub side: &'a [u8],
    pub sub_ox: &'a [u8],
    pub sub_oy: &'a [u8],
    pub sub_oz: &'a [u8],
    /// One side on every axis, or SUB_CELL_NONE for a run that fills its cells.
    pub sub_s: &'a [u8],
}

const RUN_I32_FIELDS: u32 = 3;
const RUN_U8_FIELDS: u32 = 8;

pub fn run_set_bytes(count: u32, span: u32) -> u32 {
    (span * span + 1) * 4 + count * (RUN_I32_FIELDS * 4 + RUN_U8_FIELDS)
}

impl<'a> RunSet<'a> {
    /// Alignment and length are checked, so a wrong block panics rather than misreads.
    pub fn carve(bytes: &'a [u8], count: u32, span: u32, apron: u32) -> RunSet<'a> {
        let need = run_set_bytes(count, span) as usize;
        xpute_kit::ensure!(bytes.len() >= need, EINVAL, bytes.len(), need);
        xpute_kit::ensure!((bytes.as_ptr() as usize).is_multiple_of(4), EINVAL);
        let base = bytes.as_ptr();
        let n = count as usize;
        let cols = (span * span + 1) as usize;
        let mut at = 0usize;
        // SAFETY: every span lies inside `bytes`, whose length was checked against the
        // layout's total, and the word lanes start 4-aligned as the base was checked to.
        let mut words = |len: usize| unsafe {
            let p = base.add(at);
            at += len * 4;
            p
        };
        let start = unsafe { core::slice::from_raw_parts(words(cols).cast::<u32>(), cols) };
        let z0 = unsafe { core::slice::from_raw_parts(words(n).cast::<i32>(), n) };
        let z1 = unsafe { core::slice::from_raw_parts(words(n).cast::<i32>(), n) };
        let layer_taps = unsafe { core::slice::from_raw_parts(words(n).cast::<u32>(), n) };
        let mut bytes_at = at;
        let mut lane = || unsafe {
            let s = core::slice::from_raw_parts(base.add(bytes_at), n);
            bytes_at += n;
            s
        };
        RunSet {
            count,
            span,
            apron,
            start,
            z0,
            z1,
            layer_taps,
            r: lane(),
            g: lane(),
            b: lane(),
            side: lane(),
            sub_ox: lane(),
            sub_oy: lane(),
            sub_oz: lane(),
            sub_s: lane(),
        }
    }
}

#[derive(Default)]
pub struct OwnedRunSet {
    pub span: u32,
    pub apron: u32,
    pub start: Vec<u32>,
    pub z0: Vec<i32>,
    pub z1: Vec<i32>,
    pub layer_taps: Vec<u32>,
    pub r: Vec<u8>,
    pub g: Vec<u8>,
    pub b: Vec<u8>,
    pub side: Vec<u8>,
    pub sub_ox: Vec<u8>,
    pub sub_oy: Vec<u8>,
    pub sub_oz: Vec<u8>,
    pub sub_s: Vec<u8>,
}

impl OwnedRunSet {
    pub fn clear(&mut self) {
        self.start.clear();
        self.z0.clear();
        self.z1.clear();
        self.layer_taps.clear();
        for lane in [
            &mut self.r,
            &mut self.g,
            &mut self.b,
            &mut self.side,
            &mut self.sub_ox,
            &mut self.sub_oy,
            &mut self.sub_oz,
            &mut self.sub_s,
        ] {
            lane.clear();
        }
    }

    /// A chunk is a cube while its column's runs pass through every chunk over it,
    /// so the set is cut to the chunk's heights; the chunk across a cut face draws the rest.
    pub fn clip_from(&mut self, set: &RunSet, z0: i32, z1: i32) -> Result<(), Refused> {
        self.clear();
        self.span = set.span;
        self.apron = set.apron;
        let columns = (set.span * set.span) as usize;
        grow(&mut self.start, columns + 1)?;
        self.grow_runs(set.count as usize)?;
        self.start.push(0);
        for col in 0..columns {
            for i in set.start[col] as usize..set.start[col + 1] as usize {
                if set.z1[i] < z0 || set.z0[i] >= z1 {
                    continue;
                }
                self.z0.push(set.z0[i].max(z0));
                self.z1.push(set.z1[i].min(z1 - 1));
                self.layer_taps.push(set.layer_taps[i]);
                self.r.push(set.r[i]);
                self.g.push(set.g[i]);
                self.b.push(set.b[i]);
                self.side.push(set.side[i]);
                self.sub_ox.push(set.sub_ox[i]);
                self.sub_oy.push(set.sub_oy[i]);
                self.sub_oz.push(set.sub_oz[i]);
                self.sub_s.push(set.sub_s[i]);
            }
            self.start.push(self.z0.len() as u32);
        }
        Ok(())
    }

    pub(crate) fn grow_runs(&mut self, n: usize) -> Result<(), Refused> {
        grow(&mut self.z0, n)?;
        grow(&mut self.z1, n)?;
        grow(&mut self.layer_taps, n)?;
        for lane in [
            &mut self.r,
            &mut self.g,
            &mut self.b,
            &mut self.side,
            &mut self.sub_ox,
            &mut self.sub_oy,
            &mut self.sub_oz,
            &mut self.sub_s,
        ] {
            grow(lane, n)?;
        }
        Ok(())
    }

    pub fn view(&self) -> RunSet<'_> {
        RunSet {
            count: self.z0.len() as u32,
            span: self.span,
            apron: self.apron,
            start: &self.start,
            z0: &self.z0,
            z1: &self.z1,
            layer_taps: &self.layer_taps,
            r: &self.r,
            g: &self.g,
            b: &self.b,
            side: &self.side,
            sub_ox: &self.sub_ox,
            sub_oy: &self.sub_oy,
            sub_oz: &self.sub_oz,
            sub_s: &self.sub_s,
        }
    }
}
