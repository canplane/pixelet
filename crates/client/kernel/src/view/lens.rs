// pxl-kernel/view/lens.rs

//! The camera grid through the screen: the quality the eye's rays are cast
//! at (`tau`), the grid's cells across the screen, and what the GPU's cast of
//! the screen's rays reads (`gpu_cast`), one rule wherever the eye is
//! (doc/minutes/20261012-the-ground-is-a-grid-then-overwrites.md, "What is
//! drawn").

use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::camera::grid::{target_cell_px, Tau, SPLIT};
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world::point;

use crate::query::asked;
use crate::view::frame::pose::view;
use crate::view::frame::resolution::world_ratio;

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
pub fn tau() -> Tau {
    let v = view();
    let tan_up = libm::tan(v.fov.to_radians() / 2.0);
    let half = target_cell_px() * tan_up / (v.height * world_ratio());
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

    /// The camera grid's cells across and up the screen at `tau`, at equal
    /// steps of angle about the optical axis.
    fn across(&self, tau: Tau) -> [u32; 2] {
        self.tan.map(|t| ((2.0 * libm::atan(t) / cell_angle(tau)).ceil() as u32).max(1))
    }

    /// A cell of `across`, as its angles across and up: the view's angle
    /// on each axis cut into as many.
    fn step(&self, across: [u32; 2]) -> [f64; 2] {
        core::array::from_fn(|k| 2.0 * libm::atan(self.tan[k]) / f64::from(across[k].max(1)))
    }

    /// The tangent of the middle of cell `n` of `across` on axis `k`, from the
    /// optical axis.
    fn tangent(&self, across: [u32; 2], k: usize, n: u32) -> f32 {
        libm::tan(-libm::atan(self.tan[k]) + self.step(across)[k] * (f64::from(n) + 0.5)) as f32
    }
}

/// The camera grid's cells across and up the screen at `tau`; none without
/// a screen.
pub fn grid_across(o: &Observation, tau: Tau) -> Option<[u32; 2]> {
    Lens::of(o).map(|l| l.across(tau))
}

/// The eye's words `gpu_cast` writes.
pub const EYE_WORDS: usize = 20;

/// What a GPU's cast of `across` cells reads (pxl-render
/// frame/shader/cast.gen.wgsl `cast_cs`, group 0): `eye`'s first
/// `EYE_WORDS`, binding 0, as five vec4s — the eye's Q3 position; the unit
/// forward and the reach; the unit right and τ; the unit up; the cells
/// across and up and the zooms a block is asked at — and `tangents`, binding
/// 5, each column's tangent then each row's. None without a screen.
pub fn gpu_cast(o: &Observation, tau: Tau, across: [u32; 2], eye: &mut [u32], tangents: &mut Vec<f32>) -> Option<()> {
    let lens = Lens::of(o)?;
    let q = point(o.eye[0], o.eye[1], o.eye[2]);
    let w = |v: f64| (v as f32).to_bits();
    let [f, r, u] = [lens.forward, lens.right, lens.up];
    let [coarsest, finest] = asked();
    eye[..EYE_WORDS].copy_from_slice(&[
        q[0] as u32,
        q[1] as u32,
        q[2] as u32,
        0,
        w(f[0]),
        w(f[1]),
        w(f[2]),
        w(o.far()),
        w(r[0]),
        w(r[1]),
        w(r[2]),
        w(tau.scale()),
        w(u[0]),
        w(u[1]),
        w(u[2]),
        0,
        across[0],
        across[1],
        coarsest,
        finest,
    ]);
    tangents.clear();
    tangents.extend((0..across[0]).map(|i| lens.tangent(across, 0, i)));
    tangents.extend((0..across[1]).map(|j| lens.tangent(across, 1, j)));
    Some(())
}

fn unit(v: V3) -> Option<V3> {
    let l = v3::len(&v);
    (l > 1e-12 && l.is_finite()).then(|| v.map(|c| c / l))
}
