// pxl-core/domain/view/lens.rs

//! The camera grid laid over the screen. A grid cell spans a fixed angle, so
//! it widens with distance from the eye, by one rule wherever the eye is.
//! This module gives a cell's width at a distance, how long a motion takes
//! to be seen to move there and a clock stepped by that time, and the words
//! the GPU's cast of the screen's rays reads.

use pxl_engine::{beam, eye};
use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::camera::grid::CELL;
use pxl_spec::space::coord::world::origin;

use crate::domain::query::zoom_range;
use crate::domain::view::observation;
use crate::domain::view::param;

/// The width, in world units, of one camera grid cell at `dist` from the
/// eye. Something narrower than a cell is not observed at that distance, and
/// a motion shorter than a cell is not seen.
pub fn cell_there(dist: f64) -> f64 {
    CELL * dist
}

/// The time, in seconds, a motion at `speed` units a second takes to carry
/// what it moves one visible step (`VISIBLE_STEP_CELLS` of the cell at
/// `dist`). A clock that animates the motion is stepped by this time, and
/// what it animates holds still between steps. The time is infinite where
/// nothing is drawn (`dist` is infinite) or nothing moves.
pub fn visible_step_s(dist: f64, speed: f64) -> f64 {
    param::VISIBLE_STEP_CELLS * cell_there(dist) / speed
}

/// A clock that advances a quantum at a time, so what it animates holds
/// still between steps. It shows the source's reading at its last step. It
/// steps to the source's current reading once the source reaches the next
/// step, and at once when the source goes back or on its first reading. The
/// caller passes the source's reading each time it reads the clock.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepClock(Option<f64>);

impl StepClock {
    /// Reads the clock with its source at `t`; `next(at)` gives the reading
    /// of the step after a reading `at`. Returns what the clock shows, the
    /// reading it next steps at, and whether it stepped now.
    pub fn at(&mut self, t: f64, next: impl Fn(f64) -> f64) -> (f64, f64, bool) {
        if let Some(at) = self.0 {
            let due = next(at);
            if !(t >= due || t < at) {
                return (at, due, false);
            }
        }
        self.0 = Some(t);
        (t, next(t), true)
    }
}

/// Writes what the GPU's cast of the screen's rays reads of the eye
/// (pxl-engine `eye`, pxl-render eye_bind.wgsl). The cast sends one ray
/// through the middle of each drawn pixel. The eye's position is written in
/// Q3 as the rays' origin, which may stand over the world (pxl-spec
/// `world::origin`). The scene writes the preview's cuts (`eye::CUTS`).
/// None without a screen.
pub fn gpu_cast(o: &Observation, words: &mut [u32; eye::WORDS]) -> Option<()> {
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
