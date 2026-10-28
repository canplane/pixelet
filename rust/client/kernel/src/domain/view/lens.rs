// pxl-kernel/domain/view/lens.rs

//! The camera grid through the screen: the cell a distance holds
//! (`cell_there`), and what the GPU's cast of the screen's rays reads
//! (`gpu_cast`), a ray a drawn pixel, one rule wherever the eye is.

use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::camera::grid::CELL;
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world::origin;
use pxl_spec_core::{beam, eye};

use crate::domain::query::asked;

/// The camera grid's cell `distance` from the eye, in units: what spans it is
/// observed there, and a motion that carries something this far is seen.
pub fn cell_there(distance: f64) -> f64 {
    CELL * distance
}

/// The screen as the eye sees it: its axes and the tangents of its half-angles.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Lens {
    forward: V3,
    right: V3,
    up: V3,
    tan: [f64; 2],
}

impl Lens {
    /// From the frustum's planes (left, right, bottom, top, near, far; normals
    /// in): the axes are differences of opposite normals and the tangents their
    /// ratios. None without a screen.
    fn of(o: &Observation) -> Option<Lens> {
        let planes = o.frustum?.planes;
        let n = |k: usize| -> V3 { [planes[k * 4], planes[k * 4 + 1], planes[k * 4 + 2]] };
        let forward = unit(n(4))?;
        let right = unit(v3::sub(&n(0), &n(1)))?;
        let up = unit(v3::sub(&n(2), &n(3)))?;
        let tan = [
            v3::dot(&n(0), &forward) / v3::dot(&n(0), &right),
            v3::dot(&n(2), &forward) / v3::dot(&n(2), &up),
        ];
        tan.iter()
            .all(|t| t.is_finite() && *t > 0.0)
            .then_some(Lens {
                forward,
                right,
                up,
                tan,
            })
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

fn unit(v: V3) -> Option<V3> {
    let l = v3::len(&v);
    (l > 1e-12 && l.is_finite()).then(|| v.map(|c| c / l))
}
