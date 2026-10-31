// The air as the frame shows it: before each point, read off the air's
// volume (gpu/atmosphere.rs `froxel`), which the march to its froxels made
// (atmos_froxel.wgsl, atmos_view.wgsl `air_held`); and the sky, and the land
// past the terrain, seen through it.

@group(0) @binding(15) var moon_view_lut: texture_2d<f32>;
@group(0) @binding(16) var earth_map: texture_2d_array<f32>;
@group(0) @binding(17) var earth_tiles: texture_2d_array<f32>;
@group(0) @binding(22) var air_volume: texture_3d<f32>;

/* the air before a point */

// The cubic B-spline about texel coordinate `x` along one axis as two
// linear reads (Sigg and Hadwiger 2005): their places, texels with centers
// at whole numbers, then their weights.
fn bspline_taps(x: f32) -> vec4f {
  let i = floor(x);
  let f = x - i;
  let f2 = f * f;
  let f3 = f2 * f;
  let w0 = (1.0 - 3.0 * f + 3.0 * f2 - f3) / 6.0;
  let w1 = (4.0 - 6.0 * f2 + 3.0 * f3) / 6.0;
  let w3 = f3 / 6.0;
  let g0 = w0 + w1;
  let g1 = 1.0 - g0;
  return vec4f(i - 1.0 + w1 / g0, i + 1.0 + w3 / max(g1, 1e-6), g0, g1);
}

// The volume at `uv` across and slice `k` deep by the cubic B-spline in all
// three: smooth to its second derivative across every column and slice,
// eight trilinear reads. It smooths what it reads by a sixth of the square
// of a texel's step times its curvature, which the slow light per share
// taken has little of.
fn air_held_at(uv: vec2f, k: f32) -> vec3f {
  let n = f32(AIR_FROXELS);
  let x = bspline_taps(uv.x * n - 0.5);
  let y = bspline_taps(uv.y * n - 0.5);
  let z = bspline_taps(k);
  var sum = vec3f(0.0);
  for (var c = 0u; c < 8u; c++) {
    let at = vec3f(select(x.x, x.y, (c & 1u) != 0u), select(y.x, y.y, (c & 2u) != 0u), select(z.x, z.y, (c & 4u) != 0u));
    let weight = select(x.z, x.w, (c & 1u) != 0u) * select(y.z, y.w, (c & 2u) != 0u) * select(z.z, z.w, (c & 4u) != 0u);
    sum += weight * textureSampleLevel(air_volume, atmos_sampler, (at + 0.5) / n, 0.0).rgb;
  }
  return sum;
}

// The volume `d` units from the eye along `dir`, at `uv` across
// (Hillaire 2020's aerial-perspective volume, 32 a side and deep,
// frame/shader/word.rs `AIR_FROXELS`). What the air lets through is this
// ray's own, e^-τ(d), per channel, taken as the march takes it
// (atmos_march.wgsl `gather_between`): from where the ray enters the air to the
// ground at the latest. What it scatters is that ray's 1 - e^-τ(d) times the
// light per share taken (atmos_view.wgsl `air_held`), read off the volume
// smooth (`air_held_at`). Only that slow factor is read off the grid; the
// fast one is the pixel's own. The volume was exposed at the stops it holds
// beside every froxel; it is taken to the eye's stops now, exactly itself
// where they stand.
fn air_before(uv: vec2f, dir: vec3f, d: f32) -> vec4f {
  let made = textureLoad(air_volume, vec3i(0), 0).a;
  let held = air_held_at(uv, air_slice(d)) * exp2(exposure_stops() - made);
  let km = atmos_km_per_u();
  let r = camera_r();
  let mu = dot(frame_up(), dir);
  let end = short_of_ground(r, mu, d * km);
  let low = ray_low(r, mu, end);
  let a = min(to_sphere(r, mu, atmos_top_radius()), end);
  let transmittance = exp(-max(depth_along(r, mu, low, end) - depth_along(r, mu, low, a), vec3f(0.0)));
  return vec4f(held * (vec3f(1.0) - transmittance), dot(transmittance, LUMA));
}

// The air before a point as the volume holds it at the point's place on the
// screen and its distance: what the eye's view projects it to, so a point
// off the screen or behind the eye reads the volume's nearest edge. A point
// past the air's reach reads the air to its end. The lights at a point are the point's own, read where it
// stands (atmos_view.wgsl `light_of`): they turn with its height, and a
// froxel stands higher or lower than what is drawn in it.
fn seen_from_eye(world_pos: vec3f) -> Seen {
  let clip = frame_view_proj() * vec4f(world_pos, 1.0);
  let w = max(clip.w, 1e-6);
  let uv = vec2f(clip.x / w * 0.5 + 0.5, 0.5 - clip.y / w * 0.5);
  let to = world_pos - frame_camera_pos();
  let d = length(to);
  let air = air_before(uv, to / max(d, 1e-6), d);
  var s: Seen;
  s.scattered = air.rgb;
  s.transmittance = air.a;
  s.light = light_of(world_pos);
  return s;
}

/* sky */

fn moon_view(dir: vec3f) -> vec3f {
  return table_view(moon_view_lut, dir, atmos_moon_dir());
}

/* the Earth past the terrain */

// Where a point lies on the map, as Mercator's longitude and ordinate.
fn map_of(p: vec3f) -> vec2f {
  return frame_map().xy + p.xy / frame_map().z;
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

// On where night falls on them, as the sky's night falls (param.rs
// `night::FALL`).
fn city_shown(lights: f32, up: vec3f) -> vec3f {
  return atmos_city_light() * atmos_white() * lights * lights * exposure() * (1.0 - smoothstep(NIGHT_FALL.x, NIGHT_FALL.y, dot(up, atmos_sun_dir())));
}

// What the eye sees along `dir` where nothing was drawn: the sky where it
// rises; where it falls, the land it meets, the plane under the eye at its
// ground's height, seen through the air to it and lit as the terrain is, so
// the drawn edge is no seam. Its land comes in over the octave to where the
// rays end; nearer, the ground is the voxels', or the air's alone.
fn beyond(dir: vec3f) -> vec3f {
  if (!falls_to_land(dir)) {
    var sky = sky_view(dir, atmos_sun_dir()) * shown();
    if (atmos_moon_light() > 0.0) {
      sky += moon_view(dir) * moon_shown();
    }
    return sky;
  }
  let d = to_land(dir);
  let p = dir * d;
  let seen = seen_from_eye(p);
  let octaves = log2(d / atmos_earth_reach());
  let n = frame_up();
  let face = earth_face(map_of(p), octaves);
  let lit = LIGHT_EARTH_GAIN * face.rgb * surface_light(n, n, seen.light) + city_shown(face.a, n);
  let land = clamp(log2(d / atmos_drawn()) + 1.0, 0.0, 1.0);
  return seen.scattered + seen.transmittance * land * lit;
}

