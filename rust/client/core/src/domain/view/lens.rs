// pxl-core/domain/view/lens.rs

//! The camera grid through the screen: the cell a distance holds
//! (`cell_there`), how long a motion takes to be seen there and a clock
//! stepped by it (`seen_step_s`, `Stepped`), how much of it a thing spans, a
//! model, an impostor or culled, and the clip levels the ground's decor is
//! generated at by it (`decor_zooms`), and what the GPU's cast of the
//! screen's rays reads (`gpu_cast`), a ray a drawn pixel, one rule wherever
//! the eye is.

use pxl_content::scatter;
use pxl_engine::clipmap::FIRST_CELL;
use pxl_engine::LEAF_CELL;
use pxl_engine::{beam, eye};
use pxl_spec::asset::decor::{Decor, DECORS};
use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::camera::grid::{split_dist, CELL, CLIP_CELLS_A_RAD, CLIP_MARGIN};
use pxl_spec::space::coord::world::origin;
use pxl_spec::space::coord::zoom::Zoom;

use crate::domain::query::zoom_range;
use crate::domain::view::observation;
use crate::domain::view::param;
use crate::domain::view::setting::store::settings_store;
use crate::sys::io::loader::catalog;

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

/// A decor is drawn as a model down to `DECOR_MODEL_CELLS` of a camera
/// cell, the model's edge, an angle about the eye in radians (pxl-spec
/// space/camera/grid.rs `CELL`), so no screen's size or ratio moves it.
pub fn decor_model_angle() -> f64 {
    param::DECOR_MODEL_CELLS * CELL
}

/// The impostor's edge, `DECOR_IMPOSTOR_CELLS` of a camera cell, as an angle as
/// the model's is (`decor_model_angle`).
pub fn decor_impostor_angle() -> f64 {
    param::DECOR_IMPOSTOR_CELLS * CELL
}

/// How far a thing `extent_u` wide is an impostor's worth as a clip level holds
/// it, in units, past which it is culled: where it spans
/// `DECOR_IMPOSTOR_CELLS` of the clip level's cells (`CLIP_CELLS_A_RAD`),
/// `CLIP_MARGIN` times as far as `decor_impostor_angle` puts the
/// impostor's edge, an upper bound. `is_decor_shown` takes both its
/// sides at the clip level's, so its zooms are the view's.
pub const fn cull_dist_u(extent_u: f64) -> f64 {
    extent_u * CLIP_CELLS_A_RAD / param::DECOR_IMPOSTOR_CELLS
}

/// Whether a decor `extent_u` wide is an impostor's worth anywhere cells of
/// `cell_zoom` are drawn: as it spans where that zoom begins.
fn is_decor_shown(extent_u: f64, cell_zoom: Zoom) -> bool {
    split_dist(cell_zoom) * CLIP_MARGIN <= cull_dist_u(extent_u)
}

/// Each kind's clip level zooms, a bit each, where the ground generates its
/// decor: the leaves', and each coarser one where one of its variants stands
/// as itself, its own size no narrower than the clip level's cell (pxl-engine
/// `scatter::stands_at`), the same cells' decor read at every one, each where
/// the kind is an impostor's worth; none with the decor setting off, nor
/// before the catalog that measures the variants is read. A finer clip level
/// lies within the leaves'. What the frame generates (domain/scene/decor/gpu.rs
/// `frame`) and what the pointer strikes of it (domain/control/edit/pick.rs
/// `ray_pick`) are both these.
pub fn decor_zooms() -> [u32; DECORS.len()] {
    if !settings_store().decor {
        return [0; DECORS.len()];
    }
    zooms_by(|d| d.extent(catalog()))
}

/// `decor_zooms` with the setting on, each kind's extent `extent` gives.
fn zooms_by(extent: impl Fn(&Decor) -> f64) -> [u32; DECORS.len()] {
    core::array::from_fn(|k| {
        let (d, extent) = (&DECORS[k], extent(&DECORS[k]));
        (FIRST_CELL..=LEAF_CELL)
            .filter(|&cell| {
                let stands = scatter::any_stands_at(d.row, cell);
                stands && is_decor_shown(extent, cell)
            })
            .fold(0, |bits, cell| bits | 1 << (cell - FIRST_CELL))
    })
}

/// What a GPU's cast of the screen's rays reads of the eye (pxl-engine
/// `eye`, pxl-render eye_bind.wgsl): its Q3 position as a ray's origin, over
/// the world where it stands there (pxl-spec `world::origin`); the unit forward
/// and how far it draws (observation.rs `eye_draw_dist`); the unit right;
/// the unit up; the tangents of the
/// screen's half-angles across and up and the zooms a cell is asked at;
/// the camera grid the cast lays over the screen off them, once, its beam
/// rays and cells a cell apart (pxl-engine `beam::Grid::of`). A
/// ray a drawn pixel, through its middle. The cuts are the scene's. None
/// without a screen.
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
    // The grid off the very f32s the words above hold, so every reader's is
    // this one (pxl-engine `beam::Grid`).
    put(
        eye::GRID,
        &beam::Grid::of(lens.tan.map(|t| t as f32)).words(),
    );
    Some(())
}

#[cfg(test)]
#[path = "lens.test.rs"]
mod test;
