//! A chunk's column runs to one mesh: render/layer/pipeline/chunk_mesher.ts,
//! in Rust.
//!
//! The same phases in the same steps — face masks, then greedy merge, then
//! emit; a coarse tier first summarises its cells into a run set of its own
//! and bakes that — so a cursor stepped under the frame budget advances as
//! far per step as the TypeScript one does, and the mesh comes out byte for
//! byte the same. Why each mask bit, merge rule and coarse quirk is what it
//! is lives in the TypeScript, which stays the reference until it is gone.
//!
//! What the port does not carry over: the scratch is plain vectors owned by
//! the context rather than lanes carved from the core memory. The mesh is
//! written where the caller says — a block from the mesh section, in the
//! TypeScript's layout — which the caller provides once the bake knows the
//! mesh's size (`Step::NeedMesh`) and may fail to, as a pool refuses; the
//! bake then waits at that step, as the TypeScript one does.

use crate::fdlibm;
use crate::run_set::{OwnedRunSet, RunSet};
use crate::srgb::SRGB_TO_LINEAR;
use crate::voxel::{pack_sub_cell_fraction, unpack_sub_cell_fraction, unpack_sub_cell_size, MATERIAL_LAND, MATERIAL_OBJECT, NO_LAYER, SUB_CELL_NONE};

/// Cells to a block, and the widest span a bake is handed.
const U_PER_CELL: f64 = 1.0;
const MAX_BAKE_SPAN: usize = 64;

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

const FACE_PX: u32 = 1 << 0;
const FACE_NX: u32 = 1 << 1;
const FACE_PY: u32 = 1 << 2;
const FACE_NY: u32 = 1 << 3;
const FACE_PZ: u32 = 1 << 4;
const FACE_NZ: u32 = 1 << 5;
const ALL_FACES: u32 = (1 << 6) - 1;
const OPEN_BOTTOM_FACES: u32 = ALL_FACES & !FACE_NZ;
const RISES_PX: u32 = 1 << 6;
const RISES_NX: u32 = 1 << 7;
const RISES_PY: u32 = 1 << 8;
const RISES_NY: u32 = 1 << 9;
const DIPS_PX: u32 = 1 << 10;
const DIPS_NX: u32 = 1 << 11;
const DIPS_PY: u32 = 1 << 12;
const DIPS_NY: u32 = 1 << 13;
const DIAG_PP: u32 = 1 << 14;
const DIAG_PN: u32 = 1 << 15;
const DIAG_NP: u32 = 1 << 16;
const DIAG_NN: u32 = 1 << 17;
const CREASE_PX: u32 = 1 << 18;
const CREASE_NX: u32 = 1 << 19;
const CREASE_PY: u32 = 1 << 20;
const CREASE_NY: u32 = 1 << 21;
const ALL_CREASES: u32 = CREASE_PX | CREASE_NX | CREASE_PY | CREASE_NY;
const CAP_PZ: u32 = 1 << 22;
const CAP_NZ: u32 = 1 << 23;

const X_SEAM_BITS: u32 = FACE_PX | CREASE_PX | RISES_PX | DIPS_PX;
const Y_SEAM_BITS: u32 = FACE_PY | CREASE_PY | RISES_PY | DIPS_PY;
const FROM_FAR_BITS: u32 = X_SEAM_BITS | Y_SEAM_BITS | DIAG_PP | DIAG_PN | DIAG_NP;
const X_BORDER_BITS: u32 = FACE_PX | FACE_NX | CREASE_PX | CREASE_NX | RISES_PX | RISES_NX | DIPS_PX | DIPS_NX | DIAG_PP | DIAG_PN | DIAG_NP | DIAG_NN;
const Y_BORDER_BITS: u32 = FACE_PY | FACE_NY | CREASE_PY | CREASE_NY | RISES_PY | RISES_NY | DIPS_PY | DIPS_NY | DIAG_PP | DIAG_PN | DIAG_NP | DIAG_NN;

const HIDES_SIDE: u32 = 1;
const RISES_ABOVE: u32 = 2;
const DIPS_BELOW: u32 = 4;
const ABUTS_SIDE: u32 = 8;
const ALL_RELATIONS: u32 = HIDES_SIDE | ABUTS_SIDE | RISES_ABOVE | DIPS_BELOW;

const INSET_SET: u32 = 1 << 16;

const BAKE_ROWS_PER_STEP: u32 = 8;
const BAKE_BOXES_PER_STEP: usize = 512;

/// f32 words a vertex carries — position, normal, local position, box scale,
/// rgb, four layer taps, the face mask — and the visibility word after them.
pub const F32_PER_VERTEX: u32 = 3 + 3 + 3 + 3 + 3 + 4 + 1;
pub const VIS_F32_PER_VERTEX: u32 = 1;

/// Words a mesh of `vertex_cnt` vertices and `index_cnt` indices takes, in
/// chunk_mesher.ts#carve_baked_mesh's layout.
pub fn mesh_words(vertex_cnt: u32, index_cnt: u32) -> u32 {
    vertex_cnt * (F32_PER_VERTEX + VIS_F32_PER_VERTEX) + index_cnt
}

/// What bakes have counted, for the ledger. Cumulative, like the context's
/// in the TypeScript.
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

/// The coarse tier's runs while they are packed, one lane per field.
#[derive(Default)]
struct CoarseLanes {
    z0: Vec<i32>,
    z1: Vec<i32>,
    rgb: Vec<u32>,
    layer_taps: Vec<u32>,
    sub: Vec<u32>,
}

impl CoarseLanes {
    fn clear(&mut self) {
        self.z0.clear();
        self.z1.clear();
        self.rgb.clear();
        self.layer_taps.clear();
        self.sub.clear();
    }
}

struct FineBake {
    span: u32,
    xy_scale: u32,
    phase: u8,
    row: u32,
    emit_at: usize,
    vertex_total: u32,
    index_total: u32,
}

struct CoarseBake {
    lod_step: u32,
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
    cell_range: Vec<u32>,
    co: CoarseLanes,
    co_start: Vec<u32>,
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
            cell_range: Vec::with_capacity(2 << 10),
            co: CoarseLanes::default(),
            co_start: vec![0; MAX_BAKE_SPAN * MAX_BAKE_SPAN + 1],
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

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Opens a bake of `set` at `lod_step`. False when one is already open.
    pub fn begin(&mut self, set: &RunSet, lod_step: u32) -> bool {
        if self.open.is_some() {
            return false;
        }
        assert!(set.span as usize <= MAX_BAKE_SPAN, "bake span {} past {}", set.span, MAX_BAKE_SPAN);
        self.done = false;
        self.mesh = None;
        self.mesh_words = 0;
        self.v_cnt = 0;
        self.i_cnt = 0;
        self.open = Some(if lod_step == 1 {
            Open::Fine(self.begin_fine(set.span, 1))
        } else {
            assert!(
                lod_step.is_power_of_two() && set.span.is_multiple_of(lod_step),
                "lod step {} does not divide span {}",
                lod_step,
                set.span
            );
            Open::Coarse(CoarseBake {
                lod_step,
                res: set.span / lod_step,
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
        self.cell_range.clear();
        self.buckets.clear();
    }

    fn begin_fine(&mut self, span: u32, xy_scale: u32) -> FineBake {
        self.merge_run[..(span * span) as usize].fill(None);
        FineBake {
            span,
            xy_scale,
            phase: 0,
            row: 0,
            emit_at: 0,
            vertex_total: 0,
            index_total: 0,
        }
    }

    fn step_fine(&mut self, b: &mut FineBake, set: &RunSet) -> bool {
        let span = b.span;

        if b.phase == 0 {
            let row_end = span.min(b.row + BAKE_ROWS_PER_STEP);
            for cy in b.row..row_end {
                for cx in 0..span {
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
            if row_end >= span {
                b.phase = 1;
                b.row = 0;
            }
            return false;
        }

        if b.phase == 1 {
            let row_end = span.min(b.row + BAKE_ROWS_PER_STEP);
            for cy in b.row..row_end {
                for cx in 0..span {
                    let col = (cy * span + cx) as usize;
                    let Some(run) = self.merge_run[col] else {
                        continue;
                    };
                    let run = run as usize;
                    let span_u = span as usize;
                    let mask = self.merge_mask[col];
                    let mut w = 1usize;
                    while cx as usize + w < span_u {
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
                    'outer: while cy as usize + h < span_u {
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
            if row_end < span {
                return false;
            }
            let (mut vertex_total, mut index_total) = (0u32, 0u32);
            for boxed in self.surviving.chunks_exact(6) {
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
        let emit_end = self.surviving.len().min(b.emit_at + BAKE_BOXES_PER_STEP * 6);
        let mut s_i = b.emit_at;
        while s_i < emit_end {
            let s = &self.surviving[s_i..s_i + 6];
            let (cx, cy, i, mask, w, h) = (s[0] as f64, s[1] as f64, s[2] as usize, s[3], s[4] as f64, s[5] as f64);
            let sub_sz = set.sub_sz[i];
            let depth = (set.z1[i] as f64 + 1.0 - set.z0[i] as f64) * U_PER_CELL;
            let (min_x, min_y, min_z, size_x, size_y, size_z);
            if sub_sz == SUB_CELL_NONE {
                min_x = cx * unit;
                min_y = cy * unit;
                min_z = set.z0[i] as f64 * U_PER_CELL;
                size_x = w * unit;
                size_y = h * unit;
                size_z = depth;
            } else {
                min_x = cx * unit + unpack_sub_cell_fraction(set.sub_ox[i]) * unit;
                min_y = cy * unit + unpack_sub_cell_fraction(set.sub_oy[i]) * unit;
                min_z = set.z0[i] as f64 * U_PER_CELL + unpack_sub_cell_fraction(set.sub_oz[i]) * depth;
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
        (near & !FROM_FAR_BITS) | (far_x & (X_SEAM_BITS | DIAG_PN)) | (far_y & (Y_SEAM_BITS | DIAG_NP)) | (far_xy & DIAG_PP)
    }

    /// Writes one box's faces. Positions are summed in f64 and narrowed once,
    /// as a Float32Array store narrows them.
    fn emit_box(&mut self, vertex_total: u32, min: [f64; 3], size: [f64; 3], rgb: [f64; 3], taps: [u32; 4], faces: u32) {
        let vt = vertex_total as usize;
        let lane = |k: usize| k * vt;
        let (position, normal, local_pos, box_scale, color, layer_taps, face_lane) = (lane(0), lane(3), lane(6), lane(9), lane(12), lane(15), lane(19));
        let index = lane((F32_PER_VERTEX + VIS_F32_PER_VERTEX) as usize);
        let centre = [min[0] + size[0] * 0.5, min[1] + size[1] * 0.5, min[2] + size[2] * 0.5];
        let target = self.mesh.expect("emit with no mesh provided");
        // SAFETY: provide_mesh's contract — the range is the caller's, valid
        // and untouched until the bake closes.
        let m = unsafe { core::slice::from_raw_parts_mut(target.ptr, target.words) };
        for f in 0..FULL_FACE_CNT as usize {
            if faces & (1 << f) == 0 {
                continue;
            }
            let n = &FACE_NORMAL[f * 3..f * 3 + 3];
            let base_v = self.v_cnt;
            for c in 0..4 {
                let k = (f * 4 + c) * 3;
                let l = &FACE_CORNER[k..k + 3];
                let vi = self.v_cnt as usize;
                self.v_cnt += 1;
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
                m[face_lane + vi] = f32_word(faces as f64);
            }
            for d in [0, 1, 2, 0, 2, 3] {
                m[index + self.i_cnt as usize] = base_v + d;
                self.i_cnt += 1;
            }
        }
    }

    fn step_coarse(&mut self, b: &mut CoarseBake, set: &RunSet) -> bool {
        let (lod_step, res) = (b.lod_step, b.res);

        if b.phase == 0 {
            let row_end = res.min(b.row + coarse_rows_per_step(lod_step));
            for by in b.row..row_end {
                for bx in 0..res {
                    let start = self.buckets.len() as u32;
                    let mut cnt = 0u32;
                    for j in 0..lod_step {
                        let cy = by * lod_step + j;
                        for k in 0..lod_step {
                            let cx = bx * lod_step + k;
                            let col = cy * set.span + cx;
                            let end = set.start[col as usize + 1];
                            for i in set.start[col as usize]..end {
                                cnt = self.accumulate_run(start, cnt, lod_step, set, i as usize, col + 1);
                            }
                        }
                    }
                    self.cell_range.push(start);
                    self.cell_range.push(cnt);
                }
            }
            b.row = row_end;
            if row_end >= res {
                b.phase = 1;
                b.row = 0;
                self.co.clear();
            }
            return false;
        }

        if b.phase == 1 {
            let row_end = res.min(b.row + coarse_rows_per_step(lod_step));
            for c in (b.row * res)..(row_end * res) {
                self.co_start[c as usize] = self.co.z0.len() as u32;
                let ri = c as usize * 2;
                self.pack_coarse_cell(self.cell_range[ri], self.cell_range[ri + 1], lod_step);
            }
            b.row = row_end;
            if row_end < res {
                return false;
            }
            let len = self.co.z0.len();
            self.co_start[(res * res) as usize] = len as u32;
            self.fill_coarse_set(res);
            self.cell_range.clear();
            self.buckets.clear();
            b.fine = Some(self.begin_fine(res, lod_step));
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

    /// Copies the packed lanes into the coarse tier's run set, in the run
    /// set's own byte fields.
    fn fill_coarse_set(&mut self, res: u32) {
        let (co, out) = (&self.co, &mut self.coarse);
        let len = co.z0.len();
        out.span = res;
        out.start.clear();
        out.start.extend_from_slice(&self.co_start[..(res * res + 1) as usize]);
        out.z0.clone_from(&co.z0);
        out.z1.clone_from(&co.z1);
        out.layer_taps.clone_from(&co.layer_taps);
        for lane in [
            &mut out.r,
            &mut out.g,
            &mut out.b,
            &mut out.material_class,
            &mut out.sub_ox,
            &mut out.sub_oy,
            &mut out.sub_oz,
            &mut out.sub_sx,
            &mut out.sub_sy,
            &mut out.sub_sz,
        ] {
            lane.clear();
            lane.resize(len, 0);
        }
        for i in 0..len {
            let rgb = co.rgb[i];
            out.r[i] = (rgb >> 16) as u8;
            out.g[i] = (rgb >> 8) as u8;
            out.b[i] = rgb as u8;
            out.material_class[i] = MATERIAL_LAND;
            let sub = co.sub[i];
            let (inset, scale) = (sub as u8, (sub >> 8) as u8);
            out.sub_sz[i] = if sub & INSET_SET == 0 { SUB_CELL_NONE } else { scale.max(1) };
            out.sub_sx[i] = scale;
            out.sub_sy[i] = scale;
            out.sub_ox[i] = inset;
            out.sub_oy[i] = inset;
            out.sub_oz[i] = inset;
        }
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

    fn accumulate_run(&mut self, start: u32, mut cnt: u32, lod_step: u32, set: &RunSet, i: usize, tag: u32) -> u32 {
        let (run_z0, run_z1) = (set.z0[i], set.z1[i]);
        let (run_r, run_g, run_b) = (set.r[i] as f64, set.g[i] as f64, set.b[i] as f64);
        let run_layer = set.layer_taps[i] & 255;
        let step = lod_step as i32;
        if set.sub_sz[i] != SUB_CELL_NONE {
            let frac = unpack_sub_cell_size(set.sub_sz[i]);
            let w = frac * frac * frac;
            let idx = self.find_or_create_bucket(start, cnt, run_z0.div_euclid(step));
            let acc = &mut self.buckets[idx as usize];
            acc.coverage += w;
            collect_surface(acc, tag, run_r, run_g, run_b);
            collect_layer_tap(acc, run_layer);
            return cnt.max(idx - start + 1);
        }
        for bz in run_z0.div_euclid(step)..=run_z1.div_euclid(step) {
            let bucket_lo = bz * step;
            let bucket_hi = bucket_lo + step - 1;
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

    /// Packs one coarse cell's buckets bottom to top. A cell's buckets have
    /// distinct heights (find_or_create_bucket), so sorting them by height
    /// visits them in the order the TypeScript's repeated minimum search did.
    fn pack_coarse_cell(&mut self, start: u32, cnt: u32, lod_step: u32) {
        let step = lod_step as i32;
        let cell = start as usize..(start + cnt) as usize;
        self.buckets[cell.clone()].sort_unstable_by_key(|acc| acc.bz);
        let mut open: Option<usize> = None;
        let mut last_top = 0i32;
        for k in cell {
            let acc = self.buckets[k];
            let bucket_lo = acc.bz * step;
            let bucket_hi = bucket_lo + step - 1;
            let cube = lod_step as f64;
            let fill = (acc.coverage / (cube * cube * cube)).min(1.0);
            self.counters.coarse_total += 1;
            if fill < LOW_FILL_THRESHOLD {
                self.counters.coarse_low_fill += 1;
            }
            let linear = fdlibm::cbrt(fill);
            let sub = if linear >= 1.0 { 0 } else { pack_inset(linear) };
            let n = acc.surfaces.max(1) as f64;
            let rgb = (to_u8(acc.sr / n) << 16) | (to_u8(acc.sg / n) << 8) | to_u8(acc.sb / n);
            let taps = bucket_taps(&acc);
            let co = &mut self.co;
            if let Some(o) = open.filter(|_| sub & INSET_SET == 0 && bucket_lo == last_top + 1) {
                if co.rgb[o] == rgb && co.layer_taps[o] == taps && co.sub[o] & INSET_SET == 0 {
                    co.z1[o] = bucket_hi;
                    last_top = bucket_hi;
                    continue;
                }
            }
            open = Some(co.z0.len());
            co.z0.push(bucket_lo);
            co.z1.push(bucket_hi);
            co.rgb.push(rgb);
            co.layer_taps.push(taps);
            co.sub.push(sub);
            last_top = bucket_hi;
        }
    }
}

fn f32_word(v: f64) -> u32 {
    (v as f32).to_bits()
}

fn column_spans_range(set: &RunSet, col: u32, z0: i32, z1: i32, skip_objects: bool) -> bool {
    let end = set.start[col as usize + 1] as usize;
    for i in set.start[col as usize] as usize..end {
        if set.z0[i] > z0 {
            return false;
        }
        if set.sub_sz[i] != SUB_CELL_NONE {
            continue;
        }
        if skip_objects && set.material_class[i] == MATERIAL_OBJECT {
            continue;
        }
        if set.z1[i] >= z1 {
            return true;
        }
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
    if column_spans_range(set, col, z1 + 1, z1 + 1, true) {
        mask &= !FACE_PZ;
    }
    if mask & FACE_NZ != 0 && column_spans_range(set, col, z0 - 1, z0 - 1, true) {
        mask &= !FACE_NZ;
    }
    if column_spans_range(set, col, z1 + 1, z1 + 1, false) {
        mask |= CAP_PZ;
    }
    if column_spans_range(set, col, z0 - 1, z0 - 1, false) {
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
    if cx + 1 < span && cy + 1 < span && wants_diagonal(mask, FACE_PX, FACE_PY) && column_spans_range(set, col + span + 1, z0, z1, false) {
        mask |= DIAG_PP;
    }
    if cx + 1 < span && cy > 0 && wants_diagonal(mask, FACE_PX, FACE_NY) && column_spans_range(set, col - span + 1, z0, z1, false) {
        mask |= DIAG_PN;
    }
    if cx > 0 && cy + 1 < span && wants_diagonal(mask, FACE_NX, FACE_PY) && column_spans_range(set, col + span - 1, z0, z1, false) {
        mask |= DIAG_NP;
    }
    if cx > 0 && cy > 0 && wants_diagonal(mask, FACE_NX, FACE_NY) && column_spans_range(set, col - span - 1, z0, z1, false) {
        mask |= DIAG_NN;
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
    if layer_idx == NO_LAYER || acc.tap_cnt >= 4 {
        return;
    }
    for i in 0..acc.tap_cnt {
        if (acc.taps >> (i * 8)) & 255 == layer_idx {
            return;
        }
    }
    acc.taps |= layer_idx << (acc.tap_cnt * 8);
    acc.tap_cnt += 1;
}

fn bucket_taps(acc: &ZBucketAccum) -> u32 {
    if acc.tap_cnt == 0 {
        return NO_LAYER * 0x0101_0101;
    }
    let mut packed = 0;
    for i in 0..4 {
        packed |= ((acc.taps >> ((i % acc.tap_cnt) * 8)) & 255) << (i * 8);
    }
    packed
}

/// JavaScript's `Math.round` and Rust's `round` differ only on negative
/// ties, and the averages rounded here are never negative.
fn to_u8(v: f64) -> u32 {
    v.round().clamp(0.0, 255.0) as u32
}

fn pack_inset(linear: f64) -> u32 {
    pack_sub_cell_fraction((1.0 - linear) * 0.5) | (pack_sub_cell_fraction(linear) << 8) | INSET_SET
}

fn coarse_rows_per_step(lod_step: u32) -> u32 {
    (BAKE_ROWS_PER_STEP / lod_step).max(1)
}
