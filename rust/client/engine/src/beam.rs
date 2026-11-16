// pxl-engine/beam.rs

//! wgsl-port: the camera grid's beam. A beam ray is cast through each corner of the
//! grid's cells and walks the pyramid until it meets a cell that fills or lacks, or a split
//! narrower than it can share with the rays around it (`cast::narrow`). Each cell starts from
//! the least distance of the beam rays around it, shortened by a margin (`start_dist`). The
//! samples, `SAMPLE` to an angular cell (`grid::CELL`) a side, resume from the line that beam
//! ray left and walk to the cell that fills. The cells they stopped at are drawn as boxes, and
//! each drawn pixel takes the nearest one its own ray enters (`raster`); a pixel walks from
//! its cell's start only where that is a node a sample lacks at, or where no box covers it.
//! The lattice is the grid's own, so neither the beam rays nor the samples follow the screen's
//! pixels.

use crate::WORKGROUP_CAP;
use crate::cast;
use crate::param::beam;
use crate::param::beam::NEAR;
use crate::param::beam::SAMPLE;
use crate::raster::FACES_LANE_CNT;
use crate::ray;
use crate::world;
use pxl_spec::space::camera::grid::{CELL, CELL_INVERSE};

/// The camera grid as a frame lays it over the screen, worked out once from the screen's
/// tangents (`of`) where the core writes the eye's words (pxl-core domain/view/lens.rs
/// `gpu_cast`, `eye::GRID`). Every thread of the cast reads it from those words (pxl-render
/// grid.wgsl), and the renderer sizes the cast's dispatches and its buffer from the same
/// words.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    /// The tangents of the screen's half-angles, across and up. They are the eye's own
    /// (`eye::TAN`), so they are not among the grid's words.
    pub tan: [f32; 2],
    /// The factor `1 − γ` a beam ray's hit is shortened by, where `γ` is the margin within which
    /// two rays enter a cube under `2√2 · s · d` wide apart (`start_dist`).
    keep: f32,
    /// How far a cube that a beam ray must cross reaches from its center, `√6 · s`, as an angle
    /// with the margin, and its tangent (`window`).
    radius: f32,
    radius_tan: f32,
    /// The corners cast past the screen on each side: the widest window of the screen's cells,
    /// which is the last cell's, whose far corner lies under a cell past the screen where the
    /// cells do not divide it evenly.
    past: u32,
    /// The screen's cells across and up, as many as span it from its left and top (`span`); the
    /// grid's corners, as many as span it and `past` more on each side, so that every corner a
    /// cell's window reads is cast (`corner_lattice`); and the samples, `SAMPLE` a cell
    /// (`grid::CELL`) and `NEAR` past the screen on each side (`sample_lattice`).
    pub span: [u32; 2],
    pub corner_lattice: [u32; 2],
    pub sample_lattice: [u32; 2],
    /// Where, among the beam rays' words, the distances the rays about each beam ray start at
    /// begin: past the lines (`start_dist_at`).
    start_dists_at: u32,
    /// Where, among the beam rays' words, the cells' starts and the samples' hits begin, past the
    /// lines and the start distances (`start_dist_at`). After them come the asks of the
    /// drawable entries' faces, with room for one a sample (`asks_at`; the voxels themselves are
    /// the node cache's, `record`), then the holes' hits (`hits_at`) and the asks of the faces
    /// they opened (`hit_asks_at`), `ray::HITS_A_FRAME` of each.
    cells_at: u32,
    samples_at: u32,
    /// The boxes' asks (`ask_at`). Until the samples' boxes lay them, the beam rays' and the
    /// samples' runs list here the items they park, one word each (pxl-render item_list.wgsl).
    /// One run's list is done with before the next begins.
    pub asks_at: u32,
    pub(crate) hits_at: u32,
    pub hit_asks_at: u32,
    /// Where the beam rays' runs park them, one state each (`ray::STATE_WORDS`, pxl-render
    /// corner.wgsl), in the corners' order.
    pub corner_states_at: u32,
}

/// The samples' spacing in the screen's tangents at its axis: a cell divided by its samples
/// a side (`Grid::sample`). Its reciprocal is what a tangent is multiplied by to count
/// samples (`Grid::sample_before`), and the CPU and the GPU use the same constant
/// (pxl-render cast.wgsl `sample_before`, sample.eye.wgsl).
pub const SAMPLE_SPACING: f32 = CELL / SAMPLE as f32;
pub const SAMPLE_SPACING_INVERSE: f32 = 1.0 / SAMPLE_SPACING;

/// Where each of the grid's fields lies among the eye's words (`eye::GRID`, `words`). First
/// come `keep`, `radius` and `radius_tan` as f32 bits. Then come `past`, `span`, `corner_lattice`
/// and `sample_lattice`, each across then up; the words where the cells' starts, the samples' hits,
/// the boxes' asks and the holes' hits begin; where the eye's holes lie past the grid's words
/// (`walked_at`, `ray::holes_words`); and where the corners' states begin. Then comes the slope
/// of the screen's sides across and up (`slope`), as f32 bits rounded once by the host, and
/// last where the start distances begin (`start_dist_at`) and where the asks opened by the
/// holes' hits begin (`hit_asks_at`).
pub const GRID_KEEP: usize = 0;
pub const GRID_RADIUS: usize = 1;
pub const GRID_RADIUS_TAN: usize = 2;
pub const GRID_PAST: usize = 3;
pub const GRID_SPAN: usize = 4;
pub const GRID_CORNER_LATTICE: usize = 6;
pub const GRID_SAMPLE_LATTICE: usize = 8;
pub const GRID_CELLS_AT: usize = 10;
pub const GRID_SAMPLES_AT: usize = 11;
pub const GRID_ASKS_AT: usize = 12;
pub const GRID_HITS_AT: usize = 13;
pub const GRID_WALKED_AT: usize = 14;
pub const GRID_CORNER_STATES_AT: usize = 15;
pub const GRID_SLOPE: usize = 16;
pub const GRID_START_DISTS_AT: usize = 18;
pub const GRID_HIT_ASKS_AT: usize = 19;
/// The grid's words, rounded up to whole vec4s, since the eye's words bind as a uniform
/// (`eye::WORDS`).
pub const GRID_WORDS: usize = (GRID_HIT_ASKS_AT + 1).next_multiple_of(4);

impl Grid {
    /// The grid at the screen's tangents `tan`, its beam rays and cells a
    /// cell (`grid::CELL`) apart.
    pub fn of(tan: [f32; 2]) -> Grid {
        let k = cast::NARROW_DIAGONAL * CELL;
        let r6 = k / 2.0;
        let radius = r6 / (1.0 - r6) * beam::RADIUS_SLACK;
        let span = [
            libm::ceilf(2.0 * tan[0] / CELL) as u32,
            libm::ceilf(2.0 * tan[1] / CELL) as u32,
        ];
        let sample_lattice = span.map(|n| n * SAMPLE + 1 + 2 * NEAR);
        let mut g = Grid {
            tan,
            keep: 1.0 - k / (1.0 - k),
            radius,
            radius_tan: libm::tanf(radius),
            past: 0,
            span,
            corner_lattice: [0; 2],
            sample_lattice,
            start_dists_at: 0,
            cells_at: 0,
            samples_at: 0,
            asks_at: 0,
            hits_at: 0,
            hit_asks_at: 0,
            corner_states_at: 0,
        };
        g.past = g.cell_window([span[0] - 1, span[1] - 1]);
        g.corner_lattice = [span[0] + 1 + 2 * g.past, span[1] + 1 + 2 * g.past];
        g.start_dists_at = g.corner_lattice[0] * g.corner_lattice[1] * LINE_WORDS;
        g.cells_at = g.start_dists_at + g.corner_lattice[0] * g.corner_lattice[1];
        g.samples_at = g.cells_at + span[0] * span[1] * CELL_WORDS;
        g.asks_at = g.samples_at + sample_lattice[0] * sample_lattice[1] * SAMPLE_WORDS;
        g.hits_at = g.asks_at + sample_lattice[0] * sample_lattice[1] * ASK_WORDS;
        g.hit_asks_at = g.hits_at + ray::HITS_A_FRAME * HIT_WORDS;
        g.corner_states_at = g.hit_asks_at + ray::HITS_A_FRAME * ASK_WORDS;
        g
    }

    /// Its words (`GRID_WORDS`), as `of_words` reads them.
    pub fn words(&self) -> [u32; GRID_WORDS] {
        let mut w = [0; GRID_WORDS];
        (w[GRID_KEEP], w[GRID_RADIUS], w[GRID_RADIUS_TAN]) = (
            self.keep.to_bits(),
            self.radius.to_bits(),
            self.radius_tan.to_bits(),
        );
        w[GRID_PAST] = self.past;
        (w[GRID_SPAN], w[GRID_SPAN + 1]) = (self.span[0], self.span[1]);
        (w[GRID_CORNER_LATTICE], w[GRID_CORNER_LATTICE + 1]) =
            (self.corner_lattice[0], self.corner_lattice[1]);
        (w[GRID_SAMPLE_LATTICE], w[GRID_SAMPLE_LATTICE + 1]) =
            (self.sample_lattice[0], self.sample_lattice[1]);
        (
            w[GRID_CELLS_AT],
            w[GRID_SAMPLES_AT],
            w[GRID_ASKS_AT],
            w[GRID_HITS_AT],
        ) = (self.cells_at, self.samples_at, self.asks_at, self.hits_at);
        (w[GRID_WALKED_AT], w[GRID_CORNER_STATES_AT]) = (self.size(), self.corner_states_at);
        w[GRID_START_DISTS_AT] = self.start_dists_at;
        w[GRID_HIT_ASKS_AT] = self.hit_asks_at;
        let slope = self.slope();
        (w[GRID_SLOPE], w[GRID_SLOPE + 1]) = (slope[0].to_bits(), slope[1].to_bits());
        w
    }

    /// The grid at the screen's tangents `tan` from its words `w`.
    pub fn of_words(tan: [f32; 2], w: [u32; GRID_WORDS]) -> Grid {
        let f = f32::from_bits;
        Grid {
            tan,
            keep: f(w[GRID_KEEP]),
            radius: f(w[GRID_RADIUS]),
            radius_tan: f(w[GRID_RADIUS_TAN]),
            past: w[GRID_PAST],
            span: [w[GRID_SPAN], w[GRID_SPAN + 1]],
            corner_lattice: [w[GRID_CORNER_LATTICE], w[GRID_CORNER_LATTICE + 1]],
            sample_lattice: [w[GRID_SAMPLE_LATTICE], w[GRID_SAMPLE_LATTICE + 1]],
            start_dists_at: w[GRID_START_DISTS_AT],
            cells_at: w[GRID_CELLS_AT],
            samples_at: w[GRID_SAMPLES_AT],
            asks_at: w[GRID_ASKS_AT],
            hits_at: w[GRID_HITS_AT],
            hit_asks_at: w[GRID_HIT_ASKS_AT],
            corner_states_at: w[GRID_CORNER_STATES_AT],
        }
    }

    /// The slope of the screen's sides across and up, `√(1 + tan²)`; a point's distance to a
    /// side is its offset divided by it (`eye::sees`). It is rounded once, here on the host, and
    /// handed to the shaders in words (`GRID_SLOPE`).
    pub fn slope(&self) -> [f32; 2] {
        self.tan.map(|t| libm::sqrtf(1.0 + t * t))
    }

    /// Where the rays about a beam ray that stopped at `t` start: no farther than any pixel's ray
    /// around it enters what stopped it.
    pub fn start_dist(&self, t: f32) -> f32 {
        t * self.keep
    }

    /// The side, in cells, of the window around a pixel at the screen's tangents `at`; the
    /// pixel's start is the least of the window's corners. It is the radius stretched by the
    /// screen's slope out there, plus the distance from that center to the corner nearest it.
    fn window(&self, at: [f32; 2]) -> u32 {
        let edge = libm::sqrtf(at[0] * at[0] + at[1] * at[1]);
        let beyond = (edge + self.radius_tan) / (1.0 - edge * self.radius_tan);
        let stretch = 1.0 + beyond * beyond;
        libm::ceilf(core::f32::consts::FRAC_1_SQRT_2 + self.radius * stretch * CELL_INVERSE) as u32
    }

    /// The window of the screen's cell `[x, y]`, counted from its left and top, taken at the
    /// cell's corner farthest from the screen's axis, as far out as any pixel of it lies, since
    /// the slope grows outward.
    pub fn cell_window(&self, [x, y]: [u32; 2]) -> u32 {
        let far = |t: f32, x: u32| {
            libm::fabsf(-t + x as f32 * CELL).max(libm::fabsf(-t + (x + 1) as f32 * CELL))
        };
        self.window([far(self.tan[0], x), far(self.tan[1], y)])
    }

    /// The tangents across and up of corner `[i, j]`, counted from `past` corners beyond the
    /// screen's left and top.
    pub fn corner(&self, [i, j]: [u32; 2]) -> [f32; 2] {
        let k = self.past as f32;
        [
            -self.tan[0] + (i as f32 - k) * CELL,
            self.tan[1] - (j as f32 - k) * CELL,
        ]
    }

    /// The first corner of the screen's cell `[x, y]`.
    pub fn first(&self, [x, y]: [u32; 2]) -> [u32; 2] {
        [x + self.past, y + self.past]
    }

    /// The screen's cell holding the screen's point at the tangents `[tx,
    /// ty]`.
    pub fn cell(&self, [tx, ty]: [f32; 2]) -> [u32; 2] {
        let at = |v: f32, n: u32| (libm::floorf(v * CELL_INVERSE).max(0.0) as u32).min(n - 1);
        [
            at(tx + self.tan[0], self.span[0]),
            at(self.tan[1] - ty, self.span[1]),
        ]
    }

    /// The tangents across and up of sample `[i, j]`, counted from `NEAR` samples beyond the
    /// screen's left and top.
    pub fn sample(&self, [i, j]: [u32; 2]) -> [f32; 2] {
        let d = SAMPLE_SPACING;
        [
            -self.tan[0] + (i as f32 - NEAR as f32) * d,
            self.tan[1] - (j as f32 - NEAR as f32) * d,
        ]
    }

    /// The sample at or before the screen's point at the tangents `[tx, ty]`, across and up,
    /// counted from `NEAR` samples beyond the screen's left and top. With the next one across, the
    /// next down and the next both ways, it makes the four around the point.
    pub fn sample_before(&self, [tx, ty]: [f32; 2]) -> [u32; 2] {
        let at = |v: f32| libm::floorf(v * SAMPLE_SPACING_INVERSE).max(0.0) as u32 + NEAR;
        [at(tx + self.tan[0]), at(self.tan[1] - ty)]
    }

    /// The screen's cell sample `[i, j]` starts from: the one holding it, or
    /// the nearest where it lies past the screen, as only the screen's cells
    /// have a start.
    pub fn sample_cell(&self, [i, j]: [u32; 2]) -> [u32; 2] {
        let (n, k) = (self.span, SAMPLE);
        let at = |v: u32| (v.max(NEAR) - NEAR) / k;
        [at(i).min(n[0] - 1), at(j).min(n[1] - 1)]
    }

    /// The first word of corner `c`'s line, by its index.
    pub fn line(c: u32) -> u32 {
        c * LINE_WORDS
    }

    /// The word holding where the rays about corner `c`'s beam ray start (`start_dist`), as f32
    /// bits.
    /// There is one word a corner past the lines, in the corners' order, so a cell's window reads
    /// its corners row by row from one run of words, not from one line each.
    pub fn start_dist_at(&self, c: u32) -> u32 {
        self.start_dists_at + c
    }

    /// The first word of the screen's cell `[x, y]`'s start.
    pub fn cell_at(&self, [x, y]: [u32; 2]) -> u32 {
        self.cells_at + (y * self.span[0] + x) * CELL_WORDS
    }

    /// The first word of sample `[i, j]`'s hit.
    pub fn sample_at(&self, [i, j]: [u32; 2]) -> u32 {
        self.samples_at + (j * self.sample_lattice[0] + i) * SAMPLE_WORDS
    }

    /// The first word of the ask of the box at `place` among the boxes
    /// (`raster::ARGS_ASKS`).
    pub fn ask_at(&self, place: u32) -> u32 {
        self.asks_at + place * ASK_WORDS
    }

    /// Where the eye's holes lie among the beam rays' words (`ray::holes_words`): past the grid's
    /// words, one bit a drawn pixel and a pool of parked rays, which the buffer holds beyond
    /// `size`.
    pub fn walked_at(&self) -> u32 {
        self.size()
    }

    /// The beam rays' words: the lines, the start distances, the cells' starts, the samples'
    /// hits, the boxes' asks, the missed pixels' hits and their asks, and the corners' states.
    /// Their count follows the grid, whatever the pixels drawn.
    pub fn size(&self) -> u32 {
        self.corner_states_at + self.corner_lattice[0] * self.corner_lattice[1] * ray::STATE_WORDS
    }
}

/// A node as a beam ray's line and a sample's hit each hold it, in words:
/// its zoom, its corner, its roots.
pub const NODE_ZOOM: u32 = 0;
pub const NODE_CORNER: u32 = 1;
pub const NODE_ROOTS: u32 = 4;
pub const NODE_WORDS: u32 = 7;

/// What a beam ray leaves for the pixels around it, in words: the node it stopped at
/// (`NODE_WORDS`); the steps its walk took from the root (`LINE_STEPS`), which the ray steps view
/// counts into every ray that goes on from the line (pxl-render grid.wgsl `beam_step_cnt`); then,
/// for every node from the root to it, what was handed that node (`world::Inheritance::words`),
/// `LINE_STEP` words a zoom, with its own at its zoom. A node's entry in the node cache is found by
/// its key (pxl-render walk_node.wgsl `bucket_of`), so a line holds no entry. The beam ray's start
/// distance (`Grid::start_dist`) lies apart from its line, one word a corner past the lines
/// (`Grid::start_dist_at`), because the cells' windows read it alone. The light's beam rays leave
/// the same, with their origin after it (`light::LINE_ORIGIN`), and keep their start distances
/// apart as well (`light::start_dist_at`).
pub const LINE_NODE: u32 = 0;
pub const LINE_STEPS: u32 = LINE_NODE + NODE_WORDS;
pub const LINE_INHERITANCE: u32 = LINE_STEPS + 1;
pub const LINE_STEP: u32 = world::INHERITANCE_WORDS;
pub const LINE_WORDS: u32 = LINE_INHERITANCE + LINE_STEP * cast::DEPTH as u32;

/// What a cell hands its rays, in words, for every observer (`light::cell`): where they
/// start, as f32 bits; the corner with the least start, as an index into the corners row by
/// row, from whose line the faces they stop on are asked; and whether they resume from that
/// line (1) or walk from the world's root (0), the latter where a beam ray of the window asked
/// other zooms than they do (pxl-render cell.wgsl, light_cell.wgsl). There is one for each of
/// the screen's cells, in the beam rays' buffer past the lines (`Grid::cell_at`), so one
/// binding serves both.
pub const CELL_T: u32 = 0;
pub const CELL_CORNER: u32 = 1;
pub const CELL_RESUMES: u32 = 2;
pub const CELL_WORDS: u32 = 3;

/// The samples: rays on a lattice of `SAMPLE` a cell (`grid::CELL`) a side, each resumed
/// from its cell's start as a pixel is and walked to the cell that fills. A pixel takes the
/// nearest of the cells they stopped at that its own ray enters (`raster`), and walks only
/// where none does. The lattice runs `NEAR` samples past the screen on each side.
///
/// What a sample leaves for the boxes, in words: the node it stopped at (`NODE_WORDS`), with
/// its zoom `SAMPLE_NONE` where it went on to its reach; the asset there; the lack word, with
/// `SAMPLE_LACK` set where that node lacks, which a pixel entering it before any cell walks
/// itself, and above it, from `SAMPLE_STEPS_SHIFT`, the steps its walk took from its cell's
/// start, which the ray steps view shows at the pixels that take no walk (pxl-render cast.wgsl
/// `taken_word`);
/// and the axis the sample entered it across, a face its ray reached through air
/// (`raster::faces`). While its ray has not ended, its zoom is `SAMPLE_PARKED`, and its
/// words hold the state the queue parked it in (pxl-render sample.wgsl): its distance in the
/// asset's first word (`SAMPLE_PARKED_T`), its node's corner in the node's words, and the
/// rest in the lack word (`SAMPLE_PARKED_STATE`).
pub const SAMPLE_NODE: u32 = 0;
pub const SAMPLE_ASSET: u32 = SAMPLE_NODE + NODE_WORDS;
pub const SAMPLE_LACK_WORD: u32 = SAMPLE_ASSET + 2;
pub const SAMPLE_AXIS: u32 = SAMPLE_LACK_WORD + 1;
pub const SAMPLE_WORDS: u32 = SAMPLE_AXIS + 1;
pub const SAMPLE_LACK: u32 = 1;
pub const SAMPLE_STEPS_SHIFT: u32 = 1;

pub const SAMPLE_NONE: u32 = u32::MAX;
pub const SAMPLE_PARKED: u32 = u32::MAX - 1;
pub const SAMPLE_PARKED_T: u32 = SAMPLE_ASSET;
pub const SAMPLE_PARKED_STATE: u32 = SAMPLE_LACK_WORD;

/// The side of a workgroup of the passes over the cells of the light's list, one thread a
/// cell (pxl-render light_cell.wgsl, light_list.wgsl `light_item`).
pub const SAMPLE_SIDE: u32 = 8;

/// What a hit's faces are asked from, once for each hit (pxl-render face.wgsl `face_cs`), in
/// words: the first word of its node (`NODE_WORDS`) among the beam rays' words; the face its
/// ray entered (pxl-engine `ray`); the first word of the beam ray's line its faces are asked
/// from, the line its cell's rays resumed from (`CELL_CORNER`); and the word its faces are
/// left in, `ASK_NONE` where it asks none. For a voxel's face (`Grid::ask_at`,
/// `hit_asks_at`), the ask is made by the first hit on the voxel to enter it across that
/// face's axis. Its node is then the entry in the node cache (`node_cache::entry_at`), its
/// face is left at that axis among the entry's words (`node_cache::R_FACES`), and the word is
/// named under `record::TO_REC`.
pub const ASK_NODE: u32 = 0;
pub const ASK_FACE: u32 = 1;
pub const ASK_LINE: u32 = 2;
pub const ASK_TO: u32 = 3;
pub const ASK_WORDS: u32 = 4;
pub const ASK_NONE: u32 = u32::MAX;

/// The hits of the holes' rays (pxl-render miss_word.wgsl `hit_put`, queue.wgsl), every one,
/// whatever it stopped on, which the record takes (`record`; pxl-render record_hits.wgsl).
/// There are at most `ray::HITS_A_FRAME`, numbered as the walks take them. In words: the node
/// (`NODE_WORDS`); the face its ray entered, with `HIT_LACK` set where it lacks; its pixel,
/// with the row above the column; the asset it stopped at; and the first word of the line its
/// pixel's cell's rays resumed from, from which its face's word is asked (`ASK_LINE`). A pixel
/// whose hit's face word is being asked holds `HIT_WAIT` and the hit's number in its last
/// ray word until the word is put there from the record (`ray::FACE_WORD`, record_hits.wgsl
/// `rec_faces_cs`), in the cast's own pass.
pub const HIT_NODE: u32 = 0;
pub const HIT_FACE: u32 = NODE_WORDS;
pub const HIT_PX: u32 = HIT_FACE + 1;
pub const HIT_ASSET: u32 = HIT_PX + 1;
pub const HIT_LINE: u32 = HIT_ASSET + 2;
pub const HIT_WORDS: u32 = HIT_LINE + 1;
pub const HIT_LANE_CNT: u32 = 64;
pub const HIT_LACK: u32 = 1 << 31;
pub const HIT_WAIT: u32 = 1 << 31;
const _: () = assert!(
    ray::HITS_A_FRAME.div_ceil(HIT_LANE_CNT) <= WORKGROUP_CAP
        && ray::HITS_A_FRAME.div_ceil(FACES_LANE_CNT) <= WORKGROUP_CAP,
    "the kept hits, and their faces' asks, fit one dispatch"
);

#[cfg(test)]
#[path = "beam.test.rs"]
mod tests;
