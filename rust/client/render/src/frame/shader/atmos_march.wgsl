// What the air between the eye and a point leaves of it and sends toward the
// eye, unexposed and per unit of the lights: single scattering gathered over
// knots about the ray's lowest point, Hillaire's table for the orders past it.
// Between two knots the depth is exact and the light per unit of it runs
// straight from one's to the other's; the medium cancels from that ratio, so
// it changes slowly where the air's density does not.

@group(0) @binding(12) var multiscatter_lut: texture_2d<f32>;

struct Veil {
  through: vec3f,
  scattered: vec3f,
};

fn multiscatter(r: f32, mu_s: f32) -> vec3f {
  return textureSampleLevel(multiscatter_lut, atmos_sampler, multiscatter_uv(r, mu_s), 0.0).rgb;
}

// What a point at radius `r` sends along the ray, per km, from a light toward `toward`.
fn air_src(m: Medium, r: f32, up: vec3f, toward: vec3f, phase: vec2f) -> vec3f {
  let mu = dot(up, toward);
  return sun_transmittance(r, mu) * (m.rayleigh * phase.x + m.mie * phase.y) + multiscatter(r, mu) * m.scattering;
}

// Knots gather about the ray's lowest point: the eye looking up, the ground
// looking down, the tangent looking across.
fn air_knot(a: f32, b: f32, lowest: f32, u: f32) -> f32 {
  let x = mix(-sqrt(lowest - a), sqrt(b - lowest), u);
  return lowest + x * abs(x);
}

// What a stretch of depth `x` sends from its start, the light per unit of
// depth running straight from `ja` to `jb` across it; `left` is exp(-x).
fn stretch(ja: vec3f, jb: vec3f, x: vec3f, left: vec3f) -> vec3f {
  return ja * x * extinction_weight(x) + (jb - ja) * (extinction_weight(x) - left);
}

// The stretch from `t0` to `t1` km along `dir` from radius `r`, whose up is
// `up`, gathered over `knots` knots: what it leaves of what lies past it and
// what it sends toward its near end. Stretches end to end make the whole,
// T = T0·T1 and S = S0 + T0·S1, so a column of them marched once gives every
// point on it by one integral.
fn gather_between(r: f32, up: vec3f, dir: vec3f, t0: f32, t1: f32, sun: vec3f, moon: vec3f, moon_light: f32, knots: i32) -> Veil {
  let mu = dot(up, dir);
  let end = short_of_ground(r, mu, t1);
  let low = ray_low(r, mu, end);
  let a = clamp(t0, min(to_sphere(r, mu, atmos_top_radius()), end), end);
  let lowest = clamp(-r * mu, a, end);
  let sun_phase = vec2f(rayleigh_phase(dot(dir, sun)), mie_phase(dot(dir, sun)));
  let moon_phase = vec2f(rayleigh_phase(dot(dir, moon)), mie_phase(dot(dir, moon)));
  var through = vec3f(1.0);
  var light = vec3f(0.0);
  if (end <= a) {
    return Veil(through, light);
  }
  var da = depth_along(r, mu, low, a);
  var ja = per_depth(r, up, dir, a, sun, sun_phase, moon, moon_phase, moon_light);
  for (var i = 1; i <= knots; i++) {
    let tb = air_knot(a, end, lowest, f32(i) / f32(knots));
    let db = depth_along(r, mu, low, tb);
    let jb = per_depth(r, up, dir, tb, sun, sun_phase, moon, moon_phase, moon_light);
    let y = max(db - da, vec3f(0.0));
    let left = exp(-y);
    light += through * stretch(ja, jb, y, left);
    through *= left;
    da = db;
    ja = jb;
  }
  return Veil(through, light);
}

// What the air `t` km along the ray sends toward the eye per unit of its depth.
fn per_depth(r: f32, up: vec3f, dir: vec3f, t: f32, sun: vec3f, sun_phase: vec2f, moon: vec3f, moon_phase: vec2f, moon_light: f32) -> vec3f {
  let mu = dot(up, dir);
  let p = up * r + dir * t;
  let pr = radius_along(r, mu, t);
  let m = medium_at(height_along(r, mu, t, pr));
  var src = air_src(m, pr, p / pr, sun, sun_phase);
  if (moon_light > 0.0) {
    src += air_src(m, pr, p / pr, moon, moon_phase) * moon_light;
  }
  return select(vec3f(0.0), src / m.extinction, m.extinction > vec3f(0.0));
}

// Up +z, to the ground or the top, gathered from the eye to where the ray ends.
fn sky_radiance(r: f32, dir: vec3f, toward: vec3f, knots: i32) -> vec3f {
  return gather_between(r, vec3f(0.0, 0.0, 1.0), dir, 0.0, air_path(r, dir.z), toward, toward, 0.0, knots).scattered;
}
