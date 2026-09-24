// pxl-kernel/layer/poi/view.rs

use crate::layer::poi::store::Relevance;
use crate::world::frame::view::View;
use pxl_render::look::globe::{Globe, MERCATOR_RADIUS_U};
use xpute_core::math::mat4::mat4_transform_point;
use xpute_core::math::vec3::Vec3;

// Presentation resolution, independent of raw population and node depth.
const REFINE_PX: f64 = 96.0;

#[derive(PartialEq)]
pub struct SelectionView {
    base: [f64; 3],
    root: [f64; 16],
    planes: [f64; 24],
    eye: [f64; 3],
    projection: [f64; 3],
    globe: Globe,
    pub ground: Vec<[f64; 7]>,
}
impl SelectionView {
    pub fn new(base: [f64; 3], v: &View, globe: Globe, ground: Vec<[f64; 7]>) -> Self {
        Self {
            base,
            root: v.root,
            planes: v.planes,
            eye: [v.eye.x, v.eye.y, v.eye.z],
            projection: [v.height, v.fov, v.near],
            globe,
            ground,
        }
    }
}

/// Keep canonical XYZ intact while enclosing displaced ground anchors.
pub fn surface_envelope(bounds: [f64; 6], ground: &[[f64; 7]]) -> [f64; 6] {
    let mut out = bounds;
    for &[x0, y0, x1, y1, z0, z1, half] in ground {
        if bounds[3] < x0 || bounds[4] < y0 || bounds[0] >= x1 || bounds[1] >= y1 {
            continue;
        }
        out[0] = out[0].min(bounds[0].max(x0) - half);
        out[1] = out[1].min(bounds[1].max(y0) - half);
        out[3] = out[3].max(bounds[3].min(x1) + half);
        out[4] = out[4].max(bounds[4].min(y1) + half);
        out[2] = out[2].min(z0);
        out[5] = out[5].max(z1);
    }
    out
}

pub fn assess_bounds(b: [f64; 6], base: [f64; 3], v: &View, globe: Globe) -> Option<Relevance> {
    let center = [(b[0] + b[3]) * 0.5, (b[1] + b[4]) * 0.5, (b[2] + b[5] + 1.0) * 0.5];
    let half = [(b[3] - b[0]) * 0.5, (b[4] - b[1]) * 0.5, (b[5] + 1.0 - b[2]) * 0.5];
    let mut flat = Vec3::default();
    mat4_transform_point(&mut flat, &v.root, center[0] - base[0], center[1] - base[1], center[2] - base[2]);
    let p = globe.place_turned([flat.x, flat.y, flat.z], v.root[1], v.root[0]);
    // A sphere must enclose the curved extent, not just its eight corners.
    // Mercator-to-globe horizontal scale is at most sec(latitude at centre).
    // Height adds a tangential scale and a radial shear, each <= |z| / R.
    let scale = (1.0 + 2.0 * (flat.z.abs() + half[2]) / MERCATOR_RADIUS_U) / globe.cos_lat;
    let radius = half[0].hypot(half[1]).hypot(half[2]) * scale;
    // A sphere around a wide, flat region includes branches entirely beside
    // the view. Test its corners against each plane, allowing for curvature
    // between corners; the margin tends to zero with the region's diameter.
    let corners: [[f64; 3]; 8] = core::array::from_fn(|i| {
        let mut q = Vec3::default();
        mat4_transform_point(
            &mut q,
            &v.root,
            b[if i & 1 == 0 { 0 } else { 3 }] - base[0],
            b[if i & 2 == 0 { 1 } else { 4 }] - base[1],
            b[if i & 4 == 0 { 2 } else { 5 }] - base[2],
        );
        globe.place_turned([q.x, q.y, q.z], v.root[1], v.root[0])
    });
    let curvature = 4.0 * radius * radius / MERCATOR_RADIUS_U;
    if v.planes
        .as_chunks::<4>()
        .0
        .iter()
        .any(|plane| corners.iter().all(|q| plane[0] * q[0] + plane[1] * q[1] + plane[2] * q[2] + plane[3] < -curvature))
    {
        return None;
    }
    let distance = ((p[0] - v.eye.x).powi(2) + (p[1] - v.eye.y).powi(2) + (p[2] - v.eye.z).powi(2)).sqrt();
    let near_distance = (distance - radius).max(v.near);
    let pixels = 2.0 * half[0].max(half[1]) * scale * v.height / (2.0 * (v.fov.to_radians() * 0.5).tan() * near_distance);
    // A large nearby branch must not rank behind a distant small branch
    // merely because its midpoint is far from the camera.
    let eye = [base[0] + v.eye_local.x, base[1] + v.eye_local.y, base[2] + v.eye_local.z];
    let nearest: [f64; 3] = core::array::from_fn(|i| (b[i] - eye[i]).max(eye[i] - b[i + 3]).max(0.0));
    Some(Relevance {
        distance: nearest[0].hypot(nearest[1]).hypot(nearest[2]),
        refine: pixels > REFINE_PX,
    })
}

#[cfg(test)]
#[path = "view.test.rs"]
mod test;

/// Discover nearby XY regions without a source-specific catalogue. The search
/// walks addresses, not entity summaries; unknown Z spans the camera's reach.
/// Its output limit is client discovery policy, independent of the hierarchy.
pub fn roots(base: [f64; 3], v: &View, globe: Globe, limit: usize) -> Vec<(u64, f64)> {
    use pxl_spec::poi::xy;
    let focus = [base[0] - v.tx, base[1] - v.ty, base[2] - v.tz];
    let mut pending = vec![(1u64, 0.0f64)];
    let mut out = Vec::new();
    while !pending.is_empty() && out.len() < limit {
        let at = (0..pending.len()).min_by(|&a, &b| pending[a].1.total_cmp(&pending[b].1).then(pending[a].0.cmp(&pending[b].0))).unwrap();
        let (key, distance) = pending.swap_remove(at);
        if xy::depth(key) == Some(xy::ROOT_DEPTH) {
            out.push((key, distance));
            continue;
        }
        for child in xy::children(key) {
            let b = xy::bounds(child);
            let bounds = [b[0], b[1], focus[2] - v.far, b[2], b[3], focus[2] + v.far];
            if assess_bounds(bounds, base, v, globe).is_none() {
                continue;
            }
            let dx = (b[0] - focus[0]).max(focus[0] - b[2]).max(0.0);
            let dy = (b[1] - focus[1]).max(focus[1] - b[3]).max(0.0);
            pending.push((child, dx.hypot(dy)));
        }
    }
    out
}
