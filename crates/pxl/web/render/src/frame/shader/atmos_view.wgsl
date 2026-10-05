// The air as the frame shows it: sky, ground past the terrain, and aerial perspective.

@group(0) @binding(15) var moon_view_lut: texture_2d<f32>;
@group(0) @binding(16) var earth_map: texture_2d_array<f32>;
@group(0) @binding(17) var earth_pages: texture_2d_array<f32>;
@group(0) @binding(19) var aerial_lut: texture_2d<f32>;

fn shown() -> vec3f {
  return atmos_white() * exposure();
}

// The lights where a point stands, exposed: each through the air above it, and
// the sky's on an up-facing face there. A fact of the point, wherever the eye is.
struct Light {
  sun: vec3f,
  other: vec3f,
  fill: vec3f,
};

fn light_at(r: f32, up: vec3f) -> Light {
  let e = exposure();
  var l: Light;
  l.sun = frame_sun_color() * sun_transmittance(r, dot(up, frame_sun_dir())) * e;
  l.other = frame_other_color() * sun_transmittance(r, dot(up, frame_other_dir())) * e;
  let sky = sky_ground(r, dot(up, atmos_sun_dir())).rgb + sky_ground(r, dot(up, atmos_moon_dir())).rgb * atmos_moon_light();
  l.fill = (sky * atmos_white() + atmos_night_sky()) * e;
  return l;
}

fn planet_center() -> vec3f {
  return vec3f(frame_planet().xy, frame_planet().w - frame_planet().z);
}

// A world point's radius, from the eye's: the frame's units are exact near it.
fn radius_of(world_pos: vec3f) -> f32 {
  let c = planet_center();
  return max(camera_r() + (length(world_pos - c) - length(frame_camera_pos() - c)) * atmos_km_per_u(), atmos_bottom_radius());
}

fn light_of(world_pos: vec3f) -> Light {
  return light_at(radius_of(world_pos), normalize(world_pos - planet_center()));
}

// What the ground below gives a down-facing face: what lights an up-facing one,
// at the Earth's mean albedo.
fn ground_bounce(l: Light) -> vec3f {
  return atmos_ground_albedo() * (l.sun * max(frame_sun_dir().z, 0.0) + l.other * max(frame_other_dir().z, 0.0) + l.fill);
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

// The Earth's face where a ray meets the ground past the terrain `km` from the
// eye, at the latitude and longitude of `n`, the ground's up there in the map
// center's east, north and up: its reflectance, and in alpha its lights' square
// root.
fn earth_face(n: vec3f, km: f32) -> vec4f {
  let sin_lat = frame_globe().y;
  let cos_lat = frame_globe().z;
  let center_lon = atmos_center_lon();
  let east = vec3f(-sin(center_lon), cos(center_lon), 0.0);
  let north = vec3f(-sin_lat * cos(center_lon), -sin_lat * sin(center_lon), cos_lat);
  let up = vec3f(cos_lat * cos(center_lon), cos_lat * sin(center_lon), sin_lat);
  let p = east * n.x + north * n.y + up * n.z;
  let lon = atan2(p.y, p.x);
  let lat = asin(clamp(p.z, -1.0, 1.0));
  let uv = vec2f(fract(lon / TAU + 0.5), 0.5 - lat / PI);
  let global = textureSampleLevel(earth_map, atmos_sampler, uv, 0, 0.0);
  // The Earth layer's page there, a z6 pixel a texel, where the window holds
  // one, taking over from the global picture, a z3 one, over the octave in
  // which a pixel goes from covering a z5 cell, at the pages' reach, to a z6
  // one. Distance alone decides it, as it decides what is held; a page not
  // held yet leaves the global picture.
  let share = clamp(log2(atmos_earth_reach() * atmos_km_per_u() / max(km, 1e-6)), 0.0, 1.0);
  if (share <= 0.0) {
    return global;
  }
  let w = atmos_earth_window();
  let at = vec2f(lon, asinh(tan(lat))) / TAU * f32(w.w);
  let page = vec2i(floor(at));
  let d = vec2i(((page.x - w.x) % w.w + w.w) % w.w, page.y - w.y);
  if (all(d >= vec2i(0)) && all(d < vec2i(w.z))) {
    let k = u32(d.y * w.z + d.x);
    let per_word = 32u / EARTH_PAGE_BITS;
    let layer = (atmos_earth_pages()[k / per_word] >> (EARTH_PAGE_BITS * (k % per_word))) & ((1u << EARTH_PAGE_BITS) - 1u);
    if (layer > 0u) {
      return mix(global, textureSampleLevel(earth_pages, atmos_sampler, fract(at), i32(layer - 1u), 0.0), share);
    }
  }
  return global;
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
fn surface_light(n: vec3f, lit: f32, l: Light) -> vec3f {
  // A face turned from the sun reads no shadow map, and is not its own shade.
  let shaded = select(1.0, mix(1.0, lit, SHADOW_DEPTH), dot(n, frame_sun_dir()) > 0.0);
  let up = l.sun * max(frame_sun_dir().z, 0.0) * shaded + l.other * max(frame_other_dir().z, 0.0) + l.fill;
  // three's standard material at roughness 1, metalness 0: Lambert, albedo over pi.
  let hemi = mix(ground_bounce(l), l.fill, n.z * 0.5 + 0.5);
  let directed = l.sun * max(dot(n, frame_sun_dir()), 0.0) * lit + l.other * max(dot(n, frame_other_dir()), 0.0) + hemi;
  return mix(up, directed, DIRECTED) / PI;
}

// Past the terrain the ground is lit by the terrain's own light, so the drawn
// edge is no seam; no shadow map reaches it.
fn beyond(air: Air) -> vec3f {
  var light = air.light;
  if (air.ground) {
    let n = air.normal;
    let face = earth_face(n, air.ground_km);
    light += air.end * (face.rgb * surface_light(n, 1.0, light_at(land_radius(), n)) + city_shown(face.a));
  }
  return light;
}

struct Veil {
  through: vec3f,
  scattered: vec3f,
};

// What the air between the eye and a point leaves of it and lays over it: the
// tables' transmittance, as the Earth past the terrain is dimmed, and the light
// the aerial perspective gathered to that distance along that ray.
fn air_to(world_pos: vec3f) -> Veil {
  var v: Veil;
  let to = world_pos - frame_camera_pos();
  let du = length(to);
  if (du < 1e-4) {
    v.through = vec3f(1.0);
    v.scattered = vec3f(0.0);
    return v;
  }
  let r = camera_r();
  let mu = dot(to / du, frame_up());
  let d = du * atmos_km_per_u();
  v.through = transmittance_along(r, mu, d, meets_ground(r, mu));
  v.scattered = aerial_at(world_pos, r, mu, d) * exposure();
  return v;
}

// The aerial perspective where a point falls on the screen, between the two
// slices about its distance; nearer than the first, a share of it.
fn aerial_at(world_pos: vec3f, r: f32, mu: f32, d: f32) -> vec3f {
  let clip = frame_view_proj() * vec4f(world_pos, 1.0);
  let ndc = clamp(clip.xy / max(clip.w, 1e-6), vec2f(-1.0), vec2f(1.0));
  let x = coord_of(ndc.x * 0.5 + 0.5, AERIAL_SIZE.x);
  let y = coord_of(ndc.y * 0.5 + 0.5, AERIAL_SIZE.y);
  let s = sqrt(clamp(d / max(air_path(r, mu), 1e-6), 0.0, 1.0)) * AERIAL_SIZE.z - 1.0;
  let k = clamp(floor(s), 0.0, AERIAL_SIZE.z - 1.0);
  let near = textureSampleLevel(aerial_lut, atmos_sampler, vec2f((k + x) / AERIAL_SIZE.z, y), 0.0).rgb;
  if (s < 0.0) {
    return near * (s + 1.0);
  }
  let far = textureSampleLevel(aerial_lut, atmos_sampler, vec2f((min(k + 1.0, AERIAL_SIZE.z - 1.0) + x) / AERIAL_SIZE.z, y), 0.0).rgb;
  return mix(near, far, s - k);
}

fn aerial(color: vec3f, world_pos: vec3f) -> vec3f {
  let v = air_to(world_pos);
  return color * v.through + v.scattered;
}

// Computed per vertex: every term varies slowly across a face, and it saves
// the table reads per fragment.
struct Seen {
  through: f32,
  scattered: vec3f,
  light: Light,
};

fn seen_from_eye(world_pos: vec3f) -> Seen {
  let light = light_of(world_pos);
  let v = air_to(world_pos);
  var s: Seen;
  // A lit pass's color is authored, not reflectance, so the air dims it alike
  // at every wavelength: dimmed by wavelength, a light sea's blue went and its
  // green stayed. The air's own light, and what is reflectance — the Earth's
  // picture, a cloud — keep the air's color.
  s.through = dot(v.through, LUMA);
  s.scattered = v.scattered;
  s.light = light;
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
