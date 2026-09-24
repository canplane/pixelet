// pxl-kernel/layer/poi/view.rs

use crate::layer::poi::store::Relevance;
use crate::world::frame::view::View;
use pxl_render::look::globe::{Globe, MERCATOR_RADIUS_U};
use pxl_spec::poi::Summary;
use xpute_core::math::mat4::mat4_transform_point;
use xpute_core::math::vec3::Vec3;

// Presentation resolution, independent of raw population and node depth.
const REFINE_PX: f64 = 96.0;

pub fn assess(summary: &Summary, base: [f64; 3], v: &View, globe: Globe) -> Option<Relevance> {
    assess_bounds(summary.bounds, base, v, globe)
}

fn assess_bounds(b: [f64; 6], base: [f64; 3], v: &View, globe: Globe) -> Option<Relevance> {
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
    Some(Relevance { distance, refine: pixels > REFINE_PX })
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
