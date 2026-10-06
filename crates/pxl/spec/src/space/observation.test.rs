// pxl-spec/space/observation.test.rs
//
// The estimator's laws against an exact ray-cast solver on fixtures with
// known heights.

use std::collections::HashSet;

use super::*;
use crate::space::lod::REACH_TILES;

const TAU_SCALES: [f64; 3] = [1.0, 1.5, 8.0];

#[derive(Clone, Copy)]
struct Tower {
    x: [f64; 2],
    y: [f64; 2],
    h: f64,
}

/// Flat ground at zero with towers, and a planet radius (infinite for flat).
struct World {
    towers: Vec<Tower>,
    planet: f64,
}

impl Bounds for World {
    fn contents(&self, _zoom: Zoom, region: Aabb) -> Contents {
        let (lo, hi) = (region.min, region.max);
        let over = |t: &&Tower| t.x[0] <= hi[0] && t.x[1] >= lo[0] && t.y[0] <= hi[1] && t.y[1] >= lo[1];
        let top = self.towers.iter().filter(over).map(|t| t.h).fold(0.0, f64::max);
        let column = Aabb {
            min: [lo[0], lo[1], 0.0],
            max: [hi[0], hi[1], top],
        };
        match column.meet(region) {
            Some(bound) => Contents::Holds { bound, error: [0.0; 3] },
            None => Contents::Empty,
        }
    }
}

impl World {
    /// The first hit's distance along the unit ray.
    fn first_hit(&self, e: [f64; 3], v: [f64; 3]) -> Option<f64> {
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

/// Yaw, pitch under the horizon, and half the field across and up.
#[derive(Clone, Copy)]
struct Look {
    eye: [f64; 3],
    yaw: f64,
    pitch: f64,
    half: [f64; 2],
}

const NEAR: f64 = 0.1;

impl Look {
    fn basis(self) -> [[f64; 3]; 3] {
        let (cy, sy, cp, sp) = (libm::cos(self.yaw), libm::sin(self.yaw), libm::cos(self.pitch), libm::sin(self.pitch));
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
            mix(f, libm::sin(hx), r, libm::cos(hx)),
            mix(f, libm::sin(hx), r, -libm::cos(hx)),
            mix(f, libm::sin(hy), u, libm::cos(hy)),
            mix(f, libm::sin(hy), u, -libm::cos(hy)),
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

    fn rays(self, n: u32) -> impl Iterator<Item = [f64; 3]> {
        let [f, r, u] = self.basis();
        let [tx, ty] = self.half.map(libm::tan);
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
        top: TOP_Z,
        planet,
        ground: 0.0,
    }
}

fn demand(o: &Observation, bounds: &impl Bounds, tau: Tau) -> HashSet<Tile> {
    let mut out = HashSet::new();
    ByBounds { tau }.requirements(o, bounds, &mut |r| Tiled.tiles(&r, &mut |p| _ = out.insert(p)));
    out
}

/// The exact demand: each answered ray asks for the tile of its first hit at its distance's zoom.
fn exact(world: &World, look: Look, far: f64, tau: Tau) -> HashSet<Tile> {
    let mut out = HashSet::new();
    for v in look.rays(96) {
        let Some(r) = world.first_hit(look.eye, v) else { continue };
        if r > far || r < NEAR {
            continue;
        }
        let at = [look.eye[0] + r * v[0], look.eye[1] + r * v[1]];
        let zoom = tau.z_at(r);
        let span = units(zoom);
        out.insert(tile_at(zoom, at.map(|c| (c / span).floor() as i64)));
    }
    out
}

fn next(seed: &mut u64) -> f64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*seed >> 11) as f64 / (1u64 << 53) as f64
}

/// None over `at`, where the eye stands.
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

/// A small planet, so far ground drops a lot and must still be named.
#[test]
fn ground_the_planet_draws_lower_is_named_where_it_is_drawn() {
    let world = World { towers: Vec::new(), planet: 60_000.0 };
    for look in looks() {
        let far = demand_reach(TOP_Z);
        let o = observation(look, far, world.planet, true);
        let named = demand(&o, &world, Tau::FINEST);
        for p in exact(&world, look, far, Tau::FINEST) {
            assert!(named.contains(&p), "{p:?} asked for and not named");
        }
    }
}

/// Knowing less never names less, and no zoom names more than the reach.
#[test]
fn a_zoom_names_no_more_than_the_fault_bound_and_less_known_names_more() {
    for (k, look) in looks().into_iter().enumerate() {
        let world = World {
            towers: towers([look.eye[0], look.eye[1]], 100 + k as u64),
            planet: f64::INFINITY,
        };
        for frustum in [true, false] {
            let o = observation(look, demand_reach(TOP_Z), world.planet, frustum);
            for tau in TAU_SCALES.map(Tau::of) {
                let (known, unknown) = (demand(&o, &world, tau), demand(&o, &Unknown, tau));
                assert!(known.is_subset(&unknown), "look {k}, {tau:?}: bounds named a tile nothing known did not");
                for named in [known, unknown] {
                    for zoom in TOP_Z..=LEAF_Z {
                        let n = named.iter().filter(|p| p.zoom == zoom).count() as u32;
                        assert!(n <= REACH_TILES, "look {k}, {tau:?}, z{zoom}: {n} tiles");
                    }
                }
            }
        }
    }
}

#[test]
fn tiled_storage_maps_a_square_to_its_tile_and_ground_to_every_tile_under_it() {
    for zoom in [LEAF_Z, 13, TOP_Z] {
        let span = units(zoom);
        for [x, y] in [[0i64, 0], [-3, 5], [17, -11]] {
            let mut tiles = Vec::new();
            let (x0, y0) = (x as f64 * span, y as f64 * span);
            let square = Aabb {
                min: [x0, y0, -7.0],
                max: [x0 + span, y0 + span, 900.0],
            };
            Tiled.tiles(&Requirement { zoom, region: square }, &mut |p| tiles.push(p));
            assert_eq!(tiles, vec![tile_at(zoom, [x, y])]);
            tiles.clear();
            let wider = Aabb {
                min: [x0 + 0.5 * span, y0, 0.0],
                max: [x0 + 2.0 * span, y0 + 1.5 * span, 0.0],
            };
            Tiled.tiles(&Requirement { zoom, region: wider }, &mut |p| tiles.push(p));
            assert_eq!(tiles.len(), 4, "z{zoom}: {tiles:?}");
        }
    }
}
