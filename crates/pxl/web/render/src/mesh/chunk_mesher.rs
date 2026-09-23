// pxl-render/mesh/chunk_mesher.rs

//! A chunk's column runs to one mesh.
//!
//! Three phases, each resumable: face masks, then a greedy merge, then the
//! emit. A coarse tier summarises its cells into a run set of its own first
//! and bakes that. The bake is a cursor the caller steps under a frame
//! budget, so a chunk can take as many frames as it needs without holding
//! the tick.
//!
//! The scratch is plain vectors owned by the context. The mesh itself is
//! written where the caller says: it is asked for a block once the bake
//! knows the mesh's size (`Step::NeedMesh`), and the bake waits at that step
//! for as long as the answer is that there is no room.

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::v3::V3;
use xpute_core::math::scalar::round;

use crate::mesh::volume::{pack_sub_cell_fraction, unpack_sub_cell_fraction, unpack_sub_cell_size, MATERIAL_LAND, MATERIAL_OBJECT, NO_LAYER, SUB_CELL_NONE};
use crate::mesh::voxel_scan::{OwnedRunSet, RunSet, COL_SPAN};

/// The widest span a bake is handed.
const MAX_BAKE_SPAN: usize = COL_SPAN as usize;

const LOW_FILL_THRESHOLD: f64 = 0.25;

const FULL_FACE_CNT: u32 = 6;
const OPEN_BOTTOM_FACE_CNT: u32 = FULL_FACE_CNT - 1;

#[rustfmt::skip]
const FACE_NORMAL: [f64; 18] = [
    1.0, 0.0, 0.0,  -1.0, 0.0, 0.0,
    0.0, 1.0, 0.0,   0.0, -1.0, 0.0,
    0.0, 0.0, 1.0,   0.0, 0.0, -1.0,
];

#[rustfmt::skip]
const FACE_CORNER: [f64; 72] = [
     0.5, -0.5, -0.5,   0.5,  0.5, -0.5,   0.5,  0.5,  0.5,   0.5, -0.5,  0.5,
    -0.5, -0.5, -0.5,  -0.5, -0.5,  0.5,  -0.5,  0.5,  0.5,  -0.5,  0.5, -0.5,
    -0.5,  0.5, -0.5,  -0.5,  0.5,  0.5,   0.5,  0.5,  0.5,   0.5,  0.5, -0.5,
    -0.5, -0.5, -0.5,   0.5, -0.5, -0.5,   0.5, -0.5,  0.5,  -0.5, -0.5,  0.5,
    -0.5, -0.5,  0.5,   0.5, -0.5,  0.5,   0.5,  0.5,  0.5,  -0.5,  0.5,  0.5,
    -0.5, -0.5, -0.5,  -0.5,  0.5, -0.5,   0.5,  0.5, -0.5,   0.5, -0.5, -0.5,
];

pub const FACE_PX: u32 = 1 << 0;
pub const FACE_NX: u32 = 1 << 1;
pub const FACE_PY: u32 = 1 << 2;
pub const FACE_NY: u32 = 1 << 3;
const FACE_PZ: u32 = 1 << 4;
const FACE_NZ: u32 = 1 << 5;
const ALL_FACES: u32 = (1 << 6) - 1;
const OPEN_BOTTOM_FACES: u32 = ALL_FACES & !FACE_NZ;
pub const RISES_PX: u32 = 1 << 6;
pub const RISES_NX: u32 = 1 << 7;
pub const RISES_PY: u32 = 1 << 8;
pub const RISES_NY: u32 = 1 << 9;
pub const DIPS_PX: u32 = 1 << 10;
pub const DIPS_NX: u32 = 1 << 11;
pub const DIPS_PY: u32 = 1 << 12;
pub const DIPS_NY: u32 = 1 << 13;
pub const DIAG_PP: u32 = 1 << 14;
pub const DIAG_PN: u32 = 1 << 15;
pub const DIAG_NP: u32 = 1 << 16;
pub const DIAG_NN: u32 = 1 << 17;
pub const CREASE_PX: u32 = 1 << 18;
pub const CREASE_NX: u32 = 1 << 19;
pub const CREASE_PY: u32 = 1 << 20;
pub const CREASE_NY: u32 = 1 << 21;
const ALL_CREASES: u32 = CREASE_PX | CREASE_NX | CREASE_PY | CREASE_NY;
pub const CAP_PZ: u32 = 1 << 22;
pub const CAP_NZ: u32 = 1 << 23;
/// A diagonal column filled at the cell above: the corner of a top face,
/// where the lateral RISES bits are its four edges. The mask is a u32 lane
/// of the vertex, so bits run to 31.
pub const DIAG_RISES_PP: u32 = 1 << 24;
pub const DIAG_RISES_PN: u32 = 1 << 25;
pub const DIAG_RISES_NP: u32 = 1 << 26;
pub const DIAG_RISES_NN: u32 = 1 << 27;

const X_SEAM_BITS: u32 = FACE_PX | CREASE_PX | RISES_PX | DIPS_PX;
const Y_SEAM_BITS: u32 = FACE_PY | CREASE_PY | RISES_PY | DIPS_PY;
const ALL_DIAGONALS: u32 = DIAG_PP | DIAG_PN | DIAG_NP | DIAG_NN | DIAG_RISES_PP | DIAG_RISES_PN | DIAG_RISES_NP | DIAG_RISES_NN;
const FROM_FAR_BITS: u32 = X_SEAM_BITS | Y_SEAM_BITS | DIAG_PP | DIAG_PN | DIAG_NP | DIAG_RISES_PP | DIAG_RISES_PN | DIAG_RISES_NP;
const X_BORDER_BITS: u32 = FACE_PX | FACE_NX | CREASE_PX | CREASE_NX | RISES_PX | RISES_NX | DIPS_PX | DIPS_NX | ALL_DIAGONALS;
const Y_BORDER_BITS: u32 = FACE_PY | FACE_NY | CREASE_PY | CREASE_NY | RISES_PY | RISES_NY | DIPS_PY | DIPS_NY | ALL_DIAGONALS;

const HIDES_SIDE: u32 = 1;
const RISES_ABOVE: u32 = 2;
const DIPS_BELOW: u32 = 4;
const ABUTS_SIDE: u32 = 8;
const ALL_RELATIONS: u32 = HIDES_SIDE | ABUTS_SIDE | RISES_ABOVE | DIPS_BELOW;

const BAKE_ROWS_PER_STEP: u32 = 8;
const BAKE_BOXES_PER_STEP: usize = 512;

/// f32 words a vertex carries — position, normal, local position, box scale,
/// rgb, four layer taps, the face mask — and the visibility word after them.
pub const F32_PER_VERTEX: u32 = 3 + 3 + 3 + 3 + 3 + 4 + 1;
pub const VIS_F32_PER_VERTEX: u32 = 1;

/// Words a mesh of `vertex_cnt` vertices and `index_cnt` indices takes: the
/// vertices' floats and their visibility float, then the indices.
///
/// One block for every lane: they share a lifetime exactly (the whole mesh
/// dies at once, when its owner does), so one allocation rather than eight —
/// measured: a zoom-out that rebuilds ~2000 chunks moved +344MB in a single
/// frame, 8.4s of it off-thread, when each lane was its own.
pub fn mesh_words(vertex_cnt: u32, index_cnt: u32) -> u32 {
    vertex_cnt * (F32_PER_VERTEX + VIS_F32_PER_VERTEX) + index_cnt
}

/// What bakes have counted, for the ledger. Cumulative.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Counters {
    pub total_runs: u32,
    pub culled: u32,
    pub faces_possible: u32,
    pub faces_kept: u32,
    pub coarse_total: u32,
    pub coarse_low_fill: u32,
}

/// A finished mesh: its counts, in the words the caller provided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mesh {
    pub vertex_cnt: u32,
    pub index_cnt: u32,
}

/// Where a bake stands after a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// More steps to go.
    Working,
    /// The mesh's size is known and it has nowhere to go: `provide_mesh`
    /// this many words, then step on. Returned again by every step until
    /// then.
    NeedMesh(u32),
    Done,
}

/// The words a mesh is written into: a caller's, held as a raw range so the
/// context owes them no lifetime — the caller keeps them valid until the
/// bake finishes or aborts.
#[derive(Clone, Copy)]
struct MeshTarget {
    ptr: *mut u32,
    words: usize,
}

#[derive(Clone, Copy, Default)]
struct ZBucketAccum {
    bz: i32,
    coverage: f64,
    sr: f64,
    sg: f64,
    sb: f64,
    surfaces: u32,
    tag: u32,
    tr: f64,
    tg: f64,
    tb: f64,
    taps: u32,
    tap_cnt: u32,
}

struct FineBake {
    span: u32,
    /// The set's ring, in columns: read for what lies beyond an edge, never
    /// walked and never emitted from (voxel_scan.rs's APRON).
    apron: u32,
    /// Units a column's side and a run's cell are, where the set's own grid
    /// is coarser than a unit: a coarse tier's columns are `lod_step` wide,
    /// and a level-k chunk's 2^k. A run's cell is a unit tall at every level.
    xy_scale: u32,
    z_scale: u32,
    phase: u8,
    row: u32,
    emit_at: usize,
    vertex_total: u32,
    index_total: u32,
}

struct CoarseBake {
    lod_step: u32,
    /// The set's own column, in units across.
    scale: u32,
    res: u32,
    phase: u8,
    row: u32,
    fine: Option<FineBake>,
}

enum Open {
    Fine(FineBake),
    Coarse(CoarseBake),
}

/// One bake at a time, and the scratch every bake reuses.
pub struct BakeContext {
    surviving: Vec<u32>,
    merge_run: Vec<Option<u32>>,
    merge_mask: Vec<u32>,
    buckets: Vec<ZBucketAccum>,
    /// The coarse tier's run set, packed a cell at a time straight into
    /// the run set's own fields; the fine bake then reads it as any set.
    coarse: OwnedRunSet,
    /// Every word a mesh uses is written by the emit before it is read,
    /// except the visibility lane, which the renderer owns — so the target
    /// is never cleared, which for a large mesh would write it twice.
    mesh: Option<MeshTarget>,
    /// Words the open bake's mesh takes, once its size is known.
    mesh_words: u32,
    v_cnt: u32,
    i_cnt: u32,
    open: Option<Open>,
    done: bool,
    pub counters: Counters,
}

impl Default for BakeContext {
    fn default() -> Self {
        Self::new()
    }
}

impl BakeContext {
    pub fn new() -> Self {
        BakeContext {
            surviving: Vec::with_capacity(6 << 12),
            merge_run: vec![None; MAX_BAKE_SPAN * MAX_BAKE_SPAN],
            merge_mask: vec![0; MAX_BAKE_SPAN * MAX_BAKE_SPAN],
            buckets: Vec::with_capacity(1 << 12),
            coarse: OwnedRunSet::default(),
            mesh: None,
            mesh_words: 0,
            v_cnt: 0,
            i_cnt: 0,
            open: None,
            done: false,
            counters: Counters::default(),
        }
    }

    /// Opens a bake of `set` at `lod_step`, the set's columns `scale` units
    /// across: 1 for a leaf chunk, 2^k for a level-k one. Heights are units at
    /// every level. False when one is already open.
    pub fn begin(&mut self, set: &RunSet, lod_step: u32, scale: u32) -> bool {
        if self.open.is_some() {
            return false;
        }
        assert!(set.span as usize <= MAX_BAKE_SPAN, "bake span {} past {}", set.span, MAX_BAKE_SPAN);
        // Every surviving box is at most one run, so the boxes fit this
        // many words: reserved up front rather than grown by doubling, which
        // on a dense chunk held the old block and the new together.
        self.surviving.reserve((set.count as usize) * 6);
        self.done = false;
        self.mesh = None;
        self.mesh_words = 0;
        self.v_cnt = 0;
        self.i_cnt = 0;
        self.open = Some(if lod_step == 1 {
            Open::Fine(self.begin_fine(set.span, set.apron, scale, 1))
        } else {
            // The chunk's own columns, not the apron's: a coarse tier is
            // summarised from them and carries no ring of its own.
            let inner = set.span - 2 * set.apron;
            assert!(lod_step.is_power_of_two() && inner.is_multiple_of(lod_step), "lod step {} does not divide span {}", lod_step, inner);
            Open::Coarse(CoarseBake {
                lod_step,
                scale,
                res: inner / lod_step,
                phase: 0,
                row: 0,
                fine: None,
            })
        });
        true
    }

    /// Advances the open bake by one step over `set`, the set it was opened
    /// with.
    pub fn step(&mut self, set: &RunSet) -> Step {
        if self.done {
            return Step::Done;
        }
        if self.mesh_words > 0 && self.mesh.is_none() {
            return Step::NeedMesh(self.mesh_words);
        }
        let mut open = self.open.take().expect("bake step with no bake open");
        self.done = match &mut open {
            Open::Fine(b) => self.step_fine(b, set),
            Open::Coarse(b) => self.step_coarse(b, set),
        };
        self.open = Some(open);
        if self.done {
            Step::Done
        } else if self.mesh_words > 0 && self.mesh.is_none() {
            Step::NeedMesh(self.mesh_words)
        } else {
            Step::Working
        }
    }

    /// Hands the bake the words its mesh goes into, after `Step::NeedMesh`.
    ///
    /// # Safety
    /// `ptr` must address at least the words asked for, 4-aligned, and stay
    /// valid and otherwise untouched until `finish` or `abort`.
    pub unsafe fn provide_mesh(&mut self, ptr: *mut u32, words: u32) {
        assert!(self.mesh_words > 0 && self.mesh.is_none(), "mesh provided to a bake that did not ask");
        assert!(words >= self.mesh_words, "mesh of {} words provided where {} were asked", words, self.mesh_words);
        self.mesh = Some(MeshTarget { ptr, words: self.mesh_words as usize });
    }

    /// The complete mesh's counts. Its words are the caller's.
    pub fn finish(&mut self) -> Mesh {
        assert!(self.done && self.open.is_some(), "finish on a bake that is not open and complete");
        self.open = None;
        self.mesh = None;
        self.surviving.clear();
        Mesh {
            vertex_cnt: self.v_cnt,
            index_cnt: self.i_cnt,
        }
    }

    pub fn abort(&mut self) {
        self.open = None;
        self.done = false;
        self.mesh = None;
        self.mesh_words = 0;
        self.surviving.clear();
        self.buckets.clear();
    }

    fn begin_fine(&mut self, span: u32, apron: u32, xy_scale: u32, z_scale: u32) -> FineBake {
        self.merge_run[..(span * span) as usize].fill(None);
        FineBake {
            span,
            apron,
            xy_scale,
            z_scale,
            phase: 0,
            row: apron,
            emit_at: 0,
            vertex_total: 0,
            index_total: 0,
        }
    }

    fn step_fine(&mut self, b: &mut FineBake, set: &RunSet) -> bool {
        let span = b.span;
        // The chunk's own columns. The ring outside them is read through
        // run_face_mask's neighbour lookups and is never walked: nothing is
        // emitted from a column that belongs to the next chunk.
        let (lo, hi) = (b.apron, span - b.apron);

        if b.phase == 0 {
            let row_end = hi.min(b.row + BAKE_ROWS_PER_STEP);
            for cy in b.row..row_end {
                for cx in lo..hi {
                    let col = cy * span + cx;
                    let end = set.start[col as usize + 1];
                    let mut mergeable: Option<(u32, u32)> = None;
                    for i in set.start[col as usize]..end {
                        let i = i as usize;
                        self.counters.total_runs += 1;
                        let mask = run_face_mask(set, span, cx, cy, i);
                        self.counters.faces_possible += if run_include_bottom(set, i) { FULL_FACE_CNT } else { OPEN_BOTTOM_FACE_CNT };
                        if mask & ALL_FACES == 0 {
                            self.counters.culled += 1;
                            continue;
                        }
                        if set.sub_sz[i] == SUB_CELL_NONE && mergeable.is_none() {
                            mergeable = Some((i as u32, mask));
                            continue;
                        }
                        self.push_box(cx, cy, i as u32, mask, 1, 1);
                    }
                    if let Some((run, mask)) = mergeable {
                        self.merge_run[col as usize] = Some(run);
                        self.merge_mask[col as usize] = mask;
                    }
                }
            }
            b.row = row_end;
            if row_end >= hi {
                b.phase = 1;
                b.row = lo;
            }
            return false;
        }

        if b.phase == 1 {
            let row_end = hi.min(b.row + BAKE_ROWS_PER_STEP);
            for cy in b.row..row_end {
                for cx in lo..hi {
                    let col = (cy * span + cx) as usize;
                    let Some(run) = self.merge_run[col] else {
                        continue;
                    };
                    let run = run as usize;
                    let span_u = span as usize;
                    let mask = self.merge_mask[col];
                    let hi_u = hi as usize;
                    let mut w = 1usize;
                    while cx as usize + w < hi_u {
                        if self.merge_mask[col + w - 1] & X_SEAM_BITS != 0 {
                            break;
                        }
                        let next = col + w;
                        if !self.merge_run[next].is_some_and(|next_run| runs_match(set, run, next_run as usize)) {
                            break;
                        }
                        if self.merge_mask[next] & !X_BORDER_BITS != mask & !X_BORDER_BITS {
                            break;
                        }
                        w += 1;
                    }
                    let mut h = 1usize;
                    'outer: while cy as usize + h < hi_u {
                        let row = col + h * span_u;
                        for k in 0..w {
                            if self.merge_mask[col + (h - 1) * span_u + k] & Y_SEAM_BITS != 0 {
                                break 'outer;
                            }
                            let cell = row + k;
                            if !self.merge_run[cell].is_some_and(|cell_run| runs_match(set, run, cell_run as usize)) {
                                break 'outer;
                            }
                            if self.merge_mask[cell] & !Y_BORDER_BITS != self.merge_mask[col + k] & !Y_BORDER_BITS {
                                break 'outer;
                            }
                            if self.merge_mask[cell] & Y_SEAM_BITS != self.merge_mask[row] & Y_SEAM_BITS {
                                break 'outer;
                            }
                        }
                        h += 1;
                    }
                    for j in 0..h {
                        self.merge_run[col + j * span_u..col + j * span_u + w].fill(None);
                    }
                    let merged = self.merged_mask(span_u, col, w, h);
                    self.push_box(cx, cy, run as u32, merged, w as u32, h as u32);
                }
            }
            b.row = row_end;
            if row_end < hi {
                return false;
            }
            let (mut vertex_total, mut index_total) = (0u32, 0u32);
            for boxed in self.surviving.as_chunks::<6>().0 {
                let faces = (boxed[3] & ALL_FACES).count_ones();
                self.counters.faces_kept += faces;
                vertex_total += faces * 4;
                index_total += faces * 6;
            }
            b.vertex_total = vertex_total;
            b.index_total = index_total;
            self.mesh_words = mesh_words(vertex_total, index_total);
            self.v_cnt = 0;
            self.i_cnt = 0;
            b.phase = 2;
            b.emit_at = 0;
            return false;
        }

        let unit = b.xy_scale as f64 * U_PER_CELL;
        let z_unit = b.z_scale as f64 * U_PER_CELL;
        // A box is placed by the chunk's own grid; the columns were walked
        // in the padded one.
        let apron = b.apron as f64;
        let emit_end = self.surviving.len().min(b.emit_at + BAKE_BOXES_PER_STEP * 6);
        let mut s_i = b.emit_at;
        while s_i < emit_end {
            let s = &self.surviving[s_i..s_i + 6];
            let (cx, cy, i, mask, w, h) = (s[0] as f64, s[1] as f64, s[2] as usize, s[3], s[4] as f64, s[5] as f64);
            let sub_sz = set.sub_sz[i];
            let depth = (set.z1[i] as f64 + 1.0 - set.z0[i] as f64) * z_unit;
            let (min_x, min_y, min_z, size_x, size_y, size_z);
            if sub_sz == SUB_CELL_NONE {
                min_x = (cx - apron) * unit;
                min_y = (cy - apron) * unit;
                min_z = set.z0[i] as f64 * z_unit;
                size_x = w * unit;
                size_y = h * unit;
                size_z = depth;
            } else {
                min_x = (cx - apron) * unit + unpack_sub_cell_fraction(set.sub_ox[i]) * unit;
                min_y = (cy - apron) * unit + unpack_sub_cell_fraction(set.sub_oy[i]) * unit;
                min_z = set.z0[i] as f64 * z_unit + unpack_sub_cell_fraction(set.sub_oz[i]) * depth;
                size_x = unpack_sub_cell_size(set.sub_sx[i]) * unit;
                size_y = unpack_sub_cell_size(set.sub_sy[i]) * unit;
                size_z = unpack_sub_cell_size(sub_sz) * depth;
            }
            let taps = set.layer_taps[i];
            let color = [SRGB_TO_LINEAR[set.r[i] as usize], SRGB_TO_LINEAR[set.g[i] as usize], SRGB_TO_LINEAR[set.b[i] as usize]];
            let tap_bytes = [taps & 255, (taps >> 8) & 255, (taps >> 16) & 255, (taps >> 24) & 255];
            self.emit_box(b.vertex_total, [min_x, min_y, min_z], [size_x, size_y, size_z], color, tap_bytes, mask);
            s_i += 6;
        }
        b.emit_at = emit_end;
        emit_end >= self.surviving.len()
    }

    fn push_box(&mut self, cx: u32, cy: u32, run: u32, mask: u32, w: u32, h: u32) {
        self.surviving.extend_from_slice(&[cx, cy, run, mask, w, h]);
    }

    fn merged_mask(&self, span: usize, col: usize, w: usize, h: usize) -> u32 {
        let near = self.merge_mask[col];
        if w == 1 && h == 1 {
            return near;
        }
        let far_x = self.merge_mask[col + (w - 1)];
        let far_y = self.merge_mask[col + (h - 1) * span];
        let far_xy = self.merge_mask[col + (h - 1) * span + (w - 1)];
        (near & !FROM_FAR_BITS) | (far_x & (X_SEAM_BITS | DIAG_PN | DIAG_RISES_PN)) | (far_y & (Y_SEAM_BITS | DIAG_NP | DIAG_RISES_NP)) | (far_xy & (DIAG_PP | DIAG_RISES_PP))
    }

    /// Writes one box's faces into the bake's mesh.
    fn emit_box(&mut self, vertex_total: u32, min: V3, size: [f64; 3], rgb: [f64; 3], taps: [u32; 4], faces: u32) {
        let target = self.mesh.expect("emit with no mesh provided");
        // SAFETY: provide_mesh's contract — the range is the caller's, valid
        // and untouched until the bake closes.
        let m = unsafe { core::slice::from_raw_parts_mut(target.ptr, target.words) };
        write_box(m, vertex_total, &mut self.v_cnt, &mut self.i_cnt, min, size, rgb, taps, faces);
    }

    fn step_coarse(&mut self, b: &mut CoarseBake, set: &RunSet) -> bool {
        let (lod_step, res, scale) = (b.lod_step, b.res, b.scale);
        // A coarse cell of what is not ground: as tall as it is wide.
        let bucket = (lod_step * scale) as i32;

        if b.phase == 0 {
            // Each cell is accumulated and packed before the next: a cell's
            // buckets depend on nothing outside it (find_or_create_bucket
            // searches its own range), so the packed runs come out as they
            // would from a pass over every cell's buckets held at once,
            // without holding them: held, a dense chunk's buckets run to
            // megabytes.
            if b.row == 0 {
                self.coarse.clear();
                self.coarse.span = res;
                self.coarse.apron = 0;
            }
            let row_end = res.min(b.row + coarse_rows_per_step(lod_step));
            for by in b.row..row_end {
                for bx in 0..res {
                    let start = self.buckets.len() as u32;
                    let mut cnt = 0u32;
                    let mut ground = GroundAccum::default();
                    for j in 0..lod_step {
                        let cy = by * lod_step + j + set.apron;
                        for k in 0..lod_step {
                            let cx = bx * lod_step + k + set.apron;
                            let col = cy * set.span + cx;
                            let mut top: Option<usize> = None;
                            let end = set.start[col as usize + 1];
                            for i in set.start[col as usize]..end {
                                let i = i as usize;
                                if is_ground(set, i) {
                                    ground.floor = ground.floor.min(set.z0[i]);
                                    top = Some(top.filter(|&t| set.z1[t] >= set.z1[i]).unwrap_or(i));
                                } else {
                                    cnt = self.accumulate_run(start, cnt, bucket, scale, set, i, col + 1);
                                }
                            }
                            if let Some(t) = top {
                                ground.add(set, t);
                            }
                        }
                    }
                    self.coarse.start.push(self.coarse.z0.len() as u32);
                    let ground = self.pack_ground(&ground, lod_step * lod_step);
                    self.pack_coarse_cell(start, cnt, lod_step, bucket, ground);
                    self.buckets.truncate(start as usize);
                }
            }
            b.row = row_end;
            if row_end < res {
                return false;
            }
            self.coarse.start.push(self.coarse.z0.len() as u32);
            b.fine = Some(self.begin_fine(res, 0, lod_step * scale, 1));
            b.phase = 2;
            return false;
        }

        // The coarse set is lifted out of the context for the step, so the
        // fine bake can read it while writing the context's scratch.
        let fine = b.fine.as_mut().expect("coarse bake past packing without its fine bake");
        let coarse = core::mem::take(&mut self.coarse);
        let done = self.step_fine(fine, &coarse.view());
        self.coarse = coarse;
        done
    }

    fn find_or_create_bucket(&mut self, start: u32, cnt: u32, bz: i32) -> u32 {
        for i in start..start + cnt {
            if self.buckets[i as usize].bz == bz {
                return i;
            }
        }
        self.buckets.push(ZBucketAccum { bz, ..ZBucketAccum::default() });
        self.buckets.len() as u32 - 1
    }

    /// Adds what is not ground in one run to the coarse cell's buckets,
    /// `bucket` units tall; a sub-cell run is its share of a cell `cell` units
    /// on a side.
    #[allow(clippy::too_many_arguments)]
    fn accumulate_run(&mut self, start: u32, mut cnt: u32, bucket: i32, cell: u32, set: &RunSet, i: usize, tag: u32) -> u32 {
        let (run_z0, run_z1) = (set.z0[i], set.z1[i]);
        let (run_r, run_g, run_b) = (set.r[i] as f64, set.g[i] as f64, set.b[i] as f64);
        let run_layer = set.layer_taps[i] & 255;
        if set.sub_sz[i] != SUB_CELL_NONE {
            let frac = unpack_sub_cell_size(set.sub_sz[i]);
            let w = frac * frac * frac * cell as f64;
            let idx = self.find_or_create_bucket(start, cnt, run_z0.div_euclid(bucket));
            let acc = &mut self.buckets[idx as usize];
            acc.coverage += w;
            collect_surface(acc, tag, run_r, run_g, run_b);
            collect_layer_tap(acc, run_layer);
            return cnt.max(idx - start + 1);
        }
        for bz in run_z0.div_euclid(bucket)..=run_z1.div_euclid(bucket) {
            let bucket_lo = bz * bucket;
            let bucket_hi = bucket_lo + bucket - 1;
            let overlap = run_z1.min(bucket_hi) - run_z0.max(bucket_lo) + 1;
            let idx = self.find_or_create_bucket(start, cnt, bz);
            cnt = cnt.max(idx - start + 1);
            let acc = &mut self.buckets[idx as usize];
            acc.coverage += overlap as f64;
            collect_surface(acc, tag, run_r, run_g, run_b);
            collect_layer_tap(acc, run_layer);
        }
        cnt
    }

    /// The ground of one coarse cell as one run: from the lowest floor of the
    /// columns under it to the mean of their tops, in units, its top surface
    /// the mean of theirs. A column with no ground counts at that floor, so the
    /// mean is over every column the cell covers and the ground's volume is
    /// kept. The run it wrote, if any, and the top of what it fills.
    fn pack_ground(&mut self, g: &GroundAccum, columns: u32) -> (Option<usize>, i32) {
        if g.cols == 0 {
            return (None, i32::MIN);
        }
        let sum = g.top_sum + (columns - g.cols) as i64 * g.floor as i64;
        let top = round(sum as f64 / columns as f64) as i32;
        if top <= g.floor {
            return (None, g.floor - 1);
        }
        let n = g.cols as f64;
        let co = &mut self.coarse;
        co.z0.push(g.floor);
        co.z1.push(top - 1);
        co.r.push(to_u8(g.sr / n) as u8);
        co.g.push(to_u8(g.sg / n) as u8);
        co.b.push(to_u8(g.sb / n) as u8);
        co.layer_taps.push(packed_taps(g.taps, g.tap_cnt));
        co.material_class.push(MATERIAL_LAND);
        co.sub_sz.push(SUB_CELL_NONE);
        co.sub_sx.push(0);
        co.sub_sy.push(0);
        co.sub_ox.push(0);
        co.sub_oy.push(0);
        co.sub_oz.push(0);
        (Some(co.z0.len() - 1), top - 1)
    }

    /// Packs one coarse cell's buckets bottom to top. A cell's buckets have
    /// distinct heights (find_or_create_bucket), so sorting them by height
    /// visits them lowest first.
    fn pack_coarse_cell(&mut self, start: u32, cnt: u32, lod_step: u32, bucket: i32, (mut open, mut last_top): (Option<usize>, i32)) {
        let step = bucket;
        let cell = start as usize..(start + cnt) as usize;
        self.buckets[cell.clone()].sort_unstable_by_key(|acc| acc.bz);
        // A bucket's volume in coverage's measure: units tall by columns.
        let volume = step as f64 * (lod_step * lod_step) as f64;
        for k in cell {
            let acc = self.buckets[k];
            let bucket_hi = acc.bz * step + step - 1;
            // What the ground already fills is the ground's.
            if bucket_hi <= last_top {
                continue;
            }
            let bucket_lo = (acc.bz * step).max(last_top.saturating_add(1));
            let fill = (acc.coverage / volume).min(1.0);
            self.counters.coarse_total += 1;
            if fill < LOW_FILL_THRESHOLD {
                self.counters.coarse_low_fill += 1;
            }
            let linear = libm::cbrt(fill);
            // A full cell is a plain run; a sparse one is inset, scaled by
            // the cube root of its fill about its centre on every axis.
            let inset = (linear < 1.0).then(|| pack_inset(linear));
            let n = acc.surfaces.max(1) as f64;
            let rgb = [to_u8(acc.sr / n) as u8, to_u8(acc.sg / n) as u8, to_u8(acc.sb / n) as u8];
            let taps = bucket_taps(&acc);
            let co = &mut self.coarse;
            if let Some(o) = open.filter(|_| inset.is_none() && bucket_lo == last_top + 1) {
                if [co.r[o], co.g[o], co.b[o]] == rgb && co.layer_taps[o] == taps && co.sub_sz[o] == SUB_CELL_NONE {
                    co.z1[o] = bucket_hi;
                    last_top = bucket_hi;
                    continue;
                }
            }
            open = Some(co.z0.len());
            co.z0.push(bucket_lo);
            co.z1.push(bucket_hi);
            co.r.push(rgb[0]);
            co.g.push(rgb[1]);
            co.b.push(rgb[2]);
            co.layer_taps.push(taps);
            co.material_class.push(MATERIAL_LAND);
            let (offset, scale) = inset.unwrap_or((0, 0));
            co.sub_sz.push(if inset.is_none() { SUB_CELL_NONE } else { scale.max(1) });
            co.sub_sx.push(scale);
            co.sub_sy.push(scale);
            co.sub_ox.push(offset);
            co.sub_oy.push(offset);
            co.sub_oz.push(offset);
            last_top = bucket_hi;
        }
    }
}

/// Faces and edges of a box standing alone: every face, and every lateral
/// edge a crease (the CREASE bits), so each edge shades as a silhouette.
pub const LONE_BOX_FACES: u32 = 0b11_1111 | (0b1111 << 18);
/// A lone box's vertex and index counts.
pub const LONE_BOX_VERTICES: u32 = 24;
pub const LONE_BOX_INDICES: u32 = 36;

/// Writes one box with nothing around it into a mesh block of
/// mesh_words(LONE_BOX_VERTICES, LONE_BOX_INDICES) words, in the layout a
/// bake writes: what the hover ghost is drawn from, so it is lit, textured
/// and edge-shaded as a placed cube is. `rgb` is sRGB bytes.
pub fn write_lone_box(m: &mut [u32], size: f64, rgb: [u8; 3], layer: u8) {
    let linear = [SRGB_TO_LINEAR[rgb[0] as usize], SRGB_TO_LINEAR[rgb[1] as usize], SRGB_TO_LINEAR[rgb[2] as usize]];
    let (mut v, mut i) = (0, 0);
    write_box(m, LONE_BOX_VERTICES, &mut v, &mut i, [0.0; 3], [size; 3], linear, [layer as u32; 4], LONE_BOX_FACES);
}

/// Writes the faces in `faces` of one box into mesh `m` laid out for
/// `vertex_total` vertices, at the vertex and index counts given, which it
/// advances. Positions are summed in f64 and narrowed once, as a
/// Float32Array store narrows them.
#[allow(clippy::too_many_arguments)]
fn write_box(m: &mut [u32], vertex_total: u32, v_cnt: &mut u32, i_cnt: &mut u32, min: V3, size: [f64; 3], rgb: [f64; 3], taps: [u32; 4], faces: u32) {
    let vt = vertex_total as usize;
    let lane = |k: usize| k * vt;
    let (position, normal, local_pos, box_scale, color, layer_taps, face_lane) = (lane(0), lane(3), lane(6), lane(9), lane(12), lane(15), lane(19));
    let index = lane((F32_PER_VERTEX + VIS_F32_PER_VERTEX) as usize);
    let centre = [min[0] + size[0] * 0.5, min[1] + size[1] * 0.5, min[2] + size[2] * 0.5];
    for f in 0..FULL_FACE_CNT as usize {
        if faces & (1 << f) == 0 {
            continue;
        }
        let n = &FACE_NORMAL[f * 3..f * 3 + 3];
        let base_v = *v_cnt;
        for c in 0..4 {
            let k = (f * 4 + c) * 3;
            let l = &FACE_CORNER[k..k + 3];
            let vi = *v_cnt as usize;
            *v_cnt += 1;
            let (po, lo) = (vi * 3, vi * 4);
            for a in 0..3 {
                m[position + po + a] = f32_word(centre[a] + l[a] * size[a]);
                m[normal + po + a] = f32_word(n[a]);
                m[local_pos + po + a] = f32_word(l[a]);
                m[box_scale + po + a] = f32_word(size[a]);
                m[color + po + a] = f32_word(rgb[a]);
            }
            for (t, tap) in taps.iter().enumerate() {
                m[layer_taps + lo + t] = f32_word(*tap as f64);
            }
            m[face_lane + vi] = faces;
        }
        for d in [0, 1, 2, 0, 2, 3] {
            m[index + *i_cnt as usize] = base_v + d;
            *i_cnt += 1;
        }
    }
}

fn f32_word(v: f64) -> u32 {
    (v as f32).to_bits()
}

/// Whether one whole-cell run of column `col` spans all of z0..=z1.
fn column_spans_range(set: &RunSet, col: u32, z0: i32, z1: i32) -> bool {
    let end = set.start[col as usize + 1] as usize;
    for i in set.start[col as usize] as usize..end {
        if set.z0[i] > z0 {
            return false;
        }
        if set.sub_sz[i] != SUB_CELL_NONE {
            continue;
        }
        if set.z1[i] >= z1 {
            return true;
        }
    }
    false
}

/// Whether a whole-cell run of column `col` covers `z`, and whether one
/// that is not an object does, in one walk.
/// Whether a whole cell of `col` is filled at `z`. A sub-cell run does not
/// count: it leaves the face it would hide partly open.
fn column_covers(set: &RunSet, col: u32, z: i32) -> bool {
    let end = set.start[col as usize + 1] as usize;
    for i in set.start[col as usize] as usize..end {
        if set.z0[i] > z {
            break;
        }
        if set.sub_sz[i] != SUB_CELL_NONE || set.z1[i] < z {
            continue;
        }
        return true;
    }
    false
}

fn neighbour_relation(set: &RunSet, col: u32, z0: i32, z1: i32) -> u32 {
    let above = z1 + 1;
    let below = z0 - 1;
    let mut rel = 0;
    let mut covered_to = z0 - 1;
    let end = set.start[col as usize + 1] as usize;
    for i in set.start[col as usize] as usize..end {
        let r_z0 = set.z0[i];
        if r_z0 > above {
            break;
        }
        if set.sub_sz[i] != SUB_CELL_NONE {
            continue;
        }
        let r_z1 = set.z1[i];
        if r_z0 <= covered_to + 1 && r_z1 > covered_to {
            covered_to = r_z1;
        }
        if covered_to >= z1 {
            rel |= ABUTS_SIDE;
        }
        if r_z0 <= z0 && r_z1 >= z1 && set.material_class[i] != MATERIAL_OBJECT {
            rel |= HIDES_SIDE;
        }
        if r_z0 <= above && r_z1 >= above {
            rel |= RISES_ABOVE;
        }
        if r_z0 <= below && r_z1 >= below {
            rel |= DIPS_BELOW;
        }
        if rel == ALL_RELATIONS {
            break;
        }
    }
    rel
}

fn run_include_bottom(set: &RunSet, i: usize) -> bool {
    set.material_class[i] == MATERIAL_OBJECT
}

fn wants_diagonal(mask: u32, a: u32, b: u32) -> bool {
    (mask & a != 0) != (mask & b != 0)
}

fn side_bits(mut mask: u32, rel: u32, face: u32, crease: u32, rises: u32, dips: u32) -> u32 {
    if rel & HIDES_SIDE != 0 {
        mask &= !face;
    }
    if rel & ABUTS_SIDE == 0 {
        mask |= crease;
    }
    if rel & RISES_ABOVE != 0 {
        mask |= rises;
    }
    if rel & DIPS_BELOW != 0 {
        mask |= dips;
    }
    mask
}

fn run_face_mask(set: &RunSet, span: u32, cx: u32, cy: u32, i: usize) -> u32 {
    let mask_all = if run_include_bottom(set, i) { ALL_FACES } else { OPEN_BOTTOM_FACES };
    if set.sub_sz[i] != SUB_CELL_NONE {
        return mask_all | ALL_CREASES;
    }
    let mut mask = mask_all;
    let col = cy * span + cx;
    let (z0, z1) = (set.z0[i], set.z1[i]);
    // The cell above and the cell below: a filled cell there hides the face
    // and caps it, a placed cube as much as terrain. The face between two
    // stacked cubes is seen by nothing — except the shadow map, which draws
    // both sides of everything: left in the mesh, it shadows the side faces
    // for the fifth of a cell that sun_lit offsets a sample along the normal
    // (shadow.rs's texel), so a stack of cubes striped under every seam on
    // whichever side the sun's azimuth put the offset under the overhang,
    // while merged terrain, which has no interior faces, never did.
    let above = column_covers(set, col, z1 + 1);
    let below = column_covers(set, col, z0 - 1);
    if above {
        mask &= !FACE_PZ;
        mask |= CAP_PZ;
    }
    if below {
        mask &= !FACE_NZ;
        mask |= CAP_NZ;
    }
    if cx + 1 < span {
        mask = side_bits(mask, neighbour_relation(set, col + 1, z0, z1), FACE_PX, CREASE_PX, RISES_PX, DIPS_PX);
    }
    if cx > 0 {
        mask = side_bits(mask, neighbour_relation(set, col - 1, z0, z1), FACE_NX, CREASE_NX, RISES_NX, DIPS_NX);
    }
    if cy + 1 < span {
        mask = side_bits(mask, neighbour_relation(set, col + span, z0, z1), FACE_PY, CREASE_PY, RISES_PY, DIPS_PY);
    }
    if cy > 0 {
        mask = side_bits(mask, neighbour_relation(set, col - span, z0, z1), FACE_NY, CREASE_NY, RISES_NY, DIPS_NY);
    }
    if cx + 1 < span && cy + 1 < span && wants_diagonal(mask, FACE_PX, FACE_PY) && column_spans_range(set, col + span + 1, z0, z1) {
        mask |= DIAG_PP;
    }
    if cx + 1 < span && cy > 0 && wants_diagonal(mask, FACE_PX, FACE_NY) && column_spans_range(set, col - span + 1, z0, z1) {
        mask |= DIAG_PN;
    }
    if cx > 0 && cy + 1 < span && wants_diagonal(mask, FACE_NX, FACE_PY) && column_spans_range(set, col + span - 1, z0, z1) {
        mask |= DIAG_NP;
    }
    if cx > 0 && cy > 0 && wants_diagonal(mask, FACE_NX, FACE_NY) && column_spans_range(set, col - span - 1, z0, z1) {
        mask |= DIAG_NN;
    }
    // A cube touching only at a corner shades the top face there, the way a
    // cube along an edge shades it along that edge. It is its own group of
    // bits rather than the DIAG ones above, which say something else: that
    // a diagonal column spans this run's own height, for the inside corner
    // where a side face meets a wall.
    if cx + 1 < span && cy + 1 < span && column_covers(set, col + span + 1, z1 + 1) {
        mask |= DIAG_RISES_PP;
    }
    if cx + 1 < span && cy > 0 && column_covers(set, col - span + 1, z1 + 1) {
        mask |= DIAG_RISES_PN;
    }
    if cx > 0 && cy + 1 < span && column_covers(set, col + span - 1, z1 + 1) {
        mask |= DIAG_RISES_NP;
    }
    if cx > 0 && cy > 0 && column_covers(set, col - span - 1, z1 + 1) {
        mask |= DIAG_RISES_NN;
    }
    mask
}

fn runs_match(set: &RunSet, a: usize, b: usize) -> bool {
    set.z0[a] == set.z0[b]
        && set.z1[a] == set.z1[b]
        && set.r[a] == set.r[b]
        && set.g[a] == set.g[b]
        && set.b[a] == set.b[b]
        && set.layer_taps[a] == set.layer_taps[b]
        && set.material_class[a] == set.material_class[b]
        && set.sub_sz[a] == SUB_CELL_NONE
        && set.sub_sz[b] == SUB_CELL_NONE
}

fn collect_surface(acc: &mut ZBucketAccum, tag: u32, r: f64, g: f64, b: f64) {
    if acc.tag == tag {
        acc.sr += r - acc.tr;
        acc.sg += g - acc.tg;
        acc.sb += b - acc.tb;
    } else {
        acc.sr += r;
        acc.sg += g;
        acc.sb += b;
        acc.surfaces += 1;
        acc.tag = tag;
    }
    acc.tr = r;
    acc.tg = g;
    acc.tb = b;
}

fn collect_layer_tap(acc: &mut ZBucketAccum, layer_idx: u32) {
    collect_tap(&mut acc.taps, &mut acc.tap_cnt, layer_idx);
}

fn collect_tap(taps: &mut u32, tap_cnt: &mut u32, layer_idx: u32) {
    if layer_idx == NO_LAYER || *tap_cnt >= 4 {
        return;
    }
    for i in 0..*tap_cnt {
        if (*taps >> (i * 8)) & 255 == layer_idx {
            return;
        }
    }
    *taps |= layer_idx << (*tap_cnt * 8);
    *tap_cnt += 1;
}

/// Whether a run is the ground's: a height field, coarsened by its
/// footprint alone. A placed object's, or a sub-cell's, is a volume.
fn is_ground(set: &RunSet, i: usize) -> bool {
    set.material_class[i] != MATERIAL_OBJECT && set.sub_sz[i] == SUB_CELL_NONE
}

/// The ground under one coarse cell: its columns' tops, the lowest floor
/// among them, and their top surfaces.
struct GroundAccum {
    cols: u32,
    top_sum: i64,
    floor: i32,
    sr: f64,
    sg: f64,
    sb: f64,
    taps: u32,
    tap_cnt: u32,
}

impl Default for GroundAccum {
    fn default() -> Self {
        GroundAccum {
            cols: 0,
            top_sum: 0,
            floor: i32::MAX,
            sr: 0.0,
            sg: 0.0,
            sb: 0.0,
            taps: 0,
            tap_cnt: 0,
        }
    }
}

impl GroundAccum {
    /// A column whose ground's top run is `t`.
    fn add(&mut self, set: &RunSet, t: usize) {
        self.cols += 1;
        self.top_sum += set.z1[t] as i64 + 1;
        self.sr += set.r[t] as f64;
        self.sg += set.g[t] as f64;
        self.sb += set.b[t] as f64;
        collect_tap(&mut self.taps, &mut self.tap_cnt, set.layer_taps[t] & 255);
    }
}

fn bucket_taps(acc: &ZBucketAccum) -> u32 {
    packed_taps(acc.taps, acc.tap_cnt)
}

/// Up to four distinct layers, packed a byte each and repeated round to fill
/// the four.
fn packed_taps(taps: u32, tap_cnt: u32) -> u32 {
    if tap_cnt == 0 {
        return NO_LAYER * 0x0101_0101;
    }
    let mut packed = 0;
    for i in 0..4 {
        packed |= ((taps >> ((i % tap_cnt) * 8)) & 255) << (i * 8);
    }
    packed
}

fn to_u8(v: f64) -> u32 {
    round(v).clamp(0.0, 255.0) as u32
}

/// A sparse cell's (inset, scale) as sub-cell fractions: a box of `linear`
/// a side, centred.
fn pack_inset(linear: f64) -> (u8, u8) {
    (pack_sub_cell_fraction((1.0 - linear) * 0.5) as u8, pack_sub_cell_fraction(linear) as u8)
}

fn coarse_rows_per_step(lod_step: u32) -> u32 {
    (BAKE_ROWS_PER_STEP / lod_step).max(1)
}

// ---- sRGB to linear ----
//
// sRGB byte to linear, as a table of 256 values.
//
// Written out rather than computed: `pow` is a transcendental function and
// two libms may disagree in the last bit, which would break the determinism
// a mesh is pinned to across toolchains. Generated once from
// the sRGB transfer, c / 12.92 below 0.04045 and ((c + 0.055) / 1.055)^2.4
// above, each
// written to 17 digits so it parses back to the same bits.

#[rustfmt::skip]
#[allow(clippy::excessive_precision)]
pub const SRGB_TO_LINEAR: [f64; 256] = [
    0.0000000000000000, 0.00030352698354883752, 0.00060705396709767503, 0.00091058095064651249,
    0.0012141079341953501, 0.0015176349177441874, 0.0018211619012930250, 0.0021246888848418626,
    0.0024282158683907001, 0.0027317428519395373, 0.0030352698354883748, 0.0033465357638991608,
    0.0036765073240474359, 0.0040247170184963066, 0.0043914420374102934, 0.0047769534806937292,
    0.0051815167023383860, 0.0056053916242027229, 0.0060488330228570539, 0.0065120907925944752,
    0.0069954101872653869, 0.0074990320432261753, 0.0080231929853849943, 0.0085681256180693069,
    0.0091340587022207872, 0.0097212173202378491, 0.010329823029626936, 0.010960094006488246,
    0.011612245179743885, 0.012286488356915872, 0.012983032342173012, 0.013702083047289686,
    0.014443843596092545, 0.015208514422912709, 0.015996293365509631, 0.016807375752887384,
    0.017641954488384078, 0.018500220128379697, 0.019382360956935723, 0.020288563056652401,
    0.021219010376003555, 0.022173884793387381, 0.023153366178110410, 0.024157632448504756,
    0.025186859627361630, 0.026241221894849898, 0.027320891639074894, 0.028426039504420793,
    0.029556834437808800, 0.030713443732993635, 0.031896033073011532, 0.033104766570885055,
    0.034339806808682170, 0.035601314875020343, 0.036889450401100039, 0.038204371595346502,
    0.039546235276732837, 0.040915196906853191, 0.042311410620809675, 0.043735029256973465,
    0.045186204385675541, 0.046665086336880095, 0.048171824226889419, 0.049706565984127232,
    0.051269458374043238, 0.052860647023180246, 0.054480276442442369, 0.056128490049600091,
    0.057805430191067229, 0.059511238162981199, 0.061246054231617608, 0.063010017653167674,
    0.064803266692905773, 0.066625938643772892, 0.068478169844400166, 0.070360095696595876,
    0.072271850682317479, 0.074213568380149628, 0.076185381481307851, 0.078187421805186327,
    0.080219820314468324, 0.082282707129814794, 0.084376211544148816, 0.086500462036549763,
    0.088655586285772942, 0.090841711183407683, 0.093058962846687451, 0.095307466630964705,
    0.097587347141862457, 0.099898728247113891, 0.10224173308810132, 0.10461648409110419,
    0.10702310297826761, 0.10946171077829933, 0.11193242783690560, 0.11443537382697373,
    0.11697066775851084, 0.11953842798834562, 0.12213877222960187, 0.12477181756095049,
    0.12743768043564743, 0.13013647669036429, 0.13286832155381798, 0.13563332965520566,
    0.13843161503245183, 0.14126329114027164, 0.14412847085805777, 0.14702726649759498,
    0.14995978981060856, 0.15292615199615017, 0.15592646370782740, 0.15896083506088041,
    0.16202937563911099, 0.16513219450166761, 0.16826940018969075, 0.17144110073282259,
    0.17464740365558504, 0.17788841598362912, 0.18116424424986022, 0.18447499450044100,
    0.18782077230067787, 0.19120168274079138, 0.19461783044157580, 0.19806931955994886,
    0.20155625379439707, 0.20507873639031693, 0.20863687014525575, 0.21223075741405523,
    0.21586050011389926, 0.21952619972926921, 0.22322795731680850, 0.22696587351009836,
    0.23074004852434915, 0.23455058216100522, 0.23839757381227100, 0.24228112246555486,
    0.24620132670783548, 0.25015828472995344, 0.25415209433082675, 0.25818285292159582,
    0.26225065752969623, 0.26635560480286247, 0.27049779101306581, 0.27467731206038465,
    0.27889426347681040, 0.28314874042999211, 0.28744083772691748, 0.29177064981753587,
    0.29613827079832111, 0.30054379441577650, 0.30498731406988627, 0.30946892281750854,
    0.31398871337571754, 0.31854677812509186, 0.32314320911295075, 0.32777809805654218,
    0.33245153634617935, 0.33716361504833037, 0.34191442490866092, 0.34670405635502960,
    0.35153259950043936, 0.35640014414594351, 0.36130677978350950, 0.36625259559883949,
    0.37123768047414912, 0.37626212299090650, 0.38132601143253014, 0.38642943378704903,
    0.39157247774972326, 0.39675523072562685, 0.40197777983219579, 0.40724021190173670,
    0.41254261348390375, 0.41788507084813747, 0.42326766998607168, 0.42869049661390662,
    0.43415363617474895, 0.43965717384091879, 0.44520119451622786, 0.45078578283822346,
    0.45641102318040466, 0.46207699965440707, 0.46778379611215898, 0.47353149614800955,
    0.47932018310082680, 0.48514994005607037, 0.49102084984783562, 0.49693299506087041,
    0.50288645803256871, 0.50888132085493376, 0.51491766537652139, 0.52099557320435430,
    0.52711512570581309, 0.53327640401050524, 0.53947948901210718, 0.54572446137018660,
    0.55201140151200012, 0.55834038963426791, 0.56471150570492923, 0.57112482946487308,
    0.57758044042965062, 0.58407841789116410, 0.59061884091933692, 0.59720178836376336,
    0.60382733885533779, 0.61049557080786476, 0.61720656241965111, 0.62396039167507611,
    0.63075713634614683, 0.63759687399403264, 0.64447968197058214, 0.65140563741982416,
    0.65837481727944847, 0.66538729828227205, 0.67244315695768753, 0.67954246963309384,
    0.68668531243531350, 0.69387176129198991, 0.70110189193297312, 0.70837577989168676,
    0.71569350050648073, 0.72305512892196933, 0.73046074009035367, 0.73791040877273084,
    0.74540420954038744, 0.75294221677607787, 0.76052450467529242, 0.76815114724750699,
    0.77582221831742360, 0.78353779152619352, 0.79129794033263023, 0.79910273801440901,
    0.80695225766925160, 0.81484657221610124, 0.82278575439628354, 0.83076987677465464,
    0.83879901174074001, 0.84687323150985805, 0.85499260812423383, 0.86315721345410235,
    0.87136711919879717, 0.87962239688783173, 0.88792311788196632, 0.89626935337426639,
    0.90466117439114957, 0.91309865179341920, 0.92158185627729461, 0.93011085837542373,
    0.93868572845788800, 0.94730653673319987, 0.95597335324928612, 0.96468624789446511,
    0.97344529039841254, 0.98225055033311715, 0.99110209711382979, 1.0000000000000000,];
