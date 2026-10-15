// The air as the frame shows it: sky, ground past the terrain, and aerial perspective.

@group(0) @binding(15) var moon_view_lut: texture_2d<f32>;
@group(0) @binding(16) var earth_map: texture_2d_array<f32>;
@group(0) @binding(17) var earth_tiles: texture_2d_array<f32>;


/* where a point stands */

fn planet_center() -> vec3f {
  return vec3f(frame_planet().xy, frame_planet().w - frame_planet().z);
}

// Straight up at a point; with no planet, the frame's.
fn up_at(world_pos: vec3f) -> vec3f {
  return select(vec3f(0.0, 0.0, 1.0), normalize(world_pos - planet_center()), frame_planet().z > 0.0);
}

// From the eye's radius: the frame's units are exact near it.
fn radius_of(world_pos: vec3f) -> f32 {
  let c = planet_center();
  return max(camera_r() + (length(world_pos - c) - length(frame_camera_pos() - c)) * atmos_km_per_u(), atmos_bottom_radius());
}


/* lights */

fn shown() -> vec3f {
  return atmos_white() * exposure();
}

fn moon_shown() -> vec3f {
  return shown() * atmos_moon_light();
}

// Each light through the air above a point, and the sky's on an up-facing face there.
struct Light {
  sun: vec3f,
  other: vec3f,
  fill: vec3f,
};

// Before the eye: what the exposure meters.
fn light_unexposed(r: f32, up: vec3f) -> Light {
  let sky = sky_ground(r, dot(up, atmos_sun_dir())).rgb + sky_ground(r, dot(up, atmos_moon_dir())).rgb * atmos_moon_light();
  return Light(
    frame_sun_color() * sun_transmittance(r, dot(up, frame_sun_dir())),
    frame_other_color() * sun_transmittance(r, dot(up, frame_other_dir())),
    sky * atmos_white() + atmos_night_sky(),
  );
}

fn light_at(r: f32, up: vec3f) -> Light {
  let l = light_unexposed(r, up);
  let e = exposure();
  return Light(l.sun * e, l.other * e, l.fill * e);
}

fn light_of(world_pos: vec3f) -> Light {
  return light_at(radius_of(world_pos), up_at(world_pos));
}

// The ground under a down-facing face, at the Earth's mean albedo.
fn ground_bounce(up: vec3f, l: Light) -> vec3f {
  return atmos_ground_albedo() * (l.sun * max(dot(up, frame_sun_dir()), 0.0) + l.other * max(dot(up, frame_other_dir()), 0.0) + l.fill);
}

// The share that keeps its direction; the rest, an up-facing face's, reaches
// every face alike. Directed alone, a wall turned from the sun read as a toy's.
const DIRECTED: f32 = 0.25;

// A matte face of albedo one, over pi, Lambert as three's standard material at
// roughness 1: every lit pass's, and the Earth's past the terrain.
fn surface_light(n: vec3f, up: vec3f, l: Light) -> vec3f {
  let alike = l.sun * max(dot(up, frame_sun_dir()), 0.0) + l.other * max(dot(up, frame_other_dir()), 0.0) + l.fill;
  let hemi = mix(ground_bounce(up, l), l.fill, dot(n, up) * 0.5 + 0.5);
  let directed = l.sun * max(dot(n, frame_sun_dir()), 0.0) + l.other * max(dot(n, frame_other_dir()), 0.0) + hemi;
  return mix(alike, directed, DIRECTED) / PI;
}


/* sky */

fn moon_view(dir: vec3f) -> vec3f {
  return table_view(moon_view_lut, dir, atmos_moon_dir());
}

struct Air {
  light: vec3f,
  end: vec3f,
  ground: bool,
  // The ground past the terrain is lit on its own slope of the sphere.
  normal: vec3f,
  // Its distance from the eye against the earth layer's reach, in octaves.
  octaves: f32,
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
    // The light is the sky view's, gathered to the ground along this ray.
    let d = to_land(r, mu);
    let low = ray_low(r, mu, d);
    air.end = exp(depth_along(r, mu, low, 0.0) - depth_along(r, mu, low, d));
    air.normal = normalize(frame_up() * r + dir * d);
    air.octaves = log2(d / atmos_km_per_u() / atmos_earth_reach());
  } else {
    air.end = transmittance_to_top(r, mu);
  }
  return air;
}


/* the Earth past the terrain */

// Reflectance, and in alpha its lights' square root, where `n` is the ground's up
// in the east, north and up of the sea under the eye. The global picture is read at the
// point's latitude and longitude, a held tile at the globe's inverse of it.
fn earth_face(n: vec3f, octaves: f32) -> vec4f {
  let sin_lat = frame_globe().y;
  let cos_lat = frame_globe().z;
  let center_lon = atmos_center_lon();
  let east = vec3f(-sin(center_lon), cos(center_lon), 0.0);
  let north = vec3f(-sin_lat * cos(center_lon), -sin_lat * sin(center_lon), cos_lat);
  let up = vec3f(cos_lat * cos(center_lon), cos_lat * sin(center_lon), sin_lat);
  let p = east * n.x + north * n.y + up * n.z;
  let uv = vec2f(fract(atan2(p.y, p.x) / TAU + 0.5), 0.5 - asin(clamp(p.z, -1.0, 1.0)) / PI);
  let global = textureSampleLevel(earth_map, atmos_sampler, uv, 0, 0.0);
  let flat = off_globe_of(planet_center() + n * frame_planet().z, frame_planet(), frame_globe());
  let mercator = vec2f(center_lon, log(frame_globe().x)) + flat / frame_globe().w;
  // A tile, a ground cell a texel, takes over from the global picture, eight
  // times coarser, over the octave a pixel goes from two cells, at the tiles'
  // reach, to one.
  let share = clamp(-octaves, 0.0, 1.0);
  if (share <= 0.0) {
    return global;
  }
  let w = atmos_earth_window();
  let at = mercator / TAU * f32(w.w);
  let tile = vec2i(floor(at));
  let d = vec2i(((tile.x - w.x) % w.w + w.w) % w.w, tile.y - w.y);
  if (all(d >= vec2i(0)) && all(d < vec2i(w.z))) {
    let k = u32(d.y * w.z + d.x);
    let per_word = 32u / EARTH_TILE_BITS;
    let layer = (atmos_earth_tiles()[k / per_word] >> (EARTH_TILE_BITS * (k % per_word))) & ((1u << EARTH_TILE_BITS) - 1u);
    if (layer > 0u) {
      return mix(global, textureSampleLevel(earth_tiles, atmos_sampler, fract(at), i32(layer - 1u), 0.0), share);
    }
  }
  return global;
}

// On where night falls on them, as the sky's night falls (look/environment.rs).
fn city_shown(lights: f32, up: vec3f) -> vec3f {
  return atmos_city_light() * lights * lights * exposure() * (1.0 - smoothstep(-0.1, 0.0, dot(up, atmos_sun_dir())));
}

// Doubled, picked by eye beside the original; the cities are not its to scale.
const EARTH_GAIN: f32 = 2.0;

// Lit by the terrain's own light, so the drawn edge is no seam. It comes in
// over the octave to the earth layer's reach, where
// the terrain ends seen from the globe; nearer, the ground is the voxels' or
// the air's alone.
fn beyond(air: Air) -> vec3f {
  var light = air.light;
  let shown = clamp(air.octaves + 1.0, 0.0, 1.0);
  if (air.ground && shown > 0.0) {
    let n = air.normal;
    let face = earth_face(n, air.octaves);
    light += air.end * shown * (EARTH_GAIN * face.rgb * surface_light(n, n, light_at(land_radius(), n)) + city_shown(face.a, n));
  }
  return light;
}


/* the air before a point */

// The light per unit of depth bends sharply only in the ozone: under it three
// knots err less than eight do from over it, a percent or two. Crossing the
// line, the whole frame steps by that much at once and shows no flicker.
fn air_knots() -> i32 {
  return select(8, 3, atmos_camera_altitude() < atmos_ozone_center() - atmos_ozone_half_width());
}

// The points `d` km along `dir`, nearest first, unexposed.
fn air_segments(dir: vec3f, d: vec3f) -> array<Veil, 3> {
  var v = gather(camera_r(), frame_up(), dir, d, atmos_sun_dir(), atmos_moon_dir(), atmos_moon_light(), air_knots());
  for (var i = 0; i < 3; i++) {
    v[i].scattered *= atmos_white();
  }
  return v;
}

fn air_to(world_pos: vec3f) -> Veil {
  let to = world_pos - frame_camera_pos();
  let du = length(to);
  if (du < 1e-4) {
    return Veil(vec3f(1.0), vec3f(0.0));
  }
  let v = air_segments(to / du, vec3f(du * atmos_km_per_u()))[2];
  return Veil(v.through, v.scattered * exposure());
}

// Per vertex: every term varies slowly across a face.
struct Seen {
  through: f32,
  scattered: vec3f,
  light: Light,
};

fn seen_from_eye(world_pos: vec3f) -> Seen {
  let v = air_to(world_pos);
  var s: Seen;
  // An authored color is dimmed alike at every wavelength: by wavelength, a
  // light sea's blue went and its green stayed.
  s.through = dot(v.through, LUMA);
  s.scattered = v.scattered;
  s.light = light_of(world_pos);
  return s;
}


/* glare */

// Spencer et al. 1995's halo; ISO 9358 veiling glare is 1-10%, GLARE_VEIL its middle.
const GLARE_HALO_SHARE: f32 = 0.6205;
const GLARE_VEIL: f32 = 0.03;

// Over the sky only.
fn glare(dir: vec3f) -> vec3f {
  let s = atmos_sun_dir();
  let theta = degrees(atan2(length(cross(dir, s)), dot(dir, s))) + 0.02;
  let psf = 0.478 * 20.91 / (theta * theta * theta) + 0.138 * 72.37 / (theta * theta);
  return atmos_sun_seen() * shown() * psf * (GLARE_VEIL / GLARE_HALO_SHARE);
}
