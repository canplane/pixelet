// pxl-kernel/world/observer.test.rs

use pxl_spec::space::coord::mat4::{mat4, mat4_frustum_planes, mat4_look_at, mat4_mul, mat4_perspective};
use pxl_spec::space::camera::eye::Frustum;

use super::*;

/// As world/frame/view.rs makes one.
fn viewed(eye: V3, target: V3, fov_deg: f64, aspect: f64) -> Observation {
    let (mut view, mut proj, mut vp) = (mat4(), mat4(), mat4());
    mat4_look_at(&mut view, eye, target, [0.0, 0.0, 1.0]);
    mat4_perspective(&mut proj, fov_deg.to_radians(), aspect, 0.5, 50_000.0);
    mat4_mul(&mut vp, &proj, &view);
    let mut planes = [0.0; 24];
    mat4_frustum_planes(&mut planes, &vp);
    Observation {
        eye,
        frustum: Some(Frustum { planes }),
        reach: 40_000.0,
        fog: f64::INFINITY,
        planet: f64::INFINITY,
        ground: 0.0,
    }
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[test]
fn a_lens_is_the_screen_its_frustum_was_made_from() {
    for (eye, target, fov, aspect) in [([0.0, 0.0, 10.0], [100.0, 0.0, 0.0], 50.0, 16.0 / 9.0), ([5.0, -3.0, 400.0], [-20.0, 60.0, 0.0], 70.0, 9.0 / 19.5)] {
        let lens = Lens::of(&viewed(eye, target, fov, aspect)).expect("a screen");
        let look = unit(sub3(&target, &eye)).unwrap();
        assert!(dot(lens.forward, look) > 1.0 - 1e-9, "{:?} looks elsewhere than {look:?}", lens.forward);
        let tan_up = libm::tan((fov / 2.0).to_radians());
        assert!((lens.tan[1] - tan_up).abs() < 1e-9 && (lens.tan[0] - tan_up * aspect).abs() < 1e-9, "{:?}", lens.tan);
        assert!(dot(lens.right, lens.forward).abs() < 1e-9 && dot(lens.up, lens.forward).abs() < 1e-9 && lens.up[2] > 0.0);
    }
    let around = Observation {
        frustum: None,
        ..viewed([0.0; 3], [1.0, 0.0, 0.0], 50.0, 1.0)
    };
    assert_eq!(Lens::of(&around), None, "a screen for an observer that samples every direction");
}

/// The grid is the same at every quality: it is the estimator's, not the cell's.
#[test]
fn the_patches_cover_the_screen_and_none_is_wider_than_a_patch() {
    let lens = Lens::of(&viewed([0.0, 0.0, 30.0], [100.0, 20.0, 0.0], 50.0, 16.0 / 9.0)).unwrap();
    let grid = Grid::of(lens);
    let [nx, ny] = grid.across();
    assert_eq!(grid.count(), nx * ny);
    let screen = |sx: f64, sy: f64| unit(std::array::from_fn(|k| lens.forward[k] + sx * lens.tan[0] * lens.right[k] + sy * lens.tan[1] * lens.up[k])).unwrap();
    for (corner, at) in [([0, 0], (-1.0, -1.0)), ([nx, 0], (1.0, -1.0)), ([0, ny], (-1.0, 1.0)), ([nx, ny], (1.0, 1.0))] {
        assert!(dot(grid.corner(corner), screen(at.0, at.1)) > 1.0 - 1e-12, "{corner:?} is not the screen's corner");
    }
    for j in 0..=ny {
        for i in 0..=nx {
            for next in [[i + 1, j], [i, j + 1]].into_iter().filter(|c| c[0] <= nx && c[1] <= ny) {
                let apart = libm::acos(dot(grid.corner([i, j]), grid.corner(next)).clamp(-1.0, 1.0));
                assert!(apart <= param::OBSERVED_PATCH + 1e-9, "[{i}, {j}] is {apart} from {next:?}");
            }
        }
    }
}

/// Square on, a cell spans its side over the distance on both axes; at a
/// grazing angle its depth projects small and a height whole.
#[test]
fn an_error_projects_its_size_over_its_distance_on_each_axis() {
    let spans = |angle: f64, size: f64, r: f64, reach: f64| angle >= size / r - 1e-9 && angle <= size / (r - reach) + 1e-9;
    let cell = 32f64.sqrt();
    let down = Lens::of(&viewed([0.0, 0.0, 100.0], [0.0, 1e-6, 0.0], 50.0, 1.0)).unwrap();
    let [across, up] = down.error([0.0, 0.0, 0.0], [4.0, 4.0, 0.0]);
    assert!(spans(across, 4.0, 100.0, cell) && spans(up, 4.0, 100.0, cell), "a cell seen from above: {across}, {up}");
    let level = Lens::of(&viewed([0.0, 0.0, 2.0], [100.0, 0.0, 2.0], 50.0, 1.0)).unwrap();
    let r = (1_000f64.powi(2) + 4.0).sqrt();
    let [across, up] = level.error([1_000.0, 0.0, 0.0], [4.0, 4.0, 0.0]);
    assert!(spans(across, 4.0, r, cell), "a cell seen across: {across}");
    assert!(up < 0.004 * 0.01, "a cell's depth at a grazing angle: {up}");
    let [_, up] = level.error([1_000.0, 0.0, 0.0], [0.0, 0.0, 4.0]);
    // The screen's up leans off vertical by the eye's height over the distance,
    // so a height is a hair under whole.
    assert!(up >= 4.0 * 0.999_99 / r && up <= 4.0 / (r - 4.0) + 1e-9, "a height at a grazing angle: {up}");
    assert_eq!(level.error([0.0, 0.0, 2.0], [1.0; 3]), [f64::INFINITY; 2], "an error at the eye");
    assert_eq!(level.error([1.0, 0.0, 2.0], [1.0; 3]), [f64::INFINITY; 2], "an error reaching the eye");
}

/// Every corner of the box lies within the bound on both axes, near the eye
/// and at a grazing angle, where depth brings the breadth nearer.
#[test]
fn an_error_box_projects_within_its_bound() {
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    for _ in 0..4_000 {
        let eye = [next() * 100.0, next() * 100.0, 0.5 + next() * 300.0];
        let target = [eye[0] + 50.0 + next() * 500.0, eye[1] - 250.0 + next() * 500.0, next() * 10.0];
        let lens = Lens::of(&viewed(eye, target, 50.0, 16.0 / 9.0)).unwrap();
        let (u, v, r) = ((next() * 2.0 - 1.0) * lens.tan[0] * 0.95, (next() * 2.0 - 1.0) * lens.tan[1] * 0.95, 1.0 + next() * 2_000.0);
        let dir = unit(std::array::from_fn(|k| lens.forward[k] + u * lens.right[k] + v * lens.up[k])).unwrap();
        let at: V3 = std::array::from_fn(|k| eye[k] + r * dir[k]);
        let scale = r * libm::pow(10f64, -4.0 + next() * 3.9);
        let e: V3 = [next() * scale, next() * scale, next() * scale];
        let bound = lens.error(at, e);
        for (k, axis) in [lens.right, lens.up].into_iter().enumerate() {
            let a = unit(std::array::from_fn(|j| axis[j] - dir[j] * dot(axis, dir))).unwrap();
            for corner in 0..8 {
                let delta: V3 = std::array::from_fn(|j| if corner >> j & 1 == 1 { e[j] } else { -e[j] });
                let angle = libm::atan2(dot(delta, a), r + dot(delta, dir)).abs();
                assert!(angle <= bound[k] * (1.0 + 1e-9), "{angle} past {} at {r} for {e:?}", bound[k]);
            }
        }
    }
}

fn cube(at: [f64; 2], side: f64, z: (f64, f64)) -> Aabb {
    Aabb {
        min: [at[0], at[1], z.0],
        max: [at[0] + side, at[1] + side, z.1],
    }
}

#[test]
fn a_footprint_touches_what_meets_its_outline_and_heights() {
    let at = |x: f64, y: f64, z: f64| ([x, y, z], 1.0);
    let f = Footprint::of([0.0, 0.0, 10.0], [at(10.0, -5.0, 0.0), at(10.0, 5.0, 0.0), at(100.0, -50.0, 2.0), at(100.0, 50.0, 2.0)]);
    assert!(f.touches(&cube([40.0, -1.0], 2.0, (0.0, 1.0))), "a box inside");
    assert!(f.touches(&cube([95.0, 45.0], 20.0, (1.0, 5.0))), "a box over a corner");
    assert!(!f.touches(&cube([40.0, 30.0], 2.0, (0.0, 1.0))), "a box beside the wedge");
    assert!(!f.touches(&cube([40.0, -1.0], 2.0, (3.0, 9.0))), "a box over its heights");
    assert!(!f.touches(&cube([0.0, -1.0], 2.0, (0.0, 1.0))), "a box before it");
}
