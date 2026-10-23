// pxl-portable/beam.rs

//! gpu-portable: the camera grid's beam (doc/minutes/20261022-a-cell-casts-a-beam.md).
//! A beam ray is cast through each corner of the grid's cells and walks the
//! pyramid until it meets a block that fills or lacks, or a split narrower
//! than it can share with the rays about it (`narrow`); each drawn pixel
//! starts from the least of the beam rays about its cell, shortened by a
//! margin (`handed`), and resumes from the line that beam ray left. The
//! lattice is the grid's own, so the beam rays do not follow the screen's
//! pixels.

use crate::cast::Detail;
use crate::grid::SPLIT;

/// The camera grid's cell as an angle, at the τ `detail` holds: the beam's
/// spacing, in the screen's tangents at its axis.
pub fn spacing(detail: Detail) -> f32 {
    detail.tau / SPLIT
}

/// Whether a node `side` wide, squared, its farthest point `far` squared
/// from the eye, is too narrow for a beam ray to share: under twice the
/// width that holds one of the grid's rays for certain, `2√2 · s · d`.
pub fn narrow(side2: f32, far2: f32, s: f32) -> bool {
    side2 < 8.0 * s * s * far2
}

/// The margin `γ`: two rays enter a cube under `2√2 · s · d` wide no more
/// than `γ` times where the first enters it apart.
fn margin(s: f32) -> f32 {
    let k = 2.0 * libm::sqrtf(6.0) * s;
    k / (1.0 - k)
}

/// How far a beam ray that stopped at `t` hands on: no farther than any
/// pixel's ray about it enters what stopped it.
pub fn handed(t: f32, s: f32) -> f32 {
    t * (1.0 - margin(s))
}

/// The screen's cells across and up, `tan` its half-angles' tangents: as
/// many as span it from its left and top.
pub fn span(tan: [f32; 2], s: f32) -> [u32; 2] {
    [libm::ceilf(2.0 * tan[0] / s) as u32, libm::ceilf(2.0 * tan[1] / s) as u32]
}

/// The cells about a pixel at the screen's tangents `at`, a side, whose
/// corners its start is the least of: as far as a cube a beam ray must
/// cross reaches from its centre, `√6 · s` as an angle with the margin,
/// stretched by the screen's slope there out to that reach, plus the
/// distance from that centre to the corner nearest it.
pub fn window(at: [f32; 2], s: f32) -> u32 {
    let reach = libm::sqrtf(6.0) * s / (1.0 - libm::sqrtf(6.0) * s) * 1.01;
    let edge = libm::sqrtf(at[0] * at[0] + at[1] * at[1]);
    let r = libm::tanf(reach);
    let beyond = (edge + r) / (1.0 - edge * r);
    let stretch = 1.0 + beyond * beyond;
    libm::ceilf(core::f32::consts::FRAC_1_SQRT_2 + reach * stretch / s) as u32
}

/// The window of the screen's cell `[x, y]`, from its left and top: at the
/// corner of it farthest from the screen's axis, as far as any pixel of it
/// lies, since the slope grows outward.
pub fn cell_window(tan: [f32; 2], s: f32, [x, y]: [u32; 2]) -> u32 {
    let far = |t: f32, x: u32| libm::fabsf(-t + x as f32 * s).max(libm::fabsf(-t + (x + 1) as f32 * s));
    window([far(tan[0], x), far(tan[1], y)], s)
}

/// The corners past the screen on each side: the widest window of its
/// cells, the last's, whose far corner lies past the screen by under a
/// cell where the cells do not divide it.
pub fn past(tan: [f32; 2], s: f32) -> u32 {
    let n = span(tan, s);
    cell_window(tan, s, [n[0] - 1, n[1] - 1])
}

/// The grid's corners across and up: as many as span the screen, and
/// `past` more on each side, so every corner a cell's window reads is cast.
pub fn corners(tan: [f32; 2], s: f32) -> [u32; 2] {
    let (n, k) = (span(tan, s), 2 * past(tan, s));
    [n[0] + 1 + k, n[1] + 1 + k]
}

/// The tangents across and up of corner `[i, j]`, from the screen's left
/// and top `past` the window.
pub fn corner(tan: [f32; 2], s: f32, [i, j]: [u32; 2]) -> [f32; 2] {
    let k = past(tan, s) as f32;
    [-tan[0] + (i as f32 - k) * s, tan[1] - (j as f32 - k) * s]
}

/// The first corner of the screen's cell `[x, y]`, from its left and top
/// `past` the window.
pub fn first(tan: [f32; 2], s: f32, [x, y]: [u32; 2]) -> [u32; 2] {
    let k = past(tan, s);
    [x + k, y + k]
}

/// The cell, by its first corner, holding the screen's point at the
/// tangents `[tx, ty]`.
pub fn cell(tan: [f32; 2], s: f32, [tx, ty]: [f32; 2]) -> [u32; 2] {
    let at = |v: f32| libm::floorf(v / s).max(0.0) as u32;
    first(tan, s, [at(tx + tan[0]), at(tan[1] - ty)])
}

/// What a beam ray leaves for the pixels about it, in words: how far it
/// hands on (`handed`), f32 bits, then the node it stopped at — its zoom,
/// its corner, its roots — then, for every node from the root to it, its
/// entry in the observed pyramid (`observed`, `observed::NONE` where none
/// keeps it) and what was handed it (`world::At::words`), `LINE_STEP` words
/// a zoom, its own at its zoom.
pub const LINE_T: u32 = 0;
pub const LINE_ZOOM: u32 = 1;
pub const LINE_CORNER: u32 = 2;
pub const LINE_ROOTS: u32 = 5;
pub const LINE_AT: u32 = 8;
pub const LINE_STEP: u32 = 1 + crate::world::AT_WORDS;
pub const LINE_WORDS: u32 = LINE_AT + LINE_STEP * crate::cast::DEPTH as u32;

/// What a cell hands its pixels, in words: where they start, f32 bits, and
/// the corner whose line they resume from, as an index into the corners.
/// The cells lie in the beam rays' buffer past their `corners` lines
/// (`cells_at`), one binding for both.
pub const CELL_T: u32 = 0;
pub const CELL_CORNER: u32 = 1;
pub const CELL_WORDS: u32 = 2;

/// The beam rays' buffer, the cast's own for a frame, one binding: how
/// many pixels its reading walk missed (`MISSES`, 0 before the cast), the
/// lines, a corner each (`LINES_AT`), the cells' starts (`cells_at`) and
/// the pixels that missed, a word each (`list_at`), for `corners` corners.
/// The count is at a fixed word, so the dispatch counted off it is read
/// knowing no corner count.
pub const MISSES: u32 = 0;
pub const LINES_AT: u32 = 1;

/// Where the cells start among the beam rays' words, past `corners` lines.
pub const fn cells_at(corners: u32) -> u32 {
    LINES_AT + corners * LINE_WORDS
}

/// Where the pixels that missed are listed, past `corners` cells.
pub const fn list_at(corners: u32) -> u32 {
    cells_at(corners) + corners * CELL_WORDS
}
