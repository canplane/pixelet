// The atmosphere as a medium: what fills it at an altitude, how that
// scatters, and where a ray through it ends. Every number is the uniform's
// (look/atmosphere.rs is their one home, gpu/atmosphere.rs lays them out);
// a shader that includes this declares `atmos` at a binding of its own.
// Distances are kilometres, and a point is its radius from the planet's
// centre `r` and the cosine `mu` of its ray from the local zenith, as
// Bruneton writes them.

struct Atmosphere {
  rayleigh_scattering: vec3f,
  bottom_radius: f32,
  ozone_absorption: vec3f,
  top_radius: f32,
  // The white balance, by channel: a sun overhead at sea level as white.
  white: vec3f,
  rayleigh_scale_height: f32,
  // Toward the sun, in the root frame the meshes are placed in.
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
  // What a table's radiance, for a sun of illuminance 1, is shown at under
  // a sun overhead, how far the eye raises it as the light falls at most,
  // and by what power of the fall (look/atmosphere.rs's ADAPTATION).
  exposure: f32,
  max_adaptation: f32,
  adaptation: f32,
  // Toward the moon, in the root frame, and how bright the sky it lights is
  // shown, its phase and the sun's height in it (look/environment.rs); 0
  // while the sun's sky is the one to see.
  moon_dir: vec3f,
  moon_light: f32,
  // The ground's colour past the terrain, in linear light
  // (look/atmosphere.rs's `land`).
  land: vec3f,
  _p0: f32,
  // What reaches the eye of the Sun's light, by channel, unexposed: the
  // same for every pixel of the frame, so the CPU takes it once
  // (look/atmosphere.rs's light_at). What the glare is of.
  sun_seen: vec3f,
  _p1: f32,
};

// The camera's radius from the planet's centre: over the ground, and above
// the atmosphere too, where the view is from space — the sky-view table is
// drawn from there (atmos_skyview.wgsl marches from where a ray enters the
// air) and the ground and the horizon are where the planet puts them.
fn camera_r() -> f32 {
  return atmos.bottom_radius + max(atmos.camera_altitude, 0.001);
}

struct Medium {
  rayleigh: vec3f,
  mie: vec3f,
  scattering: vec3f,
  extinction: vec3f,
};

fn medium_at(h: f32) -> Medium {
  let r = exp(-h / atmos.rayleigh_scale_height);
  let m = exp(-h / atmos.mie_scale_height);
  let o = max(0.0, 1.0 - abs(h - atmos.ozone_center) / atmos.ozone_half_width);
  var out: Medium;
  out.rayleigh = atmos.rayleigh_scattering * r;
  out.mie = vec3f(atmos.mie_scattering * m);
  out.scattering = out.rayleigh + out.mie;
  out.extinction = out.rayleigh + vec3f(atmos.mie_extinction * m) + atmos.ozone_absorption * o;
  return out;
}

fn rayleigh_phase(c: f32) -> f32 {
  return 3.0 * INV_4PI / 4.0 * (1.0 + c * c);
}

// Cornette-Shanks: Henyey-Greenstein's shape with Rayleigh's symmetry
// folded in, which is what keeps a hazy sky bright opposite the sun too.
fn mie_phase(c: f32) -> f32 {
  let g = atmos.mie_g;
  let k = 3.0 * INV_4PI / 2.0 * (1.0 - g * g) / (2.0 + g * g);
  return k * (1.0 + c * c) / pow(max(1.0 + g * g - 2.0 * g * c, 1e-4), 1.5);
}

fn meets_ground(r: f32, mu: f32) -> bool {
  return mu < 0.0 && r * r * (mu * mu - 1.0) + atmos.bottom_radius * atmos.bottom_radius >= 0.0;
}

fn to_top(r: f32, mu: f32) -> f32 {
  return max(-r * mu + sqrt(max(r * r * (mu * mu - 1.0) + atmos.top_radius * atmos.top_radius, 0.0)), 0.0);
}

fn to_ground(r: f32, mu: f32) -> f32 {
  return max(-r * mu - sqrt(max(r * r * (mu * mu - 1.0) + atmos.bottom_radius * atmos.bottom_radius, 0.0)), 0.0);
}

// A table's texel centres to [0, 1] and back: the ends of a range land on
// the first and last texels' centres, so nothing reads past them.
fn unit_of(u: f32, size: f32) -> f32 {
  return (u - 0.5 / size) / (1.0 - 1.0 / size);
}

fn coord_of(x: f32, size: f32) -> f32 {
  return 0.5 / size + x * (1.0 - 1.0 / size);
}

const TRANSMITTANCE_SIZE: vec2f = vec2f(256.0, 64.0);
const MULTISCATTER_SIZE: vec2f = vec2f(32.0, 32.0);
const SKY_VIEW_SIZE: vec2f = vec2f(192.0, 108.0);

// Bruneton's transmittance parameterisation: the height through rho, the
// distance to the top through the range a height allows, which spends the
// table's rows where the transmittance changes and not where it is flat.
fn transmittance_uv(r_in: f32, mu: f32) -> vec2f {
  let bottom = atmos.bottom_radius;
  let top = atmos.top_radius;
  // Above the top there is no air left to cross: what is left from there is
  // what is left from the top.
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

// The inverse, for the pass that draws the table: the (r, mu) a texel is.
fn transmittance_r_mu(uv: vec2f) -> vec2f {
  let bottom = atmos.bottom_radius;
  let top = atmos.top_radius;
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

// The multiple scattering's table: the sun's cosine across, the altitude up.
fn multiscatter_uv(r: f32, mu_s: f32) -> vec2f {
  let x = mu_s * 0.5 + 0.5;
  let y = (r - atmos.bottom_radius) / (atmos.top_radius - atmos.bottom_radius);
  return vec2f(coord_of(clamp(x, 0.0, 1.0), MULTISCATTER_SIZE.x), coord_of(clamp(y, 0.0, 1.0), MULTISCATTER_SIZE.y));
}

// How far below the level the horizon lies from radius `r`, as an angle:
// the sky-view table splits its rows there, the sky above and the ground
// below.
fn horizon_dip(r: f32) -> f32 {
  let v_horizon = sqrt(max(r * r - atmos.bottom_radius * atmos.bottom_radius, 0.0));
  return acos(clamp(v_horizon / r, -1.0, 1.0));
}

// The sky as seen from one altitude (Hillaire's sky-view table): the view
// zenith up the rows, squared toward the horizon from both sides, where
// the sky changes fastest; the view's azimuth from the sun across, squared
// toward the sun, where the Mie glow is. `light_cos` is the cosine of the
// angle between the view's and the sun's azimuths.
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
