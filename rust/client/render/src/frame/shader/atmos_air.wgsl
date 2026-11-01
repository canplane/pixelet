// The air as the frame shows it: before each point, read off the air's
// volume (gpu/atmosphere.rs `froxel`), which the march to its froxels made
// about each light (atmos_froxel.wgsl); and the sky, and the land past the
// terrain, seen through it.

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

// The volume about light `k`, the sun's half or the moon's, at
// column `c` across and down and `slice` deep, texel coordinates with
// centers at whole numbers, by the cubic B-spline in all three: smooth to
// its second derivative across every column and slice, eight trilinear
// reads. A tap past the half's first or last column reads that column, as
// the edge's own would. It smooths what it reads by a sixth of the square of
// a texel's step times its curvature, which the slow light per share taken
// has little of.
fn air_held_at(k: u32, c: vec2f, slice: f32) -> vec3f {
  let n = f32(AIR_FROXELS);
  let x = bspline_taps(c.x);
  let y = bspline_taps(c.y);
  let z = bspline_taps(slice);
  let across = clamp(x.xy, vec2f(0.0), vec2f(n - 1.0)) + f32(k) * n;
  var sum = vec3f(0.0);
  for (var i = 0u; i < 8u; i++) {
    let at = vec3f(select(across.x, across.y, (i & 1u) != 0u), select(y.x, y.y, (i & 2u) != 0u), select(z.x, z.y, (i & 4u) != 0u));
    let weight = select(x.z, x.w, (i & 1u) != 0u) * select(y.z, y.w, (i & 2u) != 0u) * select(z.z, z.w, (i & 4u) != 0u);
    sum += weight * textureSampleLevel(air_volume, atmos_sampler, (at + 0.5) / vec3f(2.0 * n, n, n), 0.0).rgb;
  }
  return sum;
}

// What the air `d` km along `dir` scatters per share it takes, per unit of
// light `k`, off that light's half of the volume: laid as its sky view is,
// so read at the direction's place about the light (atmos_sky.wgsl
// `table_unit`), and at the slice its depth law puts `d`
// (atmos_medium.wgsl `laid`, `air_slice`).
fn air_held_of(k: u32, dir: vec3f, d: f32) -> vec3f {
  let n = f32(AIR_FROXELS);
  return air_held_at(k, table_unit(dir, k) * (n - 1.0), air_slice(d, laid(k).yz));
}

// The air `d` units from the eye along `dir` (Hillaire 2020's
// aerial-perspective volume, frame/shader/word.rs `AIR_FROXELS`). What the
// air lets through is this ray's own, e^-τ(d), per channel, taken as the
// march takes it (atmos_march.wgsl `gather_between`): from where the ray
// enters the air to the ground at the latest. What it scatters is that ray's
// 1 - e^-τ(d) times each light per share taken, read off its light's half of
// the volume smooth (`air_held_of`) where that light reaches the air
// (atmos_medium.wgsl `reaches`), and shown as that light's sky is
// (`beyond`). Only that slow factor is read off the grid; the fast one is
// the pixel's own.
fn air_before(dir: vec3f, d: f32) -> vec4f {
  let km = atmos_km_per_u();
  var held = vec3f(0.0);
  for (var k = 0u; k < 2u; k++) {
    if (reaches(k)) {
      held += air_held_of(k, dir, d * km) * light_gain(k);
    }
  }
  held *= shown();
  let r = camera_r();
  let mu = dot(frame_up(), dir);
  let end = short_of_ground(r, mu, d * km);
  let low = ray_low(r, mu, end);
  let a = min(to_sphere(r, mu, atmos_top_radius()), end);
  let transmittance = exp(-max(depth_along(r, mu, low, end) - depth_along(r, mu, low, a), vec3f(0.0)));
  return vec4f(held * (vec3f(1.0) - transmittance), dot(transmittance, LUMA));
}

// The air before a point as the volume holds it along the point's direction
// from the eye and at its distance, whichever way it lies from the view. A
// point past the air's reach reads the air to its end. The lights at a point
// are the point's own, read where it stands (atmos_view.wgsl `light_of`):
// they turn with its height, and a froxel stands higher or lower than what
// is drawn in it.
fn seen_from_eye(world_pos: vec3f) -> Seen {
  let to = world_pos - frame_camera_pos();
  let d = length(to);
  let air = air_before(to / max(d, 1e-6), d);
  var s: Seen;
  s.scattered = air.rgb;
  s.transmittance = air.a;
  s.light = light_of(world_pos);
  return s;
}

/* sky */

fn moon_view(dir: vec3f) -> vec3f {
  return table_view(moon_view_lut, dir, 1u);
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
    // Each light's sky where it reaches the air (atmos_medium.wgsl
    // `reaches`).
    var sky = vec3f(0.0);
    if (reaches(0u)) {
      sky += sky_view(dir) * light_gain(0u);
    }
    if (reaches(1u)) {
      sky += moon_view(dir) * light_gain(1u);
    }
    return sky * shown();
  }
  let d = to_land(dir);
  let p = dir * d;
  let seen = seen_from_eye(p);
  let octaves = log2(d / atmos_earth_dist());
  let n = frame_up();
  let face = earth_face(map_of(p), octaves);
  let lit = LIGHT_EARTH_GAIN * face.rgb * surface_light(n, n, seen.light) + city_shown(face.a, n);
  let land = clamp(log2(d / atmos_drawn()) + 1.0, 0.0, 1.0);
  return seen.scattered + seen.transmittance * land * lit;
}

