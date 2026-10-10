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
fn air_knot(a: f32, b: f32, lowest: f32, u: f32) -> f32 {
  let x = mix(-sqrt(lowest - a), sqrt(b - lowest), u);
  return lowest + x * abs(x);
}

// What a stretch sends from its start to depth `x` into it, the light per unit
// of depth running straight from `ja` to `jb` across it, of which `x` is the
// share `rise`; `left` is exp(-x).
fn stretch(ja: vec3f, jb: vec3f, x: vec3f, rise: vec3f, left: vec3f) -> vec3f {
  return ja * x * extinction_weight(x) + (jb - ja) * rise * (extinction_weight(x) - left);
}

// From radius `r`, whose up is `up`, to each of `d` km along `dir`, nearest
// first, in one gather to the farthest: a nearer point is read off the stretch
// it falls in as the stretch is gathered. The moon's light weighs `moon_light`
// of the sun's.
fn gather(r: f32, up: vec3f, dir: vec3f, d: vec3f, sun: vec3f, moon: vec3f, moon_light: f32, knots: i32) -> array<Veil, 3> {
  let mu = dot(up, dir);
  let end = short_of_ground(r, mu, d.z);
  let low = ray_low(r, mu, end);
  let a = min(to_sphere(r, mu, atmos_top_radius()), end);
  let lowest = clamp(-r * mu, a, end);
  let sun_phase = vec2f(rayleigh_phase(dot(dir, sun)), mie_phase(dot(dir, sun)));
  let moon_phase = vec2f(rayleigh_phase(dot(dir, moon)), mie_phase(dot(dir, moon)));
  // No air lies between the eye and where the ray enters it.
  var through = vec3f(1.0);
  var da = depth_along(r, mu, low, a);
  var ja = per_depth(r, up, dir, a, sun, sun_phase, moon, moon_phase, moon_light);
  var light = vec3f(0.0);
  var out: array<Veil, 3>;
  var read = 0;
  for (var i = 1; i <= knots; i++) {
    let tb = air_knot(a, end, lowest, f32(i) / f32(knots));
    let db = depth_along(r, mu, low, tb);
    let jb = per_depth(r, up, dir, tb, sun, sun_phase, moon, moon_phase, moon_light);
    let y = db - da;
    // The farthest is the gather's end, read once it is done.
    while (read < 2 && d[read] < end && d[read] <= tb) {
      let x = clamp(depth_along(r, mu, low, d[read]) - da, vec3f(0.0), y);
      let rise = select(vec3f(0.0), x / y, y > vec3f(0.0));
      let left = exp(-x);
      out[read] = Veil(through * left, light + through * stretch(ja, jb, x, rise, left));
      read++;
    }
    let left = exp(-y);
    light += through * stretch(ja, jb, y, vec3f(1.0), left);
    through *= left;
    da = db;
    ja = jb;
  }
  for (; read < 3; read++) {
    out[read] = Veil(through, light);
  }
  return out;
}

// What the air `t` km along the ray sends toward the eye per unit of its depth.
fn per_depth(r: f32, up: vec3f, dir: vec3f, t: f32, sun: vec3f, sun_phase: vec2f, moon: vec3f, moon_phase: vec2f, moon_light: f32) -> vec3f {
  let mu = dot(up, dir);
  let p = up * r + dir * t;
  let pr = radius_along(r, mu, t);
  let m = medium_at(height_along(r, mu, t, pr));
  var source = air_source(m, pr, p / pr, sun, sun_phase);
  if (moon_light > 0.0) {
    source += air_source(m, pr, p / pr, moon, moon_phase) * moon_light;
  }
  return select(vec3f(0.0), source / m.extinction, m.extinction > vec3f(0.0));
}

// Up +z, to the ground or the top.
fn sky_radiance(r: f32, dir: vec3f, toward: vec3f, knots: i32) -> vec3f {
  return gather(r, vec3f(0.0, 0.0, 1.0), dir, vec3f(air_path(r, dir.z)), toward, toward, 0.0, knots)[2].scattered;
}
