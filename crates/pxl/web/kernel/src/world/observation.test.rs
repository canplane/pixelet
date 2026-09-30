// pxl-kernel/world/observation.test.rs

use std::collections::HashSet;

use pxl_spec::space::observation::{ByBounds, Paged, Storage as _};
use pxl_spec::space::v3::V3;

use super::*;
use crate::world::fixture;
use crate::world::observer::{Lens, Sight};

/// The frustum the view draws, carried into world units, sees what the view
/// looks at: the map center it orbits, from every tilt and turn.
#[test]
fn the_view_s_frustum_sees_the_ground_it_looks_at() {
    let _serial = crate::global::serial();
    let v = view();
    v.origin = ivec::gather(3_000_000, 2_000_000, 0);
    let o = crate::control::camera::orbit::orbit_view();
    o.pivot = [500.0, 300.0, 10.0];
    (v.width, v.height) = (1600.0, 900.0);
    for (dist, polar, azimuth) in [(50.0, 0.3, 0.0), (5000.0, 1.3, 2.0), (200.0, 0.8, -1.0)] {
        (o.dist, o.polar, o.azimuth) = (dist, polar, azimuth);
        crate::world::frame::view::view_compose();
        let f = frustum_of_view();
        let center = [3_000_000.0 + 500.0, 2_000_000.0 + 300.0, 10.0];
        let v = view();
        let eye = v.eye_at();
        assert!(f.sees(center, 1.0), "dist {dist}, polar {polar}, azimuth {azimuth}");
        // And not what lies straight behind the eye.
        let behind = [2.0 * eye[0] - center[0], 2.0 * eye[1] - center[1], center[2]];
        assert!(!f.sees(behind, 1.0), "behind, dist {dist}, polar {polar}, azimuth {azimuth}");
    }
}

/// What the estimator from the tiles' footprints requires of `world` seen
/// by `o` at `tau`, as the pages of the pyramid's storage.
fn footprint_pages(o: &Observation, world: &fixture::World, tau: Tau) -> HashSet<LevelPage> {
    let mut sight = Sight::default();
    sight.observe(o, world, true, &[]);
    let mut out = HashSet::new();
    ByFootprints { tau, sight: &sight }.requirements(o, world, &mut |r| Paged.pages(&r, &mut |p| _ = out.insert(p)));
    out
}

const GROUND: f64 = 10.5;

/// Ground behind a wall no tile's rays pass over is required of no level,
/// where the estimator from bounds alone asks for it too.
#[test]
fn ground_no_tile_reaches_is_not_required() {
    let _serial = crate::global::serial();
    let (x0, x1) = (1_192.0, 1_256.0);
    let world = fixture::World::flat(GROUND).with([x0, -50_000.0, GROUND], [x1, 50_000.0, GROUND + 1_000.0]);
    let o = fixture::eye([1_000.5, 2_000.25, GROUND + 2.0], Some([1.0, 0.0, 0.0]), 30_000.0, GROUND);
    let behind = |p: &LevelPage| p.rect().0 > x1;
    let seen = footprint_pages(&o, &world, Tau::FINEST);
    assert!(!seen.is_empty(), "nothing required of ground in view");
    assert!(
        seen.iter().all(|p| !behind(p)),
        "ground behind the wall required: {:?}",
        seen.iter().filter(|p| behind(p)).collect::<Vec<_>>()
    );
    let mut bounded = HashSet::new();
    ByBounds { tau: Tau::FINEST }.requirements(&o, &world, &mut |r| Paged.pages(&r, &mut |p| _ = bounded.insert(p)));
    assert!(bounded.iter().any(behind), "the bounds alone asked nothing behind the wall, so this says nothing");
}

/// Every page a sample of the screen answers from is required: the first
/// thing each of a dense grid of directions meets asks for the page under
/// it at the level its distance chooses — over flat ground and boxes, from
/// near the ground and from over it, looking out and down.
#[test]
fn every_page_a_sample_answers_from_is_required() {
    let _serial = crate::global::serial();
    let mut world = fixture::World::flat(GROUND);
    for k in 0..12 {
        let at = [1_100.0 + 173.0 * k as f64, 1_700.0 + 97.0 * (k % 5) as f64];
        world = world.with([at[0], at[1], GROUND], [at[0] + 20.0, at[1] + 30.0, GROUND + 15.0 + 40.0 * (k % 3) as f64]);
    }
    for (height, forward) in [(2.0, [1.0, 0.0, 0.0]), (40.0, [1.0, 0.3, -0.2]), (600.0, [0.4, 1.0, -0.9])] {
        let eye = [1_000.5, 2_000.25, GROUND + height];
        let o = fixture::eye(eye, Some(forward), 30_000.0, GROUND);
        let tau = Tau::FINEST;
        let required = footprint_pages(&o, &world, tau);
        let lens = Lens::of(&o).unwrap();
        let n = 97;
        for j in 0..n {
            for i in 0..n {
                let (u, v) = ((2.0 * i as f64 / (n - 1) as f64 - 1.0) * lens.tan[0], (2.0 * j as f64 / (n - 1) as f64 - 1.0) * lens.tan[1]);
                let d: V3 = std::array::from_fn(|k| lens.forward[k] + u * lens.right[k] + v * lens.up[k]);
                let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                let d = d.map(|c| c / l);
                let Some((p, t)) = world.first(eye, d, o.far) else { continue };
                let level = tau.level(t).clamp(0, o.top as i32) as u32;
                let span = pxl_spec::space::pyramid::span_of(level);
                let page = pxl_spec::space::pyramid::page_at(level, [(p[0] / span).floor() as i64, (p[1] / span).floor() as i64]);
                assert!(required.contains(&page), "at {height} looking {forward:?}: {page:?} answers a sample {t} away and is not required");
            }
        }
    }
}
