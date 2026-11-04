// pxl-engine/beam.rs

//! gpu-portable: the camera grid's beam.
//! A beam ray is cast through each corner of the grid's cells and walks the
//! pyramid until it meets a cell that fills or lacks, or a split narrower
//! than it can share with the rays about it (`cast::narrow`); each cell starts
//! from the least of the beam rays about it, shortened by a margin
//! (`hand_dist`), and what resumes from the line that beam ray left is the
//! samples, `SAMPLE` an angular cell (`grid::CELL`) a side, walked to the
//! cell that fills; the cells they stopped at are drawn as boxes, and each
//! drawn pixel takes the nearest its own ray enters (`raster`), walking from
//! its cell's start only where that is a node a sample lacks at, or none.
//! The lattice is the grid's own, so neither the beam rays nor the samples
//! follow the screen's pixels. The beam rays and cells lie `lattice` cells
//! apart (`Grid::of`), the samples `SAMPLE` a cell a side at every lattice.

use crate::cast;
use crate::grid::{CELL, LATTICE_MAX};
use crate::param::beam;
use crate::param::beam::NEAR;
use crate::param::beam::SAMPLE;
use crate::pyramid;
use crate::world;

/// The camera grid as a frame lays it over the screen, worked out once,
/// from the screen's tangents (`of`), where the kernel writes the
/// eye's words (pxl-kernel domain/view/lens.rs `gpu_cast`, pxl-engine
/// `eye::GRID`): every thread of the cast reads it off them
/// (pxl-shader `grid`), and the renderer sizes the cast's dispatches
/// and its buffer off the same words.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    /// The screen's half-angles' tangents, across and up: the eye's own
    /// (`eye::TAN`), so not among the grid's words.
    pub tan: [f32; 2],
    /// The lattice's side as an angle, `lattice` cells (`grid::CELL`), in
    /// the screen's tangents at its axis: what the beam rays lie apart by,
    /// and the screen's cells, whose starts they find, a side.
    pub s: f32,
    /// What a beam ray's hit is shortened by, `1 − γ`, the margin `γ`
    /// within which two rays enter a cube under `2√2 · s · d` wide apart
    /// (`hand_dist`).
    keep: f32,
    /// As far as a cube a beam ray must cross reaches from its center, `√6
    /// · s` as an angle with the margin, and its tangent (`window`).
    radius: f32,
    radius_tan: f32,
    /// The corners past the screen on each side: the widest window of its
    /// cells, the last's, whose far corner lies past the screen by under a
    /// cell where the cells do not divide it.
    past: u32,
    /// The screen's cells across and up, as many as span it from its left
    /// and top; the grid's corners, as many as span it and `past` more on
    /// each side, so every corner a cell's window reads is cast; and the
    /// samples, `SAMPLE` a cell (`grid::CELL`) and `NEAR` past the screen
    /// on each side, as many as the lattice of one cell lays whatever the
    /// lattice.
    pub span: [u32; 2],
    pub corners: [u32; 2],
    pub samples: [u32; 2],
    /// The lattice, in cells (`grid::CELL`) a side: a power of two from 1 to
    /// `grid::LATTICE_MAX`.
    pub lattice: u32,
    /// Where the cells' starts and the samples' hits start among the beam
    /// rays' words, past the lines and how far each hands on (`hand_at`);
    /// then the boxes' asks, a sample's room
    /// each (`asks_at`), the missed pixels' hits (`hits_at`) and their
    /// asks (`hit_asks_at`), `HITS` each.
    cells_at: u32,
    samples_at: u32,
    pub asks_at: u32,
    pub(crate) hits_at: u32,
    pub hit_asks_at: u32,
    /// The samples the walk over the observed pyramid alone left (pxl-render
    /// sample_hit.wgsl) for the walk that asks the world: their count, then
    /// each, `left_word`.
    pub lefts_at: u32,
    /// The pixels the same walk left for theirs (pxl-render miss_hit.wgsl): their
    /// count, then each, as `pyramid::MARKS_AT` lists them.
    pub misses_at: u32,
    /// The corners the same walk left for theirs (pxl-render beam_hit.wgsl): their
    /// count, then each, `left_word`.
    pub corners_at: u32,
}

/// The grid's words among the eye's (`eye::GRID`): `s`, `keep`, `radius`,
/// `radius_tan` as f32 bits, then `past`, `span`, `corners`, `samples`,
/// `cells_at`, `samples_at`, `asks_at`, `hits_at`, `hit_asks_at`, then
/// `lattice` (`GRID_LATTICE`), `lefts_at`, `misses_at` and `corners_at`: whole
/// vec4s.
pub const GRID_WORDS: usize = 20;
pub const GRID_KEEP: usize = 1;
pub const GRID_PAST: usize = 4;
pub const GRID_SPAN: usize = 5;
pub const GRID_CORNERS: usize = 7;
pub const GRID_SAMPLES: usize = 9;
pub const GRID_CELLS_AT: usize = 11;
pub const GRID_SAMPLES_AT: usize = 12;
pub const GRID_HITS_AT: usize = 14;
pub const GRID_LATTICE: usize = 16;
pub const GRID_LEFTS_AT: usize = 17;
pub const GRID_MISSES_AT: usize = 18;
pub const GRID_CORNERS_AT: usize = 19;

impl Grid {
    /// The grid at the screen's tangents `tan`, its beam rays and cells
    /// `lattice` cells apart, held to the power of two at or under it from 1
    /// to `grid::LATTICE_MAX`: the samples the very rays at every lattice
    /// (`sample`), and as many.
    pub fn of(tan: [f32; 2], lattice: u32) -> Grid {
        let lattice = 1 << (31 - lattice.clamp(1, LATTICE_MAX).leading_zeros());
        let s = lattice as f32 * CELL;
        let r6 = cast::SQRT_6 * s;
        let k = 2.0 * r6;
        let radius = r6 / (1.0 - r6) * beam::RADIUS_SLACK;
        let span = [
            libm::ceilf(2.0 * tan[0] / s) as u32,
            libm::ceilf(2.0 * tan[1] / s) as u32,
        ];
        // As many as the cell's own lattice lays, whatever this one.
        let samples = [
            libm::ceilf(2.0 * tan[0] / CELL) as u32 * SAMPLE + 1 + 2 * NEAR,
            libm::ceilf(2.0 * tan[1] / CELL) as u32 * SAMPLE + 1 + 2 * NEAR,
        ];
        let mut g = Grid {
            tan,
            s,
            keep: 1.0 - k / (1.0 - k),
            radius,
            radius_tan: libm::tanf(radius),
            past: 0,
            span,
            corners: [0; 2],
            samples,
            lattice,
            cells_at: 0,
            samples_at: 0,
            asks_at: 0,
            hits_at: 0,
            hit_asks_at: 0,
            lefts_at: 0,
            misses_at: 0,
            corners_at: 0,
        };
        g.past = g.cell_window([span[0] - 1, span[1] - 1]);
        g.corners = [span[0] + 1 + 2 * g.past, span[1] + 1 + 2 * g.past];
        g.cells_at = g.corners[0] * g.corners[1] * (LINE_WORDS + 1);
        g.samples_at = g.cells_at + span[0] * span[1] * CELL_WORDS;
        g.asks_at = g.samples_at + samples[0] * samples[1] * SAMPLE_WORDS;
        g.hits_at = g.asks_at + samples[0] * samples[1] * ASK_WORDS;
        g.hit_asks_at = g.hits_at + HITS * HIT_WORDS;
        g.lefts_at = g.hit_asks_at + HITS * ASK_WORDS;
        g.misses_at = g.lefts_at + 1 + samples[0] * samples[1];
        g.corners_at = g.misses_at + 1 + pyramid::MARKS;
        g
    }

    /// Its words (`GRID_WORDS`), as `of_words` reads them.
    pub fn words(&self) -> [u32; GRID_WORDS] {
        let f = |v: f32| v.to_bits();
        [
            f(self.s),
            f(self.keep),
            f(self.radius),
            f(self.radius_tan),
            self.past,
            self.span[0],
            self.span[1],
            self.corners[0],
            self.corners[1],
            self.samples[0],
            self.samples[1],
            self.cells_at,
            self.samples_at,
            self.asks_at,
            self.hits_at,
            self.hit_asks_at,
            self.lattice,
            self.lefts_at,
            self.misses_at,
            self.corners_at,
        ]
    }

    /// The grid at the screen's tangents `tan` from its words `w`.
    pub fn of_words(tan: [f32; 2], w: [u32; GRID_WORDS]) -> Grid {
        let f = f32::from_bits;
        Grid {
            tan,
            s: f(w[0]),
            keep: f(w[1]),
            radius: f(w[2]),
            radius_tan: f(w[3]),
            past: w[4],
            span: [w[5], w[6]],
            corners: [w[7], w[8]],
            samples: [w[9], w[10]],
            lattice: w[GRID_LATTICE],
            cells_at: w[11],
            samples_at: w[12],
            asks_at: w[13],
            hits_at: w[14],
            hit_asks_at: w[15],
            lefts_at: w[17],
            misses_at: w[18],
            corners_at: w[19],
        }
    }

    /// How far a beam ray that stopped at `t` hands on: no farther than any
    /// pixel's ray about it enters what stopped it.
    pub fn hand_dist(&self, t: f32) -> f32 {
        t * self.keep
    }

    /// The cells about a pixel at the screen's tangents `at`, a side, whose
    /// corners its start is the least of: the radius stretched by the
    /// screen's slope there out to it, plus the distance from that center
    /// to the corner nearest it.
    fn window(&self, at: [f32; 2]) -> u32 {
        let edge = libm::sqrtf(at[0] * at[0] + at[1] * at[1]);
        let beyond = (edge + self.radius_tan) / (1.0 - edge * self.radius_tan);
        let stretch = 1.0 + beyond * beyond;
        libm::ceilf(core::f32::consts::FRAC_1_SQRT_2 + self.radius * stretch / self.s) as u32
    }

    /// The window of the screen's cell `[x, y]`, from its left and top: at
    /// the corner of it farthest from the screen's axis, as far as any pixel
    /// of it lies, since the slope grows outward.
    pub fn cell_window(&self, [x, y]: [u32; 2]) -> u32 {
        let far = |t: f32, x: u32| {
            libm::fabsf(-t + x as f32 * self.s).max(libm::fabsf(-t + (x + 1) as f32 * self.s))
        };
        self.window([far(self.tan[0], x), far(self.tan[1], y)])
    }

    /// The tangents across and up of corner `[i, j]`, from the screen's
    /// left and top `past` the window.
    pub fn corner(&self, [i, j]: [u32; 2]) -> [f32; 2] {
        let k = self.past as f32;
        [
            -self.tan[0] + (i as f32 - k) * self.s,
            self.tan[1] - (j as f32 - k) * self.s,
        ]
    }

    /// The first corner of the screen's cell `[x, y]`.
    pub fn first(&self, [x, y]: [u32; 2]) -> [u32; 2] {
        [x + self.past, y + self.past]
    }

    /// The screen's cell holding the screen's point at the tangents `[tx,
    /// ty]`.
    pub fn cell(&self, [tx, ty]: [f32; 2]) -> [u32; 2] {
        let at = |v: f32, n: u32| (libm::floorf(v / self.s).max(0.0) as u32).min(n - 1);
        [
            at(tx + self.tan[0], self.span[0]),
            at(self.tan[1] - ty, self.span[1]),
        ]
    }

    /// The samples a side of a screen's cell: `SAMPLE` a `grid::CELL`.
    fn cell_samples(&self) -> u32 {
        SAMPLE * self.lattice
    }

    /// The samples' spacing, in the screen's tangents at its axis: the
    /// lattice's side over its samples a side, both a power of two times
    /// the cell's, so the very f32 at every lattice, `grid::CELL / SAMPLE`.
    fn sample_spacing(&self) -> f32 {
        self.s / self.cell_samples() as f32
    }

    /// The tangents across and up of sample `[i, j]`, from the screen's
    /// left and top `NEAR` samples past it: the same at every lattice.
    pub fn sample(&self, [i, j]: [u32; 2]) -> [f32; 2] {
        let d = self.sample_spacing();
        [
            -self.tan[0] + (i as f32 - NEAR as f32) * d,
            self.tan[1] - (j as f32 - NEAR as f32) * d,
        ]
    }

    /// The sample at or before the screen's point at the tangents `[tx,
    /// ty]`, across and up, from the screen's left and top `NEAR` samples
    /// past it: with the next across, down and both, the four about it.
    pub fn sample_before(&self, [tx, ty]: [f32; 2]) -> [u32; 2] {
        let d = self.sample_spacing();
        let at = |v: f32, n: u32| (libm::floorf(v / d).max(0.0) as u32 + NEAR).min(n - 2);
        [
            at(tx + self.tan[0], self.samples[0]),
            at(self.tan[1] - ty, self.samples[1]),
        ]
    }

    /// The screen's cell sample `[i, j]` starts from: the one holding it, or
    /// the nearest where it lies past the screen, as only the screen's cells
    /// have a start.
    pub fn sample_cell(&self, [i, j]: [u32; 2]) -> [u32; 2] {
        let (n, k) = (self.span, self.cell_samples());
        [
            ((i.max(NEAR) - NEAR) / k).min(n[0] - 1),
            ((j.max(NEAR) - NEAR) / k).min(n[1] - 1),
        ]
    }

    /// The first word of corner `c`'s line, by its index.
    pub fn line(c: u32) -> u32 {
        c * LINE_WORDS
    }

    /// The word of how far corner `c`'s beam ray hands on (`hand_dist`), f32
    /// bits, by its index: a word a corner past the lines, in their order,
    /// so a cell's window reads its corners' row by row off a run of words
    /// apiece, not a line apiece.
    pub fn hand_at(&self, c: u32) -> u32 {
        self.corners[0] * self.corners[1] * LINE_WORDS + c
    }

    /// The first word of the screen's cell `[x, y]`'s start.
    pub fn cell_at(&self, [x, y]: [u32; 2]) -> u32 {
        self.cells_at + (y * self.span[0] + x) * CELL_WORDS
    }

    /// The first word of sample `[i, j]`'s hit.
    pub fn sample_at(&self, [i, j]: [u32; 2]) -> u32 {
        self.samples_at + (j * self.samples[0] + i) * SAMPLE_WORDS
    }

    /// The first word of the ask of the box at `place` among the boxes
    /// (`raster::ARGS_ASKS`).
    pub fn ask_at(&self, place: u32) -> u32 {
        self.asks_at + place * ASK_WORDS
    }

    /// The first word of the missed pixels' hit numbered `r`.
    pub fn hit_at(&self, r: u32) -> u32 {
        self.hits_at + r * HIT_WORDS
    }

    /// The first word of the ask at `place` among the missed pixels' hits'
    /// (`ray::MISS_ARGS_ROOM`).
    pub fn hit_ask_at(&self, place: u32) -> u32 {
        self.hit_asks_at + place * ASK_WORDS
    }

    /// The beam rays' words: the lines, the cells' starts, the samples'
    /// hits, the boxes' asks and the missed pixels' hits and theirs, as
    /// many as the grid lays, whatever the pixels drawn.
    pub fn size(&self) -> u32 {
        self.corners_at + 1 + self.corners[0] * self.corners[1]
    }
}

/// A node as a beam ray's line and a sample's hit each hold it, in words:
/// its zoom, its corner, its roots.
pub const NODE_ZOOM: u32 = 0;
pub const NODE_CORNER: u32 = 1;
pub const NODE_ROOTS: u32 = 4;
pub const NODE_WORDS: u32 = 7;

/// What a beam ray leaves for the pixels about it, in words: the nodes its
/// walk visited, which the steps view counts into every walk resumed from
/// it, then the node it stopped at
/// (`NODE_WORDS`), then, for every node from the root to it, what was
/// handed it (`world::Inheritance::words`), `LINE_STEP` words a zoom, its own at its
/// zoom. A node's entry in the observed pyramid is its key's
/// (`pyramid::entry_of`), so a line holds none. How far it hands on
/// (`Grid::hand_dist`) lies apart from its line, a word a corner past the
/// lines (`Grid::hand_at`), which the cells' windows read alone. The light's
/// beam rays leave the same, their origin after it (pxl-engine
/// `light::LINE_ORIGIN`), and how far they hand on apart as well
/// (`light::hand_at`).
pub const LINE_VISITS: u32 = 0;
pub const LINE_NODE: u32 = 1;
pub const LINE_INHERITANCE: u32 = LINE_NODE + NODE_WORDS;
pub const LINE_STEP: u32 = world::INHERITANCE_WORDS;
pub const LINE_WORDS: u32 = LINE_INHERITANCE + LINE_STEP * cast::DEPTH as u32;

/// What a cell hands its rays, in words, every observer's (`light::cell`):
/// where they start, f32 bits; the corner of the least
/// start, as an index into the corners row by row (pxl-shader observer.rs
/// `idx`), whose line the faces they stop on are asked off; and whether they
/// resume from that line, 1, or walk from the world's root, 0, a beam ray of
/// its window having asked other zooms than they do (pxl-shader observer.rs
/// `cell`). A screen's cell each, in the beam rays' buffer past the lines
/// (`Grid::cell_at`), one binding for both.
pub const CELL_T: u32 = 0;
pub const CELL_CORNER: u32 = 1;
pub const CELL_RESUMES: u32 = 2;
pub const CELL_WORDS: u32 = 3;

/// The samples: rays on a lattice `SAMPLE` a cell (`grid::CELL`) a side,
/// whatever the beam rays' lattice, each resumed from
/// its cell's start as a pixel is and walked to the cell that fills; a
/// pixel takes the nearest of the cells they stopped at that its own ray
/// enters (`raster`), and walks only where none does. The lattice runs
/// `NEAR` samples past the screen on each side.
///
/// What a sample leaves for the pixels, in words: the node it stopped at
/// (`NODE_WORDS`), its zoom `SAMPLE_NONE` where it went on to its reach,
/// the asset there, 1 where that node lacks, which a pixel entering it
/// before any cell walks itself, and the axis the sample entered it
/// across: a face its ray reached through air, so one a pixel may show only
/// where it enters across the same; what lies about that face
/// (`face::word`), asked once here for every pixel that shows it; and the
/// nodes visited from the eye to it, its beam ray's and its own, which a
/// pixel that takes its cell reports in the steps view as the walk it
/// took.
pub const SAMPLE_NODE: u32 = 0;
pub const SAMPLE_ASSET: u32 = SAMPLE_NODE + NODE_WORDS;
pub const SAMPLE_LACKS: u32 = SAMPLE_ASSET + 2;
pub const SAMPLE_AXIS: u32 = SAMPLE_LACKS + 1;
pub const SAMPLE_FACES: u32 = SAMPLE_AXIS + 1;
pub const SAMPLE_VISITS: u32 = SAMPLE_FACES + 1;
pub const SAMPLE_WORDS: u32 = SAMPLE_VISITS + 1;
pub const SAMPLE_NONE: u32 = u32::MAX;

/// The side of a workgroup of the passes over a lattice's samples, a thread a sample, and of
/// those over what they left, a thread a place in the list (pxl-render sample_hit.wgsl,
/// sample.wgsl).
pub const SAMPLE_SIDE: u32 = 8;

/// What a hit's faces are asked off, once a hit (pxl-shader raster.rs
/// `face_cs`), in words: the first word of its node (`NODE_WORDS`) among
/// the beam rays' words, the face its ray entered (pxl-engine `ray`),
/// the first word of the beam ray's line its faces are asked off, the line
/// its cell's rays resumed from (`CELL_CORNER`), and the word its faces are
/// left in, `ASK_NONE` where it asks none. A box's (`Grid::ask_at`), its
/// sample's hit's, its faces left in its sample's words (`SAMPLE_FACES`);
/// a missed pixel's hit's (`Grid::hit_ask_at`), its faces left in the
/// hit (`HIT_OUT`).
pub const ASK_NODE: u32 = 0;
pub const ASK_FACE: u32 = 1;
pub const ASK_LINE: u32 = 2;
pub const ASK_TO: u32 = 3;
pub const ASK_WORDS: u32 = 4;
pub const ASK_NONE: u32 = u32::MAX;

/// The hits of the pixels that missed and walked again (pxl-shader cast.rs
/// `missed`) whose faces are asked, `HITS` at most, a number each as the
/// walks take them, in words: its node (`NODE_WORDS`), the face its ray
/// entered, its pixel, its row over its column, and what it is told
/// (`HIT_OUT`): its faces where it asks them, else `HIT_FOLLOWS` and the
/// number of the hit alike it takes them from, its pixel's left
/// neighbor's, else the one above's, within its square of `HIT_SQUARE` a
/// side, the pixels' columns and rows `HIT_SQUARE` divides. Its pixel's
/// last ray word holds `HIT_WAITS` and its number until its faces are put
/// there (`ray::FACES`), in the walk's own pass. A pixel past `HITS`
/// asks none.
pub const HIT_NODE: u32 = 0;
pub const HIT_FACE: u32 = NODE_WORDS;
pub const HIT_PX: u32 = HIT_FACE + 1;
pub const HIT_OUT: u32 = HIT_PX + 1;
pub const HIT_WORDS: u32 = HIT_OUT + 1;
pub const HITS: u32 = 1 << 19;
pub const HIT_SQUARE: u32 = 16;
pub const HIT_LANES: u32 = 64;
pub const HIT_FOLLOWS: u32 = 1 << 31;
pub const HIT_WAITS: u32 = 1 << 31;

#[cfg(test)]
#[path = "beam.test.rs"]
mod tests;
