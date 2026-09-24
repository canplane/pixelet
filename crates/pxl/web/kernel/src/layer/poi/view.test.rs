// pxl-kernel/layer/poi/view.test.rs

use super::*;
use pxl_spec::poi::Summary;

fn assess(summary: &Summary, base: [f64; 3], view: &View, globe: Globe) -> Option<Relevance> {
    assess_bounds(summary.bounds, base, view, globe)
}
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
fn proximity_measures_the_nearest_population_extent_not_its_midpoint() {
    let v = camera(100.0);
    let globe = Globe::at(0.0);
    let a = assess(&summary([-200.0, -200.0, 0.0, 200.0, 200.0, 0.0]), [0.0; 3], &v, globe).unwrap();
    let b = assess(&summary([10.0, -200.0, 0.0, 410.0, 200.0, 0.0]), [0.0; 3], &v, globe).unwrap();
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

#[test]
fn the_real_frustum_can_reach_four_deep_regions_with_the_existing_residency_budget() {
    use super::super::store::{Store, LIMITS};
    use pxl_spec::poi::{xy, Node};
    fn summary(key: u64) -> Summary {
        let b = xy::bounds(key);
        let depth = xy::depth(key).unwrap();
        Summary {
            key,
            count: 4u64.pow(xy::MAX_DEPTH - depth),
            bounds: [b[0], b[1], 0.0, b[2] - 0.000001, b[3] - 0.000001, 0.0],
            refinable: depth < xy::MAX_DEPTH,
        }
    }
    fn node(key: u64) -> Node {
        Node {
            revision: 1,
            summary: summary(key),
            children: xy::children(key).into_iter().map(summary).collect(),
        }
    }
    let root = xy::parse("132110320000000").unwrap();
    let b = xy::bounds(root);
    let base = [b[2], b[1], 0.0];
    let roots: Vec<_> = (0..4).map(|i| (root + i, i as f64)).collect();
    let view = camera(0.2);
    let assess = |_, bounds| assess_bounds(bounds, base, &view, Globe::at(0.0));
    let mut store = Store::default();
    store.select(true, &roots, assess, LIMITS);
    for &(key, _) in &roots {
        store.adopt(key, node(key), LIMITS.resident_bytes).unwrap();
    }
    for _ in 0..64 {
        store.select(false, &roots, assess, LIMITS);
        assert!(store.bytes() <= LIMITS.resident_bytes);
        let keys: Vec<_> = store.cut.iter().filter(|s| s.refinable).map(|s| s.key).collect();
        if keys.is_empty() {
            break;
        }
        for key in keys {
            store.adopt(key, node(key), LIMITS.resident_bytes).unwrap();
        }
    }
    assert!(!store.cut.is_empty());
    assert!(store.cut.iter().any(|s| xy::depth(s.key).unwrap() > 23));
    assert!(store.cut.len() <= LIMITS.candidates);
    for _ in 0..10 {
        store.select(false, &roots, assess, LIMITS);
        assert_eq!(store.pending_count(), 0, "a stable view must not re-fetch evicted ancestors or budget-blocked branches");
    }
}
