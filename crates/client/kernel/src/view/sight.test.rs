// pxl-kernel/view/sight.test.rs

use pxl_spec::space::camera::eye::Frustum;
use pxl_spec::space::coord::mat4::{mat4, mat4_frustum_planes, mat4_look_at, mat4_mul, mat4_perspective};

use super::*;

/// As view/frame/pose.rs makes one.
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

/// Square on, a cell shows its side over the distance; a flat cell at a
/// grazing angle shows little, and a cube there its side whole.
#[test]
fn an_error_shows_its_silhouette_over_its_distance() {
    let within = |w: f64, size: f64, r: f64, reach: f64| w >= size / r - 1e-9 && w <= size / (r - reach) + 1e-9;
    let down = Lens::of(&viewed([0.0, 0.0, 100.0], [0.0, 1e-6, 0.0], 50.0, 1.0)).unwrap();
    let w = down.error([0.0, 0.0, 0.0], [4.0, 4.0, 0.0]);
    assert!(within(w, 4.0, 100.0, 32f64.sqrt()), "a cell seen from above: {w}");
    let level = Lens::of(&viewed([0.0, 0.0, 2.0], [100.0, 0.0, 2.0], 50.0, 1.0)).unwrap();
    let w = level.error([1_000.0, 0.0, 0.0], [4.0, 4.0, 0.0]);
    assert!(w < 0.1 * 4.0 / 1_000.0, "a flat cell at a grazing angle: {w}");
    let w = level.error([1_000.0, 0.0, 0.0], [4.0; 3]);
    assert!(w >= 4.0 / 1_000.0 && w <= 3f64.sqrt() * 4.0 / (1_000.0 - 48f64.sqrt()), "a cube at a grazing angle: {w}");
    assert_eq!(level.error([0.0, 0.0, 2.0], [1.0; 3]), f64::INFINITY, "an error at the eye");
    assert_eq!(level.error([1.0, 0.0, 2.0], [1.0; 3]), f64::INFINITY, "an error reaching the eye");
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
