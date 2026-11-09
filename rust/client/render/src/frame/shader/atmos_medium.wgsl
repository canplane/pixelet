// The atmosphere as a medium, the rays through it and its tables' layouts. Distances
// are in km. A point on a ray is (r, mu), as Bruneton writes it: its radius from the
// planet's center and the cosine of the ray's angle from the zenith there.

// The atmosphere's words. Their fields are frame/shader/word.rs `atmos`'s, and
// build.rs writes their accessors here.
@group(0) @binding(8) var<uniform> atmos_words: array<vec4f, ATMOS_VEC4S>;

// The eye's radius, held a meter above the ground. It may lie above the top of
// the atmosphere, for an eye in space.
fn camera_r() -> f32 {
  return atmos_bottom_radius() + max(atmos_camera_alt(), 0.001);
}

struct Medium {
  rayleigh: vec3f,
  mie: vec3f,
  scattering: vec3f,
  extinction: vec3f,
};

fn medium_at(h: f32) -> Medium {
  let r = exp(-h / atmos_rayleigh_scale_height());
  let m = exp(-h / atmos_mie_scale_height());
  let o = max(0.0, 1.0 - abs(h - atmos_ozone_center()) / atmos_ozone_half_width());
  let rayleigh = atmos_rayleigh_scattering() * r;
  let mie = vec3f(atmos_mie_scattering() * m);
  return Medium(rayleigh, mie, rayleigh + mie, rayleigh + vec3f(atmos_mie_extinction() * m) + atmos_ozone_absorption() * o);
}

fn rayleigh_phase(c: f32) -> f32 {
  return 3.0 * INV_4PI / 4.0 * (1.0 + c * c);
}

// Cornette-Shanks's phase, which keeps a hazy sky bright opposite the sun.
fn mie_phase(c: f32) -> f32 {
  let g = atmos_mie_g();
  let k = 3.0 * INV_4PI / 2.0 * (1.0 - g * g) / (2.0 + g * g);
  return k * (1.0 + c * c) / pow(max(1.0 + g * g - 2.0 * g * c, 1e-4), 1.5);
}

fn meets_sphere(r: f32, mu: f32, radius: f32) -> bool {
  return mu < 0.0 && r * r * (mu * mu - 1.0) + radius * radius >= 0.0;
}

fn to_sphere(r: f32, mu: f32, radius: f32) -> f32 {
  return max(-r * mu - sqrt(max(r * r * (mu * mu - 1.0) + radius * radius, 0.0)), 0.0);
}

fn meets_ground(r: f32, mu: f32) -> bool {
  return meets_sphere(r, mu, atmos_bottom_radius());
}

// The radius `t` km along a ray from radius `r` at cosine `mu`.
fn radius_along(r: f32, mu: f32, t: f32) -> f32 {
  return sqrt(max(r * r + 2.0 * r * mu * t + t * t, 0.0));
}

// The height over the ground `t` km along the ray, where `rt` is the radius
// there. It is not taken as the difference of two radii, which rounds a short
// step near the ground away.
fn height_along(r: f32, mu: f32, t: f32, rt: f32) -> f32 {
  return max(r - atmos_bottom_radius() + t * (2.0 * r * mu + t) / (rt + r), 0.0);
}

fn to_top(r: f32, mu: f32) -> f32 {
  return max(-r * mu + sqrt(max(r * r * (mu * mu - 1.0) + atmos_top_radius() * atmos_top_radius(), 0.0)), 0.0);
}

fn to_ground(r: f32, mu: f32) -> f32 {
  return to_sphere(r, mu, atmos_bottom_radius());
}

fn air_path(r: f32, mu: f32) -> f32 {
  return select(to_top(r, mu), to_ground(r, mu), meets_ground(r, mu));
}

// How far under the eye the land it stands over lies, in km: the plane at the
// elevation of its ground. Only drawing reads it; the tables keep the radius they
// were baked at.
fn land_below_km() -> f32 {
  return atmos_camera_alt() - atmos_land_alt_km();
}

// Whether a ray from the eye along `dir` falls to that land. The world's z
// points up.
fn falls_to_land(dir: vec3f) -> bool {
  return dir.z < 0.0 && land_below_km() > 0.0;
}

// How far from the eye a ray along `dir` that falls to the land meets it, in
// units (`falls_to_land`).
fn to_land(dir: vec3f) -> f32 {
  return land_below_km() / atmos_km_per_u() / -dir.z;
}

fn land_radius() -> f32 {
  return atmos_bottom_radius() + atmos_land_alt_km();
}

// How light `k`'s sky view and its share of the air's volume were laid
// (pass/atmosphere.rs `prepare`), `k` 0 for the sun and 1 for the moon. `x` is
// how far under the level the horizon dips, in radians, seen from the eye's
// radius they were drawn at. `y` and `z` are the near and far ends of the air's
// depth law laid there, in km (`air_dist`).
fn view_layout(k: u32) -> vec3f {
  return select(atmos_sun_layout(), atmos_moon_layout(), k == 1u);
}

// Whether light `k` reaches the air by half a display level (pass/atmosphere.rs
// `REACH`). Only then are its view and its air drawn and read.
fn reaches(k: u32) -> bool {
  return select(atmos_sun_reaches(), atmos_moon_reaches(), k == 1u) > 0.5;
}

fn light_dir(k: u32) -> vec3f {
  return select(atmos_sun_dir(), atmos_moon_dir(), k == 1u);
}

// Light `k`'s illuminance relative to the sun's.
fn light_gain(k: u32) -> f32 {
  return select(1.0, atmos_moon_light(), k == 1u);
}

// The volume's depth law, one for every direction. Slice `z`, from 0 to 1, lies
// `air_dist(z, e)` km out: the cube of `z` over the span between the ends `e`
// the volume was laid with (`view_layout`).
fn air_dist(z: f32, e: vec2f) -> f32 {
  return e.x + (e.y - e.x) * z * z * z;
}

// Where a point `d` km out lies among the slices, from 0 to the last. It is the
// depth law's inverse, smooth in `d` across every slice.
fn air_slice(d: f32, e: vec2f) -> f32 {
  return pow(clamp((d - e.x) / (e.y - e.x), 0.0, 1.0), 1.0 / 3.0) * f32(AIR_FROXELS - 1u);
}

// `unit_of` and `coord_of` map between a table's coordinate and a unit range
// whose ends lie on the first and last texel centers, so no read falls past
// them.
fn unit_of(u: f32, size: f32) -> f32 {
  return (u - 0.5 / size) / (1.0 - 1.0 / size);
}

fn coord_of(x: f32, size: f32) -> f32 {
  return 0.5 / size + x * (1.0 - 1.0 / size);
}

// How far a light's mu moves over one texel of the transmittance table
// from where it stands, taking the larger of the two sides. The light is read
// through that table.
fn mu_step(r: f32, mu: f32) -> f32 {
  let uv = transmittance_uv(r, mu);
  let texel = vec2f(1.0 / TRANSMITTANCE_SIZE.x, 0.0);
  return max(abs(transmittance_r_mu(uv + texel).y - mu), abs(transmittance_r_mu(uv - texel).y - mu));
}

// Bruneton's mapping of (r, mu) into the transmittance table, which spends its
// rows where transmittance changes.
fn transmittance_uv(r_in: f32, mu: f32) -> vec2f {
  let bottom = atmos_bottom_radius();
  let top = atmos_top_radius();
  let r = min(r_in, top);
  let h = atmos_horizon_to_top();
  let rho = sqrt(max(r * r - bottom * bottom, 0.0));
  let d = to_top(r, mu);
  let d_min = top - r;
  let d_max = rho + h;
  let x_mu = (d - d_min) / (d_max - d_min);
  let x_r = rho / h;
  return vec2f(coord_of(x_mu, TRANSMITTANCE_SIZE.x), coord_of(x_r, TRANSMITTANCE_SIZE.y));
}

fn transmittance_r_mu(uv: vec2f) -> vec2f {
  let bottom = atmos_bottom_radius();
  let top = atmos_top_radius();
  let x_mu = unit_of(uv.x, TRANSMITTANCE_SIZE.x);
  let x_r = unit_of(uv.y, TRANSMITTANCE_SIZE.y);
  let h = atmos_horizon_to_top();
  let rho = h * x_r;
  let r = sqrt(rho * rho + bottom * bottom);
  let d_min = top - r;
  let d_max = rho + h;
  let d = d_min + x_mu * (d_max - d_min);
  var mu = 1.0;
  if (d > 0.0) {
    mu = clamp((h * h - rho * rho - d * d) / (2.0 * r * d), -1.0, 1.0);
  }
  return vec2f(r, mu);
}

fn multiscatter_uv(r: f32, mu_s: f32) -> vec2f {
  let x = mu_s * 0.5 + 0.5;
  let y = (r - atmos_bottom_radius()) / (atmos_top_radius() - atmos_bottom_radius());
  return vec2f(coord_of(clamp(x, 0.0, 1.0), MULTISCATTER_SIZE.x), coord_of(clamp(y, 0.0, 1.0), MULTISCATTER_SIZE.y));
}

// Hillaire's layout of directions, each coordinate from 0 to 1. Across, `x` is
// the azimuth from the light's, with its texels crowded toward the light. Up,
// `y` is the zenith angle, with its texels crowded toward the horizon from
// either side; the horizon dips `beta` under the level (`view_layout`). A
// light's sky view and its share of the air's volume are both laid by it
// (atmos_sky.wgsl `table_unit`).
fn view_unit(beta: f32, mu: f32, light_cos: f32) -> vec2f {
  let zenith_horizon = PI - beta;
  let view_zenith = acos(clamp(mu, -1.0, 1.0));
  var y: f32;
  if (view_zenith < zenith_horizon) {
    let c = clamp(view_zenith / zenith_horizon, 0.0, 1.0);
    y = (1.0 - sqrt(1.0 - c)) * 0.5;
  } else {
    let c = clamp((view_zenith - zenith_horizon) / beta, 0.0, 1.0);
    y = sqrt(c) * 0.5 + 0.5;
  }
  let x = sqrt(clamp(-light_cos * 0.5 + 0.5, 0.0, 1.0));
  return vec2f(x, y);
}

// The direction at `unit`, the inverse of `view_unit`, with up +z and the
// light's azimuth +x.
fn view_dir(beta: f32, unit: vec2f) -> vec3f {
  let zenith_horizon = PI - beta;
  var mu: f32;
  if (unit.y < 0.5) {
    let c = 1.0 - 2.0 * unit.y;
    mu = cos(zenith_horizon * (1.0 - c * c));
  } else {
    let c = 2.0 * unit.y - 1.0;
    mu = cos(zenith_horizon + beta * c * c);
  }
  let light_cos = 1.0 - 2.0 * unit.x * unit.x;
  let s = sqrt(max(1.0 - mu * mu, 0.0));
  return vec3f(s * light_cos, s * sqrt(max(1.0 - light_cos * light_cos, 0.0)), mu);
}

// The direction of a light at cosine `mu_s` over the horizon, in the frame
// `view_dir` uses.
fn view_light(mu_s: f32) -> vec3f {
  let m = clamp(mu_s, -1.0, 1.0);
  return vec3f(sqrt(max(1.0 - m * m, 0.0)), 0.0, m);
}
