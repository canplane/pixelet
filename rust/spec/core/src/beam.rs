// pxl-spec-core/beam.rs

//! gpu-portable: the camera grid's beam.
//! A beam ray is cast through each corner of the grid's cells and walks the
//! pyramid until it meets a cell that fills or lacks, or a split narrower
//! than it can share with the rays about it (`cast::narrow`); each cell starts
//! from the least of the beam rays about it, shortened by a margin
//! (`handed`), and what resumes from the line that beam ray left is the
//! samples, `SAMPLE` a cell a side, walked to the cell that fills; the
//! cells they stopped at are drawn as boxes, and each drawn pixel takes the
//! nearest its own ray enters (`raster`), walking from its cell's start
//! only where that is a node a sample lacks at, or none. The lattice is the
//! grid's own, so neither the beam rays nor the samples follow the
//! screen's pixels.

use crate::grid::CELL;
use crate::param::beam::NEAR;
use crate::param::beam::SAMPLE;

/// The camera grid as a frame lays it over the screen, worked out once,
/// from the screen's tangents (`of`), where the kernel writes the
/// eye's words (pxl-kernel domain/view/lens.rs `gpu_cast`, pxl-spec-core
/// `eye::GRID`): every thread of the cast reads it off them
/// (pxl-shader `grid`), and the renderer sizes the cast's dispatches
/// and its buffer off the same words, so no reader works any of it out
/// again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    /// The screen's half-angles' tangents, across and up: the eye's own
    /// (`eye::TAN`), so not among the grid's words.
    pub tan: [f32; 2],
    /// The cell as an angle (`grid::CELL`), in the screen's tangents at its
    /// axis.
    pub s: f32,
    /// What a beam ray's stop is shortened by, `1 − γ`, the margin `γ`
    /// within which two rays enter a cube under `2√2 · s · d` wide apart
    /// (`handed`).
    keep: f32,
    /// As far as a cube a beam ray must cross reaches from its centre, `√6
    /// · s` as an angle with the margin, and its tangent (`window`).
    reach: f32,
    reach_tan: f32,
    /// The corners past the screen on each side: the widest window of its
    /// cells, the last's, whose far corner lies past the screen by under a
    /// cell where the cells do not divide it.
    past: u32,
    /// The screen's cells across and up, as many as span it from its left
    /// and top; the grid's corners, as many as span it and `past` more on
    /// each side, so every corner a cell's window reads is cast; and the
    /// samples, `SAMPLE` a cell and `NEAR` past the screen on each side.
    pub span: [u32; 2],
    pub corners: [u32; 2],
    pub samples: [u32; 2],
    /// Where the cells' starts and the samples' stops start among the beam
    /// rays' words, past the lines.
    cells_at: u32,
    samples_at: u32,
}

/// The grid's words among the eye's (`eye::GRID`): `s`, `keep`, `reach`,
/// `reach_tan` as f32 bits, then `past`, `span`, `corners`, `samples`,
/// `cells_at`, `samples_at`, and three no reader reads, so the grid is
/// whole vec4s.
pub const GRID_WORDS: usize = 16;

impl Grid {
    /// The grid at the screen's tangents `tan`.
    pub fn of(tan: [f32; 2]) -> Grid {
        let s = CELL;
        let r6 = crate::cast::SQRT_6 * s;
        let k = 2.0 * r6;
        let reach = r6 / (1.0 - r6) * crate::param::beam::REACH_SLACK;
        let span = [
            libm::ceilf(2.0 * tan[0] / s) as u32,
            libm::ceilf(2.0 * tan[1] / s) as u32,
        ];
        let samples = [
            span[0] * SAMPLE + 1 + 2 * NEAR,
            span[1] * SAMPLE + 1 + 2 * NEAR,
        ];
        let mut g = Grid {
            tan,
            s,
            keep: 1.0 - k / (1.0 - k),
            reach,
            reach_tan: libm::tanf(reach),
            past: 0,
            span,
            corners: [0; 2],
            samples,
            cells_at: 0,
            samples_at: 0,
        };
        g.past = g.cell_window([span[0] - 1, span[1] - 1]);
        g.corners = [span[0] + 1 + 2 * g.past, span[1] + 1 + 2 * g.past];
        g.cells_at = g.corners[0] * g.corners[1] * LINE_WORDS;
        g.samples_at = g.cells_at + span[0] * span[1] * CELL_WORDS;
        g
    }

    /// Its words (`GRID_WORDS`), as `of_words` reads them.
    pub fn words(&self) -> [u32; GRID_WORDS] {
        let f = |v: f32| v.to_bits();
        [
            f(self.s),
            f(self.keep),
            f(self.reach),
            f(self.reach_tan),
            self.past,
            self.span[0],
            self.span[1],
            self.corners[0],
            self.corners[1],
            self.samples[0],
            self.samples[1],
            self.cells_at,
            self.samples_at,
            0,
            0,
            0,
        ]
    }

    /// The grid at the screen's tangents `tan` from its words `w`.
    pub fn of_words(tan: [f32; 2], w: [u32; GRID_WORDS]) -> Grid {
        let f = f32::from_bits;
        Grid {
            tan,
            s: f(w[0]),
            keep: f(w[1]),
            reach: f(w[2]),
            reach_tan: f(w[3]),
            past: w[4],
            span: [w[5], w[6]],
            corners: [w[7], w[8]],
            samples: [w[9], w[10]],
            cells_at: w[11],
            samples_at: w[12],
        }
    }

    /// How far a beam ray that stopped at `t` hands on: no farther than any
    /// pixel's ray about it enters what stopped it.
    pub fn handed(&self, t: f32) -> f32 {
        t * self.keep
    }

    /// The cells about a pixel at the screen's tangents `at`, a side, whose
    /// corners its start is the least of: the reach stretched by the
    /// screen's slope there out to it, plus the distance from that centre
    /// to the corner nearest it.
    fn window(&self, at: [f32; 2]) -> u32 {
        let edge = libm::sqrtf(at[0] * at[0] + at[1] * at[1]);
        let beyond = (edge + self.reach_tan) / (1.0 - edge * self.reach_tan);
        let stretch = 1.0 + beyond * beyond;
        libm::ceilf(core::f32::consts::FRAC_1_SQRT_2 + self.reach * stretch / self.s) as u32
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

    /// The tangents across and up of sample `[i, j]`, from the screen's
    /// left and top `NEAR` samples past it.
    pub fn sample(&self, [i, j]: [u32; 2]) -> [f32; 2] {
        let d = self.s / SAMPLE as f32;
        [
            -self.tan[0] + (i as f32 - NEAR as f32) * d,
            self.tan[1] - (j as f32 - NEAR as f32) * d,
        ]
    }

    /// The sample at or before the screen's point at the tangents `[tx,
    /// ty]`, across and up, from the screen's left and top `NEAR` samples
    /// past it: with the next across, down and both, the four about it.
    pub fn sample_before(&self, [tx, ty]: [f32; 2]) -> [u32; 2] {
        let d = self.s / SAMPLE as f32;
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
        let n = self.span;
        [
            ((i.max(NEAR) - NEAR) / SAMPLE).min(n[0] - 1),
            ((j.max(NEAR) - NEAR) / SAMPLE).min(n[1] - 1),
        ]
    }

    /// The first word of corner `c`'s line, by its index.
    pub fn line(c: u32) -> u32 {
        c * LINE_WORDS
    }

    /// The first word of the screen's cell `[x, y]`'s start.
    pub fn cell_at(&self, [x, y]: [u32; 2]) -> u32 {
        self.cells_at + (y * self.span[0] + x) * CELL_WORDS
    }

    /// The first word of sample `[i, j]`'s stop.
    pub fn sample_at(&self, [i, j]: [u32; 2]) -> u32 {
        self.samples_at + (j * self.samples[0] + i) * SAMPLE_WORDS
    }

    /// The beam rays' words: the lines, the cells' starts and the samples'
    /// stops, as many as the grid lays, whatever the pixels drawn.
    pub fn size(&self) -> u32 {
        self.samples_at + self.samples[0] * self.samples[1] * SAMPLE_WORDS
    }
}

/// A node as a beam ray's line and a sample's stop each hold it, in words:
/// its zoom, its corner, its roots.
pub const NODE_ZOOM: u32 = 0;
pub const NODE_CORNER: u32 = 1;
pub const NODE_ROOTS: u32 = 4;
pub const NODE_WORDS: u32 = 7;

/// What a beam ray leaves for the pixels about it, in words: how far it
/// hands on (`Grid::handed`), f32 bits, the nodes its walk visited, which
/// the steps view counts into every walk resumed from it, then the node it
/// stopped at
/// (`NODE_WORDS`), then, for every node from the root to it, what was
/// handed it (`world::Handed::words`), `LINE_STEP` words a zoom, its own at its
/// zoom. A node's entry in the observed pyramid is its key's
/// (`observed::entry_of`), so a line holds none. The sun's beam rays leave
/// the same, their origin after it (pxl-spec-core `sun::LINE_ORIGIN`).
pub const LINE_T: u32 = 0;
pub const LINE_VISITS: u32 = 1;
pub const LINE_NODE: u32 = 2;
pub const LINE_HANDED: u32 = LINE_NODE + NODE_WORDS;
pub const LINE_STEP: u32 = crate::world::HANDED_WORDS;
pub const LINE_WORDS: u32 = LINE_HANDED + LINE_STEP * crate::cast::DEPTH as u32;

/// What a cell hands its pixels, in words: where they start, f32 bits, and
/// the corner whose line they resume from, as an index into the corners
/// row by row (pxl-shader observer.rs `index`). A screen's cell each, in
/// the beam rays' buffer past the lines (`Grid::cell_at`), one binding for
/// both.
pub const CELL_T: u32 = 0;
pub const CELL_CORNER: u32 = 1;
pub const CELL_WORDS: u32 = 2;

/// The samples: rays on a lattice `SAMPLE` a cell a side, each resumed from
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn largest_beam_fits_binding() {
        // 4112 × 2338 drawn, a ratio of 2, at the view's FOV (the tangent of
        // its half 1/2, pxl-spec space/camera/grid.rs `HALF_TAN`), within the
        // 33,554,432 words a storage binding holds on every device
        // (pxl-render floor.rs).
        let up = 0.5;
        let words = Grid::of([up * 4112.0 / 2338.0, up]).size();
        assert!(words <= 33_554_432, "{words} words");
    }
}
