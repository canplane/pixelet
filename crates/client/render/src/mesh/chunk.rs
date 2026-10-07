// pxl-render/mesh/chunk.rs

//! A chunk's column runs to one mesh: face masks, a greedy merge, then the
//! emit, each resumable under a frame budget. A coarse tier first summarizes
//! its cells into a run set of its own. The mesh is written through caller
//! blocks of any size (`Step::NeedMesh`, `Step::Full`).

use pxl_spec::space::coord::v3::V3;
use xpute_core::math::scalar::round;

use crate::look::color::{linear, srgb};
use crate::mesh::paint::palette::layer_mean;
use crate::mesh::run::{OwnedRunSet, RunSet, COL_SPAN};
use crate::mesh::word::{pack_sub_cell_fraction, taps_of, unpack_sub_cell_fraction, unpack_sub_cell_size, MaterialClasses, NO_LAYER, SUB_CELL_NONE};
use crate::mesh::{grow, Refused};
use pxl_spec::space::camera::grid::CHUNK_CELLS;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

const LOW_FILL_THRESHOLD: f64 = 0.25;

/// A coarse cell's children one zoom finer.
const CHILDREN: f64 = pxl_spec::space::hierarchy::tree::children(3) as f64;

const FULL_FACE_CNT: u32 = 6;
const OPEN_BOTTOM_FACE_CNT: u32 = FULL_FACE_CNT - 1;

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
/// A coarse box is shrunk about its center and hangs over the ground it stands
/// for, so an underside shaded as an overhang read as a hole in the ground.
pub const UNDER_AS_TOP: u32 = 1 << 28;
/// The mask is a u32 vertex lane, so bits run to 31.
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

/// The fine format: corner and width in u16 steps of its cell, floor and height
/// in u16 mesh steps, sRGB color, four layer taps, face mask.
///
/// The coarse format drops the taps (only cells a leaf's or finer are
/// textured) and holds corner and width in a byte an axis, in eighth cells:
/// a box is whole cells but for a summarized cell, and a chunk draws
/// `CHUNK_CELLS` cells across. 24 and 16 bytes a box, against 84 a vertex once.
pub const FINE_BOX_WORDS: u32 = 6;
pub const COARSE_BOX_WORDS: u32 = 4;

pub const fn box_words(fine: bool) -> u32 {
    if fine {
        FINE_BOX_WORDS
    } else {
        COARSE_BOX_WORDS
    }
}

/// A cell is 256 steps, so a set's widest row fits a u16; the sub-cell
/// fraction's 1/255 is coarser, so nothing is lost. Heights are held against
/// each mesh's own floor and step instead, since a run's z is the world's.
const XY_STEPS: f64 = 256.0;
const _: () = assert!(COL_SPAN as f64 * XY_STEPS <= 65536.0, "a fine box's corner and width live in a u16 each");
const COARSE_XY_STEPS: f64 = 8.0;
const _: () = assert!(CHUNK_CELLS * (COARSE_XY_STEPS as u32) <= 256, "a coarse box's corner and width live in a byte each");
const Z_STEPS: f64 = 65535.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Heights {
    pub floor: f64,
    pub step: f64,
}

impl Default for Heights {
    fn default() -> Self {
        Heights { floor: 0.0, step: 1.0 }
    }
}

impl Heights {
    /// In units, from a set whose cells are `z_unit` tall.
    fn of(set: &RunSet, z_unit: f64) -> Heights {
        let (mut lo, mut hi) = (i32::MAX, i32::MIN);
        for i in 0..set.count as usize {
            lo = lo.min(set.z0[i]);
            hi = hi.max(set.z1[i] + 1);
        }
        if lo > hi {
            return Heights::default();
        }
        let span = (hi - lo) as f64 * z_unit;
        Heights {
            floor: lo as f64 * z_unit,
            step: (span / Z_STEPS).max(f64::MIN_POSITIVE),
        }
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Counters {
    pub total_runs: u32,
    pub culled: u32,
    pub faces_possible: u32,
    pub faces_kept: u32,
    pub coarse_total: u32,
    pub coarse_low_fill: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mesh {
    pub box_cnt: u32,
    pub heights: Heights,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Working,
    /// Returned by every step until `provide_mesh` hands a block of at least one box.
    NeedMesh(u32),
    /// The block is full: the caller takes its boxes and provides the next.
    /// Returned by every step until then.
    Full,
    Done,
    /// The heap refused the workspace: the bake is over, and asked for again later.
    Refused,
}

/// A raw range so the context owes it no lifetime; the caller keeps it valid
/// until the block is full, or the bake finishes or aborts.
#[derive(Clone, Copy)]
struct MeshTarget {
    ptr: *mut u32,
    words: usize,
    first: u32,
}

/// `coverage` (cells not air) sizes the box; `faces` (faces meeting air) weighs
/// each material's color, so a buried cell weighs nothing.
#[derive(Clone, Copy, Default)]
struct ZBucketAccum {
    bz: i32,
    /// Lays a floor under its box.
    ground: bool,
    coverage: f64,
    faces: f64,
    /// Summed in linear light; a bucket with no exposed face falls back to the volume.
    fr: f64,
    fg: f64,
    fb: f64,
    vr: f64,
    vg: f64,
    vb: f64,
}

impl ZBucketAccum {
    fn add(&mut self, volume: f64, faces: f64, rgb: [f64; 3]) {
        self.coverage += volume;
        self.faces += faces;
        self.fr += rgb[0] * faces;
        self.fg += rgb[1] * faces;
        self.fb += rgb[2] * faces;
        self.vr += rgb[0] * volume;
        self.vg += rgb[1] * volume;
        self.vb += rgb[2] * volume;
    }

    fn color(&self) -> [u8; 3] {
        let (sum, weight) = if self.faces > 0.0 {
            ([self.fr, self.fg, self.fb], self.faces)
        } else {
            ([self.vr, self.vg, self.vb], self.coverage.max(f64::MIN_POSITIVE))
        };
        [sum[0] / weight, sum[1] / weight, sum[2] / weight].map(srgb)
    }
}

struct FineBake {
    span: u32,
    /// Read past an edge, never walked or emitted from.
    apron: u32,
    /// A column's width and a run's cell's height, in units.
    unit: f64,
    z_unit: f64,
    phase: u8,
    row: u32,
    emit_at: usize,
}

struct CoarseBake {
    lod_step: u32,
    scale: u32,
    res: u32,
    /// A run's cell's height, in units.
    z_unit: f64,
    phase: u8,
    row: u32,
    fine: Option<FineBake>,
}

enum Open {
    Fine(FineBake),
    Coarse(CoarseBake),
}

pub struct BakeContext {
    surviving: Vec<u32>,
    merge_run: Vec<Option<u32>>,
    merge_mask: Vec<u32>,
    buckets: Vec<ZBucketAccum>,
    coarse: OwnedRunSet,
    /// Held for the whole bake, since every step reads it.
    clipped: Option<OwnedRunSet>,
    /// Every word is written before read, so the target is never cleared.
    mesh: Option<MeshTarget>,
    heights: Heights,
    box_words: u32,
    /// A coarse box carries one color, the tile mean when the pack is drawn or the
    /// swatch when not, so it matches the leaves under it either way.
    textured: bool,
    /// Rows a step bakes, the asker's to set.
    rows: u32,
    boxes: u32,
    mesh_words: u32,
    box_cnt: u32,
    open: Option<Open>,
    done: bool,
    refused: bool,
    pub counters: Counters,
}

impl Default for BakeContext {
    fn default() -> BakeContext {
        BakeContext {
            surviving: Vec::with_capacity(6 << 12),
            merge_run: vec![None; COL_SPAN as usize * COL_SPAN as usize],
            merge_mask: vec![0; COL_SPAN as usize * COL_SPAN as usize],
            buckets: Vec::with_capacity(1 << 12),
            coarse: OwnedRunSet::default(),
            clipped: None,
            mesh: None,
            heights: Heights::default(),
            box_words: FINE_BOX_WORDS,
            textured: true,
            rows: 1,
            boxes: 1,
            mesh_words: 0,
            box_cnt: 0,
            open: None,
            done: false,
            refused: false,
            counters: Counters::default(),
        }
    }
}

impl BakeContext {
    /// `unit` is a column's width and `z_unit` a run's cell's height, in units;
    /// at `lod_step` past 1 the chunk is summarized `lod_step` columns a cell.
    /// `fine` boxes carry texture taps. `clip` cuts a whole-column set to the
    /// chunk's heights, and every step reads the cut set. One bake at a time.
    #[allow(clippy::too_many_arguments)]
    pub fn begin(&mut self, set: &RunSet, lod_step: u32, unit: f64, z_unit: f64, fine: bool, textured: bool, (rows, boxes): (u32, u32), clip: Option<(i32, i32)>) -> Result<(), Refused> {
        xpute_core::ensure!(self.open.is_none(), ENOTRECOVERABLE);
        (self.rows, self.boxes) = (rows, boxes);
        self.refused = false;
        match clip {
            Some((z0, z1)) => {
                let mut cut = self.clipped.take().unwrap_or_default();
                let clipped = cut.clip_from(set, z0, z1);
                self.clipped = Some(cut);
                clipped?;
            }
            None => self.clipped = None,
        }
        let (count, span, apron, heights) = {
            let cut = self.clipped.as_ref().map(OwnedRunSet::view);
            let set = cut.as_ref().unwrap_or(set);
            (set.count, set.span, set.apron, if lod_step == 1 { Heights::of(set, z_unit) } else { Heights::default() })
        };
        self.textured = textured;
        xpute_core::ensure!(span <= COL_SPAN, ENOSPC, span, COL_SPAN);
        // Reserved up front: doubling on a dense chunk held the old block and the new together.
        self.surviving.reserve((count as usize) * 6);
        self.done = false;
        self.mesh = None;
        self.mesh_words = 0;
        self.box_cnt = 0;
        self.heights = heights;
        self.box_words = box_words(fine);
        self.open = Some(if lod_step == 1 {
            Open::Fine(self.begin_fine(span, apron, unit, z_unit))
        } else {
            let scale = round(unit / z_unit) as u32;
            let inner = span - 2 * apron;
            xpute_core::ensure!(lod_step.is_power_of_two() && inner.is_multiple_of(lod_step), EINVAL, lod_step, inner);
            Open::Coarse(CoarseBake {
                lod_step,
                scale,
                res: inner / lod_step,
                z_unit,
                phase: 0,
                row: 0,
                fine: None,
            })
        });
        Ok(())
    }

    pub fn step(&mut self, set: &RunSet) -> Step {
        if self.done {
            return Step::Done;
        }
        if self.mesh_words > 0 && self.mesh.is_none() {
            return Step::NeedMesh(self.mesh_words);
        }
        if self.full() {
            return Step::Full;
        }
        let mut open = self.open.take().or_bug(Errno::ENOTRECOVERABLE);
        let cut = self.clipped.take();
        let view = cut.as_ref().map(OwnedRunSet::view);
        let set = view.as_ref().unwrap_or(set);
        self.done = match &mut open {
            Open::Fine(b) => self.step_fine(b, set),
            Open::Coarse(b) => self.step_coarse(b, set),
        };
        self.clipped = cut;
        self.open = Some(open);
        if self.refused {
            self.abort();
            return Step::Refused;
        }
        if self.done {
            Step::Done
        } else if self.mesh_words > 0 && self.mesh.is_none() {
            Step::NeedMesh(self.mesh_words)
        } else if self.full() {
            Step::Full
        } else {
            Step::Working
        }
    }

    fn full(&self) -> bool {
        self.mesh.is_some_and(|t| (self.box_cnt - t.first) as usize * self.box_words as usize >= t.words)
    }

    pub fn left_words(&self) -> u32 {
        self.mesh_words - self.box_cnt * self.box_words
    }

    pub fn heights(&self) -> Heights {
        self.heights
    }

    /// At least one box, and never more than what is left.
    ///
    /// # Safety
    /// `ptr` must address `words` words, 4-aligned, and stay valid and
    /// otherwise untouched until the block is full, or `finish` or `abort`.
    pub unsafe fn provide_mesh(&mut self, ptr: *mut u32, words: u32) {
        xpute_core::ensure!(self.mesh_words > 0 && (self.mesh.is_none() || self.full()), ENOTRECOVERABLE);
        let left = self.left_words();
        xpute_core::ensure!(words >= self.box_words.min(left) && words <= left, ENOTRECOVERABLE, words, left);
        let words = words - words % self.box_words;
        self.mesh = Some(MeshTarget {
            ptr,
            words: words as usize,
            first: self.box_cnt,
        });
    }

    pub fn finish(&mut self) -> Mesh {
        xpute_core::ensure!(self.done && self.open.is_some(), ENOTRECOVERABLE);
        self.open = None;
        self.mesh = None;
        self.surviving.clear();
        Mesh {
            box_cnt: self.box_cnt,
            heights: self.heights,
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

    fn begin_fine(&mut self, span: u32, apron: u32, unit: f64, z_unit: f64) -> FineBake {
        self.merge_run[..(span * span) as usize].fill(None);
        FineBake {
            span,
            apron,
            unit,
            z_unit,
            phase: 0,
            row: apron,
            emit_at: 0,
        }
    }

    fn step_fine(&mut self, b: &mut FineBake, set: &RunSet) -> bool {
        let span = b.span;
        let (lo, hi) = (b.apron, span - b.apron);
        // Rim sides are drawn whatever the apron holds, down to the floor: the chunk
        // beside may be another zoom's standing lower, and hiding the side left the step
        // between them open to the sky.
        let rim = |cx: u32, cy: u32| {
            if b.apron == 0 {
                return 0;
            }
            let x = if cx + 1 == hi {
                FACE_PX
            } else if cx == lo {
                FACE_NX
            } else {
                0
            };
            let y = if cy + 1 == hi {
                FACE_PY
            } else if cy == lo {
                FACE_NY
            } else {
                0
            };
            x | y
        };

        if b.phase == 0 {
            let row_end = hi.min(b.row + self.rows);
            for cy in b.row..row_end {
                for cx in lo..hi {
                    let col = cy * span + cx;
                    let end = set.start[col as usize + 1];
                    let mut mergeable: Option<(u32, u32)> = None;
                    for i in set.start[col as usize]..end {
                        let i = i as usize;
                        self.counters.total_runs += 1;
                        let faces = run_faces(set, span, cx, cy, i);
                        // A run with no height shows its top alone.
                        let mask = if set.z1[i] >= set.z0[i] { faces | rim(cx, cy) } else { faces & !(ALL_FACES & !FACE_PZ) };
                        self.counters.faces_possible += if run_include_bottom(set, col as usize, i) { FULL_FACE_CNT } else { OPEN_BOTTOM_FACE_CNT };
                        if mask & ALL_FACES == 0 {
                            self.counters.culled += 1;
                            continue;
                        }
                        if set.sub_s[i] == SUB_CELL_NONE && mergeable.is_none() && hung_floor(set, cx, cy, i).is_none() {
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
            let row_end = hi.min(b.row + self.rows);
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
            let mut box_total = 0u32;
            for boxed in self.surviving.as_chunks::<6>().0 {
                let faces = (boxed[3] & ALL_FACES).count_ones();
                self.counters.faces_kept += faces;
                box_total += (faces > 0) as u32;
            }
            // One block for the whole mesh: one a lane moved hundreds of MB in a frame.
            self.mesh_words = box_total * self.box_words;
            self.box_cnt = 0;
            b.phase = 2;
            b.emit_at = 0;
            return false;
        }

        let (unit, z_unit) = (b.unit, b.z_unit);
        // Boxes without taps show the layer's mean where the pack is drawn, as the boxes with them do.
        let mean = self.box_words != FINE_BOX_WORDS && self.textured;
        let apron = b.apron as f64;
        let emit_end = self.surviving.len().min(b.emit_at + self.boxes as usize * 6);
        let mut s_i = b.emit_at;
        while s_i < emit_end {
            let s = &self.surviving[s_i..s_i + 6];
            let (cx, cy, i, mask, w, h) = (s[0] as f64, s[1] as f64, s[2] as usize, s[3], s[4] as f64, s[5] as f64);
            let sub_s = set.sub_s[i];
            let depth = (set.z1[i] as f64 + 1.0 - set.z0[i] as f64) * z_unit;
            let (min_x, min_y, min_z, size_x, size_y, size_z);
            if sub_s == SUB_CELL_NONE {
                let z0 = hung_floor(set, s[0], s[1], i).unwrap_or(set.z0[i]);
                min_x = (cx - apron) * unit;
                min_y = (cy - apron) * unit;
                min_z = z0 as f64 * z_unit;
                size_x = w * unit;
                size_y = h * unit;
                size_z = (set.z1[i] as f64 + 1.0 - z0 as f64) * z_unit;
            } else {
                min_x = (cx - apron) * unit + unpack_sub_cell_fraction(set.sub_ox[i]) * unit;
                min_y = (cy - apron) * unit + unpack_sub_cell_fraction(set.sub_oy[i]) * unit;
                min_z = set.z0[i] as f64 * z_unit + unpack_sub_cell_fraction(set.sub_oz[i]) * depth;
                let side = unpack_sub_cell_size(sub_s);
                size_x = side * unit;
                size_y = side * unit;
                size_z = side * depth;
            }
            if mask & ALL_FACES != 0 {
                if self.full() {
                    b.emit_at = s_i;
                    return false;
                }
                let swatch = [set.r[i], set.g[i], set.b[i]];
                let rgb = if mean { layer_mean(set.layer_taps[i] & 255, swatch) } else { swatch };
                self.emit_box([min_x, min_y, min_z], [size_x, size_y, size_z], rgb, set.layer_taps[i], mask, unit);
            }
            s_i += 6;
        }
        b.emit_at = emit_end;
        emit_end >= self.surviving.len()
    }

    fn push_box(&mut self, cx: u32, cy: u32, run: u32, mask: u32, w: u32, h: u32) {
        let grown = grow(&mut self.surviving, 6);
        if self.room(grown) {
            self.surviving.extend_from_slice(&[cx, cy, run, mask, w, h]);
        }
    }

    /// Where growth had no room the bake is refused and the step's output discarded.
    fn room(&mut self, grown: Result<(), Refused>) -> bool {
        self.refused |= grown.is_err();
        grown.is_ok()
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

    #[allow(clippy::too_many_arguments)]
    fn emit_box(&mut self, min: V3, size: [f64; 3], rgb: [u8; 3], taps: u32, faces: u32, unit: f64) {
        let target = self.mesh.or_bug(Errno::ENOTRECOVERABLE);
        // SAFETY: provide_mesh's contract; the range is valid and untouched until the bake closes.
        let m = unsafe { core::slice::from_raw_parts_mut(target.ptr, target.words) };
        let words = self.box_words as usize;
        let at = (self.box_cnt - target.first) as usize * words;
        let heights = self.heights;
        write_box(&mut m[at..at + words], min, size, rgb, taps, faces, unit, heights);
        self.box_cnt += 1;
    }

    fn step_coarse(&mut self, b: &mut CoarseBake, set: &RunSet) -> bool {
        let (lod_step, res, scale) = (b.lod_step, b.res, b.scale);
        let bucket = (lod_step * scale) as i32;

        if b.phase == 0 {
            // Each cell is packed before the next (its buckets depend on nothing outside
            // it); holding every cell's buckets at once ran to megabytes on a dense chunk.
            if b.row == 0 {
                self.coarse.clear();
                self.coarse.span = res;
                self.coarse.apron = 0;
            }
            let row_end = res.min(b.row + (self.rows / lod_step).max(1));
            for by in b.row..row_end {
                for bx in 0..res {
                    let start = self.buckets.len() as u32;
                    let mut cnt = 0u32;
                    for j in 0..lod_step {
                        let cy = by * lod_step + j + set.apron;
                        for k in 0..lod_step {
                            let cx = bx * lod_step + k + set.apron;
                            let col = cy * set.span + cx;
                            // Ground and object alike: a coarse cell is what is not air.
                            let end = set.start[col as usize + 1];
                            for i in set.start[col as usize]..end {
                                cnt = self.accumulate_run(start, cnt, bucket, scale, set, i as usize, col);
                            }
                        }
                    }
                    let grown = grow(&mut self.coarse.start, 1);
                    if !self.room(grown) {
                        return false;
                    }
                    self.coarse.start.push(self.coarse.z0.len() as u32);
                    let mut low: Option<i32> = None;
                    for j in 0..lod_step {
                        for k in 0..lod_step {
                            let (cx, cy) = (bx * lod_step + k + set.apron, by * lod_step + j + set.apron);
                            low = match (low, edge_low(set, cx, cy)) {
                                (Some(a), Some(b)) => Some(a.min(b)),
                                (a, b) => a.or(b),
                            };
                        }
                    }
                    self.pack_coarse_cell(start, cnt, lod_step, bucket, low);
                    self.buckets.truncate(start as usize);
                }
            }
            b.row = row_end;
            if row_end < res {
                return false;
            }
            let grown = grow(&mut self.coarse.start, 1);
            if !self.room(grown) {
                return false;
            }
            self.coarse.start.push(self.coarse.z0.len() as u32);
            self.heights = Heights::of(&self.coarse.view(), b.z_unit);
            b.fine = Some(self.begin_fine(res, 0, (lod_step * scale) as f64 * b.z_unit, b.z_unit));
            b.phase = 2;
            return false;
        }

        let fine = b.fine.as_mut().or_bug(Errno::ENOSPC);
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
        // The run goes into the last bucket, which nothing reads, once refused.
        let grown = grow(&mut self.buckets, 1);
        if !self.room(grown) {
            return self.buckets.len() as u32 - 1;
        }
        self.buckets.push(ZBucketAccum { bz, ..ZBucketAccum::default() });
        self.buckets.len() as u32 - 1
    }

    #[allow(clippy::too_many_arguments)]
    fn accumulate_run(&mut self, start: u32, mut cnt: u32, bucket: i32, cell: u32, set: &RunSet, i: usize, col: u32) -> u32 {
        let (mut run_z0, run_z1) = (set.z0[i], set.z1[i]);
        // Ground is solid under its surface: the column stops at the chunk's floor
        // only to save runs, and read from there a coarse sea cell was a thousandth full.
        if i == set.start[col as usize] as usize && set.material_class[i] != MaterialClasses::OBJECT {
            run_z0 = run_z0.div_euclid(bucket) * bucket;
        }
        // With the pack drawn a far patch shows its layer's mean color; with it off,
        // the leaves show the swatch, so the summary does too.
        let swatch = [set.r[i], set.g[i], set.b[i]];
        let mean = if self.textured { layer_mean(set.layer_taps[i] & 255, swatch) } else { swatch };
        // A mean of colors belongs in linear light.
        let rgb = mean.map(linear);
        if set.sub_s[i] != SUB_CELL_NONE {
            let frac = unpack_sub_cell_size(set.sub_s[i]);
            // A sub-cell thing: volume the fraction cubed, each face the fraction squared, none hidden.
            let idx = self.find_or_create_bucket(start, cnt, run_z0.div_euclid(bucket));
            let acc = &mut self.buckets[idx as usize];
            acc.ground |= set.material_class[i] != MaterialClasses::OBJECT;
            acc.add(frac * frac * frac * cell as f64, FULL_FACE_CNT as f64 * frac * frac, rgb);
            return cnt.max(idx - start + 1);
        }
        for bz in run_z0.div_euclid(bucket)..=run_z1.div_euclid(bucket) {
            let bucket_lo = bz * bucket;
            let bucket_hi = bucket_lo + bucket - 1;
            let (lo, hi) = (run_z0.max(bucket_lo), run_z1.min(bucket_hi));
            let idx = self.find_or_create_bucket(start, cnt, bz);
            cnt = cnt.max(idx - start + 1);
            let acc = &mut self.buckets[idx as usize];
            acc.ground |= set.material_class[i] != MaterialClasses::OBJECT;
            acc.add((hi - lo + 1) as f64, exposed_faces(set, col, i, lo, hi), rgb);
        }
        cnt
    }

    /// Buckets have distinct heights, so sorting visits them lowest first.
    /// `edge_low` is the lowest ground across the chunk's edge, which the skirt hangs to.
    fn pack_coarse_cell(&mut self, start: u32, cnt: u32, lod_step: u32, step: i32, edge_low: Option<i32>) {
        let grown = self.coarse.grow_runs(2 * cnt as usize);
        if !self.room(grown) {
            return;
        }
        let cell = start as usize..(start + cnt) as usize;
        self.buckets[cell.clone()].sort_unstable_by_key(|acc| acc.bz);
        let volume = step as f64 * (lod_step * lod_step) as f64;
        let (mut open, mut last_top) = (None, i32::MIN);
        for k in cell {
            let acc = self.buckets[k];
            let bucket_hi = acc.bz * step + step - 1;
            let bucket_lo = acc.bz * step;
            let fill = (acc.coverage / volume).min(1.0);
            self.counters.coarse_total += 1;
            if fill < LOW_FILL_THRESHOLD {
                self.counters.coarse_low_fill += 1;
            }
            // Kept in its children's worth, rounded down, a heuristic: a trace too
            // small to fill one child draws no box, where it floated up to a
            // kilometer over a z5 cell's ground.
            let kept = (fill * CHILDREN).floor() / CHILDREN;
            if kept == 0.0 && !acc.ground {
                continue;
            }
            let linear = libm::cbrt(kept);
            // A sparse cell is inset by the cube root of its fill on every axis.
            let inset = (linear < 1.0).then(|| pack_inset(linear));
            let rgb = acc.color();
            let taps = taps_of(NO_LAYER as u8);
            // A shrunk ground box leaves gaps that read as holes from above, so a ground
            // cell lays its full bottom face as a floor. The lowest box also hangs a skirt
            // a cell down, or a step between neighbors' floors showed daylight.
            let lowest = open.is_none() && last_top == i32::MIN;
            let floored = last_top == bucket_lo - 1 && open.is_some_and(|o| self.coarse.sub_s[o] == SUB_CELL_NONE);
            let under = if !acc.ground || floored {
                None
            } else if lowest {
                Some(((bucket_lo - step).min(edge_low.map_or(i32::MAX, |low| low + 1)), bucket_lo - 1))
            } else if inset.is_some() {
                Some((bucket_lo, bucket_lo - 1))
            } else {
                None
            };
            if let Some((z0, z1)) = under {
                let co = &mut self.coarse;
                co.z0.push(z0);
                co.z1.push(z1);
                co.r.push(rgb[0]);
                co.g.push(rgb[1]);
                co.b.push(rgb[2]);
                co.layer_taps.push(taps);
                co.material_class.push(MaterialClasses::LAND);
                co.sub_s.push(SUB_CELL_NONE);
                for lane in [&mut co.sub_ox, &mut co.sub_oy, &mut co.sub_oz] {
                    lane.push(0);
                }
                open = None;
                last_top = z1;
            }
            if kept == 0.0 {
                continue;
            }
            // A placed-things box hides none of its neighbors' faces and shows its underside.
            let class = if acc.ground { MaterialClasses::LAND } else { MaterialClasses::OBJECT };
            let co = &mut self.coarse;
            if let Some(o) = open.filter(|_| inset.is_none() && bucket_lo == last_top + 1) {
                if [co.r[o], co.g[o], co.b[o]] == rgb && co.layer_taps[o] == taps && co.sub_s[o] == SUB_CELL_NONE && co.material_class[o] == class {
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
            co.material_class.push(class);
            let (offset, scale) = inset.unwrap_or((0, 0));
            co.sub_s.push(if inset.is_none() { SUB_CELL_NONE } else { scale.max(1) });
            co.sub_ox.push(offset);
            co.sub_oy.push(offset);
            co.sub_oz.push(offset);
            last_top = bucket_hi;
        }
    }
}

/// Every lateral edge a crease, so each shades as a silhouette.
const LONE_BOX_FACES: u32 = ALL_FACES | ALL_CREASES;
/// The hover ghost's box, written as a bake writes one so it is lit as a placed cube is.
pub fn write_lone_box(m: &mut [u32], size: f64, rgb: [u8; 3], layer: u8) -> Heights {
    xpute_core::ensure!(m.len() == FINE_BOX_WORDS as usize, ENOTRECOVERABLE);
    let heights = Heights { floor: 0.0, step: size / Z_STEPS };
    write_box(m, [0.0; 3], [size; 3], rgb, taps_of(layer), LONE_BOX_FACES, size, heights);
    heights
}

#[allow(clippy::too_many_arguments)]
fn write_box(m: &mut [u32], min: V3, size: [f64; 3], rgb: [u8; 3], taps: u32, faces: u32, unit: f64, heights: Heights) {
    let z = steps(min[2] - heights.floor, heights.step) | (steps(size[2], heights.step) << 16);
    if m.len() == COARSE_BOX_WORDS as usize {
        let xy = unit / COARSE_XY_STEPS;
        // The byte holds `width − 1`: zero width would be no box, and a whole mesh across fits.
        let wide = |v: f64| steps(v, xy).clamp(1, 256) - 1;
        m[0] = faces;
        m[1] = u32::from_le_bytes([rgb[0], rgb[1], rgb[2], 0]);
        m[2] = z;
        m[3] = steps(min[0], xy).min(255) | (steps(min[1], xy).min(255) << 8) | (wide(size[0]) << 16) | (wide(size[1]) << 24);
        return;
    }
    let xy = unit / XY_STEPS;
    m[0] = steps(min[0], xy) | (steps(min[1], xy) << 16);
    m[1] = steps(size[0], xy) | (steps(size[1], xy) << 16);
    m[2] = z;
    m[3] = u32::from_le_bytes([rgb[0], rgb[1], rgb[2], 0]);
    m[4] = taps;
    m[5] = faces;
}

/// A wall down to the chunk's floor alone left the plane open below a lower
/// neighbor (a cliff, or a coarser chunk read at its cell center).
fn edge_low(set: &RunSet, cx: u32, cy: u32) -> Option<i32> {
    let (span, apron) = (set.span, set.apron);
    if apron == 0 {
        return None;
    }
    let (lo, hi) = (apron, span - apron);
    let top = |x: u32, y: u32| {
        let col = (y * span + x) as usize;
        (set.start[col]..set.start[col + 1]).map(|i| set.z1[i as usize]).max()
    };
    [
        (cx + 1 == hi).then(|| top(cx + 1, cy)),
        (cx == lo).then(|| top(cx - 1, cy)),
        (cy + 1 == hi).then(|| top(cx, cy + 1)),
        (cy == lo).then(|| top(cx, cy - 1)),
    ]
    .into_iter()
    .flatten()
    .flatten()
    .min()
}

fn hung_floor(set: &RunSet, cx: u32, cy: u32, run: usize) -> Option<i32> {
    let col = (cy * set.span + cx) as usize;
    if set.start[col] as usize != run || set.sub_s[run] != SUB_CELL_NONE {
        return None;
    }
    edge_low(set, cx, cy).map(|low| low + 1).filter(|&z0| z0 < set.z0[run])
}

fn steps(v: f64, step: f64) -> u32 {
    round(v / step).clamp(0.0, u16::MAX as f64) as u32
}

pub fn box_bounds(m: &[u32], unit: f64, heights: Heights) -> (V3, [f64; 3]) {
    let (lo, span) = box_z(m);
    let z0 = heights.floor + lo as f64 * heights.step;
    let dz = span as f64 * heights.step;
    if m.len() == COARSE_BOX_WORDS as usize {
        let xy = unit / COARSE_XY_STEPS;
        let byte = |k: u32| ((m[3] >> k) & 0xff) as f64;
        return ([byte(0) * xy, byte(8) * xy, z0], [(byte(16) + 1.0) * xy, (byte(24) + 1.0) * xy, dz]);
    }
    let xy = unit / XY_STEPS;
    let half = |w: u32, high: bool| (if high { w >> 16 } else { w & 0xffff }) as f64 * xy;
    ([half(m[0], false), half(m[0], true), z0], [half(m[1], false), half(m[1], true), dz])
}

fn box_z(m: &[u32]) -> (u32, u32) {
    (m[2] & 0xffff, m[2] >> 16)
}

fn column_spans_range(set: &RunSet, col: u32, z0: i32, z1: i32) -> bool {
    let end = set.start[col as usize + 1] as usize;
    for i in set.start[col as usize] as usize..end {
        if set.z0[i] > z0 {
            return false;
        }
        if set.sub_s[i] != SUB_CELL_NONE {
            continue;
        }
        if set.z1[i] >= z1 {
            return true;
        }
    }
    false
}

/// A sub-cell run does not count: it leaves the face it would hide partly open.
fn column_covers(set: &RunSet, col: u32, z: i32) -> bool {
    let end = set.start[col as usize + 1] as usize;
    for i in set.start[col as usize] as usize..end {
        if set.z0[i] > z {
            break;
        }
        if set.sub_s[i] != SUB_CELL_NONE || set.z1[i] < z {
            continue;
        }
        return true;
    }
    false
}

fn neighbor_relation(set: &RunSet, col: u32, z0: i32, z1: i32) -> u32 {
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
        if set.sub_s[i] != SUB_CELL_NONE {
            continue;
        }
        let r_z1 = set.z1[i];
        if r_z0 <= covered_to + 1 && r_z1 > covered_to {
            covered_to = r_z1;
        }
        if covered_to >= z1 {
            rel |= ABUTS_SIDE;
        }
        if r_z0 <= z0 && r_z1 >= z1 && set.material_class[i] != MaterialClasses::OBJECT {
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

/// Only a column's lowest run rests on the ground; any run over it, a deck's
/// slab or a placed cube, can be seen from below. A solid cell under it
/// still hides the face (`run_faces`).
fn run_include_bottom(set: &RunSet, col: usize, i: usize) -> bool {
    i != set.start[col] as usize
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

fn run_faces(set: &RunSet, span: u32, cx: u32, cy: u32, i: usize) -> u32 {
    let col = cy * span + cx;
    let mask_all = if run_include_bottom(set, col as usize, i) { ALL_FACES } else { OPEN_BOTTOM_FACES };
    if set.sub_s[i] != SUB_CELL_NONE {
        // A coarse box is the ground itself shrunk, so its underside is more ground.
        let under = if set.material_class[i] == MaterialClasses::OBJECT { 0 } else { UNDER_AS_TOP };
        return mask_all | ALL_CREASES | under;
    }
    let mut mask = mask_all;
    let (z0, z1) = (set.z0[i], set.z1[i]);
    // A filled cell above or below hides the face. Left in, the face between
    // stacked cubes striped them in the shadow map, which draws both sides.
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
        mask = side_bits(mask, neighbor_relation(set, col + 1, z0, z1), FACE_PX, CREASE_PX, RISES_PX, DIPS_PX);
    }
    if cx > 0 {
        mask = side_bits(mask, neighbor_relation(set, col - 1, z0, z1), FACE_NX, CREASE_NX, RISES_NX, DIPS_NX);
    }
    if cy + 1 < span {
        mask = side_bits(mask, neighbor_relation(set, col + span, z0, z1), FACE_PY, CREASE_PY, RISES_PY, DIPS_PY);
    }
    if cy > 0 {
        mask = side_bits(mask, neighbor_relation(set, col - span, z0, z1), FACE_NY, CREASE_NY, RISES_NY, DIPS_NY);
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
    // A corner-only neighbor shades the top face there; its own bits, since the
    // DIAG bits mean a diagonal column spanning this run's height.
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
        && set.sub_s[a] == SUB_CELL_NONE
        && set.sub_s[b] == SUB_CELL_NONE
}

/// A column past the apron counts as air, which is rare enough to leave generous.
fn exposed_faces(set: &RunSet, col: u32, i: usize, lo: i32, hi: i32) -> f64 {
    let height = (hi - lo + 1) as f64;
    let mut faces = 0.0;
    if set.z1[i] <= hi && !column_covers(set, col, hi + 1) {
        faces += 1.0;
    }
    if set.z0[i] >= lo && !column_covers(set, col, lo - 1) {
        faces += 1.0;
    }
    let span = set.span;
    let (cx, cy) = (col % span, col / span);
    let sides = [
        (cx + 1 < span).then_some(col + 1),
        (cx > 0).then(|| col - 1),
        (cy + 1 < span).then(|| col + span),
        (cy > 0).then(|| col - span),
    ];
    for side in sides {
        faces += match side {
            Some(n) => height - covered_height(set, n, lo, hi) as f64,
            None => height,
        };
    }
    faces
}

/// Runs of one column do not overlap, so their overlaps add up.
fn covered_height(set: &RunSet, col: u32, lo: i32, hi: i32) -> i32 {
    let mut covered = 0;
    let end = set.start[col as usize + 1] as usize;
    for i in set.start[col as usize] as usize..end {
        if set.z0[i] > hi {
            break;
        }
        if set.sub_s[i] != SUB_CELL_NONE {
            continue;
        }
        covered += (set.z1[i].min(hi) - set.z0[i].max(lo) + 1).max(0);
    }
    covered
}

fn pack_inset(linear: f64) -> (u8, u8) {
    (pack_sub_cell_fraction((1.0 - linear) * 0.5) as u8, pack_sub_cell_fraction(linear) as u8)
}

#[cfg(test)]
#[path = "chunk.test.rs"]
mod test;
