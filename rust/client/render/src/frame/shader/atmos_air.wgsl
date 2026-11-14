// The air as the frame shows it. The air before each point is read from the
// air's volume, which the march through its froxels made about each light
// (atmos_froxel.wgsl, pass/atmosphere.rs `froxel`). The sky, and the land past
// the terrain, are seen through the air.

@group(0) @binding(15) var moon_view_lut: texture_2d<f32>;
@group(0) @binding(16) var earth_map: texture_2d_array<f32>;
@group(0) @binding(17) var earth_tiles: texture_2d_array<f32>;
@group(0) @binding(22) var air_volume: texture_3d<f32>;

// Reads light `k`'s half of the volume, the sun's or the moon's, at column `c`
// across and down and at `slice` deep, in one trilinear read. The coordinates
// are in texels, with texel centers at whole numbers. A read past the half's
// first or last column reads that column, as a sampler clamping a texture of
// its own would, and never blends in the other half.
fn air_scatter_at(k: u32, c: vec2f, slice: f32) -> vec3f {
  let n = f32(AIR_FROXELS);
  let at = vec3f(clamp(c.x, 0.0, n - 1.0) + f32(k) * n, c.y, slice);
  return textureSampleLevel(air_volume, atmos_sampler, (at + 0.5) / vec3f(2.0 * n, n, n), 0.0).rgb;
}

// What the air up to `d` km along `dir` scatters per share of light it takes,
// per unit of light `k`, read from that light's half of the volume. The half
// is laid as the light's sky view is, so it is read where the direction lies
// about the light (atmos_sky.wgsl `table_unit`), at the slice where the depth
// law puts `d` (atmos_medium.wgsl `view_layout`, `air_slice`).
fn air_scatter_of(k: u32, dir: vec3f, d: f32) -> vec3f {
  let n = f32(AIR_FROXELS);
  return air_scatter_at(k, table_unit(dir, k) * (n - 1.0), air_slice(d, view_layout(k).yz));
}

// The air between the eye and the point `d` units away along `dir`, after
// Hillaire 2020's aerial-perspective volume (frame/shader/word.rs
// `AIR_FROXELS`). What the air lets through is this ray's own, e^-τ(d) per
// channel, measured as the march measures it (atmos_march.wgsl
// `gather_between`): from where the ray enters the air to the ground at the
// latest. What the air scatters is the ray's own 1 - e^-τ(d) times what each
// light's half of the volume holds per share taken (`air_scatter_of`), for each
// light that reaches the air (atmos_medium.wgsl `reaches`), shown as that
// light's sky is shown (`beyond`). Only that factor, which changes slowly, is
// read from the grid; the factor that changes fast is the pixel's own.
fn air_before(dir: vec3f, d: f32) -> vec4f {
  let km = atmos_km_per_u();
  var scatter = vec3f(0.0);
  for (var k = 0u; k < 2u; k++) {
    if (reaches(k)) {
      scatter += air_scatter_of(k, dir, d * km) * light_gain(k);
    }
  }
  scatter *= shown();
  let r = camera_r();
  let mu = dot(frame_up(), dir);
  let end = short_of_ground(r, mu, d * km);
  let low = ray_low(r, mu, end);
  let a = min(to_sphere(r, mu, TOP_RADIUS_KM), end);
  let transmittance = exp(-max(depth_along(r, mu, low, end) - depth_along(r, mu, low, a), vec3f(0.0)));
  return vec4f(scatter * (vec3f(1.0) - transmittance), dot(transmittance, LUMA));
}

// The air before a world point, read from the volume along the point's
// direction from the eye and at its distance, whichever way the point lies
// from the view. A point past the volume's far end reads the air up to that
// end. The light on the point is not the volume's: the point reads its own
// where it stands (atmos_view.wgsl `light_of`), as the light changes with
// height and a froxel stands higher or lower than what is drawn in it.
fn seen_from_eye(world_pos: vec3f) -> Air {
  let to = world_pos - frame_camera_pos();
  let d = length(to);
  let air = air_before(to / max(d, 1e-6), d);
  return Air(air.a, air.rgb);
}

fn moon_view(dir: vec3f) -> vec3f {
  return table_view(moon_view_lut, dir, 1u);
}

// Where a point lies on the map, as its Mercator longitude and ordinate.
fn map_of(p: vec3f) -> vec2f {
  return frame_map().xy + p.xy / frame_map().z;
}

// The Earth's reflectance at the map's place `mercator`, with the square root
// of its city lights in alpha. It reads the global picture at the place's
// latitude and longitude, which is zero until the picture has loaded, and a
// held tile over that.
fn earth_face(mercator: vec2f, octaves: f32) -> vec4f {
  let lat = 2.0 * atan(exp(mercator.y)) - 0.5 * PI;
  let uv = vec2f(fract(mercator.x / TAU + 0.5), 0.5 - lat / PI);
  let global = textureSampleLevel(earth_map, atmos_sampler, uv, 0, 0.0);
  // A tile holds a ground cell a texel and takes over from the global
  // picture, which is eight times coarser. It fades in over the octave in
  // which a pixel goes from covering two cells, at the tiles' reach, to one.
  let share = clamp(-octaves, 0.0, 1.0);
  if (share <= 0.0) {
    return global;
  }
  let w = atmos_earth_window();
  let at = mercator / TAU * f32(EARTH_AROUND);
  let tile = vec2i(floor(at));
  let d = vec2i(((tile.x - w.x) % EARTH_AROUND + EARTH_AROUND) % EARTH_AROUND, tile.y - w.y);
  if (all(d >= vec2i(0)) && all(d < vec2i(EARTH_ACROSS))) {
    let k = u32(d.y * EARTH_ACROSS + d.x);
    let per_word = 32u / EARTH_TILE_BITS;
    let layer = (atmos_earth_tiles()[k / per_word] >> (EARTH_TILE_BITS * (k % per_word))) & ((1u << EARTH_TILE_BITS) - 1u);
    if (layer > 0u) {
      return mix(global, textureSampleLevel(earth_tiles, atmos_sampler, fract(at), i32(layer - 1u), 0.0), share);
    }
  }
  return global;
}

// The city lights as shown, from `lights`, the square root `earth_face`
// holds. They come on as night falls on them, over the same heights of the
// sun as everywhere night is shown (pxl-content param.rs `night::FALL`).
fn city_shown(lights: f32, up: vec3f) -> vec3f {
  return CITY_LIGHT * atmos_white() * lights * lights * exposure() * (1.0 - smoothstep(NIGHT_FALL.x, NIGHT_FALL.y, dot(up, atmos_sun_dir())));
}

// What the eye sees along `dir` where nothing was drawn. A direction that
// rises sees the sky. A direction that falls meets the land, the plane under
// the eye at the elevation of its ground, seen through the air and lit as the
// terrain is lit, so the edge of what is drawn shows no seam. The land fades
// in over the octave that ends where the rays end (`atmos_drawn`). Nearer,
// the voxels draw the ground, and where they draw none, only the air shows.
fn beyond(dir: vec3f) -> vec3f {
  if (!falls_to_land(dir)) {
    // Only a light that reaches the air adds its sky (atmos_medium.wgsl
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
  let octaves = log2(d / EARTH_DIST);
  let n = frame_up();
  let face = earth_face(map_of(p), octaves);
  let lit = LIGHT_EARTH_GAIN * face.rgb * surface_light(n, n, light_of(p)) + city_shown(face.a, n);
  let land = clamp(log2(d / atmos_drawn()) + 1.0, 0.0, 1.0);
  return seen.scattered + seen.transmittance * land * lit;
}

