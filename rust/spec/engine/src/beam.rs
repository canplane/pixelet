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
use crate::raster::FACES_LANES;
use crate::ray;
use crate::world;
use crate::WORKGROUPS_MOST;

/// The camera grid as a frame lays it over the screen, worked out once,
/// from the screen's tangents (`of`), where the kernel writes the
/// eye's words (pxl-kernel domain/view/lens.rs `gpu_cast`, pxl-engine
/// `eye::GRID`): every thread of the cast reads it off them
/// (pxl-render grid.wgsl), and the renderer sizes the cast's dispatches
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
    /// Where how far each beam ray hands on starts among the beam rays' words, past the
    /// lines (`hand_at`).
    hands_at: u32,
    /// Where the cells' starts and the samples' hits start among the beam
    /// rays' words, past the lines and how far each hands on (`hand_at`);
    /// then the asks of the drawable entries' faces, a sample's room each (`asks_at`;
    /// the voxels themselves are the node cache's, `record`), the holes' stops
    /// (`hits_at`) and the asks of the faces they opened (`stop_asks_at`), `HITS` each.
    cells_at: u32,
    samples_at: u32,
    pub asks_at: u32,
    pub(crate) hits_at: u32,
    pub stop_asks_at: u32,
    /// Where the beam rays' runs park them, a state each (`ray::STATE_WORDS`, pxl-render
    /// corners.wgsl), in the corners' order.
    pub corner_states_at: u32,
    /// Where the samples lie off the lattice's own places, across and down, in the
    /// screen's tangents at its axis: one of the rolled lattice's offsets (`roll`), none
    /// as a grid is laid (`of`), set for the CPU's grid alone (`rolled`). A GPU reads the
    /// offset its cast casts off the rolled lattice's words (`node_cache::ROLL_K`), not off
    /// the grid's.
    pub roll: [f32; 2],
}

/// The samples' spacing, in the screen's tangents at its axis: a lattice's
/// side over its samples a side, both a power of two times the cell's, so
/// the very f32 at every lattice (`Grid::sample`); and its reciprocal, which
/// a tangent is scaled by to count samples (`Grid::sample_before`), the
/// same constant on the CPU and the GPU (pxl-render cast.wgsl
/// `sample_before`, sample.eye.wgsl).
pub const SAMPLE_SPACING: f32 = CELL / SAMPLE as f32;
pub const SAMPLE_SPACING_INVERSE: f32 = 1.0 / SAMPLE_SPACING;

/// The rolled lattice's strata a side: the spacing split this many ways across and down,
/// a power of two.
const ROLL_SIDE: u32 = 4;

/// The rolled lattice: the offsets (`roll`) a standing view's samples are cast at, the screen
/// the eye's one region. The offsets together lay a regular lattice the spacing over
/// `ROLL_SIDE` apart, `CELL`/16: a region whose projection holds a square of that side
/// aligned to it, or any disc √2 · `CELL`/16 across, holds a sample of some offset.
///
/// An offset counts only once every sample of it has ended. The samples are walked under the
/// frame's budget, the eye's lattice's share of it (`ray::caps`, `count::EYE_LATTICE`), so a
/// frame waits for no sample: one the budget does not reach is parked in its own words
/// (`SAMPLE_PARKED`) and goes on from there in the next cast, at the same offset, before any
/// other offset is cast (`node_cache::ROLL_K`; pxl-render roll.wgsl). So a view standing after its last change has
/// every offset counted within `ROLLS` + ⌈W/B⌉ casts, W the samples' work its budget has
/// not reached and B its share; until then the eye owes its lattice (`count::LATTICE_OWED`),
/// and the renderer casts while it does.
pub const ROLLS: u32 = ROLL_SIDE * ROLL_SIDE;

/// The rolled lattice's counted offsets once every one is, a bit an offset
/// (`node_cache::ROLL_DONE`).
pub const ROLLS_DONE: u32 = (1 << ROLLS) - 1;

/// The samples' offset in the `k`th cast of the rolled lattice (`ROLLS`), across and down,
/// in the screen's tangents at its axis: an angle, a stratum of the spacing split
/// `ROLL_SIDE` ways a side, each stratum once in `ROLLS` casts. The order is the ordered
/// dither's: the first is none, so the cast after a change lays the lattice's own places,
/// the first four halve the spacing, and each four after refine it evenly. Every offset
/// lies in [−spacing/2, spacing/2), so no sample lies more than half a spacing off its
/// place; one moved back past its cell's first edge starts from the cell it lies in
/// (`Grid::sample_cell`).
pub fn roll(k: u32) -> [f32; 2] {
    let (mut x, mut y) = (0, 0);
    let mut side = ROLL_SIDE;
    let mut k = k % ROLLS;
    // Each pair of `k`'s bits, low first, picks the quadrant one halving finer: 0 the
    // first, 1 the one diagonal to it, 2 the one across, 3 the one down.
    while side > 1 {
        side >>= 1;
        let q = k & 3;
        x += side * ((q >> 1) ^ (q & 1));
        y += side * (q & 1);
        k >>= 2;
    }
    // The strata past the spacing's middle taken a spacing back, about the place.
    let centered = |v: u32| {
        v as i32
            - if v < ROLL_SIDE / 2 {
                0
            } else {
                ROLL_SIDE as i32
            }
    };
    let share = SAMPLE_SPACING / ROLL_SIDE as f32;
    [centered(x) as f32 * share, centered(y) as f32 * share]
}
const _: () = assert!(ROLL_SIDE.is_power_of_two(), "the offsets halve the spacing");

/// The grid's words among the eye's (`eye::GRID`), where each field lies
/// (`words`): `s`, `keep`, `radius` and `radius_tan` as f32 bits, then
/// `past`, `span`, `corners` and `samples`, across then up, the words where
/// the cells' starts, the samples' hits, the boxes' asks and the holes' stops start, `lattice`, and where the list the walk over
/// the node cache alone leaves starts, the corners', and where the walked
/// pixels' lists lie past the grid's words (`walked_at`, `ray::walked_words`); then the reciprocal of `s`
/// (`s_inverse`) and the slope of the screen's sides across and up
/// (`slope`), f32 bits, each rounded once by the host; then where how far each
/// beam ray hands on starts (`hand_at`), where the asks the hits' stops opened start
/// (`stop_asks_at`).
pub const GRID_S: usize = 0;

pub const GRID_KEEP: usize = 1;
pub const GRID_RADIUS: usize = 2;
pub const GRID_RADIUS_TAN: usize = 3;
pub const GRID_PAST: usize = 4;
pub const GRID_SPAN: usize = 5;
pub const GRID_CORNERS: usize = 7;
pub const GRID_SAMPLES: usize = 9;
pub const GRID_CELLS_AT: usize = 11;
pub const GRID_SAMPLES_AT: usize = 12;
pub const GRID_ASKS_AT: usize = 13;
pub const GRID_HITS_AT: usize = 14;
pub const GRID_LATTICE: usize = 15;
pub const GRID_WALKED_AT: usize = 17;
pub const GRID_CORNER_STATES_AT: usize = 18;
pub const GRID_S_INVERSE: usize = 19;
pub const GRID_SLOPE: usize = 20;
pub const GRID_HANDS_AT: usize = 22;
pub const GRID_STOP_ASKS_AT: usize = 23;
/// Whole vec4s, as the eye's words bind as a uniform's (`eye::WORDS`).
pub const GRID_WORDS: usize = (GRID_STOP_ASKS_AT + 1).next_multiple_of(4);

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
            hands_at: 0,
            cells_at: 0,
            samples_at: 0,
            asks_at: 0,
            hits_at: 0,
            stop_asks_at: 0,
            corner_states_at: 0,
            roll: [0.0; 2],
        };
        g.past = g.cell_window([span[0] - 1, span[1] - 1]);
        g.corners = [span[0] + 1 + 2 * g.past, span[1] + 1 + 2 * g.past];
        g.hands_at = g.corners[0] * g.corners[1] * LINE_WORDS;
        g.cells_at = g.hands_at + g.corners[0] * g.corners[1];
        g.samples_at = g.cells_at + span[0] * span[1] * CELL_WORDS;
        g.asks_at = g.samples_at + samples[0] * samples[1] * SAMPLE_WORDS;
        g.hits_at = g.asks_at + samples[0] * samples[1] * ASK_WORDS;
        g.stop_asks_at = g.hits_at + HITS * HIT_WORDS;
        g.corner_states_at = g.stop_asks_at + HITS * ASK_WORDS;
        g
    }

    /// Its words (`GRID_WORDS`), as `of_words` reads them.
    pub fn words(&self) -> [u32; GRID_WORDS] {
        let mut w = [0; GRID_WORDS];
        (w[GRID_S], w[GRID_KEEP], w[GRID_RADIUS], w[GRID_RADIUS_TAN]) = (
            self.s.to_bits(),
            self.keep.to_bits(),
            self.radius.to_bits(),
            self.radius_tan.to_bits(),
        );
        w[GRID_PAST] = self.past;
        (w[GRID_SPAN], w[GRID_SPAN + 1]) = (self.span[0], self.span[1]);
        (w[GRID_CORNERS], w[GRID_CORNERS + 1]) = (self.corners[0], self.corners[1]);
        (w[GRID_SAMPLES], w[GRID_SAMPLES + 1]) = (self.samples[0], self.samples[1]);
        (
            w[GRID_CELLS_AT],
            w[GRID_SAMPLES_AT],
            w[GRID_ASKS_AT],
            w[GRID_HITS_AT],
        ) = (self.cells_at, self.samples_at, self.asks_at, self.hits_at);
        w[GRID_LATTICE] = self.lattice;
        (w[GRID_WALKED_AT], w[GRID_CORNER_STATES_AT]) = (self.size(), self.corner_states_at);
        w[GRID_HANDS_AT] = self.hands_at;
        w[GRID_STOP_ASKS_AT] = self.stop_asks_at;
        let slope = self.slope();
        (w[GRID_S_INVERSE], w[GRID_SLOPE], w[GRID_SLOPE + 1]) = (
            self.s_inverse().to_bits(),
            slope[0].to_bits(),
            slope[1].to_bits(),
        );
        w
    }

    /// The grid at the screen's tangents `tan` from its words `w`.
    pub fn of_words(tan: [f32; 2], w: [u32; GRID_WORDS]) -> Grid {
        let f = f32::from_bits;
        Grid {
            tan,
            s: f(w[GRID_S]),
            keep: f(w[GRID_KEEP]),
            radius: f(w[GRID_RADIUS]),
            radius_tan: f(w[GRID_RADIUS_TAN]),
            past: w[GRID_PAST],
            span: [w[GRID_SPAN], w[GRID_SPAN + 1]],
            corners: [w[GRID_CORNERS], w[GRID_CORNERS + 1]],
            samples: [w[GRID_SAMPLES], w[GRID_SAMPLES + 1]],
            lattice: w[GRID_LATTICE],
            hands_at: w[GRID_HANDS_AT],
            cells_at: w[GRID_CELLS_AT],
            samples_at: w[GRID_SAMPLES_AT],
            asks_at: w[GRID_ASKS_AT],
            hits_at: w[GRID_HITS_AT],
            stop_asks_at: w[GRID_STOP_ASKS_AT],
            corner_states_at: w[GRID_CORNER_STATES_AT],
            roll: [0.0; 2],
        }
    }

    /// The CPU's grid with its samples at the `k`th offset of the rolled lattice (`roll`), as a
    /// GPU casts them where the rolled lattice's words say `k` (`node_cache::ROLL_K`).
    pub fn rolled(self, k: u32) -> Grid {
        Grid {
            roll: roll(k),
            ..self
        }
    }

    /// The reciprocal of `s`, which a tangent is scaled by to count the
    /// lattice's sides (`cell`, `window`): rounded once here, on the host, and
    /// handed the shaders in a word (`GRID_S_INVERSE`).
    pub fn s_inverse(&self) -> f32 {
        1.0 / self.s
    }

    /// The slope of the screen's sides across and up, `√(1 + tan²)`, which a
    /// side's distance to a point is its offset over (`eye::sees`): rounded
    /// once here, on the host, and handed the shaders in words (`GRID_SLOPE`).
    pub fn slope(&self) -> [f32; 2] {
        self.tan.map(|t| libm::sqrtf(1.0 + t * t))
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
        libm::ceilf(core::f32::consts::FRAC_1_SQRT_2 + self.radius * stretch * self.s_inverse())
            as u32
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
        let at = |v: f32, n: u32| (libm::floorf(v * self.s_inverse()).max(0.0) as u32).min(n - 1);
        [
            at(tx + self.tan[0], self.span[0]),
            at(self.tan[1] - ty, self.span[1]),
        ]
    }

    /// The samples a side of a screen's cell: `SAMPLE` a `grid::CELL`.
    fn cell_samples(&self) -> u32 {
        SAMPLE * self.lattice
    }

    /// The tangents across and up of sample `[i, j]`, from the screen's
    /// left and top `NEAR` samples past it, off its place by the grid's
    /// `roll`: the same at every lattice.
    pub fn sample(&self, [i, j]: [u32; 2]) -> [f32; 2] {
        let d = SAMPLE_SPACING;
        [
            -self.tan[0] + (i as f32 - NEAR as f32) * d + self.roll[0],
            self.tan[1] - (j as f32 - NEAR as f32) * d - self.roll[1],
        ]
    }

    /// The sample at or before the screen's point at the tangents `[tx,
    /// ty]`, across and up, from the screen's left and top `NEAR` samples
    /// past it: with the next across, down and both, the four about it.
    pub fn sample_before(&self, [tx, ty]: [f32; 2]) -> [u32; 2] {
        let at = |v: f32| libm::floorf(v * SAMPLE_SPACING_INVERSE).max(0.0) as u32 + NEAR;
        [
            at(tx + self.tan[0] - self.roll[0]),
            at(self.tan[1] - ty - self.roll[1]),
        ]
    }

    /// The screen's cell sample `[i, j]` starts from: the one holding it, or
    /// the nearest where it lies past the screen, as only the screen's cells
    /// have a start. Its offset (`roll`) is under half a spacing, so where it
    /// moves the sample back, the sample lies between its place and the one
    /// before, in that one's cell.
    pub fn sample_cell(&self, [i, j]: [u32; 2]) -> [u32; 2] {
        let (n, k) = (self.span, self.cell_samples());
        let back = self.roll.map(|r| u32::from(r < 0.0));
        let at = |v: u32, b: u32, n: u32| ((v.max(NEAR + b) - NEAR - b) / k).min(n - 1);
        [at(i, back[0], n[0]), at(j, back[1], n[1])]
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
        self.hands_at + c
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

    /// Where the eye's walked pixels' lists lie among the beam rays' words (`ray::walked_words`):
    /// past the grid's, as many as the pixels drawn take, which the buffer holds beyond `size`.
    pub fn walked_at(&self) -> u32 {
        self.size()
    }

    /// The beam rays' words: the lines, the cells' starts, the samples'
    /// hits, the boxes' asks, the missed pixels' hits and theirs and the corners' list, as many
    /// as the grid lays, whatever the pixels drawn.
    pub fn size(&self) -> u32 {
        self.corner_states_at + self.corners[0] * self.corners[1] * ray::STATE_WORDS
    }
}

/// A node as a beam ray's line and a sample's hit each hold it, in words:
/// its zoom, its corner, its roots.
pub const NODE_ZOOM: u32 = 0;
pub const NODE_CORNER: u32 = 1;
pub const NODE_ROOTS: u32 = 4;
pub const NODE_WORDS: u32 = 7;

/// What a beam ray leaves for the pixels about it, in words: the node it stopped at
/// (`NODE_WORDS`), then, for every node from the root to it, what was
/// handed it (`world::Inheritance::words`), `LINE_STEP` words a zoom, its own at its
/// zoom. A node's entry in the node cache is its key's
/// (pxl-render walk_node.wgsl `bucket_of`), so a line holds none. How far it hands on
/// (`Grid::hand_dist`) lies apart from its line, a word a corner past the
/// lines (`Grid::hand_at`), which the cells' windows read alone. The light's
/// beam rays leave the same, their origin after it (pxl-engine
/// `light::LINE_ORIGIN`), and how far they hand on apart as well
/// (`light::hand_at`).
pub const LINE_NODE: u32 = 0;
pub const LINE_INHERITANCE: u32 = LINE_NODE + NODE_WORDS;
pub const LINE_STEP: u32 = world::INHERITANCE_WORDS;
pub const LINE_WORDS: u32 = LINE_INHERITANCE + LINE_STEP * cast::DEPTH as u32;

/// A beam ray's hand while it has not ended, the frame's budget having parked it
/// (pxl-render corners.wgsl): no distance, which a cell whose window holds one reads as its
/// cone not walked, its rays starting from the root (cell.wgsl, light_cell.wgsl `CELL_RESUMES`).
pub const HAND_PARKED: u32 = f32::INFINITY.to_bits();

/// What a cell hands its rays, in words, every observer's (`light::cell`):
/// where they start, f32 bits; the corner of the least
/// start, as an index into the corners row by row, whose line the faces they
/// stop on are asked off; and whether they resume from that line, 1, or walk
/// from the world's root, 0, a beam ray of its window having asked other zooms
/// than they do (pxl-render cell.wgsl, light_cell.wgsl). A screen's cell each, in the beam rays' buffer past the lines
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
/// What a sample leaves for the boxes, in words: the node it stopped at
/// (`NODE_WORDS`), its zoom `SAMPLE_NONE` where it went on to its reach,
/// the asset there, 1 where that node lacks, which a pixel entering it
/// before any cell walks itself, and the axis the sample entered it
/// across: a face its ray reached through air (`raster::faces`). While its ray has not
/// ended, its zoom is `SAMPLE_PARKED`, and the state the queue parked it in lies in its words
/// (pxl-render samples.wgsl): its distance in its asset's first (`SAMPLE_PARKED_T`), its
/// node's corner in its node's, the rest in whether it lacks (`SAMPLE_PARKED_STATE`).
pub const SAMPLE_NODE: u32 = 0;
pub const SAMPLE_ASSET: u32 = SAMPLE_NODE + NODE_WORDS;
pub const SAMPLE_LACKS: u32 = SAMPLE_ASSET + 2;
pub const SAMPLE_AXIS: u32 = SAMPLE_LACKS + 1;
pub const SAMPLE_WORDS: u32 = SAMPLE_AXIS + 1;

pub const SAMPLE_NONE: u32 = u32::MAX;
pub const SAMPLE_PARKED: u32 = u32::MAX - 1;
pub const SAMPLE_PARKED_T: u32 = SAMPLE_ASSET;
pub const SAMPLE_PARKED_STATE: u32 = SAMPLE_LACKS;

/// The side of a workgroup of the passes over the light's list's cells, a thread a cell
/// (pxl-render light_cell.wgsl, light_list.wgsl `light_item`).
pub const SAMPLE_SIDE: u32 = 8;

/// What a hit's faces are asked off, once a hit (pxl-render face.wgsl
/// `face_cs`), in words: the first word of its node (`NODE_WORDS`) among
/// the beam rays' words, the face its ray entered (pxl-engine `ray`),
/// the first word of the beam ray's line its faces are asked off, the line
/// its cell's rays resumed from (`CELL_CORNER`), and the word its faces are
/// left in, `ASK_NONE` where it asks none. A voxel's face's (`Grid::ask_at`,
/// `stop_asks_at`), asked by the first stop of the voxel to enter it across that
/// face's axis, its node the record's entry (`record::entry_at`) and its face left at
/// that axis among the entry's words (`record::R_FACES`), the word named under
/// `record::TO_RECORD`.
pub const ASK_NODE: u32 = 0;
pub const ASK_FACE: u32 = 1;
pub const ASK_LINE: u32 = 2;
pub const ASK_TO: u32 = 3;
pub const ASK_WORDS: u32 = 4;
pub const ASK_NONE: u32 = u32::MAX;

/// The stops of the holes' rays (pxl-render miss_word.wgsl `hit_put`, queue.wgsl), every one
/// whatever it stopped on, which the record takes (`record`, pxl-render record_stops.wgsl),
/// `HITS` at most, a number each as the walks take them, in words: its node (`NODE_WORDS`),
/// the face its ray entered, `HIT_LACKS` set where it lacks, its pixel, its row over its
/// column, the asset it stopped at, and the first word of the line its pixel's cell's rays
/// resumed from, which its face's word is asked off (`ASK_LINE`). A pixel whose stop's face
/// word is asked holds `HIT_WAITS` and its number in its last ray word until it is put there
/// off the record (`ray::FACES`, record_stops.wgsl `record_faces_cs`), in the cast's own pass.
/// A pixel past `HITS` lists none.
pub const HIT_NODE: u32 = 0;
pub const HIT_FACE: u32 = NODE_WORDS;
pub const HIT_PX: u32 = HIT_FACE + 1;
pub const HIT_ASSET: u32 = HIT_PX + 1;
pub const HIT_LINE: u32 = HIT_ASSET + 2;
pub const HIT_WORDS: u32 = HIT_LINE + 1;
pub const HITS: u32 = 1 << 19;
pub const HIT_LANES: u32 = 64;
pub const HIT_LACKS: u32 = 1 << 31;
pub const HIT_WAITS: u32 = 1 << 31;
const _: () = assert!(
    HITS.div_ceil(HIT_LANES) <= WORKGROUPS_MOST && HITS.div_ceil(FACES_LANES) <= WORKGROUPS_MOST,
    "the kept hits, and their faces' asks, fit one dispatch"
);

#[cfg(test)]
#[path = "beam.test.rs"]
mod tests;
