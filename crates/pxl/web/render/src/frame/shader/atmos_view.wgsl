// The air as the frame shows it: sky, ground past the terrain, and aerial perspective.

@group(0) @binding(13) var exposure_texel: texture_2d<f32>;
@group(0) @binding(15) var moon_view_lut: texture_2d<f32>;
@group(0) @binding(16) var earth_map: texture_2d_array<f32>;
@group(0) @binding(17) var earth_pages: texture_2d_array<f32>;

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

fn sky_fill() -> vec3f {
  var fill = (textureLoad(exposure_texel, vec2i(1, 0), 0).rgb * atmos_white() + atmos_night_sky()) * exposure();
  if (atmos_moon_light() > 0.0) {
    fill += moon_view(frame_up()) * moon_shown() * PI;
  }
  return fill;
}

// What the ground below gives a down-facing face: what lights an up-facing one,
// at the Earth's mean albedo, an irradiance as `fill` is. `e` is the sky's
// exposure, read once by the caller, so the ground is seen by the sky's eye.
fn ground_bounce(fill: vec3f, e: f32) -> vec3f {
  let lit = (frame_sun_color() * max(frame_sun_dir().z, 0.0) + frame_other_color() * max(frame_other_dir().z, 0.0)) * e + fill;
  return atmos_ground_albedo() * lit;
}

fn moon_shown() -> vec3f {
  return shown() * atmos_moon_light();
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
  let mu = dot(dir, frame_up());
  air.ground = meets_land(r, mu);
  air.normal = frame_up();
  if (air.ground) {
    let d = to_land(r, mu);
    air.ground_km = d;
    air.end = transmittance_along(r, mu, d, true);
    air.normal = normalize(frame_up() * r + dir * d);
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

// The ground's haze, a near view's: it leaves as the view goes over to the
// Earth's picture, which no veil seen from orbit covers.
fn ground_haze() -> f32 {
  return atmos_haze_extinction() * (1.0 - frame_earth_share());
}

fn haze_through(h0: f32, h1: f32, d: f32) -> f32 {
  return exp(-ground_haze() * layer_depth(h0, h1, d, atmos_haze_scale_height()));
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
  let e = exposure();
  let around = 0.5 * (fill + ground_bounce(fill, e)) / PI;
  let sun = frame_sun_color() * e;
  let other = frame_other_color() * e;
  var l: AirLight;
  l.rayleigh = sun * rayleigh_phase(c_sun) + other * rayleigh_phase(c_other) + around;
  l.mie = sun * mie_phase(c_sun) + other * mie_phase(c_other) + around;
  return l;
}

// The Earth's face where a ray meets the ground past the terrain, at the
// latitude and longitude of `n`, the ground's up there in the map center's
// east, north and up: its reflectance, and in alpha its lights' square root.
fn earth_face(n: vec3f) -> vec4f {
  let sin_lat = frame_globe().y;
  let cos_lat = frame_globe().z;
  let center_lon = atmos_center_lon();
  let east = vec3f(-sin(center_lon), cos(center_lon), 0.0);
  let north = vec3f(-sin_lat * cos(center_lon), -sin_lat * sin(center_lon), cos_lat);
  let up = vec3f(cos_lat * cos(center_lon), cos_lat * sin(center_lon), sin_lat);
  let p = east * n.x + north * n.y + up * n.z;
  let lon = atan2(p.y, p.x);
  let lat = asin(clamp(p.z, -1.0, 1.0));
  // The Earth layer's page there, where the window holds one: a z6 pixel a texel.
  let w = atmos_earth_window();
  let at = vec2f(lon, asinh(tan(lat))) / TAU * f32(w.w);
  let page = vec2i(floor(at));
  let d = vec2i(((page.x - w.x) % w.w + w.w) % w.w, page.y - w.y);
  if (all(d >= vec2i(0)) && all(d < vec2i(w.z))) {
    let k = u32(d.y * w.z + d.x);
    let per_word = 32u / EARTH_PAGE_BITS;
    let layer = (atmos_earth_pages()[k / per_word] >> (EARTH_PAGE_BITS * (k % per_word))) & ((1u << EARTH_PAGE_BITS) - 1u);
    if (layer > 0u) {
      return textureSampleLevel(earth_pages, atmos_sampler, fract(at), i32(layer - 1u), 0.0);
    }
  }
  let uv = vec2f(fract(lon / TAU + 0.5), 0.5 - lat / PI);
  return textureSampleLevel(earth_map, atmos_sampler, uv, 0, 0.0);
}

// The lights come on as the eye opens: by day the night eye's gain on them
// would show every city lit at noon.
fn city_shown(lights: f32) -> vec3f {
  let night = (exposure() / atmos_exposure() - 1.0) / (atmos_max_adaptation() - 1.0);
  return atmos_city_light() * lights * lights * exposure() * clamp(night, 0.0, 1.0);
}

// The share of the light that keeps its direction; the rest, what an up-facing
// face takes, reaches every face alike. Lit by the sun's direction alone, a
// matte wall turned from it fell to a fifth of the ground and read as a toy's.
const DIRECTED: f32 = 0.25;
// What a cast shadow takes of the sun in the light that reaches every face:
// all of it left the shade lit by the sky alone, as dark as the toy's.
const SHADOW_DEPTH: f32 = 0.5;

// What a matte face of albedo one takes, over pi: shade.wgsl's for every lit
// pass, and the Earth's past the terrain (`beyond`), so the two meet unseamed.
fn surface_light(n: vec3f, lit: f32, fill: vec3f) -> vec3f {
  let e = exposure();
  let sun = frame_sun_color() * e;
  let other = frame_other_color() * e;
  // A face turned from the sun reads no shadow map, and is not its own shade.
  let shaded = select(1.0, mix(1.0, lit, SHADOW_DEPTH), dot(n, frame_sun_dir()) > 0.0);
  let up = sun * max(frame_sun_dir().z, 0.0) * shaded + other * max(frame_other_dir().z, 0.0) + fill;
  // three's standard material at roughness 1, metalness 0: Lambert, albedo over pi.
  let hemi = mix(ground_bounce(fill, e), fill, n.z * 0.5 + 0.5);
  let directed = sun * max(dot(n, frame_sun_dir()), 0.0) * lit + other * max(dot(n, frame_other_dir()), 0.0) + hemi;
  return mix(up, directed, DIRECTED) / PI;
}

// Past the terrain the ground is lit by the terrain's own light, so the drawn
// edge is no seam; no shadow map reaches it.
fn beyond(air: Air, fill: vec3f) -> vec3f {
  var light = air.light;
  if (air.ground) {
    let n = air.normal;
    let face = earth_face(n);
    light += air.end * (face.rgb * surface_light(n, 1.0, fill) + city_shown(face.a));
    // Haze reads as the air's own light along the ray.
    light = mix(air.light, light, haze_through(max(atmos_camera_altitude(), 0.0), 0.0, air.ground_km));
  }
  return light;
}

struct Veil {
  through: vec3f,
  scattered: vec3f,
};

fn veil(world_pos: vec3f, fill: vec3f) -> Veil {
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
  let haze = ground_haze() * layer_depth(h0, h1, d, atmos_haze_scale_height());
  // Single scattering over a segment lit uniformly, short enough for that.
  let depth = rayleigh + vec3f(mie + haze);
  let through = exp(-depth);
  // The palette is no reflectance: dimmed by wavelength, a light sea's blue went
  // and its green stayed. The air's own light keeps its color, and the Earth's
  // picture, which is reflectance, is dimmed as it is past the terrain.
  v.through = mix(vec3f(dot(through, LUMA)), through, frame_earth_share());
  let lit = air_light((world_pos - frame_camera_pos()) / du, fill);
  let turned = rayleigh * lit.rayleigh + (mie * atmos_mie_scattering() / atmos_mie_extinction() + haze * HAZE_ALBEDO) * lit.mie;
  v.scattered = (vec3f(1.0) - through) * turned / max(depth, vec3f(1e-6));
  return v;
}

fn aerial(color: vec3f, world_pos: vec3f, fill: vec3f) -> vec3f {
  let v = veil(world_pos, fill);
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

fn seen_from_eye(world_pos: vec3f) -> Seen {
  let fill = sky_fill();
  let air = air_along(normalize(world_pos - frame_camera_pos()));
  let v = veil(world_pos, fill);
  var s: Seen;
  s.through = v.through;
  s.scattered = v.scattered;
  s.past = beyond(air, fill);
  s.fill = fill;
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
