// pxl-render/mesh/voxel_scan.rs

//! A chunk's bands to its column runs. A run is emitted when it opens and its
//! length patched in when it ends, so walk and packing are one pass. The direct
//! path (`begin_direct`) writes runs one call each from a layer's own source.

use pxl_spec::bake::constant::{band_cell_z, BAND_CELLS};

use crate::mesh::paint::terrain::adapter::ChunkSample;
use crate::mesh::volume::{band_col, taps_of, unpack_material, MaterialClasses, BAND_VOXELS, SUB_CELL_NONE};
use crate::mesh::{grow, try_push, Refused};
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

/// The ring of neighbor columns a run set carries. Without it the face mask
/// read every chunk seam as open air, losing occlusion, creases and culling
/// there. Nothing is emitted from an apron column.
pub const APRON: u32 = 1;
pub const COL_SPAN: u32 = BAND_CELLS + 2 * APRON;
const COLUMN_CNT: usize = (COL_SPAN * COL_SPAN) as usize;

pub fn apron_col(cx: u32, cy: u32) -> u32 {
    (cy + APRON) * COL_SPAN + cx + APRON
}

const SCAN_ROWS_PER_STEP: u32 = 8;

pub struct ExtraView<'a> {
    pub col: &'a [u32],
    pub z: &'a [i32],
    pub material: &'a [u32],
    pub layer_idx: &'a [u8],
    pub sub: &'a [u32],
}

/// Materials packed `0xCC_RRGGBB`, zero for air.
pub struct BandView<'a> {
    pub band_z: i32,
    pub material: &'a [u32],
    pub layer_idx: &'a [u8],
    pub sub_cell: Option<&'a [u8]>,
    pub extra: ExtraView<'a>,
}

impl BandView<'_> {
    fn check(&self) {
        xpute_core::ensure!(self.material.len() >= BAND_VOXELS && self.layer_idx.len() >= BAND_VOXELS, EINVAL);
        if let Some(sub) = self.sub_cell {
            xpute_core::ensure!(sub.len() >= BAND_VOXELS * 4, EINVAL);
        }
        let e = &self.extra;
        let n = e.col.len();
        xpute_core::ensure!(e.z.len() >= n && e.material.len() >= n && e.layer_idx.len() >= n && e.sub.len() >= n, EINVAL);
    }
}

#[derive(Clone, Copy)]
struct Run {
    z0: i32,
    z1: i32,
    layer_taps: u32,
    r: u8,
    g: u8,
    b: u8,
    material_class: u8,
    sub_ox: u8,
    sub_oy: u8,
    sub_oz: u8,
    sub_s: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanStep {
    Working,
    /// Returned by every step until `provide_set` hands this many bytes.
    NeedSet(u32),
    Done,
    /// The heap refused the runs room: the scan is over, and asked for again later.
    Refused,
}

pub struct ScanContext {
    runs: Vec<Run>,
    starts: Vec<u32>,
    open: bool,
    row: u32,
    packed: bool,
    next_col: Option<u32>,
}

impl Default for ScanContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ScanContext {
    pub fn new() -> Self {
        ScanContext {
            runs: Vec::with_capacity(1 << 13),
            starts: vec![0; COLUMN_CNT + 1],
            open: false,
            row: 0,
            packed: false,
            next_col: None,
        }
    }

    /// `bands` ascending by band_z. False when one is open.
    pub fn begin(&mut self, bands: &[BandView]) -> bool {
        if self.open {
            return false;
        }
        for band in bands {
            band.check();
        }
        self.runs.clear();
        self.open = true;
        self.row = 0;
        self.packed = false;
        self.next_col = None;
        true
    }

    /// Columns in row-major order, each column's runs together. False when a scan
    /// or a writer is open.
    pub fn begin_direct(&mut self) -> bool {
        if self.open {
            return false;
        }
        self.runs.clear();
        self.open = true;
        self.row = 0;
        self.packed = false;
        self.next_col = Some(0);
        true
    }

    pub fn direct_column(&mut self, col: u32) {
        let next = self.next_col.or_bug(Errno::ENOTRECOVERABLE);
        xpute_core::ensure!(col < COLUMN_CNT as u32, ENOSPC);
        for c in next..=col {
            self.starts[c as usize] = self.runs.len() as u32;
        }
        self.next_col = Some(col + 1);
    }

    pub fn direct_run(&mut self, z0: i32, z1: i32, material: u32, layer_idx: u8) -> Result<(), Refused> {
        xpute_core::ensure!(self.next_col.is_some_and(|n| n > 0), ENOTRECOVERABLE);
        try_push(&mut self.runs, run_of(z0, z1, material, layer_idx))
    }

    pub fn close_direct(&mut self) -> u32 {
        let next = self.next_col.take().or_bug(Errno::ENOTRECOVERABLE);
        for c in next..=COLUMN_CNT as u32 {
            self.starts[c as usize] = self.runs.len() as u32;
        }
        self.row = COL_SPAN;
        self.set_bytes()
    }

    /// The rows are the padded ones, so every column is written in prefix-array order.
    /// The apron's columns are the sample's, as a chunk with no bands reads them.
    pub fn step(&mut self, bands: &[BandView], apron: &ChunkSample) -> ScanStep {
        xpute_core::ensure!(self.open, ENOTRECOVERABLE);
        if self.packed {
            return ScanStep::Done;
        }
        if self.row >= COL_SPAN {
            return ScanStep::NeedSet(self.set_bytes());
        }
        let end = COL_SPAN.min(self.row + SCAN_ROWS_PER_STEP);
        for py in self.row..end {
            for px in 0..COL_SPAN {
                let col = (py * COL_SPAN + px) as usize;
                self.starts[col] = self.runs.len() as u32;
                let own = APRON..COL_SPAN - APRON;
                let inner = own.contains(&px) && own.contains(&py);
                let walked = if inner {
                    walk_column(&mut self.runs, bands, px - APRON, py - APRON)
                } else {
                    self.apron_run(apron, col)
                };
                if walked.is_err() {
                    self.abort();
                    return ScanStep::Refused;
                }
            }
        }
        self.row = end;
        if end >= COL_SPAN {
            self.starts[COLUMN_CNT] = self.runs.len() as u32;
            ScanStep::NeedSet(self.set_bytes())
        } else {
            ScanStep::Working
        }
    }

    fn apron_run(&mut self, apron: &ChunkSample, col: usize) -> Result<(), Refused> {
        let runs = &mut self.runs;
        apron.column_runs(col, |lo, hi, material, layer| try_push(runs, run_of(lo, hi, material, layer)))
    }

    fn set_bytes(&self) -> u32 {
        run_set_bytes(self.runs.len() as u32, COL_SPAN)
    }

    /// # Safety
    /// `ptr` must address at least the bytes asked for, 4-aligned, and be
    /// the caller's to write.
    pub unsafe fn provide_set(&mut self, ptr: *mut u8, bytes: u32) {
        xpute_core::ensure!(self.open && self.row >= COL_SPAN && !self.packed, ENOTRECOVERABLE);
        let need = self.set_bytes();
        xpute_core::ensure!(bytes >= need, ENOTRECOVERABLE, bytes, need);
        let n = self.runs.len();
        let mut at = ptr;
        let mut words = |len: usize| {
            let p = at.cast::<u32>();
            at = at.add(len * 4);
            core::slice::from_raw_parts_mut(p, len)
        };
        words(COLUMN_CNT + 1).copy_from_slice(&self.starts);
        let z0 = words(n);
        let z1 = words(n);
        let taps = words(n);
        let mut lane = |len: usize| {
            let p = at;
            at = at.add(len);
            core::slice::from_raw_parts_mut(p, len)
        };
        let (r, g, b, class) = (lane(n), lane(n), lane(n), lane(n));
        let (ox, oy, oz, size) = (lane(n), lane(n), lane(n), lane(n));
        for (i, run) in self.runs.iter().enumerate() {
            z0[i] = run.z0 as u32;
            z1[i] = run.z1 as u32;
            taps[i] = run.layer_taps;
            r[i] = run.r;
            g[i] = run.g;
            b[i] = run.b;
            class[i] = run.material_class;
            ox[i] = run.sub_ox;
            oy[i] = run.sub_oy;
            oz[i] = run.sub_oz;
            size[i] = run.sub_s;
        }
        self.packed = true;
    }

    pub fn finish(&mut self) -> u32 {
        xpute_core::ensure!(self.open && self.packed, ENOTRECOVERABLE);
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
    let (material_class, [r, g, b]) = unpack_material(material);
    Run {
        z0,
        z1,
        layer_taps: taps_of(layer_idx),
        r,
        g,
        b,
        material_class,
        sub_ox: 0,
        sub_oy: 0,
        sub_oz: 0,
        sub_s: SUB_CELL_NONE,
    }
}

fn push(runs: &mut Vec<Run>, run: Run) -> Result<usize, Refused> {
    try_push(runs, run)?;
    Ok(runs.len() - 1)
}

fn walk_column(runs: &mut Vec<Run>, bands: &[BandView], cx: u32, cy: u32) -> Result<(), Refused> {
    let col = band_col(cx, cy) as usize;
    let mut open: Option<usize> = None;
    let mut previous_band_z: Option<i32> = None;
    for band in bands {
        // Empty signed-Z bands break a material run; adjacent bands do not, or buried
        // faces at a band boundary would reappear.
        if previous_band_z.is_some_and(|p| band.band_z != p + 1) {
            open = None;
        }
        previous_band_z = Some(band.band_z);

        let base = col * BAND_CELLS as usize;
        for local_z in 0..BAND_CELLS as usize {
            let idx = base + local_z;
            let packed = band.material[idx];
            if packed == 0 {
                open = None;
                continue;
            }
            let z = band_cell_z(band.band_z) as i32 + local_z as i32;
            let (material_class, [r, g, b]) = unpack_material(packed);
            let layer_idx = band.layer_idx[idx];
            let sub = band.sub_cell.map(|s| &s[idx * 4..idx * 4 + 4]);
            let sub_s = sub.map_or(SUB_CELL_NONE, |s| s[3]);
            if sub_s != SUB_CELL_NONE {
                let s = sub.unwrap();
                push(
                    runs,
                    Run {
                        z0: z,
                        z1: z,
                        layer_taps: taps_of(layer_idx),
                        r,
                        g,
                        b,
                        material_class,
                        sub_ox: s[0],
                        sub_oy: s[1],
                        sub_oz: s[2],
                        sub_s,
                    },
                )?;
                open = None;
            } else if let Some(i) = open.filter(|&i| {
                let o = &runs[i];
                material_class != MaterialClasses::OBJECT && o.r == r && o.g == g && o.b == b && o.layer_taps == taps_of(layer_idx) && o.material_class == material_class
            }) {
                runs[i].z1 = z;
            } else {
                open = Some(push(
                    runs,
                    Run {
                        z0: z,
                        z1: z,
                        layer_taps: taps_of(layer_idx),
                        r,
                        g,
                        b,
                        material_class,
                        sub_ox: 0,
                        sub_oy: 0,
                        sub_oz: 0,
                        sub_s: SUB_CELL_NONE,
                    },
                )?);
            }
        }

        let e = &band.extra;
        if !e.col.is_empty() {
            let (lo, hi) = extra_range(e.col, col as u32);
            for i in lo..hi {
                let (material_class, [r, g, b]) = unpack_material(e.material[i]);
                let s = e.sub[i];
                push(
                    runs,
                    Run {
                        z0: e.z[i],
                        z1: e.z[i],
                        layer_taps: taps_of(e.layer_idx[i]),
                        r,
                        g,
                        b,
                        material_class,
                        sub_ox: s as u8,
                        sub_oy: (s >> 8) as u8,
                        sub_oz: (s >> 16) as u8,
                        sub_s: (s >> 24) as u8,
                    },
                )?;
            }
        }
    }
    Ok(())
}

fn extra_range(cols: &[u32], col: u32) -> (usize, usize) {
    let lo = cols.partition_point(|&c| c < col);
    let mut hi = lo;
    while hi < cols.len() && cols[hi] == col {
        hi += 1;
    }
    (lo, hi)
}

// The run set: one block of lanes, widest element first, so a set read back
// from its block needs no translation.

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
    pub material_class: &'a [u8],
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
        xpute_core::ensure!(bytes.len() >= need, EINVAL, bytes.len(), need);
        xpute_core::ensure!((bytes.as_ptr() as usize).is_multiple_of(4), EINVAL);
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
            material_class: lane(),
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
    pub material_class: Vec<u8>,
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
            &mut self.material_class,
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
                self.material_class.push(set.material_class[i]);
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
            &mut self.material_class,
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
            material_class: &self.material_class,
            sub_ox: &self.sub_ox,
            sub_oy: &self.sub_oy,
            sub_oz: &self.sub_oz,
            sub_s: &self.sub_s,
        }
    }
}
