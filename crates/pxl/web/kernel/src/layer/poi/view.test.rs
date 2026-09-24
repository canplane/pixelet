// pxl-kernel/layer/poi/view.test.rs

use super::*;
use xpute_core::math::mat4::{mat4, mat4_frustum_planes, mat4_look_at, mat4_mul, mat4_perspective};

fn camera(height: f64) -> View {
    let eye = Vec3 { x: 0.0, y: 0.0, z: height };
    let mut vm = mat4();
    mat4_look_at(&mut vm, eye, Vec3::default(), Vec3 { x: 0.0, y: 1.0, z: 0.0 });
    let mut proj = mat4();
    mat4_perspective(&mut proj, 60.0f64.to_radians(), 1.0, 0.1, 100_000.0);
    let mut vp = mat4();
    mat4_mul(&mut vp, &proj, &vm);
    let mut planes = [0.0; 24];
    mat4_frustum_planes(&mut planes, &vp);
    View {
        dist: height,
        polar: 0.0,
        azimuth: 0.0,
        tx: 0.0,
        ty: 0.0,
        tz: 0.0,
        origin: 0,
        width: 800.0,
        height: 800.0,
        fov: 60.0,
        near: 0.1,
        far: 100_000.0,
        eye,
        eye_local: eye,
        view_matrix: vm,
        inv_view_matrix: mat4(),
        proj,
        view_proj: vp,
        inv_view_proj: mat4(),
        root: mat4(),
        inv_root: mat4(),
        local_view_proj: vp,
        planes,
    }
}
fn summary(bounds: [f64; 6]) -> Summary {
    Summary {
        key: 1,
        count: 1000,
        bounds,
        refinable: true,
    }
}

#[test]
fn the_actual_frustum_gates_refinement_before_proximity() {
    let v = camera(100.0);
    let globe = Globe::at(0.0);
    assert!(assess(&summary([-10.0, -10.0, 0.0, 10.0, 10.0, 0.0]), [0.0; 3], &v, globe).unwrap().refine);
    assert!(assess(&summary([200.0, -1.0, 0.0, 202.0, 1.0, 0.0]), [0.0; 3], &v, globe).is_none());
    assert!(assess(&summary([-1.0, -1.0, 200.0, 1.0, 1.0, 200.0]), [0.0; 3], &v, globe).is_none());
    // An ancestor is relevant when some of its population is on screen, even
    // if its eventual midpoint badge would lie outside the screen.
    assert!(assess(&summary([-300.0, -10.0, 0.0, 10.0, 10.0, 0.0]), [0.0; 3], &v, globe).is_some());
}

#[test]
fn projected_resolution_changes_with_zoom_but_not_population() {
    let globe = Globe::at(0.0);
    let mut s = summary([-10.0, -10.0, 0.0, 10.0, 10.0, 0.0]);
    assert!(!assess(&s, [0.0; 3], &camera(10_000.0), globe).unwrap().refine);
    assert!(assess(&s, [0.0; 3], &camera(100.0), globe).unwrap().refine);
    s.count = u64::MAX;
    assert!(!assess(&s, [0.0; 3], &camera(10_000.0), globe).unwrap().refine);
    assert!(assess(&s, [0.0; 3], &camera(100.0), globe).unwrap().refine);
}

#[test]
fn overlapping_extents_still_have_a_camera_proximity_order() {
    let v = camera(100.0);
    let globe = Globe::at(0.0);
    let a = assess(&summary([-200.0, -200.0, 0.0, 200.0, 200.0, 0.0]), [0.0; 3], &v, globe).unwrap();
    let b = assess(&summary([-180.0, -200.0, 0.0, 220.0, 200.0, 0.0]), [0.0; 3], &v, globe).unwrap();
    assert!(a.distance < b.distance);
}

#[test]
fn a_long_offscreen_region_does_not_consume_the_view_cut() {
    let v = camera(100.0);
    // Its bounding sphere crosses the entire viewport; its population does not.
    assert!(assess(&summary([100.0, -500.0, 0.0, 101.0, 500.0, 0.0]), [0.0; 3], &v, Globe::at(0.0)).is_none());
}

#[test]
fn discovery_follows_the_view_across_regions_with_bounded_disjoint_addresses() {
    use pxl_spec::poi::xy;
    let v = camera(100.0);
    let globe = Globe::at(0.0);
    let a = roots([5919914.0, 1887562.0, 0.0], &v, globe, 4);
    let b = roots([5917184.0, 1891328.0, 0.0], &v, globe, 4);
    assert_eq!(a.len(), 4);
    assert_eq!(b.len(), 4);
    assert!(a.iter().all(|(k, _)| xy::depth(*k) == Some(xy::ROOT_DEPTH)));
    assert!(a.iter().all(|(k, _)| !b.iter().any(|(other, _)| other == k)));
    assert!(roots([0.0; 3], &v, globe, 0).is_empty());
}
