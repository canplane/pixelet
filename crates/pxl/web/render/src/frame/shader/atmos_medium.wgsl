// The atmosphere as a medium. Distances are km; a point is (r, mu) as Bruneton writes them.

struct Atmosphere {
  rayleigh_scattering: vec3f,
  bottom_radius: f32,
  ozone_absorption: vec3f,
  top_radius: f32,
  // A sun overhead at sea level as white.
  white: vec3f,
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
  max_adaptation: f32,
  adaptation: f32,
  moon_dir: vec3f,
  moon_light: f32,
  // `land` is drawn at the elevation under the eye: a sea-level surface would
  // leave the terrain floating, its chunk walls showing at the edge.
  land: vec3f,
  land_altitude_km: f32,
  // The same for every pixel, so the CPU computes it once.
  sun_seen: vec3f,
  _p1: f32,
  // A moonless sky's irradiance on an up-facing face. It lights the ground and
  // is not drawn: at the gain that lets the ground read, the sky drew pale blue.
  night_sky: vec3f,
  _p2: f32,
  cloud_base: vec2u,
  cloud_frac: vec2f,
  cloud_time: f32,
  cloud_count: u32,
  cloud_altitude_km: f32,
  cloud_gone_km: f32,
  // Aerosol the tables do not carry, applied only in aerial perspective.
  haze_extinction: f32,
  haze_scale_height: f32,
};

fn atmos_rayleigh_scattering() -> vec3f { return atmos_words[0].xyz; }
fn atmos_bottom_radius() -> f32 { return atmos_words[0].w; }
fn atmos_ozone_absorption() -> vec3f { return atmos_words[1].xyz; }
fn atmos_top_radius() -> f32 { return atmos_words[1].w; }
fn atmos_white() -> vec3f { return atmos_words[2].xyz; }
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
fn atmos_max_adaptation() -> f32 { return atmos_words[6].z; }
fn atmos_adaptation() -> f32 { return atmos_words[6].w; }
fn atmos_moon_dir() -> vec3f { return atmos_words[7].xyz; }
fn atmos_moon_light() -> f32 { return atmos_words[7].w; }
fn atmos_night_sky() -> vec3f { return atmos_words[10].xyz; }
fn atmos_land() -> vec3f { return atmos_words[8].xyz; }
fn atmos_land_altitude_km() -> f32 { return atmos_words[8].w; }
fn atmos_sun_seen() -> vec3f { return atmos_words[9].xyz; }
fn atmos_cloud_base() -> vec2u { return bitcast<vec2u>(atmos_words[11].xy); }
fn atmos_cloud_frac() -> vec2f { return atmos_words[11].zw; }
fn atmos_cloud_time() -> f32 { return atmos_words[12].x; }
fn atmos_cloud_count() -> u32 { return bitcast<u32>(atmos_words[12].y); }
fn atmos_cloud_altitude_km() -> f32 { return atmos_words[12].z; }
fn atmos_cloud_gone_km() -> f32 { return atmos_words[12].w; }
fn atmos_haze_extinction() -> f32 { return atmos_words[13].x; }
fn atmos_haze_scale_height() -> f32 { return atmos_words[13].y; }

// Can be above the atmosphere: the view from space.
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

// Cornette-Shanks: Henyey-Greenstein with Rayleigh's symmetry, which keeps a
// hazy sky bright opposite the sun.
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

fn to_top(r: f32, mu: f32) -> f32 {
  return max(-r * mu + sqrt(max(r * r * (mu * mu - 1.0) + atmos_top_radius() * atmos_top_radius(), 0.0)), 0.0);
}

fn to_ground(r: f32, mu: f32) -> f32 {
  return to_sphere(r, mu, atmos_bottom_radius());
}

// Raised to the ground under the map center for drawing only; the tables keep
// the radius they were baked at.
fn land_radius() -> f32 {
  return atmos_bottom_radius() + atmos_land_altitude_km();
}

fn meets_land(r: f32, mu: f32) -> bool {
  return meets_sphere(r, mu, land_radius());
}

fn to_land(r: f32, mu: f32) -> f32 {
  return to_sphere(r, mu, land_radius());
}

// Range ends land on the first and last texel centers, so nothing reads past them.
fn unit_of(u: f32, size: f32) -> f32 {
  return (u - 0.5 / size) / (1.0 - 1.0 / size);
}

fn coord_of(x: f32, size: f32) -> f32 {
  return 0.5 / size + x * (1.0 - 1.0 / size);
}

const TRANSMITTANCE_SIZE: vec2f = vec2f(256.0, 64.0);
const MULTISCATTER_SIZE: vec2f = vec2f(32.0, 32.0);
const SKY_VIEW_SIZE: vec2f = vec2f(192.0, 108.0);

// Bruneton's parameterization, which spends rows where transmittance changes.
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

// Hillaire's sky-view table: zenith squared toward the horizon, azimuth
// squared toward the sun, where the sky changes fastest.
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
