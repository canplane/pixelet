// The air as the frame shows it: sky, ground past the terrain, and aerial perspective.

@group(0) @binding(13) var exposure_texel: texture_2d<f32>;
@group(0) @binding(15) var moon_view_lut: texture_2d<f32>;

fn exposure() -> f32 {
  return textureLoad(exposure_texel, vec2i(0, 0), 0).r;
}

fn shown() -> vec3f {
  return atmos_white() * exposure();
}

// One texel from the exposure pass rather than a table read per fragment.
fn zenith_sky() -> vec3f {
  let t = textureLoad(exposure_texel, vec2i(0, 0), 0);
  return t.gba * atmos_white() * t.r;
}

// On the terrain's scale, before adaptation: the terrain's direct light does
// not adapt, so a fill that did would wash the ground out at sunset.
fn sky_fill() -> vec3f {
  var fill = textureLoad(exposure_texel, vec2i(1, 0), 0).rgb * atmos_white() * atmos_exposure();
  if (atmos_moon_light() > 0.0) {
    fill += moon_view(atmos_up()) * moon_shown() * PI;
  }
  return fill;
}

// The land's color under everything lighting an up-facing face, over pi.
fn ground_bounce(fill: vec3f) -> vec3f {
  let up = vec3f(0.0, 0.0, 1.0);
  let lit = frame_sun_color() * max(frame_sun_dir().z, 0.0) + frame_other_color() * max(frame_other_dir().z, 0.0) + frame_ambient() + fill;
  return atmos_land() * (lit / PI + frame_floor());
}

// Set by the eye's night adaptation, not the moon's light.
fn moon_shown() -> vec3f {
  return atmos_white() * atmos_exposure() * atmos_moon_light();
}

fn moon_view(dir: vec3f) -> vec3f {
  return table_view(moon_view_lut, dir, atmos_moon_dir());
}

// Bruneton: a ratio of two lookups to the top, reversed for a ray that meets
// the ground.
fn transmittance_along(r: f32, mu: f32, d: f32, ground: bool) -> vec3f {
  let rd = clamp(sqrt(d * d + 2.0 * r * mu * d + r * r), atmos_bottom_radius(), atmos_top_radius());
  let mud = clamp((r * mu + d) / rd, -1.0, 1.0);
  if (ground) {
    return min(transmittance_to_top(rd, -mud) / max(transmittance_to_top(r, -mu), vec3f(1e-6)), vec3f(1.0));
  }
  return min(transmittance_to_top(r, mu) / max(transmittance_to_top(rd, mud), vec3f(1e-6)), vec3f(1.0));
}

struct Air {
  light: vec3f,
  end: vec3f,
  ground: bool,
  ground_km: f32,
  // The ground past the terrain is lit on its own slope of the sphere.
  normal: vec3f,
};

fn air_along(dir: vec3f) -> Air {
  let r = camera_r();
  var air: Air;
  air.light = sky_view(dir, atmos_sun_dir()) * shown();
  if (atmos_moon_light() > 0.0) {
    air.light += moon_view(dir) * moon_shown();
  }
  let mu = dot(dir, atmos_up());
  air.ground = meets_land(r, mu);
  air.normal = atmos_up();
  if (air.ground) {
    let d = to_land(r, mu);
    air.ground_km = d;
    air.end = transmittance_along(r, mu, d, true);
    air.normal = normalize(atmos_up() * r + dir * d);
  } else {
    air.end = transmittance_to_top(r, mu);
  }
  return air;
}

// Closed-form optical depth of an exponential layer along a flat segment;
// a ratio of two table reads is noise at these lengths.
fn layer_depth(h0: f32, h1: f32, d: f32, scale: f32) -> f32 {
  let dh = h1 - h0;
  if (abs(dh) < 1e-3) {
    return d * exp(-(h0 + h1) * 0.5 / scale);
  }
  return d * scale * (exp(-h0 / scale) - exp(-h1 / scale)) / dh;
}

fn haze_through(h0: f32, h1: f32, d: f32) -> f32 {
  return exp(-atmos_haze_extinction() * layer_depth(h0, h1, d, atmos_haze_scale_height()));
}

// Continental haze single-scattering albedo, near OPAC's continental types at
// green (Hess, Koepke and Schult, 1998); nothing else fixes it.
const HAZE_ALBEDO: f32 = 0.9;

// Lit where the air is, not along the ray: a veil taken from the sky's ray
// laid a bright line over the ground at the horizon toward a low sun.
struct AirLight {
  rayleigh: vec3f,
  mie: vec3f,
};

fn air_light(dir: vec3f, fill: vec3f) -> AirLight {
  let c_sun = dot(dir, frame_sun_dir());
  let c_other = dot(dir, frame_other_dir());
  let around = 0.5 * ((fill + frame_ambient()) / PI + ground_bounce(fill));
  var l: AirLight;
  l.rayleigh = frame_sun_color() * rayleigh_phase(c_sun) + frame_other_color() * rayleigh_phase(c_other) + around;
  l.mie = frame_sun_color() * mie_phase(c_sun) + frame_other_color() * mie_phase(c_other) + around;
  return l;
}

// Past the terrain the ground is lit as shade.wgsl lights the terrain, so the
// drawn edge is no seam.
fn beyond(air: Air, fill: vec3f) -> vec3f {
  var light = air.light;
  if (air.ground) {
    let n = air.normal;
    let lit = frame_sun_color() * max(dot(frame_sun_dir(), n), 0.0) + frame_other_color() * max(dot(frame_other_dir(), n), 0.0) + frame_ambient() + sky_fill();
    light += air.end * atmos_land() * (lit / PI + frame_floor());
    // Haze reads as the air's own light along the ray.
    light = mix(air.light, light, haze_through(max(atmos_camera_altitude(), 0.0), 0.0, air.ground_km));
  }
  return light;
}

struct Veil {
  through: vec3f,
  scattered: vec3f,
};

fn veil(world_pos: vec3f) -> Veil {
  var v: Veil;
  let du = length(world_pos - frame_camera_pos());
  if (du < 1e-4) {
    v.through = vec3f(1.0);
    v.scattered = vec3f(0.0);
    return v;
  }
  let d = du * atmos_km_per_u();
  let h0 = clamp(atmos_camera_altitude(), 0.0, atmos_top_radius() - atmos_bottom_radius());
  let planet = vec3f(frame_planet().xy, frame_planet().w - frame_planet().z);
  let h1 = max(h0 + (length(world_pos - planet) - length(frame_camera_pos() - planet)) * atmos_km_per_u(), 0.0);
  let rayleigh = atmos_rayleigh_scattering() * layer_depth(h0, h1, d, atmos_rayleigh_scale_height());
  let mie = atmos_mie_extinction() * layer_depth(h0, h1, d, atmos_mie_scale_height());
  let haze = atmos_haze_extinction() * layer_depth(h0, h1, d, atmos_haze_scale_height());
  // Single scattering over a segment lit uniformly, short enough for that.
  let depth = rayleigh + vec3f(mie + haze);
  v.through = exp(-depth);
  let lit = air_light((world_pos - frame_camera_pos()) / du, sky_fill());
  let turned = rayleigh * lit.rayleigh + (mie * atmos_mie_scattering() / atmos_mie_extinction() + haze * HAZE_ALBEDO) * lit.mie;
  v.scattered = (vec3f(1.0) - v.through) * turned / max(depth, vec3f(1e-6));
  return v;
}

fn aerial(color: vec3f, world_pos: vec3f) -> vec3f {
  let v = veil(world_pos);
  return color * v.through + v.scattered;
}

// Computed per vertex: every term varies slowly across a face, and it saves
// the table reads per fragment.
struct Seen {
  through: vec3f,
  scattered: vec3f,
  past: vec3f,
  fill: vec3f,
};

// `seen_from_eye` where the frame asks for it, and the air left out where
// it does not: the shadow map's passes write depth alone, and a vertex there
// would pay for the air and the sky's fill for nothing. A
// branch on a uniform, which every vertex of the draw takes the same way.
fn seen_at_vertex(world_pos: vec3f) -> Seen {
  if (frame_air_at_vertices() > 0.5) {
    return seen_from_eye(world_pos);
  }
  return Seen(vec3f(1.0), vec3f(0.0), vec3f(0.0), vec3f(0.0));
}

fn seen_from_eye(world_pos: vec3f) -> Seen {
  let air = air_along(normalize(world_pos - frame_camera_pos()));
  let v = veil(world_pos);
  var s: Seen;
  s.through = v.through;
  s.scattered = v.scattered;
  s.past = shoulder(beyond(air));
  s.fill = sky_fill();
  return s;
}

// Spencer et al. 1995 point-spread halo. The strength is a camera's: ISO 9358
// veiling glare is 1-10%, GLARE_VEIL is its middle. Drawn over the sky only.
const GLARE_HALO_SHARE: f32 = 0.6205;
const GLARE_VEIL: f32 = 0.03;

fn glare(dir: vec3f) -> vec3f {
  let s = atmos_sun_dir();
  let theta = degrees(atan2(length(cross(dir, s)), dot(dir, s))) + 0.02;
  let psf = 0.478 * 20.91 / (theta * theta * theta) + 0.138 * 72.37 / (theta * theta);
  return atmos_sun_seen() * shown() * psf * (GLARE_VEIL / GLARE_HALO_SHARE);
}

// Soft shoulder so the sun's glow and lit clouds do not clip flat.
fn shoulder(c: vec3f) -> vec3f {
  let k = 0.75;
  return select(c, k + (1.0 - k) * (1.0 - exp(-(c - k) / (1.0 - k))), c > vec3f(k));
}
