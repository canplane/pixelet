// The atmosphere as a medium. Distances are km; a point is (r, mu) as Bruneton writes them.

struct Atmosphere {
  rayleigh_scattering: vec3f,
  bottom_radius: f32,
  ozone_absorption: vec3f,
  top_radius: f32,
  // Free.
  white_free: vec3f,
  rayleigh_scale_height: f32,
  sun_dir: vec3f,
  mie_scattering: f32,
  camera_altitude: f32,
  km_per_u: f32,
  mie_extinction: f32,
  mie_scale_height: f32,
  mie_g: f32,
  ozone_center: f32,
  ozone_half_width: f32,
  ground_albedo: f32,
  sun_angular_radius: f32,
  exposure: f32,
  night_stops: f32,
  shown_fall: f32,
  moon_dir: vec3f,
  moon_light: f32,
  // Free.
  free: vec3f,
  // Free.
  free_lon: f32,
  // Lights the ground, never drawn: drawn, the sky went pale blue.
  night_sky: vec3f,
  // The Earth's face under the eye: at sea level the terrain floated on it.
  land_altitude_km: f32,
  // A lit city's radiance where the picture holds all of it.
  city_light: vec3f,
  // In units.
  earth_reach: f32,
  // The south-west tile, tiles across the window and around the world; then a
  // tile's layer plus one in four bits, row by row, zero for the global picture.
  earth_window: vec4i,
  earth_tiles: vec4u,
};

// Field by field, never as a struct: a struct lost Safari's WebGPU its GPU process.
@group(0) @binding(8) var<uniform> atmos_words: array<vec4f, ATMOS_VEC4S>;

fn atmos_rayleigh_scattering() -> vec3f { return atmos_words[0].xyz; }
fn atmos_bottom_radius() -> f32 { return atmos_words[0].w; }
fn atmos_ozone_absorption() -> vec3f { return atmos_words[1].xyz; }
fn atmos_top_radius() -> f32 { return atmos_words[1].w; }
fn atmos_rayleigh_scale_height() -> f32 { return atmos_words[2].w; }
fn atmos_sun_dir() -> vec3f { return atmos_words[3].xyz; }
fn atmos_mie_scattering() -> f32 { return atmos_words[3].w; }
fn atmos_camera_altitude() -> f32 { return atmos_words[4].x; }
fn atmos_km_per_u() -> f32 { return atmos_words[4].y; }
fn atmos_mie_extinction() -> f32 { return atmos_words[4].z; }
fn atmos_mie_scale_height() -> f32 { return atmos_words[4].w; }
fn atmos_mie_g() -> f32 { return atmos_words[5].x; }
fn atmos_ozone_center() -> f32 { return atmos_words[5].y; }
fn atmos_ozone_half_width() -> f32 { return atmos_words[5].z; }
fn atmos_ground_albedo() -> f32 { return atmos_words[5].w; }
fn atmos_sun_angular_radius() -> f32 { return atmos_words[6].x; }
fn atmos_exposure() -> f32 { return atmos_words[6].y; }
fn atmos_night_stops() -> f32 { return atmos_words[6].z; }
fn atmos_shown_fall() -> f32 { return atmos_words[6].w; }
fn atmos_moon_dir() -> vec3f { return atmos_words[7].xyz; }
fn atmos_moon_light() -> f32 { return atmos_words[7].w; }
fn atmos_night_sky() -> vec3f { return atmos_words[9].xyz; }
fn atmos_land_altitude_km() -> f32 { return atmos_words[9].w; }
fn atmos_city_light() -> vec3f { return atmos_words[10].xyz; }
fn atmos_earth_reach() -> f32 { return atmos_words[10].w; }
fn atmos_earth_window() -> vec4i { return bitcast<vec4i>(atmos_words[11]); }
fn atmos_earth_tiles() -> vec4u { return bitcast<vec4u>(atmos_words[12]); }


/* the medium */

// Above the atmosphere from space.
fn camera_r() -> f32 {
  return atmos_bottom_radius() + max(atmos_camera_altitude(), 0.001);
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
  var out: Medium;
  out.rayleigh = atmos_rayleigh_scattering() * r;
  out.mie = vec3f(atmos_mie_scattering() * m);
  out.scattering = out.rayleigh + out.mie;
  out.extinction = out.rayleigh + vec3f(atmos_mie_extinction() * m) + atmos_ozone_absorption() * o;
  return out;
}

fn rayleigh_phase(c: f32) -> f32 {
  return 3.0 * INV_4PI / 4.0 * (1.0 + c * c);
}

// Cornette-Shanks: keeps a hazy sky bright opposite the sun.
fn mie_phase(c: f32) -> f32 {
  let g = atmos_mie_g();
  let k = 3.0 * INV_4PI / 2.0 * (1.0 - g * g) / (2.0 + g * g);
  return k * (1.0 + c * c) / pow(max(1.0 + g * g - 2.0 * g * c, 1e-4), 1.5);
}


/* rays */

fn meets_sphere(r: f32, mu: f32, radius: f32) -> bool {
  return mu < 0.0 && r * r * (mu * mu - 1.0) + radius * radius >= 0.0;
}

fn to_sphere(r: f32, mu: f32, radius: f32) -> f32 {
  return max(-r * mu - sqrt(max(r * r * (mu * mu - 1.0) + radius * radius, 0.0)), 0.0);
}

fn meets_ground(r: f32, mu: f32) -> bool {
  return meets_sphere(r, mu, atmos_bottom_radius());
}

// From radius `r` at cosine `mu`, `t` km along the ray.
fn radius_along(r: f32, mu: f32, t: f32) -> f32 {
  return sqrt(max(r * r + 2.0 * r * mu * t + t * t, 0.0));
}

// Over the ground, `rt` the radius there: the difference of two radii lost a
// short step near the ground.
fn height_along(r: f32, mu: f32, t: f32, rt: f32) -> f32 {
  return max(r - atmos_bottom_radius() + t * (2.0 * r * mu + t) / (rt + r), 0.0);
}

fn to_top(r: f32, mu: f32) -> f32 {
  return max(-r * mu + sqrt(max(r * r * (mu * mu - 1.0) + atmos_top_radius() * atmos_top_radius(), 0.0)), 0.0);
}

fn to_ground(r: f32, mu: f32) -> f32 {
  return to_sphere(r, mu, atmos_bottom_radius());
}

// Through the air until the ground or the top.
fn air_path(r: f32, mu: f32) -> f32 {
  return select(to_top(r, mu), to_ground(r, mu), meets_ground(r, mu));
}

// For drawing only; the tables keep the radius they were baked at.
fn land_radius() -> f32 {
  return atmos_bottom_radius() + atmos_land_altitude_km();
}

fn meets_land(r: f32, mu: f32) -> bool {
  return meets_sphere(r, mu, land_radius());
}

fn to_land(r: f32, mu: f32) -> f32 {
  return to_sphere(r, mu, land_radius());
}


/* tables */

// Range ends on the first and last texel centers, so nothing reads past them.
fn unit_of(u: f32, size: f32) -> f32 {
  return (u - 0.5 / size) / (1.0 - 1.0 / size);
}

fn coord_of(x: f32, size: f32) -> f32 {
  return 0.5 / size + x * (1.0 - 1.0 / size);
}

const TRANSMITTANCE_SIZE: vec2f = vec2f(256.0, 64.0);
const MULTISCATTER_SIZE: vec2f = vec2f(32.0, 32.0);
const SKY_VIEW_SIZE: vec2f = vec2f(192.0, 108.0);
const IRRADIANCE_SIZE: vec2f = vec2f(64.0, 16.0);

// Bruneton's: rows where transmittance changes.
fn transmittance_uv(r_in: f32, mu: f32) -> vec2f {
  let bottom = atmos_bottom_radius();
  let top = atmos_top_radius();
  let r = min(r_in, top);
  let h = sqrt(top * top - bottom * bottom);
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
  let h = sqrt(top * top - bottom * bottom);
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

fn horizon_dip(r: f32) -> f32 {
  let v_horizon = sqrt(max(r * r - atmos_bottom_radius() * atmos_bottom_radius(), 0.0));
  return acos(clamp(v_horizon / r, -1.0, 1.0));
}

// Hillaire's directions: across, the azimuth from the light's, squared toward
// it; up, the zenith, squared toward the horizon from either side.
fn sky_view_uv(r: f32, mu: f32, light_cos: f32) -> vec2f {
  let beta = horizon_dip(r);
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
  return vec2f(coord_of(x, SKY_VIEW_SIZE.x), coord_of(y, SKY_VIEW_SIZE.y));
}

// The direction at `unit`, `sky_view_uv`'s from 0 to 1, up +z and the light's azimuth +x.
fn view_dir(r: f32, unit: vec2f) -> vec3f {
  let beta = horizon_dip(r);
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

// A light at cosine `mu_s` over the horizon, as `view_dir` measures it.
fn view_light(mu_s: f32) -> vec3f {
  let m = clamp(mu_s, -1.0, 1.0);
  return vec3f(sqrt(max(1.0 - m * m, 0.0)), 0.0, m);
}
