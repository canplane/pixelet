// pxl-kernel/decor/strike.rs

//! A decor thing struck by its model, as it is drawn: turned, stood up and
//! scaled into the model's frame, met against its triangles. Before its model
//! arrives it is not drawn, and nothing strikes it.

use pxl_spec::block::decor::{Decor, DECORS};
use pxl_spec::block::DecorId;
use pxl_spec::space::coord::v3::{cross3, dot3, sub3, V3};
use xpute_guest::mem::base::ptr_at;

use crate::decor::assets::decor_assets_for;
use crate::layer::instance::{Instance, Meta};
use crate::model::registry::Models;

/// How far any decor, its box or its model, reaches from its box's min corner.
pub fn reach() -> f64 {
    DECORS.iter().map(Decor::reach).fold(0.0, f64::max)
}

/// Where a ray from `o` along `d` first meets `i`'s model, nearer than
/// `limit`; hidden or not, since what an eraser is over is what it hid.
pub fn strike(models: &Models, i: &Instance, o: V3, d: V3, limit: f64) -> Option<f64> {
    let Meta::Decor(m) = i.meta;
    let decor = Decor::of(DecorId::of(i.id)?);
    let mesh = decor_assets_for(decor)?.models.get(m.variant as usize).copied().flatten().and_then(|h| models.mesh(h))?;
    // SAFETY: the model's block, positions first, indices past the vertex words.
    let (positions, indices) = unsafe {
        (
            core::slice::from_raw_parts(ptr_at::<f32>(mesh.at), mesh.vertex_cnt as usize * 3),
            core::slice::from_raw_parts(
                (ptr_at::<u32>(mesh.at)).add(mesh.vertex_cnt as usize * pxl_render::frame::draw::MODEL_VERTEX_WORDS as usize),
                mesh.index_cnt as usize,
            ),
        )
    };
    let s = m.scale * mesh.scale as f64;
    let p = sub3(&o, &i.at);
    if !ray_meets_sphere(&p, &d, mesh.radius as f64 * s, limit) {
        return None;
    }
    // Into the model's frame: spin undone, the stand-up quarter turn about X, scale.
    let (sn, cs) = libm::sincos(-m.spin());
    let unturn = |v: V3| [v[0] * cs - v[1] * sn, v[0] * sn + v[1] * cs, v[2]];
    let to_model = |v: V3| {
        let u = unturn(v);
        [u[0] / s, u[2] / s, -u[1] / s]
    };
    let (mo, md) = (to_model(p), to_model(d));
    let mut nearest: Option<f64> = None;
    for tri in indices.as_chunks::<3>().0 {
        let at = |k: u32| &positions[k as usize * 3..k as usize * 3 + 3];
        if let Some(t) = ray_triangle(&mo, &md, at(tri[0]), at(tri[1]), at(tri[2])).filter(|&t| t < nearest.unwrap_or(limit)) {
            nearest = Some(t);
        }
    }
    nearest
}

/// `p` is relative to the sphere's center.
fn ray_meets_sphere(p: &V3, d: &V3, r: f64, limit: f64) -> bool {
    let b = dot3(p, d);
    let c = dot3(p, p) - r * r;
    if c <= 0.0 {
        return true;
    }
    let disc = b * b - c;
    if disc < 0.0 {
        return false;
    }
    let t = -b - disc.sqrt();
    t >= 0.0 && t <= limit
}

/// Möller–Trumbore, both sides.
fn ray_triangle(o: &[f64; 3], d: &[f64; 3], a: &[f32], b: &[f32], c: &[f32]) -> Option<f64> {
    let e1 = [b[0] as f64 - a[0] as f64, b[1] as f64 - a[1] as f64, b[2] as f64 - a[2] as f64];
    let e2 = [c[0] as f64 - a[0] as f64, c[1] as f64 - a[1] as f64, c[2] as f64 - a[2] as f64];
    let p = cross3(d, &e2);
    let det = dot3(&e1, &p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = [o[0] - a[0] as f64, o[1] - a[1] as f64, o[2] - a[2] as f64];
    let u = dot3(&s, &p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross3(&s, &e1);
    let v = dot3(d, &q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = dot3(&e2, &q) * inv;
    (t > 0.0).then_some(t)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_ray_down_the_z_axis_meets_the_triangle_under_it_at_its_height() {
        let t = ray_triangle(&[0.2, 0.2, 5.0], &[0.0, 0.0, -1.0], &[0.0, 0.0, 1.0], &[1.0, 0.0, 1.0], &[0.0, 1.0, 1.0]);
        assert!(t.is_some_and(|t| (t - 4.0).abs() < 1e-12), "{t:?}");
        assert!(ray_triangle(&[0.2, 0.2, -5.0], &[0.0, 0.0, 1.0], &[0.0, 0.0, 1.0], &[1.0, 0.0, 1.0], &[0.0, 1.0, 1.0]).is_some());
        assert!(ray_triangle(&[0.8, 0.8, 5.0], &[0.0, 0.0, -1.0], &[0.0, 0.0, 1.0], &[1.0, 0.0, 1.0], &[0.0, 1.0, 1.0]).is_none());
    }

    #[test]
    fn a_sphere_is_met_within_reach_and_missed_past_it() {
        assert!(ray_meets_sphere(&[-10.0, 0.5, 0.0], &[1.0, 0.0, 0.0], 1.0, 100.0));
        assert!(!ray_meets_sphere(&[-10.0, 2.0, 0.0], &[1.0, 0.0, 0.0], 1.0, 100.0));
        assert!(!ray_meets_sphere(&[-10.0, 0.5, 0.0], &[1.0, 0.0, 0.0], 1.0, 5.0));
    }
}
