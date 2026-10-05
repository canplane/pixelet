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
fn air_source(m: Medium, r: f32, up: vec3f, toward: vec3f, phase: vec2f) -> vec3f {
  let mu = dot(up, toward);
  return sun_transmittance(r, mu) * (m.rayleigh * phase.x + m.mie * phase.y) + multiscatter(r, mu) * m.scattering;
}

// Knots gather about the ray's lowest point: the eye looking up, the ground
// looking down, the tangent looking across.
fn air_knot(a: f32, b: f32, low: f32, u: f32) -> f32 {
  let x = mix(-sqrt(low - a), sqrt(b - low), u);
  return low + x * abs(x);
}

// From radius `r`, whose up is `up`, `d` km along `dir`; the moon's light
// weighs `moon_light` of the sun's.
fn gather(r: f32, up: vec3f, dir: vec3f, d: f32, sun: vec3f, moon: vec3f, moon_light: f32, knots: i32) -> Veil {
  let mu = dot(up, dir);
  let low = ray_low(r, mu);
  let eye = depth_along(r, mu, low, 0.0);
  let top = atmos_top_radius();
  let end = short_of_ground(r, mu, d);
  let a = min(max(-r * mu - sqrt(max(r * r * (mu * mu - 1.0) + top * top, 0.0)), 0.0), end);
  let lowest = clamp(-r * mu, a, end);
  let sun_phase = vec2f(rayleigh_phase(dot(dir, sun)), mie_phase(dot(dir, sun)));
  let moon_phase = vec2f(rayleigh_phase(dot(dir, moon)), mie_phase(dot(dir, moon)));
  var da = depth_along(r, mu, low, a);
  var ja = per_depth(r, up, dir, a, sun, sun_phase, moon, moon_phase, moon_light);
  var light = vec3f(0.0);
  for (var i = 1; i <= knots; i++) {
    let tb = air_knot(a, end, lowest, f32(i) / f32(knots));
    let db = depth_along(r, mu, low, tb);
    let jb = per_depth(r, up, dir, tb, sun, sun_phase, moon, moon_phase, moon_light);
    let y = db - da;
    light += exp(eye - da) * (ja * y * extinction_weight(y) + (jb - ja) * (extinction_weight(y) - exp(-y)));
    da = db;
    ja = jb;
  }
  return Veil(exp(eye - da), light);
}

// What the air `t` km along the ray sends toward the eye per unit of its depth.
fn per_depth(r: f32, up: vec3f, dir: vec3f, t: f32, sun: vec3f, sun_phase: vec2f, moon: vec3f, moon_phase: vec2f, moon_light: f32) -> vec3f {
  let p = up * r + dir * t;
  let pr = length(p);
  let m = medium_at(height_along(r, dot(up, dir), t, pr));
  var source = air_source(m, pr, p / pr, sun, sun_phase);
  if (moon_light > 0.0) {
    source += air_source(m, pr, p / pr, moon, moon_phase) * moon_light;
  }
  return select(vec3f(0.0), source / m.extinction, m.extinction > vec3f(0.0));
}

// Up +z, to the ground or the top.
fn sky_radiance(r: f32, dir: vec3f, toward: vec3f, knots: i32) -> vec3f {
  return gather(r, vec3f(0.0, 0.0, 1.0), dir, air_path(r, dir.z), toward, toward, 0.0, knots).scattered;
}
