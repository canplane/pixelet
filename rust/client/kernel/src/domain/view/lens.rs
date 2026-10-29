// pxl-kernel/domain/view/lens.rs

//! The camera grid through the screen: the cell a distance holds
//! (`cell_there`), and what the GPU's cast of the screen's rays reads
//! (`gpu_cast`), a ray a drawn pixel, one rule wherever the eye is
//! (doc/minutes/20261012-the-ground-is-a-grid-then-overwrites.md, "What is
//! drawn").

use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::camera::grid::CELL;
use pxl_spec::space::coord::world::origin;
use pxl_spec_core::{beam, eye};

use crate::domain::query::asked;
use crate::domain::view::observation;
use crate::domain::view::param;

/// The camera grid's cell `distance` from the eye, in units: what spans it is
/// observed there, and a motion that carries something this far is seen.
pub fn cell_there(dist: f64) -> f64 {
    CELL * dist
}

/// How long a motion at `speed` u/s takes to carry what it moves the seen
/// step (`SEEN_STEP_CELLS`) of the camera cell `distance` from the eye
/// (`cell_there`), in seconds: the quantum a clock animating it is stepped
/// by, its words standing between steps. Infinite where nothing is drawn
/// (`distance` infinite) or nothing moves.
pub fn seen_step_s(dist: f64, speed: f64) -> f64 {
    param::SEEN_STEP_CELLS * cell_there(dist) / speed
}

/// A clock as it is seen, stepped a quantum at a time: what it shows stands
/// between steps, its source's reading where it last stepped, taken anew
/// once the source reaches the next step from there, or at once where the
/// source went back or it never stepped. Its source is its reader's.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stepped(Option<f64>);

impl Stepped {
    /// Its source at `t`, the next step from a reading `at` of it
    /// `next(at)`: what it shows, the reading it next steps at, and whether
    /// it stepped.
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

/// What a GPU's cast of the screen's rays reads of the eye (pxl-spec-core
/// `eye`, pxl-shader `Eye`): its Q3 position as a ray's origin, over
/// the world where it stands there (pxl-spec `world::origin`), and whether each ray
/// reports the nodes its descent asked, not what it met; the unit forward
/// and how far it draws (observation.rs `eye_draw_reach`); the unit right;
/// the unit up; the tangents of the
/// screen's half-angles across and up and the zooms a cell is asked at;
/// the camera grid the cast lays over the screen off them, once. A
/// ray a drawn pixel, through its middle. The cuts are the scene's. None
/// without a screen.
pub fn gpu_cast(o: &Observation, steps: bool, words: &mut [u32; eye::WORDS]) -> Option<()> {
    let lens = Lens::of(o)?;
    let reach = crate::domain::view::observation::eye_draw_reach()?;
    let q = origin(o.eye[0], o.eye[1], o.eye[2]);
    let w = |v: f64| (v as f32).to_bits();
    let mut put = |at: usize, v: &[u32]| words[at..at + v.len()].copy_from_slice(v);
    put(eye::ORIGIN, &q.map(|v| v as u32));
    put(eye::STEPS, &[u32::from(steps)]);
    put(eye::FORWARD, &lens.forward.map(w));
    put(eye::REACH, &[w(reach)]);
    put(eye::RIGHT, &lens.right.map(w));
    put(eye::UP, &lens.up.map(w));
    put(eye::TAN, &lens.tan.map(w));
    put(eye::RANGE, &asked());
    // The grid off the very f32s the words above hold, so every reader's is
    // this one (pxl-spec-core `beam::Grid`).
    put(
        eye::GRID,
        &beam::Grid::of(lens.tan.map(|t| t as f32)).words(),
    );
    Some(())
}

#[cfg(test)]
#[path = "lens.test.rs"]
mod test;
