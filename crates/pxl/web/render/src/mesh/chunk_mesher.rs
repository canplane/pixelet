// pxl-render/mesh/chunk_mesher.rs

//! A chunk's column runs to one mesh.
//!
//! Three phases, each resumable: face masks, then a greedy merge, then the
//! emit. A coarse tier summarizes its cells into a run set of its own first
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

use crate::mesh::paint::palette::shader_meta::layer_mean;
use crate::mesh::volume::{pack_sub_cell_fraction, unpack_sub_cell_fraction, unpack_sub_cell_size, MATERIAL_LAND, MATERIAL_OBJECT, NO_LAYER, SUB_CELL_NONE};
use crate::mesh::voxel_scan::{OwnedRunSet, RunSet, COL_SPAN};
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

/// The widest span a bake is handed.
const MAX_BAKE_SPAN: usize = COL_SPAN as usize;

const LOW_FILL_THRESHOLD: f64 = 0.25;

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
/// The underside is the same surface as the top, and is lit as one: a coarse
/// cell's box is shrunk about its center and hangs over the rest of the
/// ground it stands for, so an underside shaded as an overhang reads as a
/// hole in the ground rather than as ground seen from below.
pub const UNDER_AS_TOP: u32 = 1 << 28;
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

/// Words a box takes in a mesh, of the two formats a box has: chunk.wgsl
/// reads whichever the draw names, and unfolds a cube from it — a mesh is its
/// boxes and nothing per vertex.
///
/// **A leaf's own tier (`FINE_BOX_WORDS`)** holds its corner and its width
/// across x and y, a u16 each in steps of its cell (XY_STEPS); its floor and
/// its height, a u16 each in the mesh's own steps (Heights); its color, sRGB
/// bytes; its four layer taps, a byte each; its face mask.
///
/// **Every tier over it (`COARSE_BOX_WORDS`)** drops two of those six. The
/// taps go because a box that merges cells is its color alone — the texture
/// pack is for a leaf's cells, where a top and a side mean something. The
/// corner and the width shrink to a byte an axis because of how the ladder
/// draws: a level-`k` chunk is never drawn nearer than `192·2^k`
/// (pxl-render look/lod.rs, kernel world/pyramid/domain.rs), where a finer
/// level covers it, so its cell is between one and two TARGET_CELL_PX however
/// the camera moves — under the reference criterion, which a view over its
/// budget loosens, and its cells grow on screen with it. An eighth of that cell is at most one and a half pixels
/// and its mesh is at most 32 cells across, so a cell index and an eighth
/// land in a byte exactly. The leaf's tier keeps the whole u16, because it
/// is drawn from arm's length, where its cell is hundreds of pixels.
///
/// A box as six f32 was 36 bytes; the leaf's is 24 and every other 16.
/// Before that a box was vertices — a face's four corners each carrying
/// position, normal, local position, the box's scale, color, taps and the
/// mask, 84 bytes a vertex and an index list besides, over a kilobyte for a
/// box of three faces. Every one of those numbers was the box's, repeated.
pub const FINE_BOX_WORDS: u32 = 6;
pub const COARSE_BOX_WORDS: u32 = 4;

/// The words of the format a bake of `lod_step` writes.
pub const fn box_words(lod_step: u32) -> u32 {
    if lod_step == 1 {
        FINE_BOX_WORDS
    } else {
        COARSE_BOX_WORDS
    }
}

/// Steps a box's own cell is cut into across x and y, which is what its
/// corner and its width are held in: a mesh is the arena's largest tenant by
/// far (world/chunk/mesh_slot.rs), and a box that held six floats spent
/// twenty-four bytes saying what sixteen bits an axis say exactly. The
/// mesher's own resolution across a cell is the sub-cell fraction's 1/255
/// (mesh/volume.rs), so nothing is lost; a chunk is 64 cells, which is
/// 16,384 steps, and that is the whole u16.
///
/// Height is held the same way but against the mesh itself. A chunk is not
/// divided in z — its key is `(page, bx, by)` — so a run's z is the world's,
/// in units, and what a mesh spans there is whatever relief its columns
/// have. Each mesh therefore carries its own floor and its own step, and a
/// box's height is in those: the floor rides on the draw's translation,
/// which had nothing in z, and the step beside it.
const XY_STEPS: f64 = 256.0;
/// The same for a box that merges cells, where a byte holds the cell and the
/// step together (COARSE_BOX_WORDS).
const COARSE_XY_STEPS: f64 = 8.0;
/// Cells a coarse mesh is across at its widest: a chunk's 64 over the
/// smallest step that merges them.
const COARSE_RES_MAX: u32 = 64 / 2;
const _: () = assert!(COARSE_RES_MAX * (COARSE_XY_STEPS as u32) <= 256, "a coarse box's corner and width live in a byte each");
/// Steps a mesh's own height is cut into: the whole u16.
const Z_STEPS: f64 = 65535.0;

/// The floor and the step a mesh's heights are held in: what `Mesh` carries
/// and a draw is given.
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
    /// The heights a set spans, in units: its lowest run's floor, and the
    /// step that puts its whole span in a u16.
    fn of(set: &RunSet, z_scale: f64) -> Heights {
        let (mut lo, mut hi) = (i32::MAX, i32::MIN);
        for i in 0..set.count as usize {
            lo = lo.min(set.z0[i]);
            hi = hi.max(set.z1[i] + 1);
        }
        if lo > hi {
            return Heights::default();
        }
        let span = (hi - lo) as f64 * z_scale;
        Heights {
            floor: lo as f64 * z_scale,
            step: (span / Z_STEPS).max(f64::MIN_POSITIVE),
        }
    }
}

/// Words a mesh of `box_cnt` boxes takes.
///
/// One block for the whole mesh: it has one lifetime (the whole mesh dies at
/// once, when its owner does), so one allocation — measured: a zoom-out that
/// rebuilds ~2000 chunks moved +344MB in a single frame, 8.4s of it
/// off-thread, when each lane of the old vertices was its own.
pub fn mesh_words(box_cnt: u32, words: u32) -> u32 {
    box_cnt * words
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
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mesh {
    pub box_cnt: u32,
    /// What its boxes' heights are held in (Heights).
    pub heights: Heights,
    /// Words a box of it takes (FINE_BOX_WORDS).
    pub box_words: u32,
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

/// What one coarse cell holds between two heights, as it is counted.
///
/// Two measures, both in the set's own cells — a column's footprint by a
/// unit. `coverage` is how much of the bucket is not air, which sizes the
/// box; `faces` is how many of those cells' six faces meet air, which
/// colors it. A cell buried on every side weighs nothing, so flat ground
/// takes the color of its top and a wall the color of its side.
#[derive(Clone, Copy, Default)]
struct ZBucketAccum {
    bz: i32,
    /// Whether any of it is ground rather than a placed thing: what lays a
    /// floor under its box.
    ground: bool,
    coverage: f64,
    faces: f64,
    /// The exposed faces' color and the whole volume's, both summed in
    /// linear light: a bucket with no face at all falls back to the volume.
    fr: f64,
    fg: f64,
    fb: f64,
    vr: f64,
    vg: f64,
    vb: f64,
}

impl ZBucketAccum {
    /// `volume` cells of it are not air, `faces` of their faces meet it, and
    /// they are `rgb` in linear light.
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

    /// The color the box shows on every face: the exposed surface's mean,
    /// or the volume's where nothing of the bucket faces the air — which
    /// happens once its neighbors have shrunk away from it.
    fn color(&self) -> [u8; 3] {
        let (sum, weight) = if self.faces > 0.0 {
            ([self.fr, self.fg, self.fb], self.faces)
        } else {
            ([self.vr, self.vg, self.vb], self.coverage.max(f64::MIN_POSITIVE))
        };
        srgb_of([sum[0] / weight, sum[1] / weight, sum[2] / weight])
    }
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
    /// The set the open bake reads where it was opened on a chunk's heights
    /// (`begin`'s `clip`): what it was handed, cut to them, and held until
    /// the bake is done, since every step reads it.
    clipped: Option<OwnedRunSet>,
    /// Every word a mesh uses is written by the emit before it is read,
    /// except the visibility lane, which the renderer owns — so the target
    /// is never cleared, which for a large mesh would write it twice.
    mesh: Option<MeshTarget>,
    /// What the open bake's heights are held in, from the set it reads.
    heights: Heights,
    box_words: u32,
    /// Whether the pack is drawn, which decides what a summary is drawn in:
    /// the tile's own mean where it is, the catalog's swatch where it is not,
    /// so a coarse tier matches the leaves under it either way. A box carries
    /// one color and the bake picks it, rather than carrying both for the
    /// frame to pick — a coarse box has no room for a second and the setting
    /// re-sources every chunk when it changes (shell/setting/store.rs).
    textured: bool,
    /// Words the open bake's mesh takes, once its size is known.
    mesh_words: u32,
    /// Boxes written so far.
    box_cnt: u32,
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
            clipped: None,
            mesh: None,
            heights: Heights::default(),
            box_words: FINE_BOX_WORDS,
            textured: true,
            mesh_words: 0,
            box_cnt: 0,
            open: None,
            done: false,
            counters: Counters::default(),
        }
    }

    /// Opens a bake of `set` at `lod_step`, the set's columns `scale` units
    /// across: 1 for a leaf chunk, 2^k for a level-k one. Heights are units at
    /// every level. `textured` is whether the pack is drawn. `clip`, the
    /// heights a chunk spans where its set is its whole column's, cuts the
    /// set to them (`OwnedRunSet::clip_from`), and every step of the bake
    /// reads the cut set in place of the one it is handed. False when one is
    /// already open.
    pub fn begin(&mut self, set: &RunSet, lod_step: u32, scale: u32, textured: bool, clip: Option<(i32, i32)>) -> bool {
        if self.open.is_some() {
            return false;
        }
        match clip {
            Some((z0, z1)) => {
                let mut cut = self.clipped.take().unwrap_or_default();
                cut.clip_from(set, z0, z1);
                self.clipped = Some(cut);
            }
            None => self.clipped = None,
        }
        let (count, span, apron, heights) = {
            let cut = self.clipped.as_ref().map(OwnedRunSet::view);
            let set = cut.as_ref().unwrap_or(set);
            // A coarse bake summarizes the set into one of its own and takes
            // its heights from that, once it is packed (step_coarse).
            (set.count, set.span, set.apron, if lod_step == 1 { Heights::of(set, 1.0) } else { Heights::default() })
        };
        self.textured = textured;
        xpute_core::ensure!(span as usize <= MAX_BAKE_SPAN, ENOSPC, span, MAX_BAKE_SPAN);
        // Every surviving box is at most one run, so the boxes fit this
        // many words: reserved up front rather than grown by doubling, which
        // on a dense chunk held the old block and the new together.
        self.surviving.reserve((count as usize) * 6);
        self.done = false;
        self.mesh = None;
        self.mesh_words = 0;
        self.box_cnt = 0;
        self.heights = heights;
        self.box_words = box_words(lod_step);
        self.open = Some(if lod_step == 1 {
            Open::Fine(self.begin_fine(span, apron, scale, 1))
        } else {
            // The chunk's own columns, not the apron's: a coarse tier is
            // summarized from them and carries no ring of its own.
            let inner = span - 2 * apron;
            xpute_core::ensure!(lod_step.is_power_of_two() && inner.is_multiple_of(lod_step), EINVAL, lod_step, inner);
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
        xpute_core::ensure!(self.mesh_words > 0 && self.mesh.is_none(), ENOTRECOVERABLE);
        xpute_core::ensure!(words >= self.mesh_words, ENOTRECOVERABLE, words, self.mesh_words);
        self.mesh = Some(MeshTarget { ptr, words: self.mesh_words as usize });
    }

    /// The complete mesh's counts. Its words are the caller's.
    pub fn finish(&mut self) -> Mesh {
        xpute_core::ensure!(self.done && self.open.is_some(), ENOTRECOVERABLE);
        self.open = None;
        self.mesh = None;
        self.surviving.clear();
        Mesh {
            box_cnt: self.box_cnt,
            heights: self.heights,
            box_words: self.box_words,
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
        }
    }

    fn step_fine(&mut self, b: &mut FineBake, set: &RunSet) -> bool {
        let span = b.span;
        // The chunk's own columns. The ring outside them is read through
        // run_face_mask's neighbor lookups and is never walked: nothing is
        // emitted from a column that belongs to the next chunk.
        let (lo, hi) = (b.apron, span - b.apron);
        // The sides a column on the chunk's rim faces out of it: drawn
        // whatever the apron beside them holds. The apron is the chunk's own
        // level read past its edge, and the chunk drawn there may be another
        // level whose ground stands lower than that reading — with the side
        // hidden against the apron, the step between them was open to the
        // sky. Drawn down to the chunk's floor, the rim closes on whatever
        // lies beside it, so no two chunks need to be a level apart or less.
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
            let row_end = hi.min(b.row + BAKE_ROWS_PER_STEP);
            for cy in b.row..row_end {
                for cx in lo..hi {
                    let col = cy * span + cx;
                    let end = set.start[col as usize + 1];
                    let mut mergeable: Option<(u32, u32)> = None;
                    for i in set.start[col as usize]..end {
                        let i = i as usize;
                        self.counters.total_runs += 1;
                        let mask = run_face_mask(set, span, cx, cy, i) | if set.z1[i] >= set.z0[i] { rim(cx, cy) } else { 0 };
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
            let mut box_total = 0u32;
            for boxed in self.surviving.as_chunks::<6>().0 {
                let faces = (boxed[3] & ALL_FACES).count_ones();
                self.counters.faces_kept += faces;
                box_total += (faces > 0) as u32;
            }
            self.mesh_words = mesh_words(box_total, self.box_words);
            self.box_cnt = 0;
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
            if mask & ALL_FACES != 0 {
                self.emit_box([min_x, min_y, min_z], [size_x, size_y, size_z], [set.r[i], set.g[i], set.b[i]], set.layer_taps[i], mask, unit);
            }
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

    /// Writes one box's faces into the bake's mesh. `unit` is the box grid's
    /// own cell in units, which its corner and width are measured in.
    #[allow(clippy::too_many_arguments)]
    fn emit_box(&mut self, min: V3, size: [f64; 3], rgb: [u8; 3], taps: u32, faces: u32, unit: f64) {
        let target = self.mesh.or_bug(Errno::ENOTRECOVERABLE);
        // SAFETY: provide_mesh's contract — the range is the caller's, valid
        // and untouched until the bake closes.
        let m = unsafe { core::slice::from_raw_parts_mut(target.ptr, target.words) };
        let words = self.box_words as usize;
        let at = self.box_cnt as usize * words;
        let heights = self.heights;
        write_box(&mut m[at..at + words], min, size, rgb, taps, faces, unit, heights);
        self.box_cnt += 1;
    }

    fn step_coarse(&mut self, b: &mut CoarseBake, set: &RunSet) -> bool {
        let (lod_step, res, scale) = (b.lod_step, b.res, b.scale);
        // A coarse cell is as tall as it is wide.
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
                    for j in 0..lod_step {
                        let cy = by * lod_step + j + set.apron;
                        for k in 0..lod_step {
                            let cx = bx * lod_step + k + set.apron;
                            let col = cy * set.span + cx;
                            // Ground and object alike: a coarse cell is what
                            // is not air, whatever put it there
                            // (issue/20260923-a-coarse-cell-is-isotropic.md).
                            let end = set.start[col as usize + 1];
                            for i in set.start[col as usize]..end {
                                cnt = self.accumulate_run(start, cnt, bucket, scale, set, i as usize, col);
                            }
                        }
                    }
                    self.coarse.start.push(self.coarse.z0.len() as u32);
                    self.pack_coarse_cell(start, cnt, lod_step, bucket);
                    self.buckets.truncate(start as usize);
                }
            }
            b.row = row_end;
            if row_end < res {
                return false;
            }
            self.coarse.start.push(self.coarse.z0.len() as u32);
            self.heights = Heights::of(&self.coarse.view(), 1.0);
            b.fine = Some(self.begin_fine(res, 0, lod_step * scale, 1));
            b.phase = 2;
            return false;
        }

        // The coarse set is lifted out of the context for the step, so the
        // fine bake can read it while writing the context's scratch.
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
        self.buckets.push(ZBucketAccum { bz, ..ZBucketAccum::default() });
        self.buckets.len() as u32 - 1
    }

    /// Adds one run of column `col` to the coarse cell's buckets, each
    /// `bucket` units tall: how much of the bucket it fills, and how much of
    /// its surface meets air, which is what the bucket's color is weighed
    /// by. A sub-cell run is its share of a cell `cell` units on a side.
    #[allow(clippy::too_many_arguments)]
    fn accumulate_run(&mut self, start: u32, mut cnt: u32, bucket: i32, cell: u32, set: &RunSet, i: usize, col: u32) -> u32 {
        let (run_z0, run_z1) = (set.z0[i], set.z1[i]);
        // What the run reads as from here. With the pack drawn, a patch of
        // this material shows its layer's own mean color once the tile is
        // too small to see, and the catalog's swatch is only the fallback for
        // a type the pack has no tile for. With the pack off there is no tile
        // to average, and the leaves under this tier show the swatch, so the
        // summary shows it too.
        let swatch = [set.r[i], set.g[i], set.b[i]];
        let mean = if self.textured { layer_mean(set.layer_taps[i] & 255, swatch) } else { swatch };
        let rgb = linear_of(mean[0], mean[1], mean[2]);
        if set.sub_sz[i] != SUB_CELL_NONE {
            let frac = unpack_sub_cell_size(set.sub_sz[i]);
            // A thing inside one cell: its volume is the fraction cubed, and
            // every one of its six faces the fraction squared. It stands in
            // the cell's air, so none of them is hidden.
            let idx = self.find_or_create_bucket(start, cnt, run_z0.div_euclid(bucket));
            let acc = &mut self.buckets[idx as usize];
            acc.ground |= set.material_class[i] != MATERIAL_OBJECT;
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
            acc.ground |= set.material_class[i] != MATERIAL_OBJECT;
            acc.add((hi - lo + 1) as f64, exposed_faces(set, col, i, lo, hi), rgb);
        }
        cnt
    }

    /// Packs one coarse cell's buckets bottom to top. A cell's buckets have
    /// distinct heights (find_or_create_bucket), so sorting them by height
    /// visits them lowest first.
    fn pack_coarse_cell(&mut self, start: u32, cnt: u32, lod_step: u32, bucket: i32) {
        let step = bucket;
        let cell = start as usize..(start + cnt) as usize;
        self.buckets[cell.clone()].sort_unstable_by_key(|acc| acc.bz);
        // A bucket's volume in coverage's measure: units tall by columns.
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
            let linear = libm::cbrt(fill);
            // A full cell is a plain run; a sparse one is inset, scaled by
            // the cube root of its fill about its center on every axis.
            let inset = (linear < 1.0).then(|| pack_inset(linear));
            let rgb = acc.color();
            // No layer of its own: a box that merges cells is its color
            // and nothing else. The texture pack is for a leaf's own cells,
            // where a top and a side mean something; a box summarizing a
            // patch of ground has no cut through earth to show (chunk.wgsl).
            let taps = packed_taps(0, 0);
            // What lies under the box. A box shrunk about its center stands
            // clear of its cell on every side, and a view from above reads
            // those gaps as holes rather than as one surface
            // (issue/20260923-a-coarse-cell-is-isotropic.md), so a cell of
            // ground lays its floor: the cell's own bottom face, its whole
            // footprint, in the box's color — a face and not a run up to
            // the box, so the box stands over it as a placed thing stands
            // over the ground. A cell of placed things alone lays none: its
            // box floats at the cell's center.
            //
            // The cell's lowest box hangs a cell below its floor too — the
            // skirt every terrain LOD hangs off its tiles: two cells whose
            // ground lands in different buckets leave a step between their
            // floors, and a view along the ground would see daylight through
            // it. Its top is the floor. A box over a whole box needs neither:
            // the one under it is the floor already.
            let lowest = open.is_none() && last_top == i32::MIN;
            let floored = last_top == bucket_lo - 1 && open.is_some_and(|o| self.coarse.sub_sz[o] == SUB_CELL_NONE);
            let under = if !acc.ground || floored {
                None
            } else if lowest {
                Some((bucket_lo - step, bucket_lo - 1))
            } else if inset.is_some() {
                // A run with no height: its top alone, at the cell's bottom.
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
                co.material_class.push(MATERIAL_LAND);
                co.sub_sz.push(SUB_CELL_NONE);
                for lane in [&mut co.sub_sx, &mut co.sub_sy, &mut co.sub_ox, &mut co.sub_oy, &mut co.sub_oz] {
                    lane.push(0);
                }
                // The run above is not this box, so nothing merges across it.
                open = None;
                last_top = z1;
            }
            // A box of placed things alone is one of them: it hides none of
            // its neighbors' faces and shows its underside, as a placed cube
            // does over the ground.
            let class = if acc.ground { MATERIAL_LAND } else { MATERIAL_OBJECT };
            let co = &mut self.coarse;
            if let Some(o) = open.filter(|_| inset.is_none() && bucket_lo == last_top + 1) {
                if [co.r[o], co.g[o], co.b[o]] == rgb && co.layer_taps[o] == taps && co.sub_sz[o] == SUB_CELL_NONE && co.material_class[o] == class {
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
/// Writes one box with nothing around it into a mesh block of
/// `mesh_words(1)` words, as a bake writes a box: what the hover ghost is
/// drawn from, so it is lit, textured and edge-shaded as a placed cube is.
/// `rgb` is sRGB bytes.
pub fn write_lone_box(m: &mut [u32], size: f64, rgb: [u8; 3], layer: u8) -> Heights {
    xpute_core::ensure!(m.len() == FINE_BOX_WORDS as usize, ENOTRECOVERABLE);
    let heights = Heights { floor: 0.0, step: size / Z_STEPS };
    write_box(m, [0.0; 3], [size; 3], rgb, layer as u32 * 0x0101_0101, LONE_BOX_FACES, size, heights);
    heights
}

/// One box's words, a cell of `unit` units across, in the format `m` is long
/// for (FINE_BOX_WORDS): its corner and width across x and y in steps of its
/// cell, and its floor and height in the mesh's own (Heights).
#[allow(clippy::too_many_arguments)]
fn write_box(m: &mut [u32], min: V3, size: [f64; 3], rgb: [u8; 3], taps: u32, faces: u32, unit: f64, heights: Heights) {
    let z = steps(min[2] - heights.floor, heights.step) | (steps(size[2], heights.step) << 16);
    if m.len() == COARSE_BOX_WORDS as usize {
        let xy = unit / COARSE_XY_STEPS;
        // A width of zero would be a box of nothing, so one step is the
        // least: the byte holds `width − 1`, and a whole mesh across fits.
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

fn steps(v: f64, step: f64) -> u32 {
    round(v / step).clamp(0.0, u16::MAX as f64) as u32
}

/// A box's corner and size in units, as the renderer unfolds them — its
/// height from the mesh's floor — for whoever reads a mesh back rather than
/// drawing it (the chunk scheduler's bounds). `m` is one box, and its length
/// says which format.
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

/// A box's floor and height, in its mesh's height steps (`Heights`): the
/// one word both formats keep them in.
pub fn box_z(m: &[u32]) -> (u32, u32) {
    (m[2] & 0xffff, m[2] >> 16)
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

/// Whether a run's underside is drawn: a placed cube's is, since it may
/// stand clear of the ground, and so is an inset box's — a coarse cell's
/// box is shrunk about its center and rests on nothing.
fn run_include_bottom(set: &RunSet, i: usize) -> bool {
    set.material_class[i] == MATERIAL_OBJECT || set.sub_sz[i] != SUB_CELL_NONE
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
    let mask = run_faces(set, span, cx, cy, i);
    // A run with no height is a face and nothing else: a coarse cell's floor
    // (pack_coarse_cell), drawn by its top alone.
    if set.z1[i] < set.z0[i] {
        return mask & !(ALL_FACES & !FACE_PZ);
    }
    mask
}

fn run_faces(set: &RunSet, span: u32, cx: u32, cy: u32, i: usize) -> u32 {
    let mask_all = if run_include_bottom(set, i) { ALL_FACES } else { OPEN_BOTTOM_FACES };
    if set.sub_sz[i] != SUB_CELL_NONE {
        // A thing placed in a cell is lit as it stands; a coarse cell's box
        // is the ground itself, shrunk, and its underside is more ground.
        let under = if set.material_class[i] == MATERIAL_OBJECT { 0 } else { UNDER_AS_TOP };
        return mask_all | ALL_CREASES | under;
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

/// How much of one run's surface meets air, between `lo` and `hi`, in the
/// faces of the set's own cells: its top and its bottom where they fall in
/// that span and nothing of its own column is there, and along each of the
/// four sides whatever the neighboring column does not cover. A column
/// outside the set — past the apron — counts as air, which is the edge of
/// the chunk and is rare enough to leave generous.
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

/// How much of `lo..=hi` whole cells of column `col` fill. Runs of one
/// column do not overlap, so their overlaps with the span add up.
fn covered_height(set: &RunSet, col: u32, lo: i32, hi: i32) -> i32 {
    let mut covered = 0;
    let end = set.start[col as usize + 1] as usize;
    for i in set.start[col as usize] as usize..end {
        if set.z0[i] > hi {
            break;
        }
        if set.sub_sz[i] != SUB_CELL_NONE {
            continue;
        }
        covered += (set.z1[i].min(hi) - set.z0[i].max(lo) + 1).max(0);
    }
    covered
}

/// sRGB bytes as linear light, 0 to 1 — where a mean of colors belongs, the
/// transfer being what the eye needs and not what light does.
fn linear_of(r: u8, g: u8, b: u8) -> [f64; 3] {
    [linear_channel(r), linear_channel(g), linear_channel(b)]
}

fn linear_channel(v: u8) -> f64 {
    let c = v as f64 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        libm::pow((c + 0.055) / 1.055, 2.4)
    }
}

/// Linear light back to the sRGB bytes a run carries.
fn srgb_of(rgb: [f64; 3]) -> [u8; 3] {
    core::array::from_fn(|k| {
        let c = rgb[k].clamp(0.0, 1.0);
        let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * libm::pow(c, 1.0 / 2.4) - 0.055 };
        to_u8(s * 255.0) as u8
    })
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
/// a side, centered.
fn pack_inset(linear: f64) -> (u8, u8) {
    (pack_sub_cell_fraction((1.0 - linear) * 0.5) as u8, pack_sub_cell_fraction(linear) as u8)
}

#[cfg(test)]
#[path = "chunk_mesher.test.rs"]
mod test;

fn coarse_rows_per_step(lod_step: u32) -> u32 {
    (BAKE_ROWS_PER_STEP / lod_step).max(1)
}
