// The air as the frame shows it: before each point, read off the air's
// volume (gpu/atmosphere.rs `froxel`), which the march to its froxels made
// (atmos_froxel.wgsl, atmos_view.wgsl `air_held`); and the sky, and the land
// past the terrain, seen through it.

@group(0) @binding(15) var moon_view_lut: texture_2d<f32>;
@group(0) @binding(16) var earth_map: texture_2d_array<f32>;
@group(0) @binding(17) var earth_tiles: texture_2d_array<f32>;
@group(0) @binding(22) var air_volume: texture_3d<f32>;


/* the air before a point */

// Column `c` of the volume `d` units from the eye, as deep as its own span
// puts it (atmos_froxel.wgsl `column_span`): Hillaire 2020's
// aerial-perspective volume, 32 a side and 32 deep, as Unreal's
// SkyAtmosphere lays it out (frame/shader/word.rs `AIR_FROXELS`), its
// slices' texels at depths 0 to 1 of the span and the span in the slice past
// them.
fn air_column(c: vec2u, d: f32) -> vec4f {
  let n = f32(AIR_FROXELS);
  let span = air_span_of(textureLoad(air_volume, vec3u(c, AIR_FROXELS), 0).xy);
  let k = air_slice(span, d);
  return textureSampleLevel(air_volume, atmos_sampler, vec3f((vec2f(c) + 0.5) / n, (0.5 + k) / (n + 1.0)), 0.0);
}

// The volume at `uv` across, `d` units from the eye: the four columns about
// it, each read at that distance by its own span. Read at one depth in all
// four, as one filtered sample would, a column whose span runs far past its
// neighbour's — toward the horizon, where the sea's plane runs off — gave a
// point the air of another distance, a line along the horizon.
fn air_before(uv: vec2f, d: f32) -> vec4f {
  let last = vec2f(f32(AIR_FROXELS - 1u));
  let at = clamp(uv * f32(AIR_FROXELS) - 0.5, vec2f(0.0), last);
  let c0 = vec2u(floor(at));
  let c1 = vec2u(min(floor(at) + 1.0, last));
  let w = at - floor(at);
  let top = mix(air_column(c0, d), air_column(vec2u(c1.x, c0.y), d), w.x);
  let bottom = mix(air_column(vec2u(c0.x, c1.y), d), air_column(c1, d), w.x);
  return mix(top, bottom, w.y);
}

// The air before a point as the volume holds it at the point's place on the
// screen and its distance: what the eye's view projects it to, so a point
// off the screen or behind the eye reads the volume's nearest edge. A point
// past a column's span, under the sea or past the air's ground, reads the
// air to its end. The lights at a point are the point's own, read where it
// stands (atmos_view.wgsl `light_of`): they turn with its height, and a
// froxel stands higher or lower than what is drawn in it.
fn seen_from_eye(world_pos: vec3f) -> Seen {
  let clip = frame_view_proj() * vec4f(world_pos, 1.0);
  let w = max(clip.w, 1e-6);
  let uv = vec2f(clip.x / w * 0.5 + 0.5, 0.5 - clip.y / w * 0.5);
  let air = air_before(uv, length(world_pos - frame_camera_pos()));
  var s: Seen;
  s.scattered = air.rgb;
  s.through = air.a;
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
  let lit = EARTH_GAIN * face.rgb * surface_light(n, n, seen.light) + city_shown(face.a, n);
  return seen.scattered + seen.through * clamp(octaves + 1.0, 0.0, 1.0) * lit;
}

