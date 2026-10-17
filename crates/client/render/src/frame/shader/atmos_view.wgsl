// The air as the frame shows it: sky, the land past the terrain, and aerial perspective.

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

fn light_at(r: f32, up: vec3f) -> Light {
  let l = light_unexposed(r, up);
  let e = exposure();
  return Light(l.sun * e, l.other * e, l.fill * e);
}

fn light_of(world_pos: vec3f) -> Light {
  return light_at(radius_of(world_pos), up_at(world_pos));
}

/* sky */

fn moon_view(dir: vec3f) -> vec3f {
  return table_view(moon_view_lut, dir, atmos_moon_dir());
}


/* the Earth past the terrain */

// The land the eye's ground lies on, as far as it runs: the plane at that
// height, from the eye, in units.
fn land_z() -> f32 {
  return frame_planet().w + atmos_land_altitude_km() / atmos_km_per_u();
}

// Where a point lies on the map, as Mercator's longitude and ordinate.
fn map_of(p: vec3f) -> vec2f {
  return frame_map().xy + (p.xy - frame_planet().xy) / frame_map().z;
}

// Reflectance, and in alpha its lights' square root, at the map's place
// `mercator`: the global picture at its latitude and longitude, a held tile
// over it.
fn earth_face(mercator: vec2f, octaves: f32) -> vec4f {
  let lat = 2.0 * atan(exp(mercator.y)) - 0.5 * PI;
  let uv = vec2f(fract(mercator.x / TAU + 0.5), 0.5 - lat / PI);
  let global = textureSampleLevel(earth_map, atmos_sampler, uv, 0, 0.0);
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
  return atmos_city_light() * atmos_white() * lights * lights * exposure() * (1.0 - smoothstep(-0.1, 0.0, dot(up, atmos_sun_dir())));
}

// Doubled, picked by eye beside the original; the cities are not its to scale.
const EARTH_GAIN: f32 = 2.0;

// What the eye sees along `dir` where nothing was drawn: the sky where it
// rises; where it falls, the land it meets, the plane under the eye at its
// ground's height, seen through the air to it and lit as the terrain is, so
// the drawn edge is no seam. Its picture comes in over the octave to the
// earth layer's reach; nearer, the ground is the voxels', or the air's alone.
fn beyond(dir: vec3f) -> vec3f {
  let rise = dot(dir, frame_up());
  if (rise >= 0.0 || land_z() >= 0.0) {
    var sky = sky_view(dir, atmos_sun_dir()) * shown();
    if (atmos_moon_light() > 0.0) {
      sky += moon_view(dir) * moon_shown();
    }
    return sky;
  }
  let d = land_z() / rise;
  let p = dir * d;
  let seen = seen_from_eye(p);
  let octaves = log2(d / atmos_earth_reach());
  let n = frame_up();
  let face = earth_face(map_of(p), octaves);
  let lit = EARTH_GAIN * face.rgb * surface_light(n, n, seen.light) + city_shown(face.a, n);
  return seen.scattered + seen.through * clamp(octaves + 1.0, 0.0, 1.0) * lit;
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

// Over the sky only.
fn glare(dir: vec3f) -> vec3f {
  let s = atmos_sun_dir();
  return glare_toward(dir, s, dot(s, frame_up()), exposure());
}
