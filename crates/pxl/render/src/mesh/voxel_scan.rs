// pxl-render/mesh/voxel_scan.rs

//! A chunk's bands to its column runs: the whole-chunk scan.
//!
//! A row of columns a step, each column read bottom to top across its bands,
//! a run opened at its first voxel and extended while the material holds, a
//! band's overflow sub-voxels pushed after its voxels. The set is packed in
//! voxel_scan.rs's layout.
//!
//! A run is emitted when it opens rather than when it closes because the
//! sink writes forward only: a run's length is patched into the word already
//! written once the run ends, which is what lets the walk and the packing be
//! one pass. The sub-voxels trail their band for the same reason — they are
//! found after the band's voxels and are appended where the walk is.
//!
//! The runs are gathered into one vector of structs as the walk finds them,
//! since the count is known only at the end; the set they are packed into
//! is the caller's (`ScanStep::NeedSet`, `provide_set`), acquired then and
//! refusable, as the mesher's mesh is.
//!
//! The same scratch and packing serve the direct path (`begin_direct`),
//! where the runs are not walked out of bands but written one call each by
//! a layer's own source — one a column for terrain — and the same column
//! walk serves the raycast (`ColumnRuns`), which asks for one column of a
//! volume on a hover and reads the answer once.

use crate::mesh::volume::{BAND_SPAN, BAND_VOXELS, MATERIAL_OBJECT, SUB_CELL_NONE};

/// Cells on a band's side; a chunk is one column of bands.
const BAND_SPAN2: usize = (BAND_SPAN * BAND_SPAN) as usize;
const COLUMN_CNT: usize = BAND_SPAN2;
const SCAN_ROWS_PER_STEP: u32 = 8;

/// One band's overflow sub-voxels, sorted by column (volume.rs's Extras).
pub struct ExtraView<'a> {
    pub col: &'a [u32],
    pub z: &'a [i32],
    pub material: &'a [u32],
    pub layer_idx: &'a [u8],
    pub sub: &'a [u32],
}

/// One band as the scan reads it (volume.rs's Band): materials packed
/// `0xCC_RRGGBB`, zero for air; four sub-cell bytes a voxel when the band
/// has any.
pub struct BandView<'a> {
    pub band_z: i32,
    pub material: &'a [u32],
    pub layer_idx: &'a [u8],
    pub sub_cell: Option<&'a [u8]>,
    pub extra: ExtraView<'a>,
}

impl BandView<'_> {
    fn check(&self) {
        assert!(self.material.len() >= BAND_VOXELS && self.layer_idx.len() >= BAND_VOXELS, "band shorter than its voxels");
        if let Some(sub) = self.sub_cell {
            assert!(sub.len() >= BAND_VOXELS * 4, "sub-cell lane shorter than its voxels");
        }
        let e = &self.extra;
        let n = e.col.len();
        assert!(
            e.z.len() >= n && e.material.len() >= n && e.layer_idx.len() >= n && e.sub.len() >= n,
            "extra lanes shorter than their count"
        );
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
    sub_sz: u8,
}

/// Where a scan stands after a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanStep {
    Working,
    /// Every column is walked and the set's size is known: `provide_set`
    /// this many bytes. Returned again by every step until then.
    NeedSet(u32),
    Done,
}

/// One scan at a time, and the scratch every scan reuses.
pub struct ScanContext {
    runs: Vec<Run>,
    starts: Vec<u32>,
    open: bool,
    row: u32,
    packed: bool,
    /// The direct path's next column to start, while its writer is open.
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

    /// Opens a scan of `bands`, ascending by band_z. False when one is open.
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

    /// Opens the direct path's writer instead of a scan: the runs come one
    /// call each from a layer's own source, columns in row-major order,
    /// each column's runs together, then `close_direct` asks for the set as
    /// a finished scan does. False when a scan or a writer is open.
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

    /// Starts column `col`; the columns skipped since the last are empty,
    /// their prefix entries all pointing at the current run count.
    pub fn direct_column(&mut self, col: u32) {
        let next = self.next_col.expect("a column with no direct writer open");
        assert!(col < COLUMN_CNT as u32, "a column past the chunk");
        for c in next..=col {
            self.starts[c as usize] = self.runs.len() as u32;
        }
        self.next_col = Some(col + 1);
    }

    /// One full-cell run `[z0, z1]` of the column last started, in a
    /// packed 0xCC_RRGGBB material on `layer_idx`.
    pub fn direct_run(&mut self, z0: i32, z1: i32, material: u32, layer_idx: u8) {
        assert!(self.next_col.is_some_and(|n| n > 0), "a run with no column started");
        self.runs.push(Run {
            z0,
            z1,
            layer_taps: taps_of(layer_idx),
            r: (material >> 16) as u8,
            g: (material >> 8) as u8,
            b: material as u8,
            material_class: (material >> 24) as u8,
            sub_ox: 0,
            sub_oy: 0,
            sub_oz: 0,
            sub_sz: SUB_CELL_NONE,
        });
    }

    /// Closes the writer: the columns not started are empty, and the set
    /// is asked for — `provide_set` this many bytes, then `finish`.
    pub fn close_direct(&mut self) -> u32 {
        let next = self.next_col.take().expect("close with no direct writer open");
        for c in next..=COLUMN_CNT as u32 {
            self.starts[c as usize] = self.runs.len() as u32;
        }
        self.row = BAND_SPAN;
        self.set_bytes()
    }

    /// Walks up to a step's rows of `bands`, the bands it was opened with.
    pub fn step(&mut self, bands: &[BandView]) -> ScanStep {
        assert!(self.open, "scan step with no scan open");
        if self.packed {
            return ScanStep::Done;
        }
        if self.row >= BAND_SPAN {
            return ScanStep::NeedSet(self.set_bytes());
        }
        let end = BAND_SPAN.min(self.row + SCAN_ROWS_PER_STEP);
        for cy in self.row..end {
            for cx in 0..BAND_SPAN {
                self.starts[(cy * BAND_SPAN + cx) as usize] = self.runs.len() as u32;
                walk_column(&mut self.runs, bands, cx, cy);
            }
        }
        self.row = end;
        if end >= BAND_SPAN {
            self.starts[COLUMN_CNT] = self.runs.len() as u32;
            ScanStep::NeedSet(self.set_bytes())
        } else {
            ScanStep::Working
        }
    }

    fn set_bytes(&self) -> u32 {
        run_set_bytes(self.runs.len() as u32, BAND_SPAN)
    }

    /// Packs the walked runs into `bytes` bytes at `ptr`, in
    /// voxel_scan.rs's layout, after `ScanStep::NeedSet`.
    ///
    /// # Safety
    /// `ptr` must address at least the bytes asked for, 4-aligned, and be
    /// the caller's to write.
    pub unsafe fn provide_set(&mut self, ptr: *mut u8, bytes: u32) {
        assert!(self.open && self.row >= BAND_SPAN && !self.packed, "set provided to a scan that did not ask");
        let need = self.set_bytes();
        assert!(bytes >= need, "set of {} bytes provided where {} were asked", bytes, need);
        let n = self.runs.len();
        let mut at = ptr;
        let mut words = |len: usize| {
            let p = at as *mut u32;
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
        let (ox, oy, oz, sx, sy, sz) = (lane(n), lane(n), lane(n), lane(n), lane(n), lane(n));
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
            // A scanned sub-voxel is a cube: one extent on every axis. Only
            // the coarse mesher writes them apart.
            sx[i] = run.sub_sz;
            sy[i] = run.sub_sz;
            sz[i] = run.sub_sz;
        }
        self.packed = true;
    }

    /// Closes the packed scan: how many runs the set holds.
    pub fn finish(&mut self) -> u32 {
        assert!(self.open && self.packed, "finish on a scan that is not open and packed");
        self.open = false;
        self.runs.len() as u32
    }

    pub fn abort(&mut self) {
        self.open = false;
        self.runs.clear();
        self.next_col = None;
    }
}

/// One column's runs for the raycast: the scan's walk, kept apart from the
/// scan's own scratch so a hover while a scan is open across frames does
/// not disturb it, and laid out as lanes to read once.
pub struct ColumnRuns {
    runs: Vec<Run>,
    pub z0: Vec<i32>,
    pub z1: Vec<i32>,
    pub material_class: Vec<u8>,
    /// The inset packed as the band stores it: ox, oy, oz, then sz.
    pub sub: Vec<u32>,
}

impl Default for ColumnRuns {
    fn default() -> Self {
        Self::new()
    }
}

impl ColumnRuns {
    pub fn new() -> Self {
        ColumnRuns {
            runs: Vec::new(),
            z0: Vec::new(),
            z1: Vec::new(),
            material_class: Vec::new(),
            sub: Vec::new(),
        }
    }

    /// Walks column (cx, cy) of `bands`, ascending by band_z; how many runs.
    pub fn walk(&mut self, bands: &[BandView], cx: u32, cy: u32) -> usize {
        for band in bands {
            band.check();
        }
        self.runs.clear();
        walk_column(&mut self.runs, bands, cx, cy);
        self.z0.clear();
        self.z1.clear();
        self.material_class.clear();
        self.sub.clear();
        for run in &self.runs {
            self.z0.push(run.z0);
            self.z1.push(run.z1);
            self.material_class.push(run.material_class);
            self.sub
                .push(run.sub_ox as u32 | ((run.sub_oy as u32) << 8) | ((run.sub_oz as u32) << 16) | ((run.sub_sz as u32) << 24));
        }
        self.runs.len()
    }
}

fn push(runs: &mut Vec<Run>, run: Run) -> usize {
    runs.push(run);
    runs.len() - 1
}

fn walk_column(runs: &mut Vec<Run>, bands: &[BandView], cx: u32, cy: u32) {
    let col = (cy * BAND_SPAN + cx) as usize;
    let mut open: Option<usize> = None;
    let mut previous_band_z: Option<i32> = None;
    for band in bands {
        // Empty signed-Z bands break a material run. Adjacent bands do
        // not: buried faces at a cache boundary must not reappear.
        if previous_band_z.is_some_and(|p| band.band_z != p + 1) {
            open = None;
        }
        previous_band_z = Some(band.band_z);

        let base = col * BAND_SPAN as usize;
        for local_z in 0..BAND_SPAN as usize {
            let idx = base + local_z;
            let packed = band.material[idx];
            if packed == 0 {
                open = None;
                continue;
            }
            let z = band.band_z * BAND_SPAN as i32 + local_z as i32;
            let (r, g, b) = ((packed >> 16) as u8, (packed >> 8) as u8, packed as u8);
            let material_class = (packed >> 24) as u8;
            let layer_idx = band.layer_idx[idx];
            let sub = band.sub_cell.map(|s| &s[idx * 4..idx * 4 + 4]);
            let sub_sz = sub.map_or(SUB_CELL_NONE, |s| s[3]);
            if sub_sz != SUB_CELL_NONE {
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
                        sub_sz,
                    },
                );
                open = None;
            } else if let Some(i) = open.filter(|&i| {
                let o = &runs[i];
                material_class != MATERIAL_OBJECT && o.r == r && o.g == g && o.b == b && o.layer_taps == taps_of(layer_idx) && o.material_class == material_class
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
                        sub_sz: SUB_CELL_NONE,
                    },
                ));
            }
        }

        let e = &band.extra;
        if !e.col.is_empty() {
            let (lo, hi) = extra_range(e.col, col as u32);
            for i in lo..hi {
                let m = e.material[i];
                let s = e.sub[i];
                push(
                    runs,
                    Run {
                        z0: e.z[i],
                        z1: e.z[i],
                        layer_taps: taps_of(e.layer_idx[i]),
                        r: (m >> 16) as u8,
                        g: (m >> 8) as u8,
                        b: m as u8,
                        material_class: (m >> 24) as u8,
                        sub_ox: s as u8,
                        sub_oy: (s >> 8) as u8,
                        sub_oz: (s >> 16) as u8,
                        sub_sz: (s >> 24) as u8,
                    },
                );
            }
        }
    }
}

/// A fine run has one material, written into all four taps.
fn taps_of(layer_idx: u8) -> u32 {
    layer_idx as u32 * 0x0101_0101
}

/// The range of a band's sorted extras in column `col`.
fn extra_range(cols: &[u32], col: u32) -> (usize, usize) {
    let lo = cols.partition_point(|&c| c < col);
    let mut hi = lo;
    while hi < cols.len() && cols[hi] == col {
        hi += 1;
    }
    (lo, hi)
}

// ---- the run set ----
//
// A chunk's column runs.
//
// One block of lanes in carve order, widest element first, so a set read
// back out of the block it was written into needs no translation and the
// views below are the block itself.

/// Views over one run set's lanes.
pub struct RunSet<'a> {
    pub count: u32,
    /// Columns on a side; column `(cx, cy)` is `cy * span + cx`.
    pub span: u32,
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
    pub sub_sx: &'a [u8],
    pub sub_sy: &'a [u8],
    pub sub_sz: &'a [u8],
}

const RUN_I32_FIELDS: u32 = 3;
const RUN_U8_FIELDS: u32 = 10;

/// The bytes a set of `count` runs over `span`² columns occupies.
pub fn run_set_bytes(count: u32, span: u32) -> u32 {
    (span * span + 1) * 4 + count * (RUN_I32_FIELDS * 4 + RUN_U8_FIELDS)
}

impl<'a> RunSet<'a> {
    /// Carves the lanes out of `bytes`, which must start 4-aligned and hold
    /// at least `run_set_bytes(count, span)`; both are checked, so a wrong
    /// block is a panic and never a misread.
    pub fn carve(bytes: &'a [u8], count: u32, span: u32) -> RunSet<'a> {
        let need = run_set_bytes(count, span) as usize;
        assert!(bytes.len() >= need, "run set of {} bytes shorter than {}", bytes.len(), need);
        assert!((bytes.as_ptr() as usize).is_multiple_of(4), "run set not 4-aligned");
        let base = bytes.as_ptr();
        let n = count as usize;
        let cols = (span * span + 1) as usize;
        let mut at = 0usize;
        // SAFETY: every span below lies inside `bytes` — the length was
        // checked against the layout's total — and the word lanes start
        // 4-aligned, as the base was checked to.
        let mut words = |len: usize| unsafe {
            let p = base.add(at);
            at += len * 4;
            p
        };
        let start = unsafe { core::slice::from_raw_parts(words(cols) as *const u32, cols) };
        let z0 = unsafe { core::slice::from_raw_parts(words(n) as *const i32, n) };
        let z1 = unsafe { core::slice::from_raw_parts(words(n) as *const i32, n) };
        let layer_taps = unsafe { core::slice::from_raw_parts(words(n) as *const u32, n) };
        let mut bytes_at = at;
        let mut lane = || unsafe {
            let s = core::slice::from_raw_parts(base.add(bytes_at), n);
            bytes_at += n;
            s
        };
        RunSet {
            count,
            span,
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
            sub_sx: lane(),
            sub_sy: lane(),
            sub_sz: lane(),
        }
    }
}

/// A run set the mesher builds itself — a coarse tier's — with the same
/// lanes, owned.
#[derive(Default)]
pub struct OwnedRunSet {
    pub span: u32,
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
    pub sub_sx: Vec<u8>,
    pub sub_sy: Vec<u8>,
    pub sub_sz: Vec<u8>,
}

impl OwnedRunSet {
    /// Empties every lane, keeping their capacity.
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
            &mut self.sub_sx,
            &mut self.sub_sy,
            &mut self.sub_sz,
        ] {
            lane.clear();
        }
    }

    pub fn view(&self) -> RunSet<'_> {
        RunSet {
            count: self.z0.len() as u32,
            span: self.span,
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
            sub_sx: &self.sub_sx,
            sub_sy: &self.sub_sy,
            sub_sz: &self.sub_sz,
        }
    }
}
