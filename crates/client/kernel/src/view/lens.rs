// pxl-kernel/view/lens.rs

//! The camera grid through the screen: the quality the eye's rays are cast
//! at (`tau`), and what the GPU's cast of the screen's rays reads
//! (`gpu_cast`), a ray a drawn pixel, one rule wherever the eye is
//! (doc/minutes/20261012-the-ground-is-a-grid-then-overwrites.md, "What is
//! drawn").

use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::camera::grid::{target_cell_px, Tau, SPLIT};
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world::origin;
use pxl_spec_core::{beam, eye};

use crate::query::asked;
use crate::view::frame::pose::view;

/// The camera grid's cell at `tau`, as an angle.
pub fn cell_angle(tau: Tau) -> f64 {
    tau.scale() / SPLIT
}

/// The camera grid's cell `distance` from the eye, in units: what spans it is
/// observed there, and a motion that carries something this far is seen.
pub fn cell_there(distance: f64) -> f64 {
    cell_angle(tau()) * distance
}

/// The quality: a cell spans as many drawn pixels at the optical axis as
/// the camera grid's at the reference view (`target_cell_px`), never finer
/// than `SPLIT`, which is stated at the finest ratio: at a lower one the
/// cell is wider by as much. The screen plane stands `height / (2 · tan(fov /
/// 2))` drawn pixels from the eye, so `n` of them span `2 · atan(n / 2 /
/// that)`.
/// Held by the view, worked out where its height or its surface's ratio
/// change (`tau_of`).
pub fn tau() -> Tau {
    view().tau
}

/// `tau` of a view `height` CSS pixels tall, drawn at `ratio`.
pub fn tau_of(height: f64, ratio: f64) -> Tau {
    let tan_up = libm::tan(crate::param::FOV.to_radians() / 2.0);
    let half = target_cell_px() * tan_up / (height * ratio);
    Tau::of((2.0 * libm::atan(half) * SPLIT).max(1.0))
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
        let tan = [v3::dot(&n(0), &forward) / v3::dot(&n(0), &right), v3::dot(&n(2), &forward) / v3::dot(&n(2), &up)];
        tan.iter().all(|t| t.is_finite() && *t > 0.0).then_some(Lens { forward, right, up, tan })
    }
}

/// What a GPU's cast of the screen's rays reads of the eye (pxl-spec-core
/// `eye`, pxl-shader `Eye`): its Q3 position as a ray's origin, over
/// the world where it stands there (pxl-spec `world::origin`), and whether each ray
/// reports the nodes its descent asked, not what it met; the unit forward
/// and the reach; the unit right and τ; the unit up; the tangents of the
/// screen's half-angles across and up and the zooms a block is asked at;
/// the camera grid the cast lays over the screen off them, once. A
/// ray a drawn pixel, through its middle. The cuts are the scene's. None
/// without a screen.
pub fn gpu_cast(o: &Observation, tau: Tau, steps: bool, words: &mut [u32; eye::WORDS]) -> Option<()> {
    let lens = Lens::of(o)?;
    let q = origin(o.eye[0], o.eye[1], o.eye[2]);
    let w = |v: f64| (v as f32).to_bits();
    let mut put = |at: usize, v: &[u32]| words[at..at + v.len()].copy_from_slice(v);
    put(eye::ORIGIN, &q.map(|v| v as u32));
    put(eye::STEPS, &[u32::from(steps)]);
    put(eye::FORWARD, &lens.forward.map(w));
    put(eye::REACH, &[w(o.reach)]);
    put(eye::RIGHT, &lens.right.map(w));
    put(eye::TAU, &[w(tau.scale())]);
    put(eye::UP, &lens.up.map(w));
    put(eye::TAN, &lens.tan.map(w));
    put(eye::RANGE, &asked());
    // The grid off the very f32s the words above hold, so every reader's is
    // this one (pxl-spec-core `beam::Grid`).
    put(eye::GRID, &beam::Grid::of(lens.tan.map(|t| t as f32), tau.scale() as f32).words());
    Some(())
}

fn unit(v: V3) -> Option<V3> {
    let l = v3::len(&v);
    (l > 1e-12 && l.is_finite()).then(|| v.map(|c| c / l))
}
