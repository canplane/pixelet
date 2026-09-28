// pxl-spec/space/observation.test.rs
//
// The estimator's laws, against an exact solver on fixtures: flat ground
// with towers standing on it, whose heights are known exactly, and bare
// ground on a small planet. The exact demand is every sample cast through
// the frustum to its first hit, asking for the page holding the hit at the
// level its distance chooses.

use std::collections::HashSet;

use super::*;
use crate::space::lod::FAULT_PAGES;

/// The qualities every law is held at: the criterion itself, and coarser
/// ones, which bring every level's band nearer.
const TAUS: [Tau; 3] = [Tau::FINEST, Tau::of(1.5), Tau::of(8.0)];

/// A box standing on the ground: its footprint and its height.
#[derive(Clone, Copy)]
struct Tower {
    x: [f64; 2],
    y: [f64; 2],
    h: f64,
}

/// Flat ground at zero, towers on it, and the planet's radius: infinite for
/// a flat world, finite for bare ground drawn lower as it recedes.
struct World {
    towers: Vec<Tower>,
    planet: f64,
}

impl Bounds for World {
    fn heights(&self, _level: u32, (x0, y0, side): Rect) -> Option<(f64, f64)> {
        let over = |t: &&Tower| t.x[0] <= x0 + side && t.x[1] >= x0 && t.y[0] <= y0 + side && t.y[1] >= y0;
        Some((0.0, self.towers.iter().filter(over).map(|t| t.h).fold(0.0, f64::max)))
    }
}

impl World {
    /// Where a ray from `e` along the unit `v` first meets anything, as a
    /// distance along it.
    fn first_hit(&self, e: [f64; 3], v: [f64; 3]) -> Option<f64> {
        // The ground drawn at z = −d² / 2R, d the run along it from the eye.
        let (a, b, c) = ((v[0] * v[0] + v[1] * v[1]) / (2.0 * self.planet), v[2], e[2]);
        let ground = if a == 0.0 {
            (b < 0.0).then(|| -c / b)
        } else {
            let disc = b * b - 4.0 * a * c;
            (disc >= 0.0).then(|| (-b - disc.sqrt()) / (2.0 * a)).filter(|&t| t > 0.0)
        };
        let tower = self.towers.iter().filter_map(|t| {
            let (mut lo, mut hi) = (0.0f64, f64::INFINITY);
            for (k, [a, b]) in [t.x, t.y, [0.0, t.h]].into_iter().enumerate() {
                if v[k] == 0.0 {
                    if e[k] < a || e[k] > b {
                        return None;
                    }
                    continue;
                }
                let (t0, t1) = ((a - e[k]) / v[k], (b - e[k]) / v[k]);
                (lo, hi) = (lo.max(t0.min(t1)), hi.min(t0.max(t1)));
            }
            (lo <= hi).then_some(lo)
        });
        tower.chain(ground).min_by(f64::total_cmp)
    }
}

/// An eye and where it looks: a yaw about the vertical, a pitch under the
/// horizon, and half the field across and up.
#[derive(Clone, Copy)]
struct Look {
    eye: [f64; 3],
    yaw: f64,
    pitch: f64,
    half: [f64; 2],
}

const NEAR: f64 = 0.1;

impl Look {
    /// Forward, right and up.
    fn basis(self) -> [[f64; 3]; 3] {
        let (cy, sy, cp, sp) = (self.yaw.cos(), self.yaw.sin(), self.pitch.cos(), self.pitch.sin());
        let f = [cy * cp, sy * cp, -sp];
        let r = [sy, -cy, 0.0];
        let u = [cy * sp, sy * sp, cp];
        [f, r, u]
    }

    fn frustum(self, far: f64) -> Frustum {
        let [f, r, u] = self.basis();
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let mix = |a: [f64; 3], s: f64, b: [f64; 3], t: f64| [a[0] * s + b[0] * t, a[1] * s + b[1] * t, a[2] * s + b[2] * t];
        let [hx, hy] = self.half;
        let sides = [
            mix(f, hx.sin(), r, hx.cos()),
            mix(f, hx.sin(), r, -hx.cos()),
            mix(f, hy.sin(), u, hy.cos()),
            mix(f, hy.sin(), u, -hy.cos()),
        ];
        let mut planes = [0.0; 24];
        for (k, n) in sides.into_iter().enumerate() {
            planes[k * 4..k * 4 + 4].copy_from_slice(&[n[0], n[1], n[2], -dot(n, self.eye)]);
        }
        let back = mix(f, -1.0, f, 0.0);
        planes[16..20].copy_from_slice(&[f[0], f[1], f[2], -dot(f, self.eye) - NEAR]);
        planes[20..24].copy_from_slice(&[back[0], back[1], back[2], dot(f, self.eye) + far]);
        Frustum { planes }
    }

    /// Unit directions across the field, `n` × `n`.
    fn rays(self, n: u32) -> impl Iterator<Item = [f64; 3]> {
        let [f, r, u] = self.basis();
        let [tx, ty] = self.half.map(f64::tan);
        (0..n).flat_map(move |j| {
            (0..n).map(move |i| {
                let (a, b) = ((2.0 * i as f64 / (n - 1) as f64 - 1.0) * tx, (2.0 * j as f64 / (n - 1) as f64 - 1.0) * ty);
                let v = [f[0] + a * r[0] + b * u[0], f[1] + a * r[1] + b * u[1], f[2] + a * r[2] + b * u[2]];
                let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                v.map(|c| c / len)
            })
        })
    }
}

fn observation(look: Look, far: f64, planet: f64, frustum: bool) -> Observation {
    Observation {
        eye: look.eye,
        frustum: frustum.then(|| look.frustum(far)),
        far,
        top: TOP_LEVEL,
        planet,
        ground: 0.0,
    }
}

/// The pages the estimator's requirements map to at `tau`.
fn demand(o: &Observation, bounds: &impl Bounds, tau: Tau) -> HashSet<LevelPage> {
    let mut out = HashSet::new();
    ByBounds { tau }.requirements(o, bounds, &mut |r| Paged.pages(&r, &mut |p| _ = out.insert(p)));
    out
}

/// The exact demand of `look`'s samples at `tau`: each ray's first hit, if
/// it is answered, asks for the page holding it at the level its distance
/// chooses.
fn exact(world: &World, look: Look, far: f64, tau: Tau) -> HashSet<LevelPage> {
    let mut out = HashSet::new();
    for v in look.rays(96) {
        let Some(r) = world.first_hit(look.eye, v) else { continue };
        if r > far || r < NEAR {
            continue;
        }
        let at = [look.eye[0] + r * v[0], look.eye[1] + r * v[1]];
        let l = tau.level(r).clamp(0, TOP_LEVEL as i32) as u32;
        let span = span_of(l);
        out.insert(page_at(l, at.map(|c| (c / span).floor() as i64)));
    }
    out
}

fn next(seed: &mut u64) -> f64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*seed >> 11) as f64 / (1u64 << 53) as f64
}

/// Towers scattered about `at`, some tall and near, some wide and far, and
/// none over `at` itself, where the eye stands.
fn towers(at: [f64; 2], seed: u64) -> Vec<Tower> {
    let mut seed = seed;
    (0..60)
        .map(|k| {
            let reach = if k % 3 == 0 { 20_000.0 } else { 1_500.0 };
            let (x, y) = (at[0] + (next(&mut seed) - 0.5) * reach, at[1] + (next(&mut seed) - 0.5) * reach);
            let (w, h) = (4.0 + next(&mut seed) * 300.0, next(&mut seed) * 900.0);
            Tower { x: [x, x + w], y: [y, y + w], h }
        })
        .filter(|t| !(t.x[0] <= at[0] && at[0] <= t.x[1] && t.y[0] <= at[1] && at[1] <= t.y[1]))
        .collect()
}

fn looks() -> Vec<Look> {
    let mut out = Vec::new();
    for (k, height) in [1.7, 30.0, 400.0, 3_000.0, 40_000.0].into_iter().enumerate() {
        for (yaw, pitch) in [(0.3, 0.05), (2.0, 0.4), (-1.2, 1.2), (4.0, -0.1)] {
            let eye = [12_345.6 + 700.0 * k as f64, -98_765.4 + 300.0 * k as f64, height];
            out.push(Look { eye, yaw, pitch, half: [0.6, 0.4] });
        }
    }
    out
}

/// Every page the samples ask for, the estimator names: over towers that
/// hide what is behind them, at every height and tilt, whatever the far
/// edge, and at every quality.
#[test]
fn the_bounds_estimator_names_every_page_the_exact_demand_does() {
    for (k, look) in looks().into_iter().enumerate() {
        let world = World {
            towers: towers([look.eye[0], look.eye[1]], k as u64),
            planet: f64::INFINITY,
        };
        for far in [fault_radius(TOP_LEVEL), 30_000.0] {
            let o = observation(look, far, world.planet, true);
            for tau in TAUS {
                let named = demand(&o, &world, tau);
                for p in exact(&world, look, far, tau) {
                    assert!(named.contains(&p), "look {k}, far {far}, {tau:?}: {p:?} asked for and not named");
                }
            }
        }
    }
}

/// Where the planet draws the far ground lower, what it lowers is still
/// named: a small planet, so the drop is large.
#[test]
fn ground_the_planet_draws_lower_is_named_where_it_is_drawn() {
    let world = World { towers: Vec::new(), planet: 60_000.0 };
    for look in looks() {
        let far = fault_radius(TOP_LEVEL);
        let o = observation(look, far, world.planet, true);
        let named = demand(&o, &world, Tau::FINEST);
        for p in exact(&world, look, far, Tau::FINEST) {
            assert!(named.contains(&p), "{p:?} asked for and not named");
        }
    }
}

/// Knowing less never names less: with nothing known, every page the bounds
/// name is still named. And a level names no more than the fault bound,
/// with the frustum or looking all around, known or not, at every quality.
#[test]
fn a_level_names_no_more_than_the_fault_bound_and_less_known_names_more() {
    for (k, look) in looks().into_iter().enumerate() {
        let world = World {
            towers: towers([look.eye[0], look.eye[1]], 100 + k as u64),
            planet: f64::INFINITY,
        };
        for frustum in [true, false] {
            let o = observation(look, fault_radius(TOP_LEVEL), world.planet, frustum);
            for tau in TAUS {
                let (known, unknown) = (demand(&o, &world, tau), demand(&o, &Unknown, tau));
                assert!(known.is_subset(&unknown), "look {k}, {tau:?}: bounds named a page nothing known did not");
                for named in [known, unknown] {
                    for l in 0..=TOP_LEVEL {
                        let n = named.iter().filter(|p| p.level == l).count() as u32;
                        assert!(n <= FAULT_PAGES, "look {k}, {tau:?}, level {l}: {n} pages");
                    }
                }
            }
        }
    }
}

/// A square of a level's lattice is one page, and a wider ground every page
/// under it.
#[test]
fn paged_storage_maps_a_square_to_its_page_and_ground_to_every_page_under_it() {
    for level in [0, 3, TOP_LEVEL] {
        let span = span_of(level);
        for [x, y] in [[0i64, 0], [-3, 5], [17, -11]] {
            let mut pages = Vec::new();
            let square = (x as f64 * span, y as f64 * span, span);
            Paged.pages(&Requirement { level, ground: square }, &mut |p| pages.push(p));
            assert_eq!(pages, vec![page_at(level, [x, y])]);
            pages.clear();
            let wider = (x as f64 * span + 0.5 * span, y as f64 * span, 1.5 * span);
            Paged.pages(&Requirement { level, ground: wider }, &mut |p| pages.push(p));
            assert_eq!(pages.len(), 4, "level {level}: {pages:?}");
        }
    }
}
