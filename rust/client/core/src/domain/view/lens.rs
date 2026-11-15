// pxl-core/domain/view/lens.rs

//! The camera grid laid over the screen. A grid cell spans a fixed angle, so
//! it widens with distance from the eye, by one rule wherever the eye is.
//! This module gives a cell's width at a distance, how long a motion takes
//! to be seen to move there and a clock stepped by that time, the lod a thing
//! is drawn at by how many cells it spans, the clip levels the ground
//! generates its decor at, and the words the GPU's cast of the screen's rays
//! reads.

use pxl_content::scatter;
use pxl_engine::clipmap::FIRST_CELL_ZOOM;
use pxl_engine::{beam, eye};
use pxl_spec::asset::decor::{DECORS, Decor};
use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::camera::grid::{CELLS_A_RAD, CLIP_CELLS_A_RAD, CLIP_MARGIN, split_dist};
use pxl_spec::space::coord::world::origin;
use pxl_spec::space::coord::zoom::{LEAF_CELL_ZOOM, Zoom};

use crate::domain::query::zoom_range;
use crate::domain::view::observation;
use crate::domain::view::param;
use crate::sys::io::loader::catalog;

/// The view's camera grid cell, an angle in radians, 1/`CELLS_A_RAD` in a double (pxl-spec
/// space/camera/grid.rs `CELL`).
pub(crate) const CELL: f64 = 1.0 / CELLS_A_RAD as f64;

/// The width, in world units, of one camera grid cell at `dist` from the
/// eye. Something narrower than a cell is not observed at that distance, and
/// a motion shorter than a cell is not seen.
pub(crate) fn cell_there(dist: f64) -> f64 {
    CELL * dist
}

/// The time, in seconds, a motion at `speed` units a second takes to carry
/// what it moves one visible step (`VISIBLE_STEP_CELLS` of the cell at
/// `dist`). A clock that animates the motion is stepped by this time, and
/// what it animates holds still between steps. The time is infinite where
/// nothing is drawn (`dist` is infinite) or nothing moves.
pub(crate) fn visible_step_s(dist: f64, speed: f64) -> f64 {
    param::VISIBLE_STEP_CELLS * cell_there(dist) / speed
}

/// A clock that advances a quantum at a time, so what it animates holds
/// still between steps. It shows the source's reading at its last step. It
/// steps to the source's current reading once the source reaches the next
/// step, and at once when the source goes back or on its first reading. The
/// caller passes the source's reading each time it reads the clock.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct StepClock(Option<f64>);

impl StepClock {
    /// Reads the clock with its source at `t`; `next(at)` gives the reading
    /// of the step after a reading `at`. Returns what the clock shows, the
    /// reading it next steps at, and whether it stepped now.
    pub(crate) fn at(&mut self, t: f64, next: impl Fn(f64) -> f64) -> (f64, f64, bool) {
        if let Some(at) = self.0 {
            let due = next(at);
            if !(t >= due || t < at) {
                return (at, due, false);
            }
        }
        self.0 = Some(t);
        (t, next(t), true)
    }

    /// What the clock shows: its source's reading at its last step, or 0
    /// before its first.
    pub(crate) fn time(self) -> f64 {
        self.0.unwrap_or(0.0)
    }
}

/// The angle about the eye, in radians, below which a decor is drawn as an
/// impostor instead of a model: `DECOR_MODEL_CELLS` camera grid cells
/// (`CELL`). Being an angle, it does not change
/// with the screen's size or aspect ratio.
pub(crate) fn decor_model_angle() -> f64 {
    param::DECOR_MODEL_CELLS * CELL
}

/// The angle about the eye, in radians, below which a decor's impostor is
/// culled: `DECOR_IMPOSTOR_CELLS` camera grid cells.
pub(crate) fn decor_impostor_angle() -> f64 {
    param::DECOR_IMPOSTOR_CELLS * CELL
}

/// The distance, in units, past which a clip level culls a decor `extent_u`
/// wide: where it spans `DECOR_IMPOSTOR_CELLS` of the clip level's cells
/// (`CLIP_CELLS_A_RAD`). A clip level's cells are `CLIP_MARGIN` times finer
/// than the view's, so this distance is `CLIP_MARGIN` times the one at which
/// the view culls it (`decor_impostor_angle`), an upper bound.
/// `is_decor_shown` scales its other side by the same margin, so the zooms it
/// picks are the ones the view would pick.
pub(crate) const fn cull_dist_u(extent_u: f64) -> f64 {
    extent_u * CLIP_CELLS_A_RAD as f64 / param::DECOR_IMPOSTOR_CELLS
}

/// Whether a decor `extent_u` wide is still large enough for an impostor
/// somewhere cells of `cell_zoom` are drawn. The nearest such place is where
/// that zoom begins, so the decor is measured there.
fn is_decor_shown(extent_u: f64, cell_zoom: Zoom) -> bool {
    split_dist(cell_zoom) * f64::from(CLIP_MARGIN) <= cull_dist_u(extent_u)
}

/// The clip level zooms the ground generates each decor at, one bit for each
/// zoom from `FIRST_CELL_ZOOM`. A decor is generated at the leaves' zoom, and
/// at each coarser zoom where one of its variants stands as itself, by its
/// own size rather than the clip level's cell (pxl-content
/// `scatter::stands_at`), and is still large enough for an impostor. Every
/// one of those zooms reads the decor of the same cells. No zoom finer than
/// the leaves' is needed, since a finer clip level lies within the leaves'
/// one. `on` is the decor setting, which the caller reads, because the
/// settings stand above the view. With it off, or before the catalog that
/// measures the variants is read, the ground generates its decor at no zoom,
/// while placed decor is still drawn. The frame hands these zooms to the GPU
/// (domain/scene/decor/gpu.rs `frame`) and the pointer's pick strikes by them
/// (domain/control/edit/pick.rs `ray_pick`), so the pointer strikes only the
/// ground's decor the frame generates.
pub(crate) fn decor_zooms(on: bool) -> [u32; DECORS.len()] {
    if !on {
        return [0; DECORS.len()];
    }
    zooms_by(|d| d.extent(catalog()))
}

/// Of `zooms`, the decors the frame generates at the leaves' clip level, one
/// bit a decor in `DECORS`' order.
pub(crate) fn leaf_decors(zooms: &[u32; DECORS.len()]) -> u32 {
    let leaves = |z: u32| z >> (LEAF_CELL_ZOOM - FIRST_CELL_ZOOM) & 1;
    zooms
        .iter()
        .enumerate()
        .fold(0, |bits, (k, &z)| bits | leaves(z) << k)
}

/// `decor_zooms` with the decor setting on, with each decor's extent taken
/// from `extent` instead of the catalog.
fn zooms_by(extent: impl Fn(&Decor) -> f64) -> [u32; DECORS.len()] {
    core::array::from_fn(|k| {
        let (d, extent) = (&DECORS[k], extent(&DECORS[k]));
        (FIRST_CELL_ZOOM..=LEAF_CELL_ZOOM)
            .filter(|&cell_zoom| {
                let stands = cell_zoom >= scatter::first_cell_zoom(d.row);
                stands && is_decor_shown(extent, cell_zoom)
            })
            .fold(0, |bits, cell_zoom| {
                bits | 1 << (cell_zoom - FIRST_CELL_ZOOM)
            })
    })
}

/// Writes what the GPU's cast of the screen's rays reads of the eye
/// (pxl-engine `eye`, pxl-render eye_bind.wgsl). The cast sends one ray
/// through the middle of each drawn pixel. The eye's position is written in
/// Q3 as the rays' origin, which may stand over the world (pxl-spec
/// `world::origin`). The scene writes the preview's cuts (`object::CUTS`).
/// None without a screen.
pub(crate) fn gpu_cast(o: &Observation, words: &mut [u32; eye::WORDS]) -> Option<()> {
    let lens = o.frustum?.lens()?;
    let far = observation::eye_draw_dist()?;
    let q = origin(o.eye[0], o.eye[1], o.eye[2]);
    let w = |v: f64| (v as f32).to_bits();
    let mut put = |at: usize, v: &[u32]| words[at..at + v.len()].copy_from_slice(v);
    put(eye::ORIGIN, &q.map(|v| v as u32));
    put(eye::FORWARD, &lens.forward.map(w));
    put(eye::FAR, &[w(far)]);
    put(eye::RIGHT, &lens.right.map(w));
    put(eye::UP, &lens.up.map(w));
    put(eye::TAN, &lens.tan.map(w));
    put(eye::RANGE, &zoom_range());
    // The grid is built from the same f32 tangents the words above hold, so
    // every reader of these words derives the same grid (pxl-engine
    // `beam::Grid`).
    put(
        eye::GRID,
        &beam::Grid::of(lens.tan.map(|t| t as f32)).words(),
    );
    Some(())
}

#[cfg(test)]
#[path = "lens.test.rs"]
mod test;
